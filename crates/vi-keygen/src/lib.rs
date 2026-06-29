//! Verifier-key generation.
//!
//! Filled in by RFC-0004 implementation issues.

// RFC-0004 failures must flow through the shared `ViError` taxonomy.
#![allow(clippy::result_large_err)]

use std::{
    env, fs,
    fs::OpenOptions,
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

use commitllm_core::serialize;
use reqwest::{
    blocking::Client,
    header::{HeaderMap, HeaderValue, RANGE},
    StatusCode,
};
use sha2::{Digest, Sha256};
use vi_errors::{NetworkErrorKind, ViError};
use vi_receipt::{encode_viky_payload, Envelope, KeyBindingHeader, Magic};

/// Default Hugging Face Hub endpoint used for checkpoint mirror downloads.
pub const HUGGING_FACE_ENDPOINT: &str = "https://huggingface.co";

/// Full upstream `CommitLLM` commit SHA used by key generation.
pub const COMMITLLM_PIN: &str = "25541e83347655e44ad6e84eb901e1e7ae392a66";

/// Buyer-facing short `CommitLLM` pin carried in demo proof bundles.
pub const COMMITLLM_SHORT_PIN: &str = "25541e83";

/// Canonical checkpoint hash result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointHash {
    /// SHA-256 digest over the RFC-0004 canonical checkpoint file byte stream.
    pub hash: [u8; 32],
    /// Relative file paths used as hash input, in canonical order.
    pub files: Vec<PathBuf>,
}

impl CheckpointHash {
    /// Lowercase hex representation of `hash`.
    #[must_use]
    pub fn hash_hex(&self) -> String {
        let mut hex = String::with_capacity(64);
        for byte in self.hash {
            use std::fmt::Write as _;
            let _ = write!(&mut hex, "{byte:02x}");
        }
        hex
    }
}

/// Key generation request options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeygenOptions {
    /// Public model identifier to bind into the verifier-key envelope.
    pub model_id: String,
    /// Local checkpoint directory containing `config.json`, safetensors, and tokenizer metadata.
    pub checkpoint: PathBuf,
    /// Destination for the emitted `VIKY` envelope.
    pub output: PathBuf,
    /// Deterministic key-generation seed bound into the `VIKY` header.
    pub seed: u64,
    /// Allow replacing existing output files.
    pub force: bool,
    /// Optional expected canonical checkpoint hash, formatted as `sha256:<hex>`.
    pub expected_checkpoint_hash: Option<String>,
    /// Convert an expected-checkpoint mismatch into a warning instead of an error.
    pub allow_checkpoint_drift: bool,
}

impl KeygenOptions {
    /// Build a key generation request with conservative defaults.
    #[must_use]
    pub fn new(
        model_id: impl Into<String>,
        checkpoint: impl AsRef<Path>,
        output: impl AsRef<Path>,
    ) -> Self {
        Self {
            model_id: model_id.into(),
            checkpoint: checkpoint.as_ref().to_path_buf(),
            output: output.as_ref().to_path_buf(),
            seed: 0,
            force: false,
            expected_checkpoint_hash: None,
            allow_checkpoint_drift: false,
        }
    }

    /// Set the deterministic key-generation seed.
    #[must_use]
    pub const fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Allow replacing existing output files.
    #[must_use]
    pub const fn with_force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }

    /// Require the local checkpoint hash to match an expected hash.
    #[must_use]
    pub fn with_expected_checkpoint_hash(mut self, checkpoint_hash: impl Into<String>) -> Self {
        self.expected_checkpoint_hash = Some(checkpoint_hash.into());
        self
    }

    /// Allow local checkpoint drift relative to the expected hash.
    #[must_use]
    pub const fn with_allow_checkpoint_drift(mut self, allow_checkpoint_drift: bool) -> Self {
        self.allow_checkpoint_drift = allow_checkpoint_drift;
        self
    }
}

/// Optional decode artifact emitted alongside a verifier key.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DecodeArtifactReport {
    /// Path where the decode artifact was written.
    pub output: PathBuf,
    /// SHA-256 of the emitted decode artifact bytes.
    pub artifact_hash: String,
    /// Number of bytes written for the decode artifact.
    pub artifact_size_bytes: usize,
}

/// Result summary for a generated verifier key.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct KeygenReport {
    /// Public model identifier bound into the `VIKY` envelope.
    pub model_id: String,
    /// Canonical local checkpoint hash.
    pub checkpoint_hash: String,
    /// SHA-256 of the emitted verifier-key envelope.
    pub key_hash: String,
    /// Number of bytes written for the verifier-key envelope.
    pub key_size_bytes: usize,
    /// Destination where the verifier-key envelope was written.
    pub output: PathBuf,
    /// Deterministic key-generation seed bound into the `VIKY` header.
    pub seed: u64,
    /// Full pinned upstream `CommitLLM` commit SHA.
    pub commitllm_revision: String,
    /// Short upstream `CommitLLM` pin carried in buyer-facing bindings.
    pub commitllm_pin: String,
    /// Optional adjacent decode artifact emitted by upstream `CommitLLM`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decode_artifact: Option<DecodeArtifactReport>,
    /// Non-fatal warnings.
    pub warnings: Vec<String>,
}

/// Hugging Face checkpoint mirror request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HfCheckpointMirror {
    /// Hugging Face endpoint, defaults to [`HUGGING_FACE_ENDPOINT`].
    pub endpoint: String,
    /// Repository identifier, for example `owner/model-name`.
    pub repo_id: String,
    /// Revision, branch, tag, or commit SHA.
    pub revision: String,
    /// Relative checkpoint files to download from the mirror.
    pub files: Vec<PathBuf>,
}

