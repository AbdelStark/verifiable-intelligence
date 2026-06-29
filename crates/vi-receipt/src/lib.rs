//! Envelope codec for `CommitLLM` receipts.
//!
//! Implements `VIKY` (verifier-key), `VIRC` (chat receipt), and `VIAU` (audit
//! receipt) headers per RFC-0003. Leaf crate: no async, no networking.

// RFC-0003 parsing failures must flow through the shared `ViError` taxonomy.
#![allow(clippy::result_large_err)]

use vi_errors::ViError;

/// Supported v1 binary-envelope version.
pub const ENVELOPE_VERSION: u8 = 1;

/// Reserved v1 flags value.
pub const ENVELOPE_FLAGS: u8 = 0;

/// Number of bytes in `[magic:4][ver:1][flags:1]`.
pub const HEADER_LEN: usize = 6;

/// Project-owned binary artifact magic prefixes.
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Magic {
    /// Verifier-key envelope, `VIKY`.
    VIKY,
    /// Chat receipt envelope, `VIRC`.
    VIRC,
    /// Audit payload envelope, `VIAU`.
    VIAU,
}

impl Magic {
    /// Four-byte ASCII magic prefix.
    #[must_use]
    pub const fn bytes(self) -> [u8; 4] {
        match self {
            Self::VIKY => [b'V', b'I', b'K', b'Y'],
            Self::VIRC => [b'V', b'I', b'R', b'C'],
            Self::VIAU => [b'V', b'I', b'A', b'U'],
        }
    }

    /// Stable ASCII name for error envelopes and logs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VIKY => "VIKY",
            Self::VIRC => "VIRC",
            Self::VIAU => "VIAU",
        }
    }

    /// Parse a four-byte ASCII magic prefix.
    pub fn from_bytes(bytes: [u8; 4]) -> Result<Self, ViError> {
        match &bytes {
            b"VIKY" => Ok(Self::VIKY),
            b"VIRC" => Ok(Self::VIRC),
            b"VIAU" => Ok(Self::VIAU),
            _ => Err(corrupt_envelope("unknown", 0, "unknown magic prefix")),
        }
    }
}

/// Project-owned binary envelope around an upstream `CommitLLM` payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    /// Artifact kind magic prefix.
    pub magic: Magic,
    /// Envelope layout version.
    pub ver: u8,
    /// Reserved v1 flags byte.
    pub flags: u8,
    /// Opaque upstream payload bytes.
    pub payload: Vec<u8>,
}

impl Envelope {
    /// Construct a v1 envelope with zero flags.
    #[must_use]
    pub fn new(magic: Magic, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            magic,
            ver: ENVELOPE_VERSION,
            flags: ENVELOPE_FLAGS,
            payload: payload.into(),
        }
    }

    /// Encode this envelope to bytes.
    pub fn encode(&self) -> Result<Vec<u8>, ViError> {
        encode(self)
    }

    /// Decode an envelope from bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, ViError> {
        decode(bytes)
    }
}

/// Encode an envelope to `[magic:4][ver:1][flags:1][payload:N]`.
pub fn encode(envelope: &Envelope) -> Result<Vec<u8>, ViError> {
    validate_version(envelope.magic, envelope.ver)?;
    validate_flags(envelope.magic, envelope.flags)?;

    let mut bytes = Vec::with_capacity(HEADER_LEN + envelope.payload.len());
    bytes.extend_from_slice(&envelope.magic.bytes());
    bytes.push(envelope.ver);
    bytes.push(envelope.flags);
    bytes.extend_from_slice(&envelope.payload);
    Ok(bytes)
}

/// Decode `[magic:4][ver:1][flags:1][payload:N]` into an envelope.
pub fn decode(bytes: &[u8]) -> Result<Envelope, ViError> {
    if bytes.len() < HEADER_LEN {
        return Err(corrupt_envelope(
            "unknown",
            bytes.len(),
            "envelope shorter than 6-byte header",
        ));
    }

    let magic = Magic::from_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])?;
    let ver = bytes[4];
    let flags = bytes[5];
    validate_version(magic, ver)?;
    validate_flags(magic, flags)?;

    Ok(Envelope {
        magic,
        ver,
        flags,
        payload: bytes[HEADER_LEN..].to_vec(),
    })
}

