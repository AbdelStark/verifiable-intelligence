//! `ratatui`-based TUI surface for the `vi` binary.
//!
//! Filled in by RFC-0008 implementation issues. Reached as a library from
//! `vi-cli`'s `tui` subcommand; never depends on `vi-cli`.

// Runtime failures must use the shared CLI taxonomy at the boundary.
#![allow(clippy::result_large_err)]

use vi_errors::ViError;

/// Options handed from `vi tui` into the TUI runtime boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOptions {
    /// Provider or broker endpoint resolved from `--endpoint` or `VI_ENDPOINT`.
    pub endpoint: Option<String>,
    /// Demo tamper mode for the next request.
    pub tamper: Option<TamperMode>,
    /// Artificial verifier phase delay in milliseconds.
    pub phase_delay_ms: u64,
}

impl RunOptions {
    /// Construct TUI runtime options.
    #[must_use]
    pub const fn new(
        endpoint: Option<String>,
        tamper: Option<TamperMode>,
        phase_delay_ms: u64,
    ) -> Self {
        Self {
            endpoint,
            tamper,
            phase_delay_ms,
        }
    }
}

/// User-visible demo tamper modes accepted by `vi tui`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TamperMode {
    /// Flip one receipt byte before verification.
    ByteFlip,
}

impl TamperMode {
    /// Stable CLI/API string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ByteFlip => "byte-flip",
        }
    }
}

/// Report returned by the current placeholder TUI runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    /// Placeholder runtime status.
    pub status: &'static str,
    /// Provider or broker endpoint passed to the runtime.
    pub endpoint: Option<String>,
    /// Demo tamper mode passed to the runtime.
    pub tamper: Option<TamperMode>,
    /// Artificial verifier phase delay in milliseconds.
    pub phase_delay_ms: u64,
}

/// Enter the TUI runtime boundary.
///
/// The renderer/event-loop internals land in the `vi-tui` issues. This
/// function gives `vi-cli` a typed handoff point now, so feature-gated CLI
/// builds do not keep routing through a bare placeholder.
pub fn run(options: RunOptions) -> Result<RunReport, ViError> {
    if options.endpoint.as_deref() == Some("") {
        return Err(ViError::Input {
            arg: "--endpoint".to_owned(),
            reason: "endpoint cannot be empty".to_owned(),
            detail: None,
        });
    }

    Ok(RunReport {
        status: "stub",
        endpoint: options.endpoint,
        tamper: options.tamper,
        phase_delay_ms: options.phase_delay_ms,
    })
}

pub fn placeholder() {}
