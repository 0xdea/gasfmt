//! Command-line entry point for `gasfmt`.

use std::fs;
use std::io::{self, Read as _, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context as _;
use clap::Parser;
use similar::TextDiff;

/// Prints a line to standard error, as `eprintln!` does, but without
/// panicking if that fails (e.g., on a closed pipe).
macro_rules! report {
    ($($arg:tt)*) => {{
        #[expect(
            clippy::let_underscore_must_use,
            reason = "a failure to write to standard error can't be reported anywhere"
        )]
        let _ = writeln!(io::stderr(), $($arg)*);
    }};
}

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
    /// Show the changes that formatting would make, without writing.
    #[arg(short = 'd', long, conflicts_with = "check")]
    diff: bool,
    /// Files to format in place [default: stdin to stdout].
    files: Vec<PathBuf>,
}

/// What to do with input that needs formatting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    /// Write the formatted text.
    Write,
    /// Only report that formatting is needed.
    Check,
    /// Only show the changes that formatting would make.
    Diff,
}

fn main() -> ExitCode {
    report!(
        "{PROGRAM} {VERSION} - {}\nCopyright (c) 2026 {AUTHORS}\n",
        DESCRIPTION.trim_end_matches('.')
    );

    let args = Arguments::parse();
    let mode = if args.diff {
        Mode::Diff
    } else if args.check {
        Mode::Check
    } else {
        Mode::Write
    };
    let result = if args.files.is_empty() {
        format_stdin(mode)
    } else {
        format_files(&args.files, mode)
    };

    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(err) => {
            report!("[!] Error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

/// Formats standard input to standard output or, depending on `mode`, only
/// reports whether it needs formatting or shows the changes.
///
/// Returns `false` if nothing was to be written and the input needs
/// formatting.
///
/// # Errors
///
/// Returns an error if standard input can't be read as UTF-8 text, or if
/// standard output can't be written.
fn format_stdin(mode: Mode) -> anyhow::Result<bool> {
    let mut source = String::new();
    io::stdin()
        .read_to_string(&mut source)
        .context("failed to read standard input")?;
    let formatted = gasfmt::format(&source);

    let needs_formatting = formatted != source;
    if mode != Mode::Write && needs_formatting {
        report!("[!] Standard input needs formatting");
    }

    let mut stdout = io::stdout().lock();
    match mode {
        Mode::Write => stdout.write_all(formatted.as_bytes()),
        Mode::Diff if needs_formatting => write_diff(stdout, "<stdin>", &source, &formatted),
        Mode::Check | Mode::Diff => Ok(()),
    }
    .context("failed to write standard output")?;

    Ok(mode == Mode::Write || !needs_formatting)
}

/// Formats `files` in place or, depending on `mode`, only reports the ones
/// that need formatting or shows the changes to them.
///
/// The path of each file that was (or needs to be) formatted is printed to
/// standard output, one per line, in the order given. When showing the
/// changes, the unified diff of each of those files is printed instead.
///
/// Returns `false` if nothing was to be written and some files need
/// formatting.
///
/// # Errors
///
/// Returns an error if a file can't be read as UTF-8 text or written, or if
/// standard output can't be written. Files before it have already been
/// processed.
fn format_files(files: &[PathBuf], mode: Mode) -> anyhow::Result<bool> {
    let mut stdout = io::stdout().lock();
    let mut changed = 0_usize;

    for path in files {
        let source = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let formatted = gasfmt::format(&source);
        if formatted == source {
            continue;
        }

        if mode == Mode::Write {
            fs::write(path, &formatted)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        // A failure to print (e.g., `head` closing the pipe early) is an
        // error, which `println!` would turn into a panic.
        if mode == Mode::Diff {
            let name = path.display().to_string();
            write_diff(&mut stdout, &name, &source, &formatted)
        } else {
            writeln!(stdout, "{}", path.display())
        }
        .context("failed to write standard output")?;
        changed = changed.saturating_add(1);
    }

    let total = files.len();
    if mode == Mode::Write {
        report!("[+] Formatted {changed} of {total} files");
    } else if changed > 0 {
        report!("[!] {changed} of {total} files need formatting");
    } else {
        report!("[+] All {total} files are formatted");
    }

    Ok(mode == Mode::Write || changed == 0)
}

/// Writes to `output` the unified diff that turns `source` into `formatted`,
/// both called `name` in its header.
///
/// # Errors
///
/// Returns an error if `output` can't be written.
fn write_diff(output: impl Write, name: &str, source: &str, formatted: &str) -> io::Result<()> {
    TextDiff::from_lines(source, formatted)
        .unified_diff()
        .header(name, name)
        .to_writer(output)
}
