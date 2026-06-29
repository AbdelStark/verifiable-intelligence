use std::process::Command;

fn vi_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_vi"))
}

fn stdout_utf8(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout should be UTF-8")
}

#[test]
fn root_help_lists_public_subcommands_and_globals() {
    let output = vi_command().arg("--help").output().expect("vi should run");

    assert!(output.status.success());
    let stdout = stdout_utf8(&output);
    assert!(stdout.contains("Usage: vi [OPTIONS] [COMMAND]"));
    assert!(stdout.contains("keygen"));
    assert!(stdout.contains("chat"));
    assert!(stdout.contains("verify"));
    assert!(stdout.contains("tui"));
    assert!(stdout.contains("--pretty"));
    assert!(stdout.contains("--log <FILTER>"));
}

#[test]
fn subcommand_help_is_available_for_public_stubs() {
    for subcommand in ["keygen", "chat", "verify", "tui"] {
        let output = vi_command()
            .args([subcommand, "--help"])
            .output()
            .expect("vi should run");

        assert!(output.status.success(), "{subcommand}");
        let stdout = stdout_utf8(&output);
        assert!(stdout.contains("Usage:"), "{subcommand}");
        assert!(stdout.contains("--help"), "{subcommand}");
    }
}

#[test]
fn endpoint_subcommand_help_lists_endpoint_flag() {
    for subcommand in ["chat", "tui"] {
        let output = vi_command()
            .args([subcommand, "--help"])
            .output()
            .expect("vi should run");

        assert!(output.status.success(), "{subcommand}");
        let stdout = stdout_utf8(&output);
        assert!(stdout.contains("--endpoint <URL>"), "{subcommand}");
    }
}

#[test]
fn keygen_help_lists_public_flags() {
    let output = vi_command()
        .args(["keygen", "--help"])
        .output()
        .expect("vi should run");

    assert!(output.status.success());
    let stdout = stdout_utf8(&output);
    assert!(stdout.contains("--model <MODEL>"));
    assert!(stdout.contains("--checkpoint <DIR>"));
    assert!(stdout.contains("--output <PATH>"));
    assert!(stdout.contains("--seed <U64>"));
    assert!(stdout.contains("--force"));
    assert!(stdout.contains("--expected-checkpoint-hash <SHA256>"));
    assert!(stdout.contains("--allow-checkpoint-drift"));
}

#[test]
fn chat_help_lists_public_flags() {
    let output = vi_command()
        .args(["chat", "--help"])
        .output()
        .expect("vi should run");

    assert!(output.status.success());
    let stdout = stdout_utf8(&output);
    assert!(stdout.contains("--endpoint <URL>"));
    assert!(stdout.contains("--prompt <TEXT>"));
    assert!(stdout.contains("--max-tokens <U32>"));
    assert!(stdout.contains("--receipt-out <PATH>"));
    assert!(stdout.contains("--no-receipt"));
}

#[test]
fn tui_help_lists_public_flags() {
    let output = vi_command()
        .args(["tui", "--help"])
        .output()
        .expect("vi should run");

    assert!(output.status.success());
    let stdout = stdout_utf8(&output);
    assert!(stdout.contains("--endpoint <URL>"));
    assert!(stdout.contains("--tamper <MODE>"));
    assert!(stdout.contains("--phase-delay <MS>"));
}

#[test]
fn verify_help_lists_public_flags() {
    let output = vi_command()
        .args(["verify", "--help"])
        .output()
        .expect("vi should run");

    assert!(output.status.success());
    let stdout = stdout_utf8(&output);
    assert!(stdout.contains("--receipt <PATH>"));
    assert!(stdout.contains("--key <PATH>"));
    assert!(stdout.contains("--tier <TIER>"));
    assert!(stdout.contains("--audit-endpoint <URL|file://PATH>"));
}
