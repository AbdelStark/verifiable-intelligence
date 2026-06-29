//! Envelope codec for `CommitLLM` receipts.
//!
//! Implements `VIKY` (verifier-key), `VIRC` (chat receipt), and `VIAU` (audit
//! receipt) headers per RFC-0003. Leaf crate: no async, no networking.

// RFC-0003 parsing failures must flow through the shared `ViError` taxonomy.
#![allow(clippy::result_large_err)]

use vi_errors::{IdentityFields, PhaseId, ViError};

/// Supported v1 binary-envelope version.
pub const ENVELOPE_VERSION: u8 = 1;

/// Supported v1 verifier-key binding schema version.
pub const KEYGEN_SCHEMA_VERSION: u16 = 1;

/// Supported v1 receipt binding schema version.
pub const RECEIPT_SCHEMA_VERSION: u16 = 1;

/// Supported v1 audit binding schema version.
pub const AUDIT_SCHEMA_VERSION: u16 = 1;

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

    /// Identity fields shared with receipts for cross-binding checks.
    #[must_use]
    pub fn identity_fields(&self) -> IdentityFields {
        IdentityFields::new(&self.model_id, &self.checkpoint_hash, &self.commitllm_pin)
    }
}

/// Receipt identity binding header carried inside `VIRC` payloads.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReceiptBindingHeader {
    /// Receipt binding schema version.
    pub receipt_schema_version: u16,
    /// Public model identifier.
    pub model_id: String,
    /// Canonical checkpoint hash.
    pub checkpoint_hash: String,
    /// Pinned upstream `CommitLLM` short revision.
    pub commitllm_pin: String,
    /// Hash of the verifier key envelope expected for this receipt.
    pub key_hash: String,
    /// Hash of canonical prompt bytes or prompt representation.
    pub prompt_hash: String,
    /// Hash of delivered answer bytes.
    pub answer_hash: String,
    /// Number of generated tokens bound by the receipt.
    pub generated_token_count: u64,
}

impl ReceiptBindingHeader {
    /// Construct a v1 receipt binding header.
    #[must_use]
    pub fn new(
        model_id: impl Into<String>,
        checkpoint_hash: impl Into<String>,
        commitllm_pin: impl Into<String>,
        key_hash: impl Into<String>,
        prompt_hash: impl Into<String>,
        answer_hash: impl Into<String>,
        generated_token_count: u64,
    ) -> Self {
        Self {
            receipt_schema_version: RECEIPT_SCHEMA_VERSION,
            model_id: model_id.into(),
            checkpoint_hash: checkpoint_hash.into(),
            commitllm_pin: commitllm_pin.into(),
            key_hash: key_hash.into(),
            prompt_hash: prompt_hash.into(),
            answer_hash: answer_hash.into(),
            generated_token_count,
        }
    }

    /// Identity fields shared with verifier keys for cross-binding checks.
    #[must_use]
    pub fn identity_fields(&self) -> IdentityFields {
        IdentityFields::new(&self.model_id, &self.checkpoint_hash, &self.commitllm_pin)
    }
}

/// Verifier-requested audit tier encoded in `VIAU` binding headers.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AuditTier {
    /// Receipt-only verification; no audit opening required.
    ReceiptOnly = 0,
    /// Routine verifier challenge.
    Routine = 1,
    /// Deep verifier challenge.
    Deep = 2,
    /// Full verifier challenge.
    Full = 3,
}

impl AuditTier {
    /// Stable byte value used in `VIAU` binding headers.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Stable tier name used by public APIs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReceiptOnly => "receipt-only",
            Self::Routine => "routine",
            Self::Deep => "deep",
            Self::Full => "full",
        }
    }
}

impl TryFrom<u8> for AuditTier {
    type Error = ViError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::ReceiptOnly),
            1 => Ok(Self::Routine),
            2 => Ok(Self::Deep),
            3 => Ok(Self::Full),
            _ => Err(ViError::UnsupportedTier {
                requested: value.to_string(),
                reason: "unknown audit tier byte".to_owned(),
            }),
        }
    }
}

impl std::fmt::Display for AuditTier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Verifier challenge tuple that a `VIAU` audit opening must bind to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuditChallenge {
    /// Requested audit tier.
    pub tier: AuditTier,
    /// Token position challenged by the verifier.
    pub token_index: u64,
    /// Layer indices opened by the provider for this challenge.
    pub layer_indices: Vec<u32>,
}

