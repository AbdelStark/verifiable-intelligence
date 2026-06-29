//! Library surface of the `vi` command-line binary.
//!
//! Subcommand dispatch is wired in by RFC-0002 issues. The umbrella crate
//! `verifiable-intelligence` calls [`run`] from its own `vi` binary so that
//! `cargo install verifiable-intelligence` delivers the same command surface.

/// Process-style entry point. Returns the process exit code.
#[must_use]
pub fn run() -> i32 {
    // The leaf-crate calls keep the workspace link graph exercised at build
    // time before real subcommand logic lands; they are cheap no-ops.
    vi_client::placeholder();
    let _ = vi_errors::USAGE_EXIT_CODE;
    vi_keygen::placeholder();
    vi_log::placeholder();
    vi_receipt::placeholder();
    vi_verifier::placeholder();
    #[cfg(feature = "tui")]
    vi_tui::placeholder();
    0
}
