use std::{fs, path::Path, process::Command};

use serde_json::Value;
use tempfile::TempDir;

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
fn keygen_outputs_schema_json_and_writes_viky() {
    let checkpoint = synthetic_commitllm_checkpoint();
    let output_dir = tempfile::tempdir().expect("output tempdir");
    let key_path = output_dir.path().join("toy.viky");

    let output = vi_command()
        .args(["keygen", "--model", "toy-model", "--checkpoint"])
        .arg(checkpoint.path())
        .arg("--output")
        .arg(&key_path)
        .args(["--seed", "7"])
        .output()
        .expect("vi should run");

    assert_eq!(output.status.code(), Some(0));
    let value = stdout_json(&output);
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["subcommand"], "keygen");
    assert_eq!(value["model_id"], "toy-model");
    assert_eq!(value["seed"], 7);
    let expected_output = key_path.to_string_lossy();
    assert_eq!(
        value["output"].as_str().expect("output should be a string"),
        expected_output.as_ref()
    );
    assert_eq!(value["commitllm_pin"], "25541e83");
    assert_eq!(
        value["commitllm_revision"],
        "25541e83347655e44ad6e84eb901e1e7ae392a66"
    );
    assert!(value["checkpoint_hash"]
        .as_str()
        .expect("checkpoint_hash should be a string")
        .starts_with("sha256:"));
    assert!(value["key_hash"]
        .as_str()
        .expect("key_hash should be a string")
        .starts_with("sha256:"));
    assert!(
        value["key_size_bytes"]
            .as_u64()
            .expect("key_size_bytes should be a number")
            > 0
    );
    assert!(value["warnings"]
        .as_array()
        .expect("warnings should be an array")
        .is_empty());

    let key_bytes = fs::read(&key_path).expect("key file should exist");
    assert_eq!(&key_bytes[..4], b"VIKY");
}

#[test]
fn keygen_existing_output_exits_input_without_force() {
    let output_dir = tempfile::tempdir().expect("output tempdir");
    let key_path = output_dir.path().join("toy.viky");
    fs::write(&key_path, "existing").expect("existing key file");

    let output = vi_command()
        .args(["keygen", "--model", "toy-model", "--checkpoint"])
        .arg(output_dir.path())
        .arg("--output")
        .arg(&key_path)
        .output()
        .expect("vi should run");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let envelope = stderr_json(&output);
    assert_eq!(envelope["category"], "input");
    assert_eq!(envelope["exit_code"], 2);
}

#[test]
fn keygen_checkpoint_mismatch_exits_hash_mismatch() {
    let checkpoint = synthetic_commitllm_checkpoint();
    let output_dir = tempfile::tempdir().expect("output tempdir");
    let key_path = output_dir.path().join("toy.viky");

    let output = vi_command()
        .args(["keygen", "--model", "toy-model", "--checkpoint"])
        .arg(checkpoint.path())
        .arg("--output")
        .arg(&key_path)
        .args(["--expected-checkpoint-hash", "sha256:0000"])
        .output()
        .expect("vi should run");

    assert_eq!(output.status.code(), Some(4));
    assert!(output.stdout.is_empty());
    assert!(!key_path.exists());
    let envelope = stderr_json(&output);
    assert_eq!(envelope["category"], "hash_mismatch");
    assert_eq!(envelope["exit_code"], 4);
}

#[test]
fn keygen_allows_checkpoint_drift_with_warning() {
    let checkpoint = synthetic_commitllm_checkpoint();
    let output_dir = tempfile::tempdir().expect("output tempdir");
    let key_path = output_dir.path().join("toy.viky");

    let output = vi_command()
        .args(["keygen", "--model", "toy-model", "--checkpoint"])
        .arg(checkpoint.path())
        .arg("--output")
        .arg(&key_path)
        .args([
            "--expected-checkpoint-hash",
            "sha256:0000",
            "--allow-checkpoint-drift",
        ])
        .output()
        .expect("vi should run");

    assert_eq!(output.status.code(), Some(0));
    let value = stdout_json(&output);
    let warnings = value["warnings"]
        .as_array()
        .expect("warnings should be an array");
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0]
        .as_str()
        .expect("warning should be a string")
        .starts_with("checkpoint_hash_mismatch_allowed:"));
    assert!(key_path.exists());
}

fn synthetic_commitllm_checkpoint() -> TempDir {
    let checkpoint = tempfile::tempdir().expect("tempdir should be created");
    fs::write(
        checkpoint.path().join("config.json"),
        r#"{"model_type":"toy","rms_norm_eps":0.00001,"rope_theta":10000.0,"torch_dtype":"float32"}"#,
    )
    .expect("config should be written");
    fs::write(checkpoint.path().join("tokenizer.json"), "{}").expect("tokenizer should be written");
    write_toy_safetensors(checkpoint.path());
    checkpoint
}

fn write_toy_safetensors(checkpoint: &Path) {
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
        &checkpoint.join("model.safetensors"),
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