impl AuditChallenge {
    /// Construct an audit challenge tuple.
    #[must_use]
    pub fn new(tier: AuditTier, token_index: u64, layer_indices: impl Into<Vec<u32>>) -> Self {
        Self {
            tier,
            token_index,
            layer_indices: layer_indices.into(),
        }
    }
}

/// Audit binding header carried inside `VIAU` payloads.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuditBindingHeader {
    /// Audit binding schema version.
    pub audit_schema_version: u16,
    /// Hash of the `VIRC` receipt this audit opening belongs to.
    pub receipt_hash: String,
    /// Requested audit tier.
    pub tier: AuditTier,
    /// Token position challenged by the verifier.
    pub token_index: u64,
    /// Layer indices opened by the provider for this challenge.
    pub layer_indices: Vec<u32>,
}

impl AuditBindingHeader {
    /// Construct a v1 audit binding header.
    #[must_use]
    pub fn new(
        receipt_hash: impl Into<String>,
        tier: AuditTier,
        token_index: u64,
        layer_indices: impl Into<Vec<u32>>,
    ) -> Self {
        Self {
            audit_schema_version: AUDIT_SCHEMA_VERSION,
            receipt_hash: receipt_hash.into(),
            tier,
            token_index,
            layer_indices: layer_indices.into(),
        }
    }

