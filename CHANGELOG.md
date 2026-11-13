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
- Keep the single-quoted strings of `.ifc` and `.ifnc` as written, instead of collapsing their whitespace.
- Keep a character constant that immediately follows its directive (e.g., `.byte' '`) as written, instead of
  padding it.
- Stop a `"` or a `/*` that follows a single quote that isn't part of a closed character constant from leaving the
  lines after it unformatted.
- Stop crashing when the reader of standard output or standard error closes the pipe early (e.g.,
  `gasfmt --check *.S | head -1`): the run now ends quietly with a failure exit status. When formatting in place,
  all the files are still formatted.
- Pass a line where a `#` follows a label (e.g., `label: #define X 1`) through unchanged, instead of moving the
  `#` to a line of its own, where a second run would move it again as a preprocessor directive. The label now
  stays on that line, which keeps its indentation and its comment as written.

## [0.1.0] - 2026-11-12

First release to be published to [crates.io](https://crates.io/).

[unreleased]: https://github.com/0xdea/gasfmt/compare/v0.1.0...HEAD
[0.1.1]: https://github.com/0xdea/gasfmt/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/0xdea/gasfmt/releases/tag/v0.1.0
