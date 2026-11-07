//! Command-line entry point. This crate contains no analysis logic. It parses
//! arguments, runs the engine, injects runtime values, and writes output.
//!
//! Output discipline: stdout carries only the JSON report. Everything else goes
//! to stderr. Exit codes follow `architecture.md` §6:
//! 0 = report written (it may contain diagnostics), 1 = usage error,
//! 2 = fatal engine error, no report produced.

mod time;

use std::io::Write;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::SystemTime;

use clap::{Parser, Subcommand, ValueEnum};
use repo_intelligence_core::{run, scan, EngineError};
use repo_intelligence_detectors::default_registry;

#[derive(Parser)]
#[command(
    name = "repo-intelligence",
    version,
    about = "Analyze a repository and print a structured JSON report"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Analyze a repository and write the report to stdout.
    Analyze {
        /// Repository root. Must be an existing directory.
        path: PathBuf,
        /// Output format. Only `json` exists in this release.
        #[arg(long, value_enum, default_value_t = Format::Json)]
        format: Format,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Json,
}

const EXIT_USAGE: u8 = 1;
const EXIT_FATAL: u8 = 2;

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            // `--help` and `--version` are successes; other parse errors are usage errors.
            let _ = err.print();
            return if err.use_stderr() {
                ExitCode::from(EXIT_USAGE)
            } else {
                ExitCode::SUCCESS
            };
        }
    };
    match cli.command {
        Command::Analyze { path, format } => analyze(&path, format),
    }
}

fn analyze(path: &std::path::Path, format: Format) -> ExitCode {
    let ctx = match scan(path) {
        Ok(ctx) => ctx,
        Err(err) => {
            eprintln!("repo-intelligence: {err}");
            return match err {
                EngineError::RootNotFound(_) | EngineError::RootNotADirectory(_) => {
                    ExitCode::from(EXIT_USAGE)
                }
                EngineError::RootUnreadable(..) => ExitCode::from(EXIT_FATAL),
            };
        }
    };

    // A panicking detector must not leave partial output on stdout.
    let report = catch_unwind(AssertUnwindSafe(|| run(&ctx, &default_registry())));
    let Ok(mut report) = report else {
        eprintln!("repo-intelligence: internal error: analysis panicked");
        return ExitCode::from(EXIT_FATAL);
    };

    report.tool_version = env!("CARGO_PKG_VERSION").to_string();
    report.analyzed_at = time::rfc3339_utc(SystemTime::now());

    let text = match format {
        Format::Json => serde_json::to_string_pretty(&report),
    };
    let text = match text {
        Ok(text) => text,
        Err(err) => {
            eprintln!("repo-intelligence: could not serialize report: {err}");
            return ExitCode::from(EXIT_FATAL);
        }
    };

    let mut stdout = std::io::stdout().lock();
    if writeln!(stdout, "{text}")
        .and_then(|()| stdout.flush())
        .is_err()
    {
        eprintln!("repo-intelligence: could not write report to stdout");
        return ExitCode::from(EXIT_FATAL);
    }
    ExitCode::SUCCESS
}