    /// Challenge tuple bound by this audit opening.
    #[must_use]
    pub fn challenge(&self) -> AuditChallenge {
        AuditChallenge::new(self.tier, self.token_index, self.layer_indices.clone())
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

    let mut reader = BindingReader::new("VIKY", body);
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

/// Encode a v1 `VIRC` receipt binding header.
///
/// Layout:
/// `[body_len:u32le][body_crc32c:u32le][receipt_schema_version:u16le]`
/// `[generated_token_count:u64le]` followed by `model_id`, `checkpoint_hash`,
/// `commitllm_pin`, `key_hash`, `prompt_hash`, and `answer_hash` as
/// `[len:u16le][utf8 bytes]`.
pub fn encode_receipt_binding_header(header: &ReceiptBindingHeader) -> Result<Vec<u8>, ViError> {
    validate_receipt_schema_version(header.receipt_schema_version)?;

    let mut body = Vec::new();
    body.extend_from_slice(&header.receipt_schema_version.to_le_bytes());
    body.extend_from_slice(&header.generated_token_count.to_le_bytes());
    write_string(&mut body, "model_id", &header.model_id)?;
    write_string(&mut body, "checkpoint_hash", &header.checkpoint_hash)?;
    write_string(&mut body, "commitllm_pin", &header.commitllm_pin)?;
    write_string(&mut body, "key_hash", &header.key_hash)?;
    write_string(&mut body, "prompt_hash", &header.prompt_hash)?;
    write_string(&mut body, "answer_hash", &header.answer_hash)?;

    let body_len = u32::try_from(body.len()).map_err(|_| ViError::Input {
        arg: "receipt_binding_header".to_owned(),
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

/// Decode a v1 `VIRC` receipt binding header and return its consumed byte length.
pub fn decode_receipt_binding_header(
    bytes: &[u8],
) -> Result<(ReceiptBindingHeader, usize), ViError> {
    if bytes.len() < KEY_BINDING_PREFIX_LEN {
        return Err(corrupt_envelope(
            "VIRC",
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
                "VIRC",
                KEY_BINDING_PREFIX_LEN,
                "binding header length overflows",
            )
        })?;

    if body_end > bytes.len() {
        return Err(corrupt_envelope(
            "VIRC",
            KEY_BINDING_PREFIX_LEN,
            "binding header length overruns payload",
        ));
    }

    let body = &bytes[KEY_BINDING_PREFIX_LEN..body_end];
    let actual_crc = crc32c(body);
    if actual_crc != expected_crc {
        return Err(corrupt_envelope("VIRC", 4, "binding_crc32 mismatch"));
    }

    let mut reader = BindingReader::new("VIRC", body);
    let receipt_schema_version = reader.read_u16()?;
    validate_receipt_schema_version(receipt_schema_version)?;
    let generated_token_count = reader.read_u64()?;
    let model_id = reader.read_string()?;
    let checkpoint_hash = reader.read_string()?;
    let commitllm_pin = reader.read_string()?;
    let key_hash = reader.read_string()?;
    let prompt_hash = reader.read_string()?;
    let answer_hash = reader.read_string()?;
    reader.finish()?;

    Ok((
        ReceiptBindingHeader {
            receipt_schema_version,
            model_id,
            checkpoint_hash,
            commitllm_pin,
            key_hash,
            prompt_hash,
            answer_hash,
            generated_token_count,
        },
        body_end,
    ))
}

/// Prefix opaque `CommitLLM` receipt bytes with a receipt binding header.
pub fn encode_virc_payload(
    header: &ReceiptBindingHeader,
    commitllm_receipt: &[u8],
) -> Result<Vec<u8>, ViError> {
    let mut payload = encode_receipt_binding_header(header)?;
    payload.extend_from_slice(commitllm_receipt);
    Ok(payload)
}

/// Split a `VIRC` payload into its binding header and opaque `CommitLLM` receipt bytes.
pub fn decode_virc_payload(payload: &[u8]) -> Result<(ReceiptBindingHeader, &[u8]), ViError> {
    let (header, consumed) = decode_receipt_binding_header(payload)?;
    Ok((header, &payload[consumed..]))
}

/// Ensure a receipt binds to the same identity as the verifier key.
pub fn check_receipt_identity(
    key: &KeyBindingHeader,
    receipt: &ReceiptBindingHeader,
) -> Result<(), ViError> {
    let expected = key.identity_fields();
    let actual = receipt.identity_fields();

    if expected == actual {
        Ok(())
    } else {
        Err(ViError::IdentityMismatch { expected, actual })
    }
}

/// Encode a v1 `VIAU` audit binding header.
///
/// Layout:
/// `[body_len:u32le][body_crc32c:u32le][audit_schema_version:u16le]`
/// `[tier:u8][token_index:u64le][layer_count:u16le][layer_index:u32le]*`
/// followed by `receipt_hash` as `[len:u16le][utf8 bytes]`.
pub fn encode_audit_binding_header(header: &AuditBindingHeader) -> Result<Vec<u8>, ViError> {
    validate_audit_schema_version(header.audit_schema_version)?;
    let layer_count = u16::try_from(header.layer_indices.len()).map_err(|_| ViError::Input {
        arg: "layer_indices".to_owned(),
        reason: "layer index count exceeds u16 length prefix".to_owned(),
        detail: None,
    })?;

    let mut body = Vec::new();
    body.extend_from_slice(&header.audit_schema_version.to_le_bytes());
    body.push(header.tier.as_u8());
    body.extend_from_slice(&header.token_index.to_le_bytes());
    body.extend_from_slice(&layer_count.to_le_bytes());
    for layer_index in &header.layer_indices {
        body.extend_from_slice(&layer_index.to_le_bytes());
    }
    write_string(&mut body, "receipt_hash", &header.receipt_hash)?;

    let body_len = u32::try_from(body.len()).map_err(|_| ViError::Input {
        arg: "audit_binding_header".to_owned(),
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

/// Decode a v1 `VIAU` audit binding header and return its consumed byte length.
pub fn decode_audit_binding_header(bytes: &[u8]) -> Result<(AuditBindingHeader, usize), ViError> {
    if bytes.len() < KEY_BINDING_PREFIX_LEN {
        return Err(corrupt_envelope(
            "VIAU",
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
                "VIAU",
                KEY_BINDING_PREFIX_LEN,
                "binding header length overflows",
            )
        })?;

    if body_end > bytes.len() {
        return Err(corrupt_envelope(
            "VIAU",
            KEY_BINDING_PREFIX_LEN,
            "binding header length overruns payload",
        ));
    }

    let body = &bytes[KEY_BINDING_PREFIX_LEN..body_end];
    let actual_crc = crc32c(body);
    if actual_crc != expected_crc {
        return Err(corrupt_envelope("VIAU", 4, "binding_crc32 mismatch"));
    }

    let mut reader = BindingReader::new("VIAU", body);
    let audit_schema_version = reader.read_u16()?;
    validate_audit_schema_version(audit_schema_version)?;
    let tier = AuditTier::try_from(reader.read_u8()?)?;
    let token_index = reader.read_u64()?;
    let layer_count = usize::from(reader.read_u16()?);
    let mut layer_indices = Vec::with_capacity(layer_count);
    for _ in 0..layer_count {
        layer_indices.push(reader.read_u32()?);
    }
    let receipt_hash = reader.read_string()?;
    reader.finish()?;

    Ok((
        AuditBindingHeader {
            audit_schema_version,
            receipt_hash,
            tier,
            token_index,
            layer_indices,
        },
        body_end,
    ))
}

/// Prefix opaque `CommitLLM` audit bytes with an audit binding header.
pub fn encode_viau_payload(
    header: &AuditBindingHeader,
    commitllm_audit: &[u8],
) -> Result<Vec<u8>, ViError> {
    let mut payload = encode_audit_binding_header(header)?;
    payload.extend_from_slice(commitllm_audit);
    Ok(payload)
}

/// Split a `VIAU` payload into its binding header and opaque `CommitLLM` audit bytes.
pub fn decode_viau_payload(payload: &[u8]) -> Result<(AuditBindingHeader, &[u8]), ViError> {
    let (header, consumed) = decode_audit_binding_header(payload)?;
    Ok((header, &payload[consumed..]))
}

/// Ensure an audit opening belongs to the receipt currently under verification.
pub fn check_audit_receipt_hash(
    audit: &AuditBindingHeader,
    expected_receipt_hash: &str,
) -> Result<(), ViError> {
    if audit.receipt_hash == expected_receipt_hash {
        Ok(())
    } else {
        Err(ViError::HashMismatch {
            expected: expected_receipt_hash.to_owned(),
            actual: audit.receipt_hash.clone(),
        })
    }
}

/// Ensure an audit opening binds to the verifier-requested challenge tuple.
pub fn check_audit_challenge(
    audit: &AuditBindingHeader,
    expected: &AuditChallenge,
) -> Result<(), ViError> {
    let actual = audit.challenge();

    if &actual == expected {
        Ok(())
    } else {
        Err(ViError::VerificationFailed {
            phase: PhaseId::KvProvenance,
            measured: None,
            tolerance: None,
            extra: Some(challenge_mismatch_detail(expected, &actual)),
        })
    }
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

fn validate_receipt_schema_version(version: u16) -> Result<(), ViError> {
    if version == RECEIPT_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(ViError::UnknownVersion {
            envelope: "VIRC",
            field: "receipt_schema_version",
            value: u32::from(version),
            supported: vec![u32::from(RECEIPT_SCHEMA_VERSION)],
        })
    }
}

fn validate_audit_schema_version(version: u16) -> Result<(), ViError> {
    if version == AUDIT_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(ViError::UnknownVersion {
            envelope: "VIAU",
            field: "audit_schema_version",
            value: u32::from(version),
            supported: vec![u32::from(AUDIT_SCHEMA_VERSION)],
        })
    }
}

fn challenge_mismatch_detail(
    expected: &AuditChallenge,
    actual: &AuditChallenge,
) -> serde_json::Value {
    serde_json::json!({
        "expected": audit_challenge_detail(expected),
        "actual": audit_challenge_detail(actual),
    })
}

fn audit_challenge_detail(challenge: &AuditChallenge) -> serde_json::Value {
    serde_json::json!({
        "tier": challenge.tier.as_str(),
        "token_index": challenge.token_index,
        "layer_indices": challenge.layer_indices,
    })
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
    envelope: &'static str,
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> BindingReader<'a> {
    fn new(envelope: &'static str, bytes: &'a [u8]) -> Self {
        Self {
            envelope,
            bytes,
            offset: 0,
        }
    }

    fn read_u16(&mut self) -> Result<u16, ViError> {
        let bytes = self.read_exact(2, "u16 field overruns binding header")?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u8(&mut self) -> Result<u8, ViError> {
        let bytes = self.read_exact(1, "u8 field overruns binding header")?;
        Ok(bytes[0])
    }

    fn read_u32(&mut self) -> Result<u32, ViError> {
        let bytes = self.read_exact(4, "u32 field overruns binding header")?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
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
                self.envelope,
                len_offset.max(string_offset),
                "invalid UTF-8 string",
            )
        })
    }

    fn read_exact(&mut self, len: usize, reason: &'static str) -> Result<&'a [u8], ViError> {
        let end = self.offset.checked_add(len).ok_or_else(|| {
            corrupt_envelope(
                self.envelope,
                self.offset,
                "binding header offset overflows",
            )
        })?;
        if end > self.bytes.len() {
            return Err(corrupt_envelope(self.envelope, self.offset, reason));
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
                self.envelope,
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
        check_audit_challenge, check_audit_receipt_hash, check_receipt_identity, decode,
        decode_key_binding_header, decode_viau_payload, decode_viky_payload, decode_virc_payload,
        encode, encode_audit_binding_header, encode_key_binding_header,
        encode_receipt_binding_header, encode_viau_payload, encode_viky_payload,
        encode_virc_payload, AuditBindingHeader, AuditChallenge, AuditTier, Envelope,
        KeyBindingHeader, Magic, ReceiptBindingHeader, AUDIT_SCHEMA_VERSION, ENVELOPE_FLAGS,
        ENVELOPE_VERSION, HEADER_LEN, KEYGEN_SCHEMA_VERSION, RECEIPT_SCHEMA_VERSION,
    };
    use proptest::prelude::{any, prop_oneof, Just, ProptestConfig};
    use proptest::string::string_regex;
    use proptest::{collection, proptest};
    use serde_json::json;
    use vi_errors::{IdentityFields, PhaseId, ViError};

    fn magic_strategy() -> impl proptest::strategy::Strategy<Value = Magic> {
        prop_oneof![Just(Magic::VIKY), Just(Magic::VIRC), Just(Magic::VIAU)]
    }

    fn audit_tier_strategy() -> impl proptest::strategy::Strategy<Value = AuditTier> {
        prop_oneof![
            Just(AuditTier::ReceiptOnly),
            Just(AuditTier::Routine),
            Just(AuditTier::Deep),
            Just(AuditTier::Full),
        ]
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

        #[test]
        fn receipt_binding_header_round_trips(
            model_id in string_regex("[a-z0-9:_\\-.]{0,64}").expect("valid model regex"),
            checkpoint_hash in string_regex("[a-z0-9:_\\-.]{0,96}").expect("valid checkpoint regex"),
            commitllm_pin in string_regex("[a-f0-9]{8}").expect("valid pin regex"),
            key_hash in string_regex("sha256:[a-f0-9]{8,64}").expect("valid key hash regex"),
            prompt_hash in string_regex("sha256:[a-f0-9]{8,64}").expect("valid prompt hash regex"),
            answer_hash in string_regex("sha256:[a-f0-9]{8,64}").expect("valid answer hash regex"),
            generated_token_count in any::<u64>(),
            receipt_bytes in collection::vec(any::<u8>(), 0..1024),
        ) {
            let header = ReceiptBindingHeader::new(
                model_id,
                checkpoint_hash,
                commitllm_pin,
                key_hash,
                prompt_hash,
                answer_hash,
                generated_token_count,
            );
            let payload = encode_virc_payload(&header, &receipt_bytes)?;
            let (decoded, receipt) = decode_virc_payload(&payload)?;

            proptest::prop_assert_eq!(decoded, header);
            proptest::prop_assert_eq!(receipt, receipt_bytes.as_slice());
        }

        #[test]
        fn audit_binding_header_round_trips(
            receipt_hash in string_regex("sha256:[a-f0-9]{8,64}").expect("valid receipt hash regex"),
            tier in audit_tier_strategy(),
            token_index in any::<u64>(),
            layer_indices in collection::vec(any::<u32>(), 0..64),
            audit_bytes in collection::vec(any::<u8>(), 0..1024),
        ) {
            let header = AuditBindingHeader::new(
                receipt_hash,
                tier,
                token_index,
                layer_indices,
            );
            let payload = encode_viau_payload(&header, &audit_bytes)?;
            let (decoded, audit) = decode_viau_payload(&payload)?;

            proptest::prop_assert_eq!(decoded, header);
            proptest::prop_assert_eq!(audit, audit_bytes.as_slice());
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

    #[test]
    fn receipt_binding_generated_token_count_zero_is_parseable() {
        let header = ReceiptBindingHeader::new(
            "llama-3.1-8b-w8a8",
            "sha256:checkpoint",
            "25541e83",
            "sha256:key",
            "sha256:prompt",
            "sha256:answer",
            0,
        );
        let payload =
            encode_virc_payload(&header, b"opaque-receipt").expect("header should encode");
        let (decoded, receipt) = decode_virc_payload(&payload).expect("header should decode");

        assert_eq!(decoded.generated_token_count, 0);
        assert_eq!(decoded, header);
        assert_eq!(receipt, b"opaque-receipt");
    }

    #[test]
    fn receipt_identity_mismatch_includes_expected_and_actual() {
        let key = KeyBindingHeader::new("llama-3.1-8b-w8a8", "sha256:checkpoint", "25541e83", 7);
        let receipt = ReceiptBindingHeader::new(
            "qwen2.5-7b-w8a8",
            "sha256:checkpoint",
            "25541e83",
            "sha256:key",
            "sha256:prompt",
            "sha256:answer",
            32,
        );

        let error =
            check_receipt_identity(&key, &receipt).expect_err("identity mismatch should fail");
        assert_eq!(
            error,
            ViError::IdentityMismatch {
                expected: IdentityFields::new("llama-3.1-8b-w8a8", "sha256:checkpoint", "25541e83"),
                actual: IdentityFields::new("qwen2.5-7b-w8a8", "sha256:checkpoint", "25541e83"),
            }
        );
    }

    #[test]
    fn receipt_identity_match_passes() {
        let key = realistic_key_header();
        let receipt = realistic_receipt_header();

        check_receipt_identity(&key, &receipt).expect("matching identity should pass");
    }

    #[test]
    fn receipt_binding_crc_mismatch_is_corrupt_envelope() {
        let header = realistic_receipt_header();
        let mut encoded = encode_receipt_binding_header(&header).expect("header should encode");
        let last = encoded
            .last_mut()
            .expect("encoded header should not be empty");
        *last ^= 0xff;

        let error =
            super::decode_receipt_binding_header(&encoded).expect_err("CRC mismatch should fail");
        assert_eq!(
            error,
            ViError::CorruptEnvelope {
                envelope: "VIRC",
                offset: 4,
                reason: "binding_crc32 mismatch",
            }
        );
    }

    #[test]
    fn receipt_binding_unknown_schema_version_is_unknown_version() {
        let header = ReceiptBindingHeader {
            receipt_schema_version: 2,
            ..realistic_receipt_header()
        };
        let error = encode_receipt_binding_header(&header).expect_err("unknown schema should fail");
        assert_eq!(
            error,
            ViError::UnknownVersion {
                envelope: "VIRC",
                field: "receipt_schema_version",
                value: 2,
                supported: vec![u32::from(RECEIPT_SCHEMA_VERSION)],
            }
        );
    }

    #[test]
    fn audit_receipt_hash_mismatch_is_rejected() {
        let header = realistic_audit_header();
        let error = check_audit_receipt_hash(&header, "sha256:expected-receipt")
            .expect_err("receipt hash mismatch should fail");

        assert_eq!(
            error,
            ViError::HashMismatch {
                expected: "sha256:expected-receipt".to_owned(),
                actual: "sha256:receipt".to_owned(),
            }
        );
    }

    #[test]
    fn audit_challenge_mismatch_is_rejected() {
        let header = realistic_audit_header();
        let expected = AuditChallenge::new(AuditTier::Routine, 8, vec![0, 4, 8]);
        let error =
            check_audit_challenge(&header, &expected).expect_err("challenge mismatch should fail");

        assert_eq!(
            error,
            ViError::VerificationFailed {
                phase: PhaseId::KvProvenance,
                measured: None,
                tolerance: None,
                extra: Some(json!({
                    "expected": {
                        "tier": "routine",
                        "token_index": 8,
                        "layer_indices": [0, 4, 8],
                    },
                    "actual": {
                        "tier": "deep",
                        "token_index": 12,
                        "layer_indices": [1, 7, 13],
                    },
                })),
            }
        );
    }

    #[test]
    fn audit_binding_checks_pass_for_matching_request() {
        let header = realistic_audit_header();
        let expected = AuditChallenge::new(AuditTier::Deep, 12, vec![1, 7, 13]);

        check_audit_receipt_hash(&header, "sha256:receipt")
            .expect("matching receipt hash should pass");
        check_audit_challenge(&header, &expected).expect("matching challenge should pass");
    }

    #[test]
    fn audit_binding_unknown_schema_version_is_unknown_version() {
        let header = AuditBindingHeader {
            audit_schema_version: 2,
            ..realistic_audit_header()
        };
        let error = encode_audit_binding_header(&header).expect_err("unknown schema should fail");
        assert_eq!(
            error,
            ViError::UnknownVersion {
                envelope: "VIAU",
                field: "audit_schema_version",
                value: 2,
                supported: vec![u32::from(AUDIT_SCHEMA_VERSION)],
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

    fn realistic_receipt_header() -> ReceiptBindingHeader {
        ReceiptBindingHeader::new(
            "llama-3.1-8b-w8a8",
            "sha256:0123456789abcdef",
            "25541e83",
            "sha256:key",
            "sha256:prompt",
            "sha256:answer",
            32,
        )
    }

    fn realistic_audit_header() -> AuditBindingHeader {
        AuditBindingHeader::new("sha256:receipt", AuditTier::Deep, 12, vec![1, 7, 13])
    }
}
