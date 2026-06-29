//! Envelope codec for `CommitLLM` receipts.
//!
//! Implements `VIKY` (verifier-key), `VIRC` (chat receipt), and `VIAU` (audit
//! receipt) headers per RFC-0003. Leaf crate: no async, no networking.

// RFC-0003 parsing failures must flow through the shared `ViError` taxonomy.
#![allow(clippy::result_large_err)]

use vi_errors::ViError;

/// Supported v1 binary-envelope version.
pub const ENVELOPE_VERSION: u8 = 1;

/// Supported v1 verifier-key binding schema version.
pub const KEYGEN_SCHEMA_VERSION: u16 = 1;

/// Reserved v1 flags value.
pub const ENVELOPE_FLAGS: u8 = 0;

/// Number of bytes in `[magic:4][ver:1][flags:1]`.
pub const HEADER_LEN: usize = 6;

/// Number of bytes in `[binding_len:u32le][binding_crc32c:u32le]`.
pub const KEY_BINDING_PREFIX_LEN: usize = 8;

const CRC32C: crc::Crc<u32> = crc::Crc::<u32>::new(&crc::CRC_32_ISCSI);

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

/// Verifier-key identity binding header carried inside `VIKY` payloads.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KeyBindingHeader {
    /// Key binding schema version.
    pub keygen_schema_version: u16,
    /// Deterministic key-generation seed.
    pub seed: u64,
    /// Public model identifier.
    pub model_id: String,
    /// Canonical checkpoint hash.
    pub checkpoint_hash: String,
    /// Pinned upstream `CommitLLM` short revision.
    pub commitllm_pin: String,
}

