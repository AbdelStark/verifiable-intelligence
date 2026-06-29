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
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    process,
    sync::Once,
};

use clap::{error::ErrorKind, Args, Parser, Subcommand};
use serde::Serialize;
use sha2::{Digest, Sha256};
use vi_errors::{ErrorEnvelope, IdentityFields, NetworkErrorKind, PhaseId, ViError};
use vi_receipt::{AuditChallenge, AuditTier, ReceiptBindingHeader};

const CLI_OUTPUT_SCHEMA_VERSION: u16 = 1;
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

    #[arg(
        short = 'p',
        long,
        value_name = "TEXT",
        help = "User prompt to send as a single OpenAI-compatible user message"
    )]
    prompt: String,

    #[arg(
        long,
        value_name = "U32",
        default_value_t = 256,
        help = "Maximum generated tokens requested from the provider"
    )]
    max_tokens: u32,

    #[arg(
        long,
        value_name = "PATH",
        help = "Destination path for the VIRC receipt; required unless --no-receipt is set"
    )]
    receipt_out: Option<PathBuf>,

    #[arg(
        long,
        help = "Send a plain OpenAI-compatible request without receipt opt-in"
    )]
    no_receipt: bool,
}

#[derive(Debug, Args)]
struct VerifyArgs {
    #[arg(long, value_name = "PATH", help = "Path to the VIRC receipt envelope")]
    receipt: PathBuf,

    #[arg(
        long,
        value_name = "PATH",
        help = "Path to the VIKY verifier-key envelope"
    )]
    key: PathBuf,

    #[arg(
        long,
        value_name = "TIER",
        default_value = "routine",
        help = "Verification tier: receipt-only, routine, deep, or full"
    )]
    tier: String,

    #[arg(
        long,
        value_name = "URL|file://PATH",
        help = "Provider audit endpoint for deep/full tiers; defaults to VI_ENDPOINT"
    )]
    audit_endpoint: Option<String>,
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

    #[arg(
        long,
        value_name = "MODE",
        help = "Tamper mode for the next request: byte-flip"
    )]
    tamper: Option<String>,

    #[arg(
        long,
        value_name = "MS",
        default_value_t = 0,
        help = "Delay between verifier phases in milliseconds"
    )]
    phase_delay: u64,
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
        Some(CliCommand::Chat(args)) => run_chat(&args, &config),
        Some(CliCommand::Verify(args)) => run_verify(&args, &config),
        Some(CliCommand::Tui(args)) => run_tui(&args, &config),
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

fn run_tui(args: &TuiArgs, config: &ResolvedConfig) -> Result<Output, ViError> {
    #[cfg(feature = "tui")]
    {
        let tamper = parse_tui_tamper(args.tamper.as_deref())?;
        let report = vi_tui::run(vi_tui::RunOptions::new(
            config.endpoint.clone(),
            tamper,
            args.phase_delay,
        ))?;
        let output = TuiOutput::from_report(report);
        json_output(&output, config, "tui")
    }

    #[cfg(not(feature = "tui"))]
    {
        let _ = (args, config);
        Err(ViError::UnsupportedTier {
            requested: "tui".to_owned(),
            reason: "the vi binary was built without the tui feature".to_owned(),
        })
    }
}