fn validate_version(magic: Magic, ver: u8) -> Result<(), ViError> {
    if ver == ENVELOPE_VERSION {
        Ok(())
    } else {
        Err(ViError::UnknownVersion {
            envelope: magic.as_str(),
            field: "ver",
            value: u32::from(ver),
            supported: vec![u32::from(ENVELOPE_VERSION)],
        })
    }
}

fn validate_flags(magic: Magic, flags: u8) -> Result<(), ViError> {
    if flags == ENVELOPE_FLAGS {
        Ok(())
    } else {
        Err(corrupt_envelope(magic.as_str(), 5, "flags must be 0 in v1"))
    }
}

fn corrupt_envelope(envelope: &'static str, offset: usize, reason: &'static str) -> ViError {
    ViError::CorruptEnvelope {
        envelope,
        offset,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::{decode, encode, Envelope, Magic, ENVELOPE_FLAGS, ENVELOPE_VERSION, HEADER_LEN};
    use proptest::prelude::{any, prop_oneof, Just, ProptestConfig};
    use proptest::{collection, proptest};
    use vi_errors::ViError;

    fn magic_strategy() -> impl proptest::strategy::Strategy<Value = Magic> {
        prop_oneof![Just(Magic::VIKY), Just(Magic::VIRC), Just(Magic::VIAU)]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn envelope_round_trips(magic in magic_strategy(), payload in collection::vec(any::<u8>(), 0..2048)) {
            let envelope = Envelope::new(magic, payload);
            let bytes = encode(&envelope)?;
            let expected_len = HEADER_LEN + envelope.payload.len();
            let decoded = decode(&bytes)?;

            proptest::prop_assert_eq!(decoded, envelope);
            proptest::prop_assert_eq!(bytes.len(), expected_len);
        }
    }

    #[test]
    fn magic_bytes_are_stable() {
        assert_eq!(Magic::VIKY.bytes(), *b"VIKY");
        assert_eq!(Magic::VIRC.bytes(), *b"VIRC");
        assert_eq!(Magic::VIAU.bytes(), *b"VIAU");
    }

    #[test]
    fn magic_mismatch_is_corrupt_envelope() {
        let bytes = [b'N', b'O', b'P', b'E', ENVELOPE_VERSION, ENVELOPE_FLAGS];
        let error = decode(&bytes).expect_err("unknown magic should fail");

        assert_eq!(
            error,
            ViError::CorruptEnvelope {
                envelope: "unknown",
                offset: 0,
                reason: "unknown magic prefix",
            }
        );
    }

    #[test]
    fn unknown_version_is_unknown_version() {
        let mut bytes = Envelope::new(Magic::VIRC, b"payload".to_vec())
            .encode()
            .expect("valid envelope should encode");
        bytes[4] = 2;

        let error = decode(&bytes).expect_err("unknown version should fail");
        assert_eq!(
            error,
            ViError::UnknownVersion {
                envelope: "VIRC",
                field: "ver",
                value: 2,
                supported: vec![1],
            }
        );
    }

    #[test]
    fn non_zero_flags_are_corrupt_envelope() {
        let mut bytes = Envelope::new(Magic::VIAU, b"payload".to_vec())
            .encode()
            .expect("valid envelope should encode");
        bytes[5] = 0b0000_0001;

        let error = decode(&bytes).expect_err("non-zero flags should fail");
        assert_eq!(
            error,
            ViError::CorruptEnvelope {
                envelope: "VIAU",
                offset: 5,
                reason: "flags must be 0 in v1",
            }
        );
    }

    #[test]
    fn short_input_is_corrupt_envelope() {
        let error = decode(b"VIRC\x01").expect_err("short header should fail");
        assert_eq!(
            error,
            ViError::CorruptEnvelope {
                envelope: "unknown",
                offset: 5,
                reason: "envelope shorter than 6-byte header",
            }
        );
    }
}
