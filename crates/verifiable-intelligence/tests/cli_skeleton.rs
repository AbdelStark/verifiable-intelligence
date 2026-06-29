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
    for subcommand in ["chat", "verify", "tui"] {
        let output = vi_command()
            .args([subcommand, "--help"])
            .output()
            .expect("vi should run");

        assert!(output.status.success(), "{subcommand}");
        let stdout = stdout_utf8(&output);
        assert!(stdout.contains("--endpoint <URL>"), "{subcommand}");
    }
}