impl HfCheckpointMirror {
    /// Build a Hugging Face checkpoint mirror request using the default endpoint.
    #[must_use]
    pub fn new(
        repo_id: impl Into<String>,
        revision: impl Into<String>,
        files: impl Into<Vec<PathBuf>>,
    ) -> Self {
        Self {
            endpoint: HUGGING_FACE_ENDPOINT.to_owned(),
            repo_id: repo_id.into(),
            revision: revision.into(),
            files: files.into(),
        }
    }

    /// Override the HTTP endpoint, primarily for tests and self-hosted mirrors.
    #[must_use]
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }
}

/// Downloaded checkpoint material and its canonical hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedCheckpoint {
    /// Cache directory containing the downloaded checkpoint files.
    pub directory: PathBuf,
    /// Canonical checkpoint hash computed after download/resume completes.
    pub hash: CheckpointHash,
}

/// Download a Hugging Face checkpoint mirror into the platform cache directory.
pub fn download_hf_checkpoint(
    mirror: &HfCheckpointMirror,
) -> Result<DownloadedCheckpoint, ViError> {
    let cache_root = default_checkpoint_cache_dir()?;
    download_hf_checkpoint_to(mirror, cache_root)
}

/// Download a Hugging Face checkpoint mirror into a caller-provided cache root.
///
/// Existing partial files are resumed with HTTP `Range` requests. If a server
/// ignores `Range`, the file is restarted from byte 0. If the server reports the
/// range is unsatisfiable for a non-empty local file, the file is treated as an
/// already-complete cache entry; later expected-hash checks can reject drift.
pub fn download_hf_checkpoint_to(
    mirror: &HfCheckpointMirror,
    cache_root: impl AsRef<Path>,
) -> Result<DownloadedCheckpoint, ViError> {
    validate_mirror(mirror)?;
    let client = Client::new();
    let directory = checkpoint_cache_dir(cache_root.as_ref(), &mirror.repo_id, &mirror.revision);
    fs::create_dir_all(&directory).map_err(|error| {
        input_error(
            directory.display().to_string(),
            format!("failed to create checkpoint cache directory: {error}"),
        )
    })?;

    for relative_path in &mirror.files {
        validate_relative_path(relative_path)?;
        let url = hf_file_url(mirror, relative_path)?;
        let destination = directory.join(relative_path);
        download_file_with_resume(&client, &url, &destination)?;
    }

    let hash = canonical_checkpoint_hash(&directory)?;
    Ok(DownloadedCheckpoint { directory, hash })
}

/// Platform cache root for checkpoint mirrors.
///
/// Unix follows `$XDG_CACHE_HOME/verifiable-intelligence/checkpoints` when set,
/// otherwise platform defaults are used.
pub fn default_checkpoint_cache_dir() -> Result<PathBuf, ViError> {
    if let Some(cache_home) = non_empty_env("XDG_CACHE_HOME") {
        return Ok(cache_home
            .join("verifiable-intelligence")
            .join("checkpoints"));
    }

    platform_cache_dir().map(|cache_dir| {
        cache_dir
            .join("verifiable-intelligence")
            .join("checkpoints")
    })
}

/// Compute the RFC-0004 canonical checkpoint hash for a local checkpoint directory.
///
/// RFC-0004 fixes the byte-stream order as:
/// `config.json`, every direct `*.safetensors` file in lexicographic filename
/// order, `tokenizer.json`, and `tokenizer.model` when present. The returned
/// `files` list records that exact relative order for callers and diagnostics.
pub fn canonical_checkpoint_hash(
    checkpoint_dir: impl AsRef<Path>,
) -> Result<CheckpointHash, ViError> {
    let checkpoint_dir = checkpoint_dir.as_ref();
    if !checkpoint_dir.is_dir() {
        return Err(input_error(
            "checkpoint",
            "checkpoint path is not a directory",
        ));
    }

    let files = canonical_checkpoint_files(checkpoint_dir)?;
    let mut hasher = Sha256::new();

    for relative_path in &files {
        let path = checkpoint_dir.join(relative_path);
        let bytes = fs::read(&path).map_err(|error| read_error(&path, &error))?;
        hasher.update(bytes);
    }

    Ok(CheckpointHash {
        hash: hasher.finalize().into(),
        files,
    })
}

/// Generate a verifier key envelope from a local checkpoint directory.
pub fn keygen(
    model_id: impl Into<String>,
    checkpoint: impl AsRef<Path>,
    seed: u64,
    output: impl AsRef<Path>,
) -> Result<KeygenReport, ViError> {
    let options = KeygenOptions::new(model_id, checkpoint, output).with_seed(seed);
    keygen_with_options(&options)
}

