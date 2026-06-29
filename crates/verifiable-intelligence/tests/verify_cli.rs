use std::{fs, process::Command};

use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use vi_receipt::{
    encode_viau_payload, encode_viky_payload, encode_virc_payload, AuditBindingHeader, AuditTier,
    Envelope, KeyBindingHeader, Magic, ReceiptBindingHeader,
};

const COMMITLLM_SHORT_PIN: &str = "25541e83";

fn vi_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_vi"))
}

fn stdout_json(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("stdout should be a JSON object")
}

fn stderr_json(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stderr).expect("stderr should be a JSON error envelope")
}

#[test]
fn verify_full_fixture_outputs_verify_report_json() {
    let fixture = FullFixture::write(false);

    let output = vi_command()
        .args(["verify", "--receipt"])
        .arg(&fixture.receipt_path)
        .arg("--key")
        .arg(&fixture.key_path)
        .args(["--tier", "full", "--audit-endpoint"])
        .arg(fixture.audit_file_endpoint())
        .output()
        .expect("vi should run");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let value = stdout_json(&output);
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["subcommand"], "verify");
    assert_eq!(value["tier"], "full");
    assert_eq!(value["verdict"], "pass");
    assert_eq!(value["checks_run"], 39);
    assert_eq!(value["checks_passed"], 39);
    assert_eq!(
        value["phases"],
        serde_json::json!([
            "embedding_merkle",
            "shell_freivalds",
            "bridge_replay",
            "attention_corridor",
            "kv_provenance",
            "lm_head",
            "decode_policy"
        ])
    );
}

#[test]
fn verify_full_challenge_mismatch_exits_verification_failed() {
    let fixture = FullFixture::write(true);

    let output = vi_command()
        .args(["verify", "--receipt"])
        .arg(&fixture.receipt_path)
        .arg("--key")
        .arg(&fixture.key_path)
        .args(["--tier", "full", "--audit-endpoint"])
        .arg(fixture.audit_file_endpoint())
        .output()
        .expect("vi should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let envelope = stderr_json(&output);
    assert_eq!(envelope["category"], "verification_failed");
    assert_eq!(envelope["exit_code"], 1);
}

#[test]
fn verify_full_requires_audit_endpoint_before_file_io() {
    let output = vi_command()
        .args([
            "verify",
            "--receipt",
            "missing.virc",
            "--key",
            "missing.viky",
            "--tier",
            "full",
        ])
        .env_remove("VI_ENDPOINT")
        .output()
        .expect("vi should run");

    assert_eq!(output.status.code(), Some(8));
    assert!(output.stdout.is_empty());
    let envelope = stderr_json(&output);
    assert_eq!(envelope["category"], "unsupported_tier");
    assert_eq!(envelope["detail"]["requested_tier"], "full");
    assert_eq!(envelope["detail"]["reason"], "missing --audit-endpoint");
}

struct FullFixture {
    _dir: TempDir,
    receipt_path: std::path::PathBuf,
    key_path: std::path::PathBuf,
    audit_path: std::path::PathBuf,
}

impl FullFixture {
    fn write(tamper_challenge: bool) -> Self {
        let dir = tempfile::tempdir().expect("fixture tempdir");
        let (receipt, key, audit) = full_audit_fixture_artifacts(tamper_challenge);
        let receipt_path = dir.path().join("receipt.virc");
        let key_path = dir.path().join("key.viky");
        let audit_path = dir.path().join("audit.viau");

        fs::write(&receipt_path, receipt).expect("receipt fixture writes");
        fs::write(&key_path, key).expect("key fixture writes");
        fs::write(&audit_path, audit).expect("audit fixture writes");

        Self {
            _dir: dir,
            receipt_path,
            key_path,
            audit_path,
        }
    }

    fn audit_file_endpoint(&self) -> String {
        format!("file://{}", self.audit_path.display())
    }
}

fn full_audit_fixture_artifacts(tamper_challenge: bool) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let raw_key = include_bytes!("../../../verifier/wasm/fixtures/v4_key_fullbridge.bin");
    let raw_audit = include_bytes!("../../../verifier/wasm/fixtures/v4_audit_fullbridge.bin");
    let model_id = "commitllm-fullbridge-fixture";
    let checkpoint_hash = "sha256:fixture-checkpoint";

    let key_header = KeyBindingHeader::new(model_id, checkpoint_hash, COMMITLLM_SHORT_PIN, 0);
    let key_payload = encode_viky_payload(&key_header, raw_key).expect("fixture VIKY encodes");
    let key = Envelope::new(Magic::VIKY, key_payload)
        .encode()
        .expect("fixture VIKY envelope encodes");

    let receipt_header = ReceiptBindingHeader::new(
        model_id,
        checkpoint_hash,
        COMMITLLM_SHORT_PIN,
        sha256_hex(&key),
        "sha256:fixture-prompt",
        "sha256:fixture-answer",
        1,
    );
    let receipt_payload = encode_virc_payload(&receipt_header, b"unused-full-audit-receipt")
        .expect("fixture VIRC encodes");
    let receipt = Envelope::new(Magic::VIRC, receipt_payload)
        .encode()
        .expect("fixture VIRC envelope encodes");

    let token_index = u64::from(tamper_challenge);
    let audit_header = AuditBindingHeader::new(
        sha256_hex(&receipt),
        AuditTier::Full,
        token_index,
        vec![0, 1],
    );
    let audit_payload =
        encode_viau_payload(&audit_header, raw_audit).expect("fixture VIAU encodes");
    let audit = Envelope::new(Magic::VIAU, audit_payload)
        .encode()
        .expect("fixture VIAU envelope encodes");

    (receipt, key, audit)
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
