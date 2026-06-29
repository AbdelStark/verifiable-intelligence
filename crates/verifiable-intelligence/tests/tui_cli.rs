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

fn stderr_utf8(output: &std::process::Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr should be UTF-8")
}

fn stdout_utf8(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout should be UTF-8")
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
fn tui_log_info_writes_logs_to_stderr_without_polluting_stdout() {
    let output = vi_command()
        .args(["--log", "info", "tui"])
        .output()
        .expect("vi should run");

    assert_eq!(output.status.code(), Some(0));
    let value = stdout_json(&output);
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["subcommand"], "tui");

    let stderr = stderr_utf8(&output);
    let events: Vec<Value> = stderr
        .lines()
        .map(|line| serde_json::from_str(line).expect("log line should be JSON"))
        .collect();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["event"], "process.start");
    assert_eq!(events[1]["event"], "process.end");
    assert!(events.iter().all(|event| event["trace_id"].is_string()));
    assert!(events.iter().all(|event| event["span"] == "cli.tui"));
}

#[cfg(feature = "tui")]
#[test]
fn tui_pretty_uses_color_unless_vi_no_color_is_set() {
    let colored = vi_command()
        .args(["tui", "--pretty", "--phase-delay", "250"])
        .env_remove("VI_NO_COLOR")
        .env_remove("NO_COLOR")
        .output()
        .expect("vi should run");
    assert_eq!(colored.status.code(), Some(0));
    assert!(colored.stderr.is_empty());
    let colored_stdout = stdout_utf8(&colored);
    assert!(colored_stdout.contains('\x1b'));
    let stripped: Value =
        serde_json::from_str(&strip_ansi(&colored_stdout)).expect("stripped output is JSON");
    assert_eq!(stripped["schema_version"], 1);
    assert_eq!(stripped["phase_delay_ms"], 250);

    let no_color = vi_command()
        .args(["tui", "--pretty", "--phase-delay", "250"])
        .env("VI_NO_COLOR", "1")
        .output()
        .expect("vi should run");
    assert_eq!(no_color.status.code(), Some(0));
    assert!(no_color.stderr.is_empty());
    let no_color_stdout = stdout_utf8(&no_color);
    assert!(!no_color_stdout.contains('\x1b'));
    let plain: Value = serde_json::from_str(&no_color_stdout).expect("plain output is JSON");

    assert_eq!(plain, stripped);
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

fn strip_ansi(value: &str) -> String {
    value.replace("\x1b[36m", "").replace("\x1b[0m", "")
}