/// Generate a verifier key envelope using explicit request options.
pub fn keygen_with_options(options: &KeygenOptions) -> Result<KeygenReport, ViError> {
    validate_keygen_options(options)?;
    refuse_overwrite(&options.output, options.force)?;

    let checkpoint_hash = canonical_checkpoint_hash(&options.checkpoint)?;
    let checkpoint_hash = checkpoint_hash_string(&checkpoint_hash);
    let mut warnings = Vec::new();
    check_expected_checkpoint_hash(options, &checkpoint_hash, &mut warnings)?;

    let upstream = commitllm_keygen::generate_key(&options.checkpoint, seed_bytes(options.seed))
        .map_err(|error| {
            input_error(
                "checkpoint",
                format!("CommitLLM key generation failed: {error}"),
            )
        })?;
    let commitllm_key = serialize::serialize_key(&upstream.key);
    let header = KeyBindingHeader::new(
        &options.model_id,
        &checkpoint_hash,
        COMMITLLM_SHORT_PIN,
        options.seed,
    );
    let payload = encode_viky_payload(&header, &commitllm_key)?;
    let key_bytes = Envelope::new(Magic::VIKY, payload).encode()?;

    let decode_artifact = upstream.decode_artifact.as_ref().map(|artifact| {
        let bytes = serialize::serialize_decode_artifact(artifact);
        let output = decode_artifact_path(&options.output);
        (output, bytes)
    });

    if let Some((output, _)) = &decode_artifact {
        refuse_overwrite(output, options.force)?;
    }

    write_output_file(&options.output, &key_bytes, options.force)?;
    let decode_artifact = decode_artifact
        .map(|(output, bytes)| {
            write_output_file(&output, &bytes, options.force)?;
            Ok::<_, ViError>(DecodeArtifactReport {
                output,
                artifact_hash: sha256_hex(&bytes),
                artifact_size_bytes: bytes.len(),
            })
        })
        .transpose()?;

    Ok(KeygenReport {
        model_id: options.model_id.clone(),
        checkpoint_hash,
        key_hash: sha256_hex(&key_bytes),
        key_size_bytes: key_bytes.len(),
        output: options.output.clone(),
        seed: options.seed,
        commitllm_revision: COMMITLLM_PIN.to_owned(),
        commitllm_pin: COMMITLLM_SHORT_PIN.to_owned(),
        decode_artifact,
        warnings,
    })
}

pub fn placeholder() {}

fn validate_keygen_options(options: &KeygenOptions) -> Result<(), ViError> {
    if options.model_id.trim().is_empty() {
        return Err(input_error("model_id", "model_id must not be empty"));
    }
    if options.output.as_os_str().is_empty() {
        return Err(input_error("output", "output path must not be empty"));
    }
    Ok(())
}

fn refuse_overwrite(path: &Path, force: bool) -> Result<(), ViError> {
    if force || !path.exists() {
        Ok(())
    } else {
        Err(input_error(
            path.display().to_string(),
            "output file exists; set force to overwrite",
        ))
    }
}

fn check_expected_checkpoint_hash(
    options: &KeygenOptions,
    actual: &str,
    warnings: &mut Vec<String>,
) -> Result<(), ViError> {
    let Some(expected) = &options.expected_checkpoint_hash else {
        return Ok(());
    };
    if expected == actual {
        return Ok(());
    }
    if options.allow_checkpoint_drift {
        warnings.push(format!(
            "checkpoint_hash_mismatch_allowed: expected {expected}, actual {actual}"
        ));
        Ok(())
    } else {
        Err(ViError::HashMismatch {
            expected: expected.clone(),
            actual: actual.to_owned(),
        })
    }
}

fn checkpoint_hash_string(hash: &CheckpointHash) -> String {
    format!("sha256:{}", hash.hash_hex())
}

fn seed_bytes(seed: u64) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    bytes[..8].copy_from_slice(&seed.to_le_bytes());
    bytes
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity("sha256:".len() + digest.len() * 2);
    output.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn decode_artifact_path(output: &Path) -> PathBuf {
    let mut path = output.as_os_str().to_os_string();
    path.push(".decode");
    PathBuf::from(path)
}

fn write_output_file(path: &Path, bytes: &[u8], force: bool) -> Result<(), ViError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| {
            input_error(
                parent.display().to_string(),
                format!("failed to create output directory: {error}"),
            )
        })?;
    }

    let mut options = OpenOptions::new();
    options.write(true);
    if force {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }

    let mut file = options.open(path).map_err(|error| {
        input_error(
            path.display().to_string(),
            format!("failed to open output file: {error}"),
        )
    })?;
    file.write_all(bytes).map_err(|error| {
        input_error(
            path.display().to_string(),
            format!("failed to write output file: {error}"),
        )
    })
}

fn validate_mirror(mirror: &HfCheckpointMirror) -> Result<(), ViError> {
    if mirror.endpoint.trim().is_empty() {
        return Err(input_error("endpoint", "HF endpoint must not be empty"));
    }
    if mirror.repo_id.trim().is_empty() {
        return Err(input_error("repo_id", "HF repo id must not be empty"));
    }
    if mirror.revision.trim().is_empty() {
        return Err(input_error("revision", "HF revision must not be empty"));
    }
    if mirror.files.is_empty() {
        return Err(input_error(
            "files",
            "checkpoint mirror manifest must list at least one file",
        ));
    }
    for relative_path in &mirror.files {
        validate_relative_path(relative_path)?;
    }
    Ok(())
}

