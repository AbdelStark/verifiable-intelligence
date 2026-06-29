use std::process::Command;

use serde_json::Value;

fn vi_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_vi"))
}

fn stderr_json(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stderr).expect("stderr should be a JSON error envelope")
}

#[test]
fn chat_requires_endpoint_from_flag_or_env() {
    let output = vi_command()
        .args(["chat", "--prompt", "hello", "--receipt-out", "receipt.virc"])
        .env_remove("VI_ENDPOINT")
        .output()
        .expect("vi should run");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let envelope = stderr_json(&output);
    assert_eq!(envelope["category"], "input");
    assert_eq!(envelope["detail"]["arg"], "--endpoint");
}

#[test]
fn chat_requires_receipt_out_unless_no_receipt() {
    let output = vi_command()
        .args([
            "chat",
            "--endpoint",
            "https://provider.example",
            "--prompt",
            "hello",
        ])
        .output()
        .expect("vi should run");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let envelope = stderr_json(&output);
    assert_eq!(envelope["category"], "input");
    assert_eq!(envelope["detail"]["arg"], "--receipt-out");
}

#[test]
fn chat_rejects_receipt_out_with_no_receipt() {
    let output = vi_command()
        .args([
            "chat",
            "--endpoint",
            "https://provider.example",
            "--prompt",
            "hello",
            "--receipt-out",
            "receipt.virc",
            "--no-receipt",
        ])
        .output()
        .expect("vi should run");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let envelope = stderr_json(&output);
    assert_eq!(envelope["category"], "input");
    assert_eq!(envelope["detail"]["arg"], "--receipt-out");
}
