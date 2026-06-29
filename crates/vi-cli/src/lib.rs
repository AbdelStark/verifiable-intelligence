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
    path::PathBuf,
    process,
    sync::Once,
};

use clap::{error::ErrorKind, Args, Parser, Subcommand};
use vi_errors::{ErrorEnvelope, IdentityFields, NetworkErrorKind, PhaseId, ViError};

const SUBCOMMAND: &str = "vi";
const TEST_HOOKS_ENV: &str = "VI_ENABLE_TEST_HOOKS";
const ENDPOINT_ENV: &str = "VI_ENDPOINT";
const API_KEY_ENV: &str = "VI_API_KEY";
const LOG_ENV: &str = "VI_LOG";
const RUST_LOG_ENV: &str = "RUST_LOG";
const NO_COLOR_ENV: &str = "VI_NO_COLOR";
const ANSI_NO_COLOR_ENV: &str = "NO_COLOR";
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
#[command(
    name = "vi",
    version,
    about = "Verifiable Intelligence CLI",
    disable_help_subcommand = true
)]
struct Cli {
    #[arg(
        long,
        global = true,
        help = "Format the same output object for humans instead of compact JSON"
    )]
    pretty: bool,

    #[arg(
        long,
        global = true,
        value_name = "FILTER",
        help = "Log filter; overrides VI_LOG and RUST_LOG"
    )]
    log: Option<String>,

    #[arg(
        long,
        global = true,
        value_name = "TOKEN",
        help = "Provider API key; VI_API_KEY is preferred"
    )]
    api_key: Option<String>,

    #[arg(long, global = true, help = "Disable ANSI color in pretty output")]
    no_color: bool,

    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    /// Generate a verifier key envelope.
    Keygen(KeygenArgs),
    /// Send a chat request to a receipt-capable provider.
    Chat(ChatArgs),
    /// Verify a receipt against a verifier key.
    Verify(VerifyArgs),
    /// Run the deferred terminal demo UI.
    Tui(TuiArgs),
    #[command(name = "__panic", hide = true)]
    Panic,
    #[command(name = "__error", hide = true)]
    Error {
        #[arg(value_name = "CATEGORY")]
        category: String,
    },
}

#[derive(Debug, Args)]
struct KeygenArgs {
    #[arg(
        long,
        value_name = "MODEL",
        help = "Public model identifier to bind into the verifier key"
    )]
    model: String,

    #[arg(
        long,
        value_name = "DIR",
        help = "Local checkpoint directory containing config, safetensors, and tokenizer files"
    )]
    checkpoint: PathBuf,

    #[arg(
        short = 'o',
        long,
        value_name = "PATH",
        help = "Destination path for the generated VIKY verifier-key envelope"
    )]
    output: PathBuf,

    #[arg(
        long,
        value_name = "U64",
        default_value_t = 0,
        help = "Deterministic key-generation seed"
    )]
    seed: u64,

    #[arg(long, help = "Overwrite an existing output file")]
    force: bool,

    #[arg(
        long,
        value_name = "SHA256",
        help = "Expected canonical checkpoint hash formatted as sha256:<hex>"
    )]
    expected_checkpoint_hash: Option<String>,

    #[arg(
        long,
        help = "Warn instead of failing when --expected-checkpoint-hash mismatches"
    )]
    allow_checkpoint_drift: bool,
}

#[derive(Debug, Args)]
struct ChatArgs {
    #[arg(
        short = 'e',
        long,
        value_name = "URL",
        help = "Provider or broker endpoint; defaults to VI_ENDPOINT"
    )]
    endpoint: Option<String>,
}

#[derive(Debug, Args)]
struct VerifyArgs {
    #[arg(
        short = 'e',
        long,
        value_name = "URL",
        help = "Provider or audit endpoint; defaults to VI_ENDPOINT"
    )]
    endpoint: Option<String>,
}

#[derive(Debug, Args)]
struct TuiArgs {
    #[arg(
        short = 'e',
        long,
        value_name = "URL",
        help = "Provider or broker endpoint; defaults to VI_ENDPOINT"
    )]
    endpoint: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ResolvedConfig {
    pretty: bool,
    log: Option<String>,
    api_key: Option<String>,
    no_color: bool,
    endpoint: Option<String>,
}

impl ResolvedConfig {
    fn from_env(cli: &Cli) -> Self {
        Self::from_sources(cli, |name| std::env::var_os(name))
    }

