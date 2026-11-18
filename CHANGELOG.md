# Changelog for gasfmt

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Enable Cargo lints and fail CI on their warnings.
- Allow `unwrap`, `expect`, `panic!`, and indexing in tests via `clippy.toml`.
- Add a docs.rs documentation link to `Cargo.toml` and a doc badge to the README.

## [0.1.1] - 2026-11-15

### Added

- Add the `--diff` option, which shows the changes that formatting would make as a unified diff, without writing
  them.

### Changed

- Pass lines holding a single quote that isn't part of a closed character constant (`'a'` or `'\n'`) through
  unchanged, instead of formatting them. This includes constants written without their closing quote.
- Replace the logo with a lighter version.

### Removed

- Remove the screenshot from `README.md`.

### Fixed

- Handle character constants correctly: recognize a `//` comment, a `/* */` comment, a `;` separator, or a
  string literal right after the closing quote, normalize the operands that follow it, and never pad inside a
  constant that immediately follows its directive (e.g., `.byte' '`).
- Keep the operands of `.ifc` and `.ifnc` as written, quoted or not, instead of normalizing their commas and
  whitespace: clang compares them whitespace included, so `.ifc a  b,a b` took the other branch once formatted.
- Pass a line that starts with a preprocessor macro invocation (e.g., `ENTRY(main)` or `CALL(f, x0)`) through
  unchanged, instead of indenting it as an instruction and aligning what follows its first space.
- Place the comment lines of a `.macro` body at column 0, like the code around them, instead of keeping their
  indentation.
- Recognize labels and assignments whose names hold characters outside ASCII or the macro escapes `\@` and
  `\()` (e.g., `.L\@: ret`), instead of leaving them on the line of their code as if they were instructions.
- Keep a UTF-8 byte order mark at the start of the source in front of the first line, instead of formatting it as
  a part of that line.
- Treat a carriage return that isn't followed by a newline as a line ending, instead of merging the lines of a
  source with CR-only line endings.
- Report an error, instead of crashing, when the reader of standard output or standard error closes the pipe
  early (e.g., `gasfmt --check *.S | head -1`).

## [0.1.0] - 2026-11-12

First release to be published to [crates.io](https://crates.io/).

[unreleased]: https://github.com/0xdea/gasfmt/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/0xdea/gasfmt/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/0xdea/gasfmt/releases/tag/v0.1.0
