# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**gasfmt** is a formatter for GNU assembler (GAS) sources (CLI + library). It targets the ARM64 dialect accepted by gas and clang only: `//` comments, and sources run through the C preprocessor (`.S` files). Other architectures and comment styles (`@`, `;`, mid-line `#`) are out of scope.

The README's **Features** and **Limitations** sections are the specification of the output style. Keep them in sync with any behavior change.

## Commands

```bash
# Build
cargo build --locked
cargo build --release --locked

# Lint and format (must pass CI)
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked
cargo audit

# Tests
cargo test --locked
cargo test --locked -- test_name          # a single test, by name
cargo test --locked --lib                 # unit tests only
cargo test --locked --test main           # integration tests only

# Run
cargo run --locked -- file.S              # format in place, print the paths that changed
cargo run --locked -- --check file.S      # only report, exit status 1 if formatting is needed
cargo run --locked -- --diff file.S       # only show the changes as a unified diff, same exit status
cargo run --locked < file.S               # filter: standard input to standard output
```

CI (`.github/workflows/build.yml`) runs fmt, build, clippy, audit, doc, test, and `cargo-semver-checks` on Linux, build and test on macOS and Windows, and `zizmor` against the workflows in a separate job.

## Architecture

Single Rust crate (edition 2024) that exposes a binary and a one-function library API, `gasfmt::format(source: &str) -> String`. The library is a two-stage pipeline:

```
source text
  → line::Parser::parse()    classify every source line into a `Line`   (src/line.rs)
  → render::render()         print the lines back, aligning each block  (src/render.rs)
```

gasfmt is line-oriented by design: it never parses instructions, operands, or expressions. It only decides what kind of line each one is and normalizes whitespace.

**Stage 1, classification (`src/line.rs`).** `Parser::parse_line` turns each source line into one or more `Line` values:

- `Blank`, `Comment`, `Label`, `Preprocessor`, `Statement` (`head` + normalized `tail` + optional trailing comment), and `Verbatim`.
- `scan` walks a line once to find the `//` comment and to track the lexical `Context` (code, `/* */` comment, string literal).
- `split_label` peels leading `name:` labels (code sharing a line with its label moves to its own line), `split_statement` splits the rest into head and tail (an assignment's tail is `= expression`), and `normalize` cleans the tail's whitespace and commas while copying string literals and character constants untouched.
- `Parser::indent` decides indentation: instructions and the directives in `DATA_DIRECTIVES` get one tab, everything inside `.macro`/`.endm` sits at column 0, and assignments and other directives keep the author's choice.
- The parser carries three pieces of state across lines: the `Context` the previous line ended in, the `Continuation` kind (a line ending with a backslash), and the `.macro` nesting depth.

**Stage 2, rendering (`src/render.rs`).** Consecutive `Statement` and `Comment` lines form a block; any other line ends it. Per block, heads are padded to the widest head that has operands, and trailing comments start one space past the longest statement.

**The CLI (`src/main.rs`)** uses `clap`. With no files it filters standard input to standard output (the editor-formatter mode); with files it rewrites them in place and prints the changed paths to stdout. Banner, summaries, and errors go to stderr. `--check` writes nothing and makes the exit status a failure when something needs formatting. `--diff` does the same, but prints a unified diff (built by the `similar` crate) instead of the paths. The two conflict. Nothing is printed with `println!` or `eprintln!`, which panic on a closed pipe: a failed stdout write stops the printing but not the run (`is_closed_pipe`: a reader that left early is never reported and doesn't change the exit status with files, while it's a quiet failure when filtering standard input; other errors are returned at the end), and stderr goes through the `report!` macro, which ignores it.

## Invariants

- **When in doubt, pass through.** A line that can't be understood safely is emitted as `Line::Verbatim` and printed exactly as written: lines holding `/* */` comments or `;` separators, lines that start or end inside a string literal, lines ending with a backslash plus their continuation, lines where a `#` follows a label (on its own line it would become a preprocessor directive on the next pass; see `skip_labels`), and lines holding a single quote that isn't part of a closed character constant (`'a'` or `'\n'`; see `constant_len`, shared by `scan` and `normalize` so that they can't disagree). `scan` stops at such a quote, so that a line it can't understand never changes the context of the lines after it. Prefer adding a pass-through case over guessing.
- **Comments and string literals are never modified.** Whitespace inside quotes must survive `scan`, `split_statement`, and `normalize`. The operands of `.ifc` and `.ifnc` are strings even without quotes (clang compares them whitespace included), so `split_statement` only trims them.
- **Formatting is idempotent.** `format(format(x)) == format(x)` must hold for any input, including verbatim lines. Parser state must depend only on things formatting doesn't change (quotes, backslashes, comment markers).
- **Preprocessor directives** (a line starting with `#` in plain code) go to column 0; an open string is reset at the end of a directive, and of a passed-through line where a `#` follows a label, so a stray quote there can't affect later lines. An open `/* */` comment is not reset: it really continues.

## Tests

- `src/line.rs` `mod tests` asserts on classified `Line` values (use the `parse` and `statement` helpers).
- `src/lib.rs` `mod tests` asserts on the final text, including idempotency.
- `tests/main.rs` runs the built binary (`CARGO_BIN_EXE_gasfmt`) against the hand-formatted fixtures in `tests/data/`, which are embedded with `include_str!` and listed in the `FIXTURES` array: each must format to itself, and a copy scrambled by `scramble` (extra whitespace, CRLF, blank lines) must be restored exactly. A new fixture must be added to `FIXTURES`, and must already be formatted.
- The fixtures are compared byte by byte, so `.gitattributes` forces LF line endings on every platform. Don't remove it: the Windows CI job depends on it.

## Lint Policy and Toolchain

`Cargo.toml` `[workspace.lints]` (inherited via `[lints] workspace = true`) enables `missing_docs` and clippy `all`/`pedantic`/`nursery`/`cargo`/`restriction` at `warn`, minus an explicit allowlist. Every item needs a doc comment, private ones and test helpers included, and `unwrap`, `expect`, and `panic!` are not allowed outside tests. Suppress a lint with `#[expect(clippy::lint, reason = "...")]`, not `#[allow]`.

`rust-version = "1.88"` is required by the `let` chain in `split_statement`.

Release builds use LTO, opt-level 3, `codegen-units = 1`, `panic = "abort"`, and stripped binaries.

## Changelog

`CHANGELOG.md` follows Keep a Changelog. The crate is published on crates.io, so every user-visible change, including a change to the formatted output, gets an entry under `[Unreleased]`.