    fn from_sources<F>(cli: &Cli, mut env: F) -> Self
    where
        F: FnMut(&str) -> Option<OsString>,
    {
        let endpoint = command_endpoint(cli).map(ToOwned::to_owned).or_else(|| {
            if command_uses_endpoint(cli) {
                env_string(&mut env, ENDPOINT_ENV)
            } else {
                None
            }
        });
        let log = cli
            .log
            .clone()
            .or_else(|| env_string(&mut env, LOG_ENV))
            .or_else(|| env_string(&mut env, RUST_LOG_ENV));
        let api_key = cli
            .api_key
            .clone()
            .or_else(|| env_string(&mut env, API_KEY_ENV));
        let no_color = cli.no_color
            || env_present(&mut env, NO_COLOR_ENV)
            || env_present(&mut env, ANSI_NO_COLOR_ENV);

        Self {
            pretty: cli.pretty,
            log,
            api_key,
            no_color,
            endpoint,
        }
    }
}

/// Library dispatch entry point. Converts domain failures into `ViError`.
pub fn run() -> Result<Output, ViError> {
    // The leaf-crate calls keep the workspace link graph exercised at build
    // time before real subcommand logic lands; they are cheap no-ops.
    vi_client::placeholder();
    vi_keygen::placeholder();
    vi_log::placeholder();
    let _ = vi_receipt::HEADER_LEN;
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
            let exit_code = match error.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => 0,
                _ => vi_errors::USAGE_EXIT_CODE,
            };
            if let Err(print_error) = error.print() {
                eprintln!("failed to print clap error: {print_error}");
            }
            exit_code
        }
    }
}