impl KeyBindingHeader {
    /// Construct a v1 verifier-key binding header.
    #[must_use]
    pub fn new(
        model_id: impl Into<String>,
        checkpoint_hash: impl Into<String>,
        commitllm_pin: impl Into<String>,
        seed: u64,
    ) -> Self {
        Self {
            keygen_schema_version: KEYGEN_SCHEMA_VERSION,
            seed,
            model_id: model_id.into(),
            checkpoint_hash: checkpoint_hash.into(),
            commitllm_pin: commitllm_pin.into(),
        }
    }
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

/// Encode a v1 `VIKY` key binding header.
///
/// Layout:
/// `[body_len:u32le][body_crc32c:u32le][keygen_schema_version:u16le][seed:u64le]`
/// followed by `model_id`, `checkpoint_hash`, and `commitllm_pin` as
/// `[len:u16le][utf8 bytes]`.
pub fn encode_key_binding_header(header: &KeyBindingHeader) -> Result<Vec<u8>, ViError> {
    validate_keygen_schema_version(header.keygen_schema_version)?;

    let mut body = Vec::new();
    body.extend_from_slice(&header.keygen_schema_version.to_le_bytes());
    body.extend_from_slice(&header.seed.to_le_bytes());
    write_string(&mut body, "model_id", &header.model_id)?;
    write_string(&mut body, "checkpoint_hash", &header.checkpoint_hash)?;
    write_string(&mut body, "commitllm_pin", &header.commitllm_pin)?;

    let body_len = u32::try_from(body.len()).map_err(|_| ViError::Input {
        arg: "key_binding_header".to_owned(),
        reason: "header body exceeds u32 length".to_owned(),
        detail: None,
    })?;
    let crc = crc32c(&body);

    let mut bytes = Vec::with_capacity(KEY_BINDING_PREFIX_LEN + body.len());
    bytes.extend_from_slice(&body_len.to_le_bytes());
    bytes.extend_from_slice(&crc.to_le_bytes());
    bytes.extend_from_slice(&body);
    Ok(bytes)
}

/// Decode a v1 `VIKY` key binding header and return its consumed byte length.
pub fn decode_key_binding_header(bytes: &[u8]) -> Result<(KeyBindingHeader, usize), ViError> {
    if bytes.len() < KEY_BINDING_PREFIX_LEN {
        return Err(corrupt_envelope(
            "VIKY",
            bytes.len(),
            "binding header shorter than 8-byte prefix",
        ));
    }

    let body_len = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let expected_crc = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let body_end = KEY_BINDING_PREFIX_LEN
        .checked_add(body_len)
        .ok_or_else(|| {
            corrupt_envelope(
                "VIKY",
                KEY_BINDING_PREFIX_LEN,
                "binding header length overflows",
            )
        })?;

    if body_end > bytes.len() {
        return Err(corrupt_envelope(
            "VIKY",
            KEY_BINDING_PREFIX_LEN,
            "binding header length overruns payload",
        ));
    }

    let body = &bytes[KEY_BINDING_PREFIX_LEN..body_end];
    let actual_crc = crc32c(body);
    if actual_crc != expected_crc {
        return Err(corrupt_envelope("VIKY", 4, "binding_crc32 mismatch"));
    }

    let mut reader = BindingReader::new(body);
    let keygen_schema_version = reader.read_u16()?;
    validate_keygen_schema_version(keygen_schema_version)?;
    let seed = reader.read_u64()?;
    let model_id = reader.read_string()?;
    let checkpoint_hash = reader.read_string()?;
    let commitllm_pin = reader.read_string()?;
    reader.finish()?;

    Ok((
        KeyBindingHeader {
            keygen_schema_version,
            seed,
            model_id,
            checkpoint_hash,
            commitllm_pin,
        },
        body_end,
    ))
}

/// Prefix opaque `CommitLLM` key bytes with a key binding header.
pub fn encode_viky_payload(
    header: &KeyBindingHeader,
    commitllm_key: &[u8],
) -> Result<Vec<u8>, ViError> {
    let mut payload = encode_key_binding_header(header)?;
    payload.extend_from_slice(commitllm_key);
    Ok(payload)
}

/// Split a `VIKY` payload into its binding header and opaque `CommitLLM` key bytes.
pub fn decode_viky_payload(payload: &[u8]) -> Result<(KeyBindingHeader, &[u8]), ViError> {
    let (header, consumed) = decode_key_binding_header(payload)?;
    Ok((header, &payload[consumed..]))
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

fn validate_keygen_schema_version(version: u16) -> Result<(), ViError> {
    if version == KEYGEN_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(ViError::UnknownVersion {
            envelope: "VIKY",
            field: "keygen_schema_version",
            value: u32::from(version),
            supported: vec![u32::from(KEYGEN_SCHEMA_VERSION)],
        })
    }
}

fn write_string(bytes: &mut Vec<u8>, field: &str, value: &str) -> Result<(), ViError> {
    let len = u16::try_from(value.len()).map_err(|_| ViError::Input {
        arg: field.to_owned(),
        reason: "string exceeds u16 length prefix".to_owned(),
        detail: None,
    })?;
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn crc32c(bytes: &[u8]) -> u32 {
    CRC32C.checksum(bytes)
}

struct BindingReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> BindingReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_u16(&mut self) -> Result<u16, ViError> {
        let bytes = self.read_exact(2, "u16 field overruns binding header")?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u64(&mut self) -> Result<u64, ViError> {
        let bytes = self.read_exact(8, "u64 field overruns binding header")?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_string(&mut self) -> Result<String, ViError> {
        let len_offset = self.offset;
        let len = usize::from(self.read_u16()?);
        let string_offset = self.offset;
        let bytes = self.read_exact(len, "length-prefixed string overruns binding header")?;
        std::str::from_utf8(bytes).map(str::to_owned).map_err(|_| {
            corrupt_envelope(
                "VIKY",
                len_offset.max(string_offset),
                "invalid UTF-8 string",
            )
        })
    }

    fn read_exact(&mut self, len: usize, reason: &'static str) -> Result<&'a [u8], ViError> {
        let end = self.offset.checked_add(len).ok_or_else(|| {
            corrupt_envelope("VIKY", self.offset, "binding header offset overflows")
        })?;
        if end > self.bytes.len() {
            return Err(corrupt_envelope("VIKY", self.offset, reason));
        }

        let bytes = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), ViError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(corrupt_envelope(
                "VIKY",
                self.offset,
                "trailing bytes in binding header",
            ))
        }
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
    use super::{
        decode, decode_key_binding_header, decode_viky_payload, encode, encode_key_binding_header,
        encode_viky_payload, Envelope, KeyBindingHeader, Magic, ENVELOPE_FLAGS, ENVELOPE_VERSION,
        HEADER_LEN, KEYGEN_SCHEMA_VERSION,
    };
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

    #[test]
    fn key_binding_header_round_trips_with_key_payload() {
        let header = realistic_key_header();
        let key_bytes = b"opaque-commitllm-key";
        let payload = encode_viky_payload(&header, key_bytes).expect("header should encode");
        let (decoded, key) = decode_viky_payload(&payload).expect("header should decode");

        assert_eq!(decoded, header);
        assert_eq!(key, key_bytes);
    }

    #[test]
    fn key_binding_golden_bytes_fix_endianness() {
        let header = KeyBindingHeader::new("m", "sha256:abc", "25541e83", 42);
        let encoded = encode_key_binding_header(&header).expect("header should encode");

        assert_eq!(
            encoded,
            vec![
                0x23, 0x00, 0x00, 0x00, // body_len = 35
                0x3d, 0xc9, 0x2c, 0x20, // body CRC32C
                0x01, 0x00, // keygen_schema_version
                0x2a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // seed
                0x01, 0x00, b'm', // model_id
                0x0a, 0x00, b's', b'h', b'a', b'2', b'5', b'6', b':', b'a', b'b', b'c', 0x08, 0x00,
                b'2', b'5', b'5', b'4', b'1', b'e', b'8', b'3',
            ]
        );
    }

    #[test]
    fn key_binding_crc_mismatch_is_corrupt_envelope() {
        let header = realistic_key_header();
        let mut encoded = encode_key_binding_header(&header).expect("header should encode");
        let last = encoded
            .last_mut()
            .expect("encoded header should not be empty");
        *last ^= 0xff;

        let error = decode_key_binding_header(&encoded).expect_err("CRC mismatch should fail");
        assert_eq!(
            error,
            ViError::CorruptEnvelope {
                envelope: "VIKY",
                offset: 4,
                reason: "binding_crc32 mismatch",
            }
        );
    }

    #[test]
    fn key_binding_string_overrun_is_typed_error() {
        let header = realistic_key_header();
        let mut encoded = encode_key_binding_header(&header).expect("header should encode");
        let body_len =
            u32::from_le_bytes([encoded[0], encoded[1], encoded[2], encoded[3]]) as usize;
        let body_start = 8;
        let model_len_offset = body_start + 2 + 8;
        encoded[model_len_offset..model_len_offset + 2].copy_from_slice(&u16::MAX.to_le_bytes());
        let crc = super::crc32c(&encoded[body_start..body_start + body_len]);
        encoded[4..8].copy_from_slice(&crc.to_le_bytes());

        let error = decode_key_binding_header(&encoded).expect_err("string overrun should fail");
        assert_eq!(
            error,
            ViError::CorruptEnvelope {
                envelope: "VIKY",
                offset: 12,
                reason: "length-prefixed string overruns binding header",
            }
        );
    }

    #[test]
    fn key_binding_unknown_schema_version_is_unknown_version() {
        let header = KeyBindingHeader {
            keygen_schema_version: 2,
            ..realistic_key_header()
        };
        let error = encode_key_binding_header(&header).expect_err("unknown schema should fail");
        assert_eq!(
            error,
            ViError::UnknownVersion {
                envelope: "VIKY",
                field: "keygen_schema_version",
                value: 2,
                supported: vec![u32::from(KEYGEN_SCHEMA_VERSION)],
            }
        );
    }

    fn realistic_key_header() -> KeyBindingHeader {
        KeyBindingHeader::new(
            "llama-3.1-8b-w8a8",
            "sha256:0123456789abcdef",
            "25541e83",
            7,
        )
    }
}
