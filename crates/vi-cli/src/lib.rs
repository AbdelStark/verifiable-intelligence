//! Library surface of the `vi` command-line binary.
//!
//! Subcommand dispatch is wired in by RFC-0002 issues. The umbrella crate
//! `verifiable-intelligence` calls [`run`] from its own `vi` binary so that
//! `cargo install verifiable-intelligence` delivers the same command surface.

// RFC-0014 requires a concrete `Result<Output, ViError>` boundary. Keep that
// contract direct until the downstream error plumbing is in place.
#![allow(clippy::result_large_err)]

use std::{
    ffi::OsString,
    io::{self, Write},
    process,
    sync::Once,
};

use clap::{Parser, Subcommand};
use vi_errors::{ErrorEnvelope, IdentityFields, NetworkErrorKind, PhaseId, ViError};

const SUBCOMMAND: &str = "vi";
const TEST_HOOKS_ENV: &str = "VI_ENABLE_TEST_HOOKS";
const TRACE_ID_ENV: &str = "VI_TRACE_ID";

static PANIC_HOOK: Once = Once::new();
static SIGINT_HOOK: Once = Once::new();

/// Captured output from a successful command dispatch.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Output {
    /// Optional stdout payload to print at the process boundary.
    pub stdout: Option<String>,
}

#[derive(Debug, Parser)]
#[command(name = "vi", version, about = "Verifiable Intelligence CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<BoundaryCommand>,
}

#[derive(Debug, Subcommand)]
enum BoundaryCommand {
    #[command(name = "__panic", hide = true)]
    Panic,
    #[command(name = "__error", hide = true)]
    Error {
        #[arg(value_name = "CATEGORY")]
        category: String,
    },
}

/// Library dispatch entry point. Converts domain failures into `ViError`.
pub fn run() -> Result<Output, ViError> {
    // The leaf-crate calls keep the workspace link graph exercised at build
    // time before real subcommand logic lands; they are cheap no-ops.
    vi_client::placeholder();
    vi_keygen::placeholder();
    vi_log::placeholder();
    vi_receipt::placeholder();
    vi_verifier::placeholder();
    #[cfg(feature = "tui")]
    vi_tui::placeholder();
    Ok(Output::default())
}

/// Process-style entry point. Returns the process exit code.
#[must_use]
pub fn process_main() -> i32 {
    process_main_from(std::env::args_os())
}

/// Process-style entry point over explicit args, used by tests and the binary.
#[must_use]
pub fn process_main_from<I, T>(args: I) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    install_panic_hook();
    install_sigint_handler();
    let trace_id = trace_id();

    match Cli::try_parse_from(args) {
        Ok(cli) => match run_cli(cli) {
            Ok(output) => {
                print_output(output);
                0
            }
            Err(error) => {
                print_error_envelope(&error, &trace_id);
                error.exit_code()
            }
        },
        Err(error) => {
            if let Err(print_error) = error.print() {
                eprintln!("failed to print clap error: {print_error}");
            }
            vi_errors::USAGE_EXIT_CODE
        }
    }
}

fn run_cli(cli: Cli) -> Result<Output, ViError> {
    match cli.command {
        Some(BoundaryCommand::Panic) => {
            ensure_test_hooks_enabled()?;
            panic!("deliberate vi panic test hook");
        }
        Some(BoundaryCommand::Error { category }) => {
            ensure_test_hooks_enabled()?;
            Err(sample_error(&category).unwrap_or_else(|| ViError::Input {
                arg: "CATEGORY".to_owned(),
                reason: format!("unknown test category {category}"),
                detail: None,
            }))
        }
        None => run(),
    }
}

fn install_panic_hook() {
    PANIC_HOOK.call_once(|| {
        std::panic::set_hook(Box::new(|panic_info| {
            let trace_id = trace_id();
            let error = ViError::Internal {
                backtrace: panic_message(panic_info),
            };
            print_error_envelope(&error, &trace_id);
            process::exit(error.exit_code());
        }));
    });
}

fn install_sigint_handler() {
    SIGINT_HOOK.call_once(|| {
        if let Err(error) = ctrlc::set_handler(|| process::exit(vi_errors::SIGINT_EXIT_CODE)) {
            eprintln!("failed to install SIGINT handler: {error}");
        }
    });
}

fn panic_message(panic_info: &std::panic::PanicHookInfo<'_>) -> String {
    let payload = panic_info
        .payload()
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| {
            panic_info
                .payload()
                .downcast_ref::<String>()
                .map(String::as_str)
        })
        .unwrap_or("panic");

    match panic_info.location() {
        Some(location) => format!("{payload} at {}:{}", location.file(), location.line()),
        None => payload.to_owned(),
    }
}

fn print_output(output: Output) {
    if let Some(stdout) = output.stdout {
        println!("{stdout}");
    }
}

fn print_error_envelope(error: &ViError, trace_id: &str) {
    let envelope = ErrorEnvelope::new(SUBCOMMAND, trace_id, error);
    let mut stderr = io::stderr().lock();

    if serde_json::to_writer(&mut stderr, &envelope).is_ok() {
        let _ = writeln!(stderr);
    } else {
        let _ = writeln!(
            stderr,
            r#"{{"error":true,"schema_version":1,"subcommand":"vi","category":"internal","exit_code":70,"message":"failed to serialize error envelope","detail":{{}},"trace_id":"{trace_id}"}}"#
        );
    }
}

fn trace_id() -> String {
    std::env::var(TRACE_ID_ENV).unwrap_or_else(|_| format!("vi-{}", process::id()))
}

fn ensure_test_hooks_enabled() -> Result<(), ViError> {
    if std::env::var_os(TEST_HOOKS_ENV).as_deref() == Some(std::ffi::OsStr::new("1")) {
        Ok(())
    } else {
        Err(ViError::UnsupportedTier {
            requested: "test-hook".to_owned(),
            reason: "internal test hooks are disabled".to_owned(),
        })
    }
}

fn sample_error(category: &str) -> Option<ViError> {
    Some(match category {
        "verification_failed" => ViError::VerificationFailed {
            phase: PhaseId::BridgeReplay,
            measured: Some(47.0),
            tolerance: Some(10.0),
            extra: None,
        },
        "input" => ViError::Input {
            arg: "--receipt".to_owned(),
            reason: "file not found".to_owned(),
            detail: None,
        },
        "network" => ViError::Network {
            endpoint: "https://provider.example".to_owned(),
            kind: NetworkErrorKind::Timeout,
            http_status: None,
        },
        "hash_mismatch" => ViError::HashMismatch {
            expected: "sha256:expected".to_owned(),
            actual: "sha256:actual".to_owned(),
        },
        "receipt_missing" => ViError::ReceiptMissing {
            endpoint: "https://provider.example".to_owned(),
            content_type: "application/json".to_owned(),
        },
        "unknown_version" => ViError::UnknownVersion {
            envelope: "VIRC",
            field: "ver",
            value: 9,
            supported: vec![1],
        },
        "identity_mismatch" => ViError::IdentityMismatch {
            expected: IdentityFields::new("model", "sha256:abc", "25541e83"),
            actual: IdentityFields::new("other-model", "sha256:def", "25541e83"),
        },
        "unsupported_tier" => ViError::UnsupportedTier {
            requested: "full".to_owned(),
            reason: "missing --audit-endpoint".to_owned(),
        },
        "corrupt_envelope" => ViError::CorruptEnvelope {
            envelope: "VIRC",
            offset: 7,
            reason: "binding_crc32 mismatch",
        },
        "internal" => ViError::Internal {
            backtrace: "internal test error".to_owned(),
        },
        _ => return None,
    })
}
