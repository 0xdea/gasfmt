# Changelog for gasfmt

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Add the `--diff` option, which shows the changes that formatting would make as a unified diff, without writing
  them.

### Changed

- Pass lines holding a single quote that isn't part of a closed character constant (`'a'` or `'\n'`) through
  unchanged, instead of formatting them. This includes constants written without their closing quote.

### Removed

- Remove the screenshot from `README.md`.

### Fixed

- Recognize what immediately follows the closing quote of a character constant: a `//` comment, a `/* */`
  comment, a `;` separator, or a string literal.
- Normalize the commas and whitespace that follow the closing quote of a character constant.
- Keep the operands of `.ifc` and `.ifnc` as written, quoted or not, instead of normalizing their commas and
  whitespace: clang compares them whitespace included, so `.ifc a  b,a b` took the other branch once formatted.
- Stop the padding that aligns the operands from ending up inside a character constant that immediately follows
  its directive (e.g., `.byte' '` became `.byte'  '`).
- Stop a `"` or a `/*` that follows a single quote that isn't part of a closed character constant from leaving the
  lines after it unformatted.
- Recognize a `.macro` or an `.endm` on a line that is passed through unchanged (e.g., `.endm; nop` or
  `.macro m /* c */`), instead of indenting the lines that follow as if it weren't there.
- Keep a UTF-8 byte order mark at the start of the source in front of the first line, instead of formatting it as
  a part of that line: a `#define` there was indented, which broke the build.
- Treat a carriage return that isn't followed by a newline as a line ending, as the C preprocessor does, instead
  of as whitespace: the lines of a source with CR-only line endings were merged, and a carriage return at the end
  of a line that is passed through unchanged was removed only on a second run.
- Recognize labels and assignments whose names hold characters outside ASCII or the macro escapes `\@` and
  `\()` (e.g., `.L\@: ret`), instead of leaving them on the line of their code as if they were instructions.
- Stop crashing when the reader of standard output or standard error closes the pipe early (e.g.,
  `gasfmt --check *.S | head -1`). The run goes on quietly: all the files are still formatted or checked, and only
  output that was cut short when formatting standard input makes the exit status a failure.
- Pass a line where a `#` follows a label (e.g., `label: #define X 1`) through unchanged, instead of moving the
  `#` to a line of its own, where a second run would move it again as a preprocessor directive. The label now
  stays on that line, which keeps its indentation and its comment as written.

## [0.1.0] - 2026-11-12

First release to be published to [crates.io](https://crates.io/).

[unreleased]: https://github.com/0xdea/gasfmt/compare/v0.1.0...HEAD
[0.1.1]: https://github.com/0xdea/gasfmt/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/0xdea/gasfmt/releases/tag/v0.1.0
