// SPDX-License-Identifier: AGPL-3.0-only

//! Command-line entry point for deterministic documentation generation.

#![expect(
    clippy::multiple_crate_versions,
    reason = "nrz-cas uses petgraph 0.8.3 with hashbrown 0.15 while indexmap uses hashbrown 0.17"
)]

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, error::ErrorKind};

/// Parsed command-line options.
#[derive(Debug, Parser)]
#[command(
    name = "nrz-docgen",
    version,
    about = "Generate and check repository documentation"
)]
struct Cli {
    /// Documentation stage to run.
    #[command(subcommand)]
    command: Command,
    /// Repository root, defaulting to the current directory.
    #[arg(long, global = true, default_value = ".")]
    root: PathBuf,
}

/// The three supported stages.
#[derive(Debug, Subcommand)]
enum Command {
    /// Derive committed JSON datasets.
    Data,
    /// Render Markdown from committed datasets.
    Render,
    /// Check the committed generation fixed point.
    Check,
}

/// Parse arguments, execute one stage, and return the documented exit class.
fn main() -> ExitCode {
    let command = match Cli::try_parse() {
        Ok(command) => command,
        Err(problem) => return usage(&problem),
    };
    let result = match command.command {
        Command::Data => nrz_docgen::data(&command.root),
        Command::Render => nrz_docgen::render(&command.root),
        Command::Check => {
            return match nrz_docgen::check(&command.root) {
                Ok(findings) => {
                    let stdout = io::stdout();
                    let mut stream = stdout.lock();
                    for finding in &findings {
                        if writeln!(stream, "{} {}", finding.class.as_str(), finding.path).is_err()
                        {
                            return ExitCode::FAILURE;
                        }
                    }
                    if findings.is_empty() {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::from(3)
                    }
                }
                Err(problem) => runtime_failure(&problem),
            };
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => runtime_failure(&problem),
    }
}

/// Report clap help, version, or usage failure through a locked stream.
fn usage(problem: &clap::Error) -> ExitCode {
    let success = matches!(
        problem.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    );
    if success {
        let stdout = io::stdout();
        if write!(stdout.lock(), "{problem}").is_err() {
            return ExitCode::FAILURE;
        }
        ExitCode::SUCCESS
    } else {
        let stderr = io::stderr();
        if write!(stderr.lock(), "{problem}").is_err() {
            return ExitCode::FAILURE;
        }
        ExitCode::from(2)
    }
}

/// Report a path-bearing pipeline failure.
fn runtime_failure(problem: &nrz_docgen::Error) -> ExitCode {
    let stderr = io::stderr();
    if writeln!(stderr.lock(), "{problem}").is_err() {
        return ExitCode::FAILURE;
    }
    ExitCode::FAILURE
}