fn validate_relative_path(path: &Path) -> Result<(), ViError> {
    if path.as_os_str().is_empty() {
        return Err(input_error(
            "files",
            "checkpoint file path must not be empty",
        ));
    }
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(input_error(
            "files",
            format!(
                "checkpoint file path must be a clean relative path: {}",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn download_file_with_resume(
    client: &Client,
    url: &str,
    destination: &Path,
) -> Result<(), ViError> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            input_error(
                parent.display().to_string(),
                format!("failed to create checkpoint cache directory: {error}"),
            )
        })?;
    }

    let existing_len = destination.metadata().map_or(0, |metadata| metadata.len());
    let mut headers = HeaderMap::new();
    if existing_len > 0 {
        let range = HeaderValue::from_str(&format!("bytes={existing_len}-")).map_err(|error| {
            input_error(
                "range",
                format!("failed to build resume range header: {error}"),
            )
        })?;
        headers.insert(RANGE, range);
    }

    let mut request = client.get(url);
    if !headers.is_empty() {
        request = request.headers(headers);
    }

    let response = request.send().map_err(|error| network_error(url, &error))?;
    let status = response.status();

    match status {
        StatusCode::OK => write_response(destination, response, false),
        StatusCode::PARTIAL_CONTENT => write_response(destination, response, existing_len > 0),
        StatusCode::RANGE_NOT_SATISFIABLE if existing_len > 0 => Ok(()),
        status if status.is_success() => write_response(destination, response, false),
        status => Err(ViError::Network {
            endpoint: url.to_owned(),
            kind: NetworkErrorKind::HttpStatus,
            http_status: Some(status.as_u16()),
        }),
    }
}

fn write_response(
    destination: &Path,
    mut response: reqwest::blocking::Response,
    append: bool,
) -> Result<(), ViError> {
    let mut options = OpenOptions::new();
    options.create(true).write(true);
    if append {
        options.append(true);
    } else {
        options.truncate(true);
    }

    let mut file = options.open(destination).map_err(|error| {
        input_error(
            destination.display().to_string(),
            format!("failed to open checkpoint cache file: {error}"),
        )
    })?;
    io::copy(&mut response, &mut file).map_err(|error| {
        input_error(
            destination.display().to_string(),
            format!("failed to write checkpoint cache file: {error}"),
        )
    })?;
    Ok(())
}

fn network_error(endpoint: &str, error: &reqwest::Error) -> ViError {
    ViError::Network {
        endpoint: endpoint.to_owned(),
        kind: if error.is_timeout() {
            NetworkErrorKind::Timeout
        } else {
            NetworkErrorKind::Other
        },
        http_status: error.status().map(|status| status.as_u16()),
    }
}

fn hf_file_url(mirror: &HfCheckpointMirror, relative_path: &Path) -> Result<String, ViError> {
    let endpoint = mirror.endpoint.trim_end_matches('/');
    let repo = mirror
        .repo_id
        .split('/')
        .map(encode_url_segment)
        .collect::<Vec<_>>()
        .join("/");
    let revision = encode_url_segment(&mirror.revision);
    let file_path = relative_path_url(relative_path)?;
    Ok(format!("{endpoint}/{repo}/resolve/{revision}/{file_path}"))
}

