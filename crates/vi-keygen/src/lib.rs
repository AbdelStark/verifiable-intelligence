//! Verifier-key generation.
//!
//! Filled in by RFC-0004 implementation issues.

// RFC-0004 failures must flow through the shared `ViError` taxonomy.
#![allow(clippy::result_large_err)]

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use vi_errors::ViError;

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

pub fn placeholder() {}

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
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use sha2::{Digest, Sha256};
    use tempfile::TempDir;
    use vi_errors::ViError;

    use super::canonical_checkpoint_hash;

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

    fn sha256<const N: usize>(chunks: [&str; N]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        for chunk in chunks {
            hasher.update(chunk.as_bytes());
        }
        hasher.finalize().into()
    }
}
