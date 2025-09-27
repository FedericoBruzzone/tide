//! This crate allows tools to enable rust logging.
//!
//! The allowed environment variables are:
//! - `<PREFIX>_LOG`: The log level. This can be "debug", "info", "warn", "error", or "trace".
//! - `<PREFIX>_LOG_COLOR`: The color setting. This can be "always", "never", or "auto".
//! - `<PREFIX>_LOG_WRITER`: The log writer. This can be "stdout", "stderr", or a file path. If the file path does not exist, it will be created.
//! - `<PREFIX>_LOG_LINE_NUMBERS`: Whether to show line numbers in the log. This can be "1" or "0".
//!
//! The `<PREFIX>` is a prefix that can be set to any string. It is used to customize the log configuration for different tools. For example, `tidec` uses `TIDEC` as the prefix.
//!
//!
//! Suppose you're working on `tidec_lir` and want to run a minimal standalone
//! program that can be debugged with access to `debug!` logs emitted by
//! `tidec_lir`. You can do this by writing:
//!
//! ```toml
//! [dependencies]
//! tidec_lir = { path = "../tidec_lir" }
//! tidec_log = { path = "../tidec_log" }
//! ```
//!
//! And in your `main.rs`:
//!
//! ```rust
//! let _ = tidec_log::Logger::init_logger(
//!     tidec_log::LoggerConfig::from_prefix("TIDEC").unwrap(),
//!     tidec_log::FallbackDefaultEnv::Yes
//! );
//! ```
//!
//! Then run your program with:
//!
//! ```bash
//! TIDEC_LOG=debug cargo run
//! ```
//!
//! For convenience, you can also include this at the top of `main`:
//!
//! ```rust
//! unsafe { std::env::set_var("TIDEC_LOG", "debug"); }
//! ```
//!
//! This allows you to simply run `cargo run` and still see debug output.
//!
//! ---
//!
//! The `tidec_log` crate exists as a minimal, self-contained logger setup,
//! allowing you to enable logging without depending on the much larger
//! `tidec` crate. This helps you iterate quickly on individual compiler
//! components like `tidec_lir`, without requiring full rebuilds of the entire
//! compiler stack.

use std::{env::VarError, fmt::Debug, fs::File, io::IsTerminal, path::PathBuf};
use tracing::Subscriber;
use tracing_subscriber::{
    EnvFilter, Layer,
    fmt::{format::FmtSpan, layer},
    prelude::*,
    registry::LookupSpan,
    util::TryInitError,
};

/// The ZST (zero-sized type) for the logger.
pub struct Logger;

#[derive(Debug)]
/// The writer for the logger.
/// This is used to determine where the logs will be written to.
pub enum LogWriter {
    /// Write to stdout.
    Stdout,
    /// Write to stderr.
    Stderr,
    /// Write to a file.
    File(PathBuf),
}

/// The configuration for the logger.
pub struct LoggerConfig {
    /// The writer for the logger.
    pub log_writer: LogWriter,
    /// The filter for the logger.
    /// This is a string that can be "debug", "info", "warn", "error", or "trace".
    pub filter: Result<String, VarError>,
    /// Whether to use color in the logger.
    /// This is a string that can be "always", "never", or "auto".
    pub color: Result<String, VarError>,
    /// Whether to show line numbers in the logger.
    /// If this is set to "1", line numbers will be shown otherwise they will not.
    pub line_numbers: Result<String, VarError>,
    /// Whether to show file names in the logger.
    /// If this is set to "1", file names will be shown otherwise they will not.
    pub file_names: Result<String, VarError>,
}

#[derive(Debug)]
/// The error type for the logger.
pub enum LogError {
    /// The color value is not valid.
    ColorNotValid(String),
    /// The color value is not a valid unicode string.
    NotUnicode(String),
    /// Wrapping an IO error.
    IoError(std::io::Error),
    /// Wrapping a TryInitError.
    TryInitError(TryInitError),
}

