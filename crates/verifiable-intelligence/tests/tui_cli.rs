use std::process::Command;

use serde_json::Value;

fn vi_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_vi"))
}

fn stdout_json(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("stdout should be a JSON object")
}

fn stderr_json(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stderr).expect("stderr should be a JSON error envelope")
}

#[cfg(feature = "tui")]
#[test]
fn tui_dispatches_to_tui_runtime_with_public_flags() {
    let output = vi_command()
        .args([
            "tui",
            "--endpoint",
            "https://provider.example",
            "--tamper",
            "byte-flip",
            "--phase-delay",
            "250",
        ])
        .output()
        .expect("vi should run");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let value = stdout_json(&output);
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["subcommand"], "tui");
    assert_eq!(value["status"], "stub");
    assert_eq!(value["endpoint"], "https://provider.example");
    assert_eq!(value["tamper"], "byte-flip");
    assert_eq!(value["phase_delay_ms"], 250);
}

#[cfg(feature = "tui")]
#[test]
fn tui_rejects_unknown_tamper_mode_at_cli_boundary() {
    let output = vi_command()
        .args(["tui", "--tamper", "silent-rewrite"])
        .output()
        .expect("vi should run");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let envelope = stderr_json(&output);
    assert_eq!(envelope["category"], "input");
    assert_eq!(envelope["detail"]["arg"], "--tamper");
}
