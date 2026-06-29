use std::process::Command;

fn vi_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_vi"))
}

#[test]
fn root_help_matches_snapshot() {
    assert_help_snapshot(&["--help"], include_str!("snapshots/help/root.txt"));
}

#[test]
fn keygen_help_matches_snapshot() {
    assert_help_snapshot(
        &["keygen", "--help"],
        include_str!("snapshots/help/keygen.txt"),
    );
}

#[test]
fn chat_help_matches_snapshot() {
    assert_help_snapshot(&["chat", "--help"], include_str!("snapshots/help/chat.txt"));
}

#[test]
fn verify_help_matches_snapshot() {
    assert_help_snapshot(
        &["verify", "--help"],
        include_str!("snapshots/help/verify.txt"),
    );
}

#[test]
fn tui_help_matches_snapshot() {
    assert_help_snapshot(&["tui", "--help"], include_str!("snapshots/help/tui.txt"));
}

fn assert_help_snapshot(args: &[&str], expected: &str) {
    let output = vi_command().args(args).output().expect("vi should run");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert_eq!(stdout, expected);
}