/// The fallback default environment variable for the logger.
/// That is, if the <PREFIX>_LOG environment variable is not set, this will be used
/// to determine whether to use the default environment variable (`RUST_LOG`) for the logger.
pub enum FallbackDefaultEnv {
    /// Use the default environment variable for the logger.
    Yes,
    /// Do not use the default environment variable for the logger.
    No,
}

impl LoggerConfig {
    /// Create a new logger configuration from the given environment variable.
    pub fn from_prefix(prefix_env_var: &str) -> Result<Self, VarError> {
        let filter = std::env::var(format!("{}_LOG", prefix_env_var));
        let color = std::env::var(format!("{}_LOG_COLOR", prefix_env_var));
        let log_writer = std::env::var(format!("{}_LOG_WRITER", prefix_env_var))
            .map(|s| match s.as_str() {
                "stdout" => LogWriter::Stdout,
                "stderr" => LogWriter::Stderr,
                _ => LogWriter::File(s.into()),
            })
            .unwrap_or(LogWriter::Stderr);
        let line_numbers = std::env::var(format!("{}_LOG_LINE_NUMBERS", prefix_env_var));
        let file_names = std::env::var(format!("{}_LOG_FILE_NAMES", prefix_env_var));

        Ok(LoggerConfig {
            filter,
            color,
            log_writer,
            line_numbers,
            file_names,
        })
    }
}

impl Logger {
    pub fn init_logger(
        cfg: LoggerConfig,
        fallback_default_env: FallbackDefaultEnv,
    ) -> Result<(), LogError> {
        let filter = match cfg.filter {
            Ok(filter) => EnvFilter::new(filter),
            Err(_) => {
                if let FallbackDefaultEnv::Yes = fallback_default_env {
                    EnvFilter::from_default_env()
                } else {
                    EnvFilter::default().add_directive(tracing::Level::INFO.into())
                }
            }
        };

        let color_log = match cfg.color {
            Ok(color) => match color.as_str() {
                "always" => true,
                "never" => false,
                "auto" => std::io::stderr().is_terminal(),
                e => return Err(LogError::ColorNotValid(e.to_string())),
            },
            Err(VarError::NotPresent) => std::io::stderr().is_terminal(),
            Err(VarError::NotUnicode(os_string)) => {
                return Err(LogError::NotUnicode(
                    os_string.to_string_lossy().to_string(),
                ));
            }
        };

        let line_numbers = match cfg.line_numbers {
            Ok(line_numbers) => &line_numbers == "1",
            Err(_) => false,
        };

        let file_names = match cfg.file_names {
            Ok(file_names) => &file_names == "1",
            Err(_) => false,
        };

        let layer = Self::create_layer(cfg.log_writer, color_log, line_numbers, file_names);
        // Here we can add other layers

        let subscriber = tracing_subscriber::Registry::default()
            .with(filter)
            .with(layer);

        let _ = subscriber.try_init().map_err(LogError::TryInitError);

        Ok(())
    }

    fn create_layer<S>(
        log_writer: LogWriter,
        color_log: bool,
        line_numbers: bool,
        file_names: bool,
    ) -> Box<dyn Layer<S> + Send + Sync + 'static>
    where
        S: Subscriber,
        for<'a> S: LookupSpan<'a>,
    {
        let layer = layer()
            .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE) // FmtSpan::FULL
            .with_target(true)
            .with_file(file_names)
            .with_ansi(color_log)
            .with_line_number(line_numbers);

        match log_writer {
            LogWriter::Stdout => Box::new(layer.with_writer(std::io::stdout)),
            LogWriter::Stderr => Box::new(layer.with_writer(std::io::stderr)),
            LogWriter::File(path) => {
                let file = File::create(path).expect("Failed to create log file");
                Box::new(layer.with_writer(file))
            }
        }
    }
}

impl std::error::Error for LogError {}

impl std::fmt::Display for LogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogError::ColorNotValid(s) => write!(f, "Color not valid: {}", s),
            LogError::NotUnicode(s) => write!(f, "Not unicode: {}", s),
            LogError::IoError(e) => write!(f, "IO error: {}", e),
            LogError::TryInitError(e) => write!(f, "TryInit error: {:?}", e),
        }
    }
}
