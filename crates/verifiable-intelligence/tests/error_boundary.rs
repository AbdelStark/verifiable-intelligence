use std::process::Command;

use serde_json::Value;

const TEST_HOOKS_ENV: &str = "VI_ENABLE_TEST_HOOKS";
const TRACE_ID_ENV: &str = "VI_TRACE_ID";

fn vi_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_vi"))
}

fn stderr_json(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stderr).expect("stderr should be a JSON error envelope")
}

#[test]
fn clap_parse_failure_exits_usage_code() {
    let output = vi_command()
        .arg("--definitely-invalid")
        .output()
        .expect("vi should run");

    assert_eq!(output.status.code(), Some(64));
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("--definitely-invalid"));
}

#[test]
fn panic_hook_emits_internal_envelope() {
    let output = vi_command()
        .env(TEST_HOOKS_ENV, "1")
        .env(TRACE_ID_ENV, "trace-panic")
        .arg("__panic")
        .output()
        .expect("vi should run");

    assert_eq!(output.status.code(), Some(70));
    let envelope = stderr_json(&output);
    assert_eq!(envelope["error"], true);
    assert_eq!(envelope["category"], "internal");
    assert_eq!(envelope["exit_code"], 70);
    assert_eq!(envelope["trace_id"], "trace-panic");
    assert!(envelope["detail"]["backtrace"]
        .as_str()
        .expect("backtrace should be a string")
        .contains("deliberate vi panic test hook"));
}

#[test]
fn every_vi_error_category_exits_with_documented_code() {
    let cases = [
        ("verification_failed", 1),
        ("input", 2),
        ("network", 3),
        ("hash_mismatch", 4),
        ("receipt_missing", 5),
        ("unknown_version", 6),
        ("identity_mismatch", 7),
        ("unsupported_tier", 8),
        ("corrupt_envelope", 9),
        ("internal", 70),
    ];

    for (category, code) in cases {
        let trace_id = format!("trace-{category}");
        let output = vi_command()
            .env(TEST_HOOKS_ENV, "1")
            .env(TRACE_ID_ENV, &trace_id)
            .args(["__error", category])
            .output()
            .expect("vi should run");

        assert_eq!(output.status.code(), Some(code), "{category}");
        let envelope = stderr_json(&output);
        assert_eq!(envelope["error"], true, "{category}");
        assert_eq!(envelope["category"], category, "{category}");
        assert_eq!(envelope["exit_code"], code, "{category}");
        assert_eq!(envelope["trace_id"], trace_id, "{category}");
    }
}
