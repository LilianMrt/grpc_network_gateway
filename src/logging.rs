//! Process-wide log output.
//!
//! Every log line goes through `tracing`. This installs the one subscriber that
//! renders them: human-readable `fmt` text on stdout, filtered by `RUST_LOG`
//! when it is set and at `info` otherwise. ANSI colour is enabled only when
//! stdout is a terminal, so `kubectl logs` shows plain text.

use std::io::IsTerminal;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;

/// Installs the global subscriber. Call once, before anything logs.
pub fn init() {
    // Lossy rather than try_from_default_env: an empty RUST_LOG keeps the info
    // default instead of silencing everything, and an invalid directive is
    // reported on stderr and skipped rather than dropped without a word.
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(std::io::stdout().is_terminal())
        .init();
}