fn run_cli(cli: Cli) -> Result<Output, ViError> {
    let config = ResolvedConfig::from_env(&cli);

    match cli.command {
        Some(CliCommand::Keygen(args)) => run_keygen(args, &config),
        Some(CliCommand::Chat(_)) => {
            vi_client::placeholder();
            stub_output("chat", &config)
        }
        Some(CliCommand::Verify(_)) => {
            vi_verifier::placeholder();
            stub_output("verify", &config)
        }
        Some(CliCommand::Tui(_)) => run_tui_stub(&config),
        Some(CliCommand::Panic) => {
            ensure_test_hooks_enabled()?;
            panic!("deliberate vi panic test hook");
        }
        Some(CliCommand::Error { category }) => {
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

fn run_tui_stub(config: &ResolvedConfig) -> Result<Output, ViError> {
    #[cfg(feature = "tui")]
    {
        vi_tui::placeholder();
        stub_output("tui", config)
    }

    #[cfg(not(feature = "tui"))]
    {
        let _ = config;
        Err(ViError::UnsupportedTier {
            requested: "tui".to_owned(),
            reason: "the vi binary was built without the tui feature".to_owned(),
        })
    }
}

fn run_keygen(args: KeygenArgs, config: &ResolvedConfig) -> Result<Output, ViError> {
    let mut options = vi_keygen::KeygenOptions::new(args.model, args.checkpoint, args.output)
        .with_seed(args.seed)
        .with_force(args.force)
        .with_allow_checkpoint_drift(args.allow_checkpoint_drift);
    if let Some(expected_checkpoint_hash) = args.expected_checkpoint_hash {
        options = options.with_expected_checkpoint_hash(expected_checkpoint_hash);
    }

    let report = vi_keygen::keygen_with_options(&options)?;
    let mut value = serde_json::to_value(report).map_err(|error| ViError::Internal {
        backtrace: format!("failed to serialize keygen output: {error}"),
    })?;
    let object = value.as_object_mut().ok_or_else(|| ViError::Internal {
        backtrace: "keygen report did not serialize to an object".to_owned(),
    })?;
    object.insert("schema_version".to_owned(), serde_json::json!(1));
    object.insert("subcommand".to_owned(), serde_json::json!("keygen"));

    json_output(&value, config, "keygen")
}

fn stub_output(subcommand: &'static str, config: &ResolvedConfig) -> Result<Output, ViError> {
    let value = serde_json::json!({
        "schema_version": 1,
        "subcommand": subcommand,
        "status": "stub",
    });
    json_output(&value, config, subcommand)
}

fn json_output(
    value: &serde_json::Value,
    config: &ResolvedConfig,
    context: &'static str,
) -> Result<Output, ViError> {
    let stdout = if config.pretty {
        serde_json::to_string_pretty(&value)
    } else {
        serde_json::to_string(&value)
    }
    .map_err(|error| ViError::Internal {
        backtrace: format!("failed to serialize {context} output: {error}"),
    })?;

    Ok(Output {
        stdout: Some(stdout),
    })
}

fn command_endpoint(cli: &Cli) -> Option<&str> {
    match &cli.command {
        Some(CliCommand::Chat(args)) => args.endpoint.as_deref(),
        Some(CliCommand::Verify(args)) => args.endpoint.as_deref(),
        Some(CliCommand::Tui(args)) => args.endpoint.as_deref(),
        Some(CliCommand::Keygen(_) | CliCommand::Panic | CliCommand::Error { .. }) | None => None,
    }
}

fn command_uses_endpoint(cli: &Cli) -> bool {
    matches!(
        cli.command,
        Some(CliCommand::Chat(_) | CliCommand::Verify(_) | CliCommand::Tui(_))
    )
}

fn env_string<F>(env: &mut F, name: &str) -> Option<String>
where
    F: FnMut(&str) -> Option<OsString>,
{
    env(name).and_then(|value| value.into_string().ok().filter(|string| !string.is_empty()))
}

fn env_present<F>(env: &mut F, name: &str) -> bool
where
    F: FnMut(&str) -> Option<OsString>,
{
    env(name).is_some()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_env_applies_only_to_endpoint_subcommands() {
        let chat = Cli::try_parse_from(["vi", "chat"]).expect("chat parses");
        let chat_config =
            ResolvedConfig::from_sources(&chat, fake_env(&[(ENDPOINT_ENV, "https://env.example")]));
        assert_eq!(chat_config.endpoint.as_deref(), Some("https://env.example"));

        let chat_with_flag =
            Cli::try_parse_from(["vi", "chat", "--endpoint", "https://flag.example"])
                .expect("chat with endpoint parses");
        let flag_config = ResolvedConfig::from_sources(
            &chat_with_flag,
            fake_env(&[(ENDPOINT_ENV, "https://env.example")]),
        );
        assert_eq!(
            flag_config.endpoint.as_deref(),
            Some("https://flag.example")
        );

        let keygen = Cli::try_parse_from([
            "vi",
            "keygen",
            "--model",
            "model",
            "--checkpoint",
            "checkpoint",
            "--output",
            "key.viky",
        ])
        .expect("keygen parses");
        let keygen_config = ResolvedConfig::from_sources(
            &keygen,
            fake_env(&[(ENDPOINT_ENV, "https://env.example")]),
        );
        assert_eq!(keygen_config.endpoint, None);
    }

    #[test]
    fn log_precedence_is_flag_then_vi_log_then_rust_log() {
        let env_only = Cli::try_parse_from(["vi", "verify"]).expect("verify parses");
        let env_only_config = ResolvedConfig::from_sources(
            &env_only,
            fake_env(&[(LOG_ENV, "vi=debug"), (RUST_LOG_ENV, "warn")]),
        );
        assert_eq!(env_only_config.log.as_deref(), Some("vi=debug"));

        let rust_log_only = Cli::try_parse_from(["vi", "verify"]).expect("verify parses");
        let rust_log_config =
            ResolvedConfig::from_sources(&rust_log_only, fake_env(&[(RUST_LOG_ENV, "warn")]));
        assert_eq!(rust_log_config.log.as_deref(), Some("warn"));

        let flag = Cli::try_parse_from(["vi", "--log", "trace", "verify"])
            .expect("verify with log parses");
        let flag_config = ResolvedConfig::from_sources(&flag, fake_env(&[(LOG_ENV, "vi=debug")]));
        assert_eq!(flag_config.log.as_deref(), Some("trace"));
    }

    #[test]
    fn api_key_and_no_color_precedence_are_resolved() {
        let env_only = Cli::try_parse_from(["vi", "tui"]).expect("tui parses");
        let env_only_config = ResolvedConfig::from_sources(
            &env_only,
            fake_env(&[(API_KEY_ENV, "env-token"), (ANSI_NO_COLOR_ENV, "")]),
        );
        assert_eq!(env_only_config.api_key.as_deref(), Some("env-token"));
        assert!(env_only_config.no_color);

        let flag = Cli::try_parse_from(["vi", "--api-key", "flag-token", "--no-color", "tui"])
            .expect("tui with globals parses");
        let flag_config = ResolvedConfig::from_sources(
            &flag,
            fake_env(&[(API_KEY_ENV, "env-token"), (NO_COLOR_ENV, "1")]),
        );
        assert_eq!(flag_config.api_key.as_deref(), Some("flag-token"));
        assert!(flag_config.no_color);
    }

    fn fake_env<'a>(vars: &'a [(&'a str, &'a str)]) -> impl FnMut(&str) -> Option<OsString> + 'a {
        move |name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        }
    }
}