#[cfg(feature = "tui")]
fn parse_tui_tamper(value: Option<&str>) -> Result<Option<vi_tui::TamperMode>, ViError> {
    let Some(value) = value else {
        return Ok(None);
    };

    match value {
        "byte-flip" | "byte_flip" => Ok(Some(vi_tui::TamperMode::ByteFlip)),
        _ => Err(ViError::Input {
            arg: "--tamper".to_owned(),
            reason: "unsupported tamper mode; expected byte-flip".to_owned(),
            detail: None,
        }),
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

    let output = KeygenOutput::new(vi_keygen::keygen_with_options(&options)?);

    json_output(&output, config, "keygen")
}

fn run_chat(args: &ChatArgs, config: &ResolvedConfig) -> Result<Output, ViError> {
    if args.no_receipt && args.receipt_out.is_some() {
        return Err(ViError::Input {
            arg: "--receipt-out".to_owned(),
            reason: "--receipt-out cannot be used with --no-receipt".to_owned(),
            detail: None,
        });
    }
    let endpoint = config.endpoint.as_ref().ok_or_else(|| ViError::Input {
        arg: "--endpoint".to_owned(),
        reason: "provider endpoint is required; pass --endpoint or set VI_ENDPOINT".to_owned(),
        detail: None,
    })?;
    let body = chat_request_body(&args.prompt, args.max_tokens)?;
    let client = vi_client::ChatClient::new(endpoint, config.api_key.clone())?;
    let trace_id = trace_id();

    if args.no_receipt {
        let response = client.post_chat_completions_without_receipt(&trace_id, body)?;
        let text = vi_client::parse_openai_chat_response(&response)?;
        let output = ChatOutput::new(response.endpoint, response.status, text, None, Vec::new());
        return json_output(&output, config, "chat");
    }

    let receipt_out = args.receipt_out.as_ref().ok_or_else(|| ViError::Input {
        arg: "--receipt-out".to_owned(),
        reason: "--receipt-out is required unless --no-receipt is set".to_owned(),
        detail: None,
    })?;
    let response = client.post_chat_completions(&trace_id, body)?;
    let parsed = vi_client::parse_chat_response(&response)?;
    let receipt_bytes = parsed
        .receipt_bytes
        .ok_or_else(|| ViError::ReceiptMissing {
            endpoint: response.endpoint.clone(),
            content_type: response.content_type.clone().unwrap_or_default(),
        })?;
    write_binary_output(receipt_out, &receipt_bytes)?;
    let output = ChatOutput::new(
        response.endpoint,
        response.status,
        parsed.text,
        Some(ChatReceiptOutput {
            path: receipt_out,
            size_bytes: receipt_bytes.len(),
        }),
        parsed.warning.into_iter().collect(),
    );

    json_output(&output, config, "chat")
}

fn run_verify(args: &VerifyArgs, config: &ResolvedConfig) -> Result<Output, ViError> {
    let tier = parse_audit_tier(&args.tier)?;
    if tier_requires_audit(tier) && config.endpoint.is_none() {
        return Err(missing_audit_endpoint(tier));
    }

    let receipt_bytes = read_input_file(&args.receipt, "--receipt")?;
    let key_bytes = read_input_file(&args.key, "--key")?;
    let mut audit_provider = build_verify_audit_provider(tier, config, &receipt_bytes)?;
    let report = vi_verifier::verify(&receipt_bytes, &key_bytes, tier, &mut audit_provider)?;
    let output = VerifyOutput::from_report(&report);

    json_output(&output, config, "verify")
}

fn parse_audit_tier(value: &str) -> Result<AuditTier, ViError> {
    match value.to_ascii_lowercase().as_str() {
        "receipt-only" | "receipt_only" | "receipt" => Ok(AuditTier::ReceiptOnly),
        "routine" => Ok(AuditTier::Routine),
        "deep" => Ok(AuditTier::Deep),
        "full" => Ok(AuditTier::Full),
        _ => Err(ViError::UnsupportedTier {
            requested: value.to_owned(),
            reason: "unknown audit tier; expected receipt-only, routine, deep, or full".to_owned(),
        }),
    }
}

fn tier_requires_audit(tier: AuditTier) -> bool {
    matches!(tier, AuditTier::Deep | AuditTier::Full)
}

fn missing_audit_endpoint(tier: AuditTier) -> ViError {
    ViError::UnsupportedTier {
        requested: tier.as_str().to_owned(),
        reason: "missing --audit-endpoint".to_owned(),
    }
}

fn read_input_file(path: &Path, arg: &'static str) -> Result<Vec<u8>, ViError> {
    fs::read(path).map_err(|error| ViError::Input {
        arg: arg.to_owned(),
        reason: format!("failed to read {}: {error}", path.display()),
        detail: None,
    })
}

fn build_verify_audit_provider(
    tier: AuditTier,
    config: &ResolvedConfig,
    receipt_bytes: &[u8],
) -> Result<VerifyAuditProvider, ViError> {
    if !tier_requires_audit(tier) {
        return Ok(VerifyAuditProvider::None);
    }

    let endpoint = config
        .endpoint
        .as_ref()
        .ok_or_else(|| missing_audit_endpoint(tier))?;
    if let Some(path) = endpoint.strip_prefix("file://") {
        if path.is_empty() {
            return Err(ViError::Input {
                arg: "--audit-endpoint".to_owned(),
                reason: "file:// audit endpoint must include a path".to_owned(),
                detail: None,
            });
        }
        return Ok(VerifyAuditProvider::File {
            path: PathBuf::from(path),
        });
    }

    Ok(VerifyAuditProvider::Http {
        client: vi_client::ChatClient::new(endpoint, config.api_key.clone())?,
        receipt_hash: sha256_hex(receipt_bytes),
        trace_id: trace_id(),
    })
}

#[derive(Debug)]
enum VerifyAuditProvider {
    None,
    File {
        path: PathBuf,
    },
    Http {
        client: vi_client::ChatClient,
        receipt_hash: String,
        trace_id: String,
    },
}

impl vi_verifier::AuditProvider for VerifyAuditProvider {
    fn fetch_audit(
        &mut self,
        _receipt: &ReceiptBindingHeader,
        tier: AuditTier,
    ) -> Result<Vec<u8>, ViError> {
        match self {
            Self::None => Err(missing_audit_endpoint(tier)),
            Self::File { path } => fs::read(&*path).map_err(|error| ViError::Input {
                arg: "--audit-endpoint".to_owned(),
                reason: format!("failed to read {}: {error}", path.display()),
                detail: None,
            }),
            Self::Http {
                client,
                receipt_hash,
                trace_id,
            } => {
                let challenge = AuditChallenge::new(tier, 0, vec![0, 1]);
                let request = vi_client::AuditRequest::new(receipt_hash.clone(), challenge);
                client
                    .post_audit(trace_id, &request)
                    .map(|response| response.body)
            }
        }
    }
}

#[derive(Debug, Serialize)]
struct KeygenOutput {
    schema_version: u16,
    subcommand: &'static str,
    #[serde(flatten)]
    report: vi_keygen::KeygenReport,
}

impl KeygenOutput {
    fn new(report: vi_keygen::KeygenReport) -> Self {
        Self {
            schema_version: CLI_OUTPUT_SCHEMA_VERSION,
            subcommand: "keygen",
            report,
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatOutput<'a> {
    schema_version: u16,
    subcommand: &'static str,
    endpoint: String,
    status: u16,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    receipt: Option<ChatReceiptOutput<'a>>,
    warnings: Vec<String>,
}

impl<'a> ChatOutput<'a> {
    fn new(
        endpoint: String,
        status: u16,
        text: String,
        receipt: Option<ChatReceiptOutput<'a>>,
        warnings: Vec<String>,
    ) -> Self {
        Self {
            schema_version: CLI_OUTPUT_SCHEMA_VERSION,
            subcommand: "chat",
            endpoint,
            status,
            text,
            receipt,
            warnings,
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatReceiptOutput<'a> {
    path: &'a Path,
    size_bytes: usize,
}

#[derive(Debug, Serialize)]
struct VerifyOutput<'a> {
    schema_version: u16,
    subcommand: &'static str,
    tier: &'static str,
    verdict: &'static str,
    phases: Vec<&'static str>,
    checks_run: usize,
    checks_passed: usize,
    warnings: &'a [String],
    elapsed_ms: u64,
}

impl<'a> VerifyOutput<'a> {
    fn from_report(report: &'a vi_verifier::VerifyReport) -> Self {
        Self {
            schema_version: CLI_OUTPUT_SCHEMA_VERSION,
            subcommand: "verify",
            tier: report.tier.as_str(),
            verdict: match report.verdict {
                vi_verifier::VerifyVerdict::Pass => "pass",
                vi_verifier::VerifyVerdict::Fail => "fail",
            },
            phases: report.phases.iter().map(|phase| phase.as_str()).collect(),
            checks_run: report.checks_run,
            checks_passed: report.checks_passed,
            warnings: &report.warnings,
            elapsed_ms: report.elapsed_ms,
        }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(not(feature = "tui"), allow(dead_code))]
struct TuiOutput {
    schema_version: u16,
    subcommand: &'static str,
    status: &'static str,
    endpoint: Option<String>,
    tamper: Option<&'static str>,
    phase_delay_ms: u64,
}

impl TuiOutput {
    #[cfg(feature = "tui")]
    fn from_report(report: vi_tui::RunReport) -> Self {
        Self {
            schema_version: CLI_OUTPUT_SCHEMA_VERSION,
            subcommand: "tui",
            status: report.status,
            endpoint: report.endpoint,
            tamper: report.tamper.map(vi_tui::TamperMode::as_str),
            phase_delay_ms: report.phase_delay_ms,
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity("sha256:".len() + digest.len() * 2);
    output.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn chat_request_body(prompt: &str, max_tokens: u32) -> Result<String, ViError> {
    serde_json::to_string(&serde_json::json!({
        "messages": [
            {
                "role": "user",
                "content": prompt,
            }
        ],
        "max_tokens": max_tokens,
    }))
    .map_err(|error| ViError::Internal {
        backtrace: format!("failed to serialize chat request: {error}"),
    })
}

fn write_binary_output(path: &Path, bytes: &[u8]) -> Result<(), ViError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| ViError::Input {
            arg: path.display().to_string(),
            reason: format!("failed to create output directory: {error}"),
            detail: None,
        })?;
    }

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| ViError::Input {
            arg: path.display().to_string(),
            reason: format!("failed to open output file: {error}"),
            detail: None,
        })?;
    file.write_all(bytes).map_err(|error| ViError::Input {
        arg: path.display().to_string(),
        reason: format!("failed to write output file: {error}"),
        detail: None,
    })
}

fn json_output(
    value: &(impl Serialize + ?Sized),
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
        Some(CliCommand::Verify(args)) => args.audit_endpoint.as_deref(),
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
        let chat = Cli::try_parse_from(["vi", "chat", "--prompt", "hello"]).expect("chat parses");
        let chat_config =
            ResolvedConfig::from_sources(&chat, fake_env(&[(ENDPOINT_ENV, "https://env.example")]));
        assert_eq!(chat_config.endpoint.as_deref(), Some("https://env.example"));

        let chat_with_flag = Cli::try_parse_from([
            "vi",
            "chat",
            "--endpoint",
            "https://flag.example",
            "--prompt",
            "hello",
        ])
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
        let env_only = Cli::try_parse_from([
            "vi",
            "verify",
            "--receipt",
            "receipt.virc",
            "--key",
            "key.viky",
        ])
        .expect("verify parses");
        let env_only_config = ResolvedConfig::from_sources(
            &env_only,
            fake_env(&[(LOG_ENV, "vi=debug"), (RUST_LOG_ENV, "warn")]),
        );
        assert_eq!(env_only_config.log.as_deref(), Some("vi=debug"));

        let rust_log_only = Cli::try_parse_from([
            "vi",
            "verify",
            "--receipt",
            "receipt.virc",
            "--key",
            "key.viky",
        ])
        .expect("verify parses");
        let rust_log_config =
            ResolvedConfig::from_sources(&rust_log_only, fake_env(&[(RUST_LOG_ENV, "warn")]));
        assert_eq!(rust_log_config.log.as_deref(), Some("warn"));

        let flag = Cli::try_parse_from([
            "vi",
            "--log",
            "trace",
            "verify",
            "--receipt",
            "receipt.virc",
            "--key",
            "key.viky",
        ])
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

    #[test]
    fn success_output_shapes_match_snapshots() {
        assert_output_snapshot(
            &KeygenOutput::new(vi_keygen::KeygenReport {
                model_id: "toy-model".to_owned(),
                checkpoint_hash: "sha256:checkpoint".to_owned(),
                key_hash: "sha256:key".to_owned(),
                key_size_bytes: 1234,
                output: PathBuf::from("toy.viky"),
                seed: 7,
                commitllm_revision: vi_keygen::COMMITLLM_PIN.to_owned(),
                commitllm_pin: vi_keygen::COMMITLLM_SHORT_PIN.to_owned(),
                decode_artifact: None,
                warnings: Vec::new(),
            }),
            include_str!("../tests/snapshots/output/keygen.json"),
        );

        assert_output_snapshot(
            &ChatOutput::new(
                "https://provider.example/v1/chat/completions".to_owned(),
                200,
                "fixture answer".to_owned(),
                Some(ChatReceiptOutput {
                    path: Path::new("answer.virc"),
                    size_bytes: 42,
                }),
                Vec::new(),
            ),
            include_str!("../tests/snapshots/output/chat.json"),
        );

        let verify_report = vi_verifier::VerifyReport {
            tier: AuditTier::Full,
            verdict: vi_verifier::VerifyVerdict::Pass,
            phases: vi_verifier::VERIFY_PHASES.to_vec(),
            checks_run: vi_verifier::VERIFY_PHASES.len(),
            checks_passed: vi_verifier::VERIFY_PHASES.len(),
            warnings: Vec::new(),
            elapsed_ms: 11,
        };
        assert_output_snapshot(
            &VerifyOutput::from_report(&verify_report),
            include_str!("../tests/snapshots/output/verify.json"),
        );

        assert_output_snapshot(
            &TuiOutput {
                schema_version: CLI_OUTPUT_SCHEMA_VERSION,
                subcommand: "tui",
                status: "stub",
                endpoint: Some("https://provider.example".to_owned()),
                tamper: Some("byte-flip"),
                phase_delay_ms: 250,
            },
            include_str!("../tests/snapshots/output/tui.json"),
        );
    }

    #[cfg(not(feature = "tui"))]
    #[test]
    fn tui_without_feature_returns_unsupported_tier() {
        let cli = Cli::try_parse_from(["vi", "tui"]).expect("tui parses");

        let error = run_cli(cli).expect_err("tui should fail without the feature");

        assert_eq!(
            error,
            ViError::UnsupportedTier {
                requested: "tui".to_owned(),
                reason: "the vi binary was built without the tui feature".to_owned(),
            }
        );
    }

    fn assert_output_snapshot(value: &(impl Serialize + ?Sized), expected: &str) {
        let mut actual =
            serde_json::to_string_pretty(value).expect("output snapshot should serialize");
        actual.push('\n');
        assert_eq!(actual, expected);
    }

    fn fake_env<'a>(vars: &'a [(&'a str, &'a str)]) -> impl FnMut(&str) -> Option<OsString> + 'a {
        move |name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        }
    }
}
