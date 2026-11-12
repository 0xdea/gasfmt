//! Command-line entry point for `gasfmt`.

use std::fs;
use std::io::{self, Read as _, Write as _};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context as _;
use clap::Parser;

/// Binary name.
const PROGRAM: &str = env!("CARGO_BIN_NAME");
/// Package version.
const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Package description.
const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");
/// Package authors.
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

/// Command-line arguments.
#[derive(Debug, Parser)]
#[command(name = PROGRAM, about = None)]
struct Arguments {
    /// Report the files that need formatting, without writing.
    #[arg(short = 'c', long)]
    check: bool,
    /// Files to format in place [default: stdin to stdout].
    files: Vec<PathBuf>,
}

fn main() -> ExitCode {
    eprintln!(
        "{PROGRAM} {VERSION} - {}\nCopyright (c) 2026 {AUTHORS}\n",
        DESCRIPTION.trim_end_matches('.')
    );

    let args = Arguments::parse();
    let result = if args.files.is_empty() {
        format_stdin(args.check)
    } else {
        format_files(&args.files, args.check)
    };

    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(err) => {
            eprintln!("[!] Error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

/// Formats standard input to standard output or, if `check` is set, only
/// reports whether it needs formatting.
///
/// Returns `false` if `check` is set and the input needs formatting.
///
/// # Errors
///
/// Returns an error if standard input can't be read as UTF-8 text, or if
/// standard output can't be written.
fn format_stdin(check: bool) -> anyhow::Result<bool> {
    let mut source = String::new();
    io::stdin()
        .read_to_string(&mut source)
        .context("failed to read standard input")?;
    let formatted = gasfmt::format(&source);

    if check {
        let is_formatted = formatted == source;
        if !is_formatted {
            eprintln!("[!] Standard input needs formatting");
        }
        return Ok(is_formatted);
    }

    io::stdout()
        .write_all(formatted.as_bytes())
        .context("failed to write standard output")?;

    Ok(true)
}

/// Formats `files` in place or, if `check` is set, only reports the ones that
/// need formatting.
///
/// The path of each file that was (or needs to be) formatted is printed to
/// standard output, one per line, in the order given.
///
/// Returns `false` if `check` is set and some files need formatting.
///
/// # Errors
///
/// Returns an error if a file can't be read as UTF-8 text or written. Files
/// before it have already been processed.
fn format_files(files: &[PathBuf], check: bool) -> anyhow::Result<bool> {
    let mut changed = 0_usize;

    for path in files {
        let source = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let formatted = gasfmt::format(&source);
        if formatted == source {
            continue;
        }

        if !check {
            fs::write(path, formatted)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        println!("{}", path.display());
        changed = changed.saturating_add(1);
    }

    let total = files.len();
    if !check {
        eprintln!("[+] Formatted {changed} of {total} files");
    } else if changed > 0 {
        eprintln!("[!] {changed} of {total} files need formatting");
    } else {
        eprintln!("[+] All {total} files are formatted");
    }

    Ok(!check || changed == 0)
}