fn relative_path_url(path: &Path) -> Result<String, ViError> {
    validate_relative_path(path)?;
    path.components()
        .map(|component| match component {
            Component::Normal(segment) => segment
                .to_str()
                .map(encode_url_segment)
                .ok_or_else(|| input_error("files", "checkpoint file path must be UTF-8")),
            _ => Err(input_error(
                "files",
                format!("checkpoint file path must be clean: {}", path.display()),
            )),
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|segments| segments.join("/"))
}

fn encode_url_segment(segment: &str) -> String {
    let mut encoded = String::new();
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(char::from(byte));
            }
            _ => {
                use std::fmt::Write as _;
                let _ = write!(&mut encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

fn checkpoint_cache_dir(cache_root: &Path, repo_id: &str, revision: &str) -> PathBuf {
    cache_root
        .join(cache_component(repo_id))
        .join(cache_component(revision))
}

fn cache_component(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' => character,
            _ => '_',
        })
        .collect()
}

fn non_empty_env(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn platform_cache_dir() -> Result<PathBuf, ViError> {
    non_empty_env("HOME")
        .map(|home| home.join("Library").join("Caches"))
        .ok_or_else(|| input_error("cache", "HOME is not set for cache directory resolution"))
}

#[cfg(target_os = "windows")]
fn platform_cache_dir() -> Result<PathBuf, ViError> {
    non_empty_env("LOCALAPPDATA")
        .or_else(|| non_empty_env("USERPROFILE").map(|home| home.join("AppData").join("Local")))
        .ok_or_else(|| {
            input_error(
                "cache",
                "LOCALAPPDATA is not set for cache directory resolution",
            )
        })
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_cache_dir() -> Result<PathBuf, ViError> {
    non_empty_env("HOME")
        .map(|home| home.join(".cache"))
        .ok_or_else(|| input_error("cache", "HOME is not set for cache directory resolution"))
}

fn canonical_checkpoint_files(checkpoint_dir: &Path) -> Result<Vec<PathBuf>, ViError> {
    let config = required_file(checkpoint_dir, "config.json")?;
    let tokenizer = required_file(checkpoint_dir, "tokenizer.json")?;
    let tokenizer_model = optional_file(checkpoint_dir, "tokenizer.model");

    let mut safetensors = safetensor_files(checkpoint_dir)?;
    if safetensors.is_empty() {
        return Err(input_error(
            "checkpoint",
            "checkpoint directory has no *.safetensors files",
        ));
    }

    let mut files =
        Vec::with_capacity(2 + safetensors.len() + usize::from(tokenizer_model.is_some()));
    files.push(config);
    files.append(&mut safetensors);
    files.push(tokenizer);
    if let Some(tokenizer_model) = tokenizer_model {
        files.push(tokenizer_model);
    }

    Ok(files)
}

fn required_file(checkpoint_dir: &Path, file_name: &'static str) -> Result<PathBuf, ViError> {
    let relative_path = PathBuf::from(file_name);
    let path = checkpoint_dir.join(&relative_path);
    if path.is_file() {
        Ok(relative_path)
    } else {
        Err(input_error(
            "checkpoint",
            format!("checkpoint directory is missing required file {file_name}"),
        ))
    }
}

fn optional_file(checkpoint_dir: &Path, file_name: &'static str) -> Option<PathBuf> {
    let relative_path = PathBuf::from(file_name);
    checkpoint_dir
        .join(&relative_path)
        .is_file()
        .then_some(relative_path)
}

fn safetensor_files(checkpoint_dir: &Path) -> Result<Vec<PathBuf>, ViError> {
    let entries =
        fs::read_dir(checkpoint_dir).map_err(|error| read_error(checkpoint_dir, &error))?;
    let mut files = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|error| read_error(checkpoint_dir, &error))?;
        let path = entry.path();
        if !path.is_file()
            || path.extension().and_then(|extension| extension.to_str()) != Some("safetensors")
        {
            continue;
        }

        let file_name = entry.file_name().into_string().map_err(|_| {
            input_error(
                "checkpoint",
                "checkpoint contains a non-UTF-8 safetensors filename",
            )
        })?;
        files.push(file_name);
    }

    files.sort();
    Ok(files.into_iter().map(PathBuf::from).collect())
}

fn input_error(arg: impl Into<String>, reason: impl Into<String>) -> ViError {
    ViError::Input {
        arg: arg.into(),
        reason: reason.into(),
        detail: None,
    }
}

fn read_error(path: &Path, error: &io::Error) -> ViError {
    input_error(
        path.display().to_string(),
        format!("failed to read checkpoint path: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::{
        fs,
        io::{BufRead, BufReader, Write},
        net::{TcpListener, TcpStream},
        path::{Path, PathBuf},
        sync::{Arc, Mutex},
        thread::{self, JoinHandle},
    };

    use sha2::{Digest, Sha256};
    use tempfile::TempDir;
    use vi_errors::{NetworkErrorKind, ViError};
    use vi_receipt::{decode_viky_payload, Envelope, Magic};

    use super::{
        canonical_checkpoint_hash, checkpoint_cache_dir, download_hf_checkpoint_to, keygen,
        keygen_with_options, HfCheckpointMirror, KeygenOptions, COMMITLLM_PIN, COMMITLLM_SHORT_PIN,
    };

    #[test]
    fn hashes_tiny_checkpoint_in_canonical_order() {
        let checkpoint = synthetic_checkpoint([
            ("model-00002-of-00002.safetensors", "weights-b"),
            ("tokenizer.model", "tokenizer-model"),
            ("config.json", "config"),
            ("model-00001-of-00002.safetensors", "weights-a"),
            ("tokenizer.json", "tokenizer-json"),
        ]);

        let hash = canonical_checkpoint_hash(checkpoint.path()).expect("checkpoint should hash");

        assert_eq!(
            hash.files,
            vec![
                PathBuf::from("config.json"),
                PathBuf::from("model-00001-of-00002.safetensors"),
                PathBuf::from("model-00002-of-00002.safetensors"),
                PathBuf::from("tokenizer.json"),
                PathBuf::from("tokenizer.model"),
            ]
        );
        assert_eq!(
            hash.hash,
            sha256([
                "config",
                "weights-a",
                "weights-b",
                "tokenizer-json",
                "tokenizer-model"
            ])
        );
        assert_eq!(hash.hash_hex().len(), 64);
    }

    #[test]
    fn safetensor_creation_order_does_not_change_hash() {
        let first = synthetic_checkpoint([
            ("config.json", "config"),
            ("b.safetensors", "weights-b"),
            ("a.safetensors", "weights-a"),
            ("tokenizer.json", "tokenizer"),
        ]);
        let second = synthetic_checkpoint([
            ("tokenizer.json", "tokenizer"),
            ("a.safetensors", "weights-a"),
            ("config.json", "config"),
            ("b.safetensors", "weights-b"),
        ]);

        let first_hash = canonical_checkpoint_hash(first.path()).expect("first hashes");
        let second_hash = canonical_checkpoint_hash(second.path()).expect("second hashes");

        assert_eq!(first_hash, second_hash);
    }

    #[test]
    fn renaming_safetensor_changes_hash_deterministically() {
        let checkpoint = synthetic_checkpoint([
            ("config.json", "config"),
            ("a.safetensors", "weights-a"),
            ("b.safetensors", "weights-b"),
            ("tokenizer.json", "tokenizer"),
        ]);
        let original = canonical_checkpoint_hash(checkpoint.path()).expect("original hashes");

        fs::rename(
            checkpoint.path().join("a.safetensors"),
            checkpoint.path().join("z.safetensors"),
        )
        .expect("rename succeeds");
        let renamed = canonical_checkpoint_hash(checkpoint.path()).expect("renamed hashes");

        assert_eq!(
            renamed.files,
            vec![
                PathBuf::from("config.json"),
                PathBuf::from("b.safetensors"),
                PathBuf::from("z.safetensors"),
                PathBuf::from("tokenizer.json"),
            ]
        );
        assert_ne!(original.hash, renamed.hash);
        assert_eq!(
            renamed.hash,
            sha256(["config", "weights-b", "weights-a", "tokenizer"])
        );
    }

    #[test]
    fn missing_required_files_return_input_error() {
        assert_missing_file_error(
            [("tokenizer.json", "{}"), ("model.safetensors", "weights")],
            "config.json",
        );
        assert_missing_file_error(
            [("config.json", "{}"), ("model.safetensors", "weights")],
            "tokenizer.json",
        );

        let checkpoint = synthetic_checkpoint([("config.json", "{}"), ("tokenizer.json", "{}")]);
        let error = canonical_checkpoint_hash(checkpoint.path())
            .expect_err("missing safetensors should fail");
        assert_eq!(
            error,
            ViError::Input {
                arg: "checkpoint".to_owned(),
                reason: "checkpoint directory has no *.safetensors files".to_owned(),
                detail: None,
            }
        );
    }

    #[test]
    fn keygen_emits_decodable_viky_envelope_and_report_hash() {
        let checkpoint = synthetic_commitllm_checkpoint();
        let output_dir = tempfile::tempdir().expect("output tempdir");
        let output = output_dir.path().join("toy.viky");

        let report = keygen("toy-model", checkpoint.path(), 7, &output)
            .expect("keygen should produce a VIKY envelope");

        let bytes = fs::read(&output).expect("key envelope should exist");
        let envelope = Envelope::decode(&bytes).expect("envelope should decode");
        assert_eq!(envelope.magic, Magic::VIKY);

        let (header, commitllm_key) =
            decode_viky_payload(&envelope.payload).expect("VIKY payload should decode");
        commitllm_core::serialize::deserialize_key(commitllm_key)
            .expect("wrapped CommitLLM key should deserialize");

        assert_eq!(header.model_id, "toy-model");
        assert_eq!(header.seed, 7);
        assert_eq!(header.checkpoint_hash, report.checkpoint_hash);
        assert_eq!(header.commitllm_pin, COMMITLLM_SHORT_PIN);
        assert_eq!(report.model_id, "toy-model");
        assert_eq!(report.output, output);
        assert_eq!(report.seed, 7);
        assert_eq!(report.commitllm_revision, COMMITLLM_PIN);
        assert_eq!(report.commitllm_pin, COMMITLLM_SHORT_PIN);
        assert_eq!(report.key_size_bytes, bytes.len());
        assert_eq!(report.key_hash, sha256_prefixed(&bytes));
        assert!(report.decode_artifact.is_none());
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn keygen_refuses_existing_output_without_force() {
        let output_dir = tempfile::tempdir().expect("output tempdir");
        let output = output_dir.path().join("toy.viky");
        fs::write(&output, "existing").expect("existing output");

        let error = keygen("toy-model", output_dir.path(), 7, &output)
            .expect_err("existing output should fail before checkpoint work");

        assert_eq!(
            error,
            ViError::Input {
                arg: output.display().to_string(),
                reason: "output file exists; set force to overwrite".to_owned(),
                detail: None,
            }
        );
    }

    #[test]
    fn expected_checkpoint_hash_mismatch_returns_hash_mismatch() {
        let checkpoint = synthetic_commitllm_checkpoint();
        let output_dir = tempfile::tempdir().expect("output tempdir");
        let output = output_dir.path().join("toy.viky");
        let options = KeygenOptions::new("toy-model", checkpoint.path(), &output)
            .with_expected_checkpoint_hash("sha256:0000");

        let error =
            keygen_with_options(&options).expect_err("unexpected checkpoint hash should fail");

        assert!(matches!(
            error,
            ViError::HashMismatch {
                expected,
                actual
            } if expected == "sha256:0000" && actual.starts_with("sha256:")
        ));
        assert!(!output.exists());
    }

    #[test]
    fn download_checkpoint_mirror_produces_canonical_hash_inputs() {
        let files = HashMap::from([
            ("config.json", "config"),
            ("model.safetensors", "weights"),
            ("tokenizer.json", "tokenizer"),
        ]);
        let server = TestServer::new(3, move |request| {
            let file_name = request
                .path
                .rsplit('/')
                .next()
                .expect("path has a file name");
            MockResponse::ok(files.get(file_name).expect("known fixture file").as_bytes())
        });
        let mirror = test_mirror(server.base_url());
        let cache = tempfile::tempdir().expect("cache tempdir");

        let downloaded =
            download_hf_checkpoint_to(&mirror, cache.path()).expect("download should succeed");
        let requests = server.finish();

        assert_eq!(
            downloaded.hash.files,
            vec![
                PathBuf::from("config.json"),
                PathBuf::from("model.safetensors"),
                PathBuf::from("tokenizer.json"),
            ]
        );
        assert_eq!(
            downloaded.hash.hash,
            sha256(["config", "weights", "tokenizer"])
        );
        assert_eq!(
            fs::read_to_string(downloaded.directory.join("model.safetensors"))
                .expect("model file should exist"),
            "weights"
        );
        assert!(requests
            .iter()
            .all(|request| request.path.starts_with("/owner/model/resolve/main/")));
        assert!(requests.iter().all(|request| request.range.is_none()));
    }

    #[test]
    fn download_checkpoint_mirror_maps_5xx_to_network_error() {
        let server = TestServer::new(1, |_| MockResponse::status(503, "Service Unavailable"));
        let mirror =
            HfCheckpointMirror::new("owner/model", "main", vec![PathBuf::from("config.json")])
                .with_endpoint(server.base_url());
        let cache = tempfile::tempdir().expect("cache tempdir");

        let error =
            download_hf_checkpoint_to(&mirror, cache.path()).expect_err("5xx response should fail");
        let requests = server.finish();

        assert_eq!(requests.len(), 1);
        assert!(matches!(
            error,
            ViError::Network {
                kind: NetworkErrorKind::HttpStatus,
                http_status: Some(503),
                ..
            }
        ));
    }

    #[test]
    fn download_checkpoint_mirror_resumes_partial_file_with_range_request() {
        let server = TestServer::new(3, |request| {
            if request.path.ends_with("/model.safetensors") {
                assert_eq!(request.range.as_deref(), Some("bytes=3-"));
                MockResponse::partial(b"ghts", "bytes 3-6/7")
            } else {
                assert!(request.range.is_some());
                MockResponse::range_not_satisfiable()
            }
        });
        let mirror = test_mirror(server.base_url());
        let cache = tempfile::tempdir().expect("cache tempdir");
        let checkpoint_dir = checkpoint_cache_dir(cache.path(), &mirror.repo_id, &mirror.revision);
        fs::create_dir_all(&checkpoint_dir).expect("checkpoint cache dir");
        fs::write(checkpoint_dir.join("config.json"), "config").expect("config cache");
        fs::write(checkpoint_dir.join("model.safetensors"), "wei").expect("partial model cache");
        fs::write(checkpoint_dir.join("tokenizer.json"), "tokenizer").expect("tokenizer cache");

        let downloaded =
            download_hf_checkpoint_to(&mirror, cache.path()).expect("resume should succeed");
        let requests = server.finish();

        assert_eq!(
            fs::read_to_string(downloaded.directory.join("model.safetensors"))
                .expect("model file should exist"),
            "weights"
        );
        assert_eq!(
            downloaded.hash.hash,
            sha256(["config", "weights", "tokenizer"])
        );
        assert!(requests
            .iter()
            .any(|request| request.range.as_deref() == Some("bytes=3-")));
    }

    fn assert_missing_file_error<const N: usize>(files: [(&str, &str); N], missing: &str) {
        let checkpoint = synthetic_checkpoint(files);
        let error = canonical_checkpoint_hash(checkpoint.path())
            .expect_err("missing required file should fail");

        assert_eq!(
            error,
            ViError::Input {
                arg: "checkpoint".to_owned(),
                reason: format!("checkpoint directory is missing required file {missing}"),
                detail: None,
            }
        );
    }

    fn synthetic_checkpoint<const N: usize>(files: [(&str, &str); N]) -> TempDir {
        let checkpoint = tempfile::tempdir().expect("tempdir should be created");
        for (name, contents) in files {
            write_file(checkpoint.path(), name, contents);
        }
        checkpoint
    }

    fn write_file(root: &Path, name: &str, contents: &str) {
        fs::write(root.join(name), contents).expect("fixture file should be written");
    }

    fn synthetic_commitllm_checkpoint() -> TempDir {
        let checkpoint = tempfile::tempdir().expect("tempdir should be created");
        fs::write(
            checkpoint.path().join("config.json"),
            r#"{"model_type":"toy","rms_norm_eps":0.00001,"rope_theta":10000.0,"torch_dtype":"float32"}"#,
        )
        .expect("config should be written");
        fs::write(checkpoint.path().join("tokenizer.json"), "{}")
            .expect("tokenizer should be written");

        let q = i8_values(16, 1);
        let k = i8_values(16, 2);
        let v = i8_values(16, 3);
        let o = i8_values(16, 4);
        let gate = i8_values(32, 5);
        let up = i8_values(32, 6);
        let down = i8_values(32, 7);
        let lm_head = i8_values(24, 8);
        let embedding = f32_values(24, 0.01);
        let input_norm = vec![1.0_f32; 4];
        let post_norm = vec![1.0_f32; 4];
        let final_norm = vec![1.0_f32; 4];

        commitllm_keygen::write_safetensors_mixed(
            &checkpoint.path().join("model.safetensors"),
            &[
                (
                    "model.layers.0.self_attn.q_proj.weight",
                    vec![4, 4],
                    commitllm_keygen::TypedTensor::I8(&q),
                ),
                (
                    "model.layers.0.self_attn.k_proj.weight",
                    vec![4, 4],
                    commitllm_keygen::TypedTensor::I8(&k),
                ),
                (
                    "model.layers.0.self_attn.v_proj.weight",
                    vec![4, 4],
                    commitllm_keygen::TypedTensor::I8(&v),
                ),
                (
                    "model.layers.0.self_attn.o_proj.weight",
                    vec![4, 4],
                    commitllm_keygen::TypedTensor::I8(&o),
                ),
                (
                    "model.layers.0.mlp.gate_proj.weight",
                    vec![8, 4],
                    commitllm_keygen::TypedTensor::I8(&gate),
                ),
                (
                    "model.layers.0.mlp.up_proj.weight",
                    vec![8, 4],
                    commitllm_keygen::TypedTensor::I8(&up),
                ),
                (
                    "model.layers.0.mlp.down_proj.weight",
                    vec![4, 8],
                    commitllm_keygen::TypedTensor::I8(&down),
                ),
                (
                    "lm_head.weight",
                    vec![6, 4],
                    commitllm_keygen::TypedTensor::I8(&lm_head),
                ),
                (
                    "model.embed_tokens.weight",
                    vec![6, 4],
                    commitllm_keygen::TypedTensor::F32(&embedding),
                ),
                (
                    "model.layers.0.input_layernorm.weight",
                    vec![4],
                    commitllm_keygen::TypedTensor::F32(&input_norm),
                ),
                (
                    "model.layers.0.post_attention_layernorm.weight",
                    vec![4],
                    commitllm_keygen::TypedTensor::F32(&post_norm),
                ),
                (
                    "model.norm.weight",
                    vec![4],
                    commitllm_keygen::TypedTensor::F32(&final_norm),
                ),
            ],
        )
        .expect("safetensors should be written");

        checkpoint
    }

    fn i8_values(len: usize, offset: usize) -> Vec<i8> {
        const VALUES: [i8; 17] = [-8, -7, -6, -5, -4, -3, -2, -1, 0, 1, 2, 3, 4, 5, 6, 7, 8];
        (0..len)
            .map(|index| VALUES[(index + offset) % VALUES.len()])
            .collect()
    }

    fn f32_values(len: usize, step: f32) -> Vec<f32> {
        let mut value = 1.0;
        (0..len)
            .map(|_| {
                let current = value;
                value += step;
                current
            })
            .collect()
    }

    fn sha256<const N: usize>(chunks: [&str; N]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        for chunk in chunks {
            hasher.update(chunk.as_bytes());
        }
        hasher.finalize().into()
    }

    fn sha256_prefixed(bytes: &[u8]) -> String {
        let digest = Sha256::digest(bytes);
        let mut output = String::with_capacity("sha256:".len() + digest.len() * 2);
        output.push_str("sha256:");
        for byte in digest {
            use std::fmt::Write as _;
            let _ = write!(&mut output, "{byte:02x}");
        }
        output
    }

    fn test_mirror(endpoint: String) -> HfCheckpointMirror {
        HfCheckpointMirror::new(
            "owner/model",
            "main",
            vec![
                PathBuf::from("config.json"),
                PathBuf::from("model.safetensors"),
                PathBuf::from("tokenizer.json"),
            ],
        )
        .with_endpoint(endpoint)
    }

    #[derive(Debug, Clone)]
    struct RecordedRequest {
        path: String,
        range: Option<String>,
    }

    struct MockResponse {
        status: u16,
        reason: &'static str,
        headers: Vec<(&'static str, String)>,
        body: Vec<u8>,
    }

    impl MockResponse {
        fn ok(body: &[u8]) -> Self {
            Self {
                status: 200,
                reason: "OK",
                headers: Vec::new(),
                body: body.to_vec(),
            }
        }

        fn partial(body: &[u8], content_range: &str) -> Self {
            Self {
                status: 206,
                reason: "Partial Content",
                headers: vec![("Content-Range", content_range.to_owned())],
                body: body.to_vec(),
            }
        }

        fn range_not_satisfiable() -> Self {
            Self {
                status: 416,
                reason: "Range Not Satisfiable",
                headers: vec![("Content-Range", "bytes */0".to_owned())],
                body: Vec::new(),
            }
        }

        fn status(status: u16, reason: &'static str) -> Self {
            Self {
                status,
                reason,
                headers: Vec::new(),
                body: Vec::new(),
            }
        }
    }

    struct TestServer {
        base_url: String,
        requests: Arc<Mutex<Vec<RecordedRequest>>>,
        handle: JoinHandle<()>,
    }

    impl TestServer {
        fn new(
            request_count: usize,
            handler: impl Fn(&RecordedRequest) -> MockResponse + Send + Sync + 'static,
        ) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("test server binds");
            let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
            let requests = Arc::new(Mutex::new(Vec::new()));
            let thread_requests = Arc::clone(&requests);
            let handler = Arc::new(handler);
            let thread_handler = Arc::clone(&handler);

            let handle = thread::spawn(move || {
                for _ in 0..request_count {
                    let (mut stream, _) = listener.accept().expect("connection accepted");
                    let request = read_request(&stream);
                    thread_requests
                        .lock()
                        .expect("requests lock")
                        .push(request.clone());
                    let response = thread_handler(&request);
                    write_response(&mut stream, response);
                }
            });

            Self {
                base_url,
                requests,
                handle,
            }
        }

        fn base_url(&self) -> String {
            self.base_url.clone()
        }

        fn finish(self) -> Vec<RecordedRequest> {
            self.handle.join().expect("test server joins");
            self.requests.lock().expect("requests lock").clone()
        }
    }

    fn read_request(stream: &TcpStream) -> RecordedRequest {
        let mut reader = BufReader::new(stream.try_clone().expect("stream clones"));
        let mut first_line = String::new();
        reader
            .read_line(&mut first_line)
            .expect("request line reads");
        let path = first_line
            .split_whitespace()
            .nth(1)
            .expect("request path")
            .to_owned();
        let mut range = None;

        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("header reads");
            let header = line.trim_end_matches(['\r', '\n']);
            if header.is_empty() {
                break;
            }
            if let Some((name, value)) = header.split_once(':') {
                if name.eq_ignore_ascii_case("range") {
                    range = Some(value.trim().to_owned());
                }
            }
        }

        RecordedRequest { path, range }
    }

    fn write_response(stream: &mut TcpStream, response: MockResponse) {
        write!(
            stream,
            "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
            response.status,
            response.reason,
            response.body.len()
        )
        .expect("response head writes");
        for (name, value) in response.headers {
            write!(stream, "{name}: {value}\r\n").expect("response header writes");
        }
        stream
            .write_all(b"\r\n")
            .expect("response separator writes");
        stream
            .write_all(&response.body)
            .expect("response body writes");
    }
}
