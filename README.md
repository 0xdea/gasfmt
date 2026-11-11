# gasfmt

[![](https://img.shields.io/github/stars/0xdea/gasfmt.svg?style=flat&color=yellow)](https://github.com/0xdea/gasfmt)
[![](https://img.shields.io/crates/v/gasfmt?style=flat&color=green)](https://crates.io/crates/gasfmt)
[![](https://img.shields.io/crates/d/gasfmt?style=flat&color=red)](https://crates.io/crates/gasfmt)
[![](https://img.shields.io/badge/todo-0.1.1-violet)](https://todo.com/)
[![](https://img.shields.io/badge/twitter-%400xdea-blue.svg)](https://twitter.com/0xdea)
[![](https://img.shields.io/badge/mastodon-%40raptor-purple.svg)](https://infosec.exchange/@raptor)
[![build](https://github.com/0xdea/gasfmt/actions/workflows/build.yml/badge.svg)](https://github.com/0xdea/gasfmt/actions/workflows/build.yml)
[![doc](https://github.com/0xdea/gasfmt/actions/workflows/doc.yml/badge.svg)](https://github.com/0xdea/gasfmt/actions/workflows/doc.yml)

> "It's important to be quotable."
>
> -- Halvar Flake

A formatter for GNU assembler (GAS) sources, targeting the ARM64 dialect accepted by gas and clang: `//` comments
and sources run through the C preprocessor (`.S` files).

## Features

- Labels at column 0, on a line of their own.
- Instructions, macro invocations, and data directives (`.asciz`, `.word`, ...) indented with one tab.
- Macro definitions (`.macro` to `.endm`) and preprocessor directives at column 0.
- Operands aligned within a block, by padding mnemonics to the widest one in the block. The same goes for the `=`
  of assignments.
- Trailing comments aligned within a block, one space past its longest statement.
- Operands separated by a comma followed by a single space, and runs of whitespace collapsed.
- No trailing whitespace, no consecutive blank lines, and a newline at the end of the file.
- Comments and string literals are never modified.
- Formatting is idempotent.

A block is a run of consecutive statements, possibly with comment lines among them. Blank lines, labels, and
preprocessor directives end it.

## Style

Before:

```asm
_asmMain: stp x19,x20,[sp,#-0x10]!   // save registers
    enter 0x40
  // printf("Hello, world\n");
    adrl x0,fmt
    bl _printf
```

After:

```asm
_asmMain:
	stp   x19, x20, [sp, #-0x10]! // save registers
	enter 0x40
	// printf("Hello, world\n");
	adrl  x0, fmt
	bl    _printf
```

## Limitations

- Comment-only lines, assignments, and directives that don't emit data (e.g., `.balign`, `.global`) keep the
  indentation level chosen by the author, either none or one tab, because both are legitimate.
- Operand expressions aren't reformatted beyond whitespace runs and commas (e.g., `a+b` stays as it is).
- Lines holding `/* */` block comments or `;` statement separators, and the continuation lines of a preprocessor
  directive ending with a backslash, are passed through unchanged.
- Preprocessor directives are moved to column 0, so nested `#if` blocks lose their indentation.
- Other comment styles (e.g., `@` or a `#` that doesn't start a line) aren't recognized.

## Installing

The easiest way to get the latest release is via [crates.io](https://crates.io/crates/gasfmt):

```sh
cargo install gasfmt --locked
```

To install as a library, run the following command in your project directory:

```sh
cargo add gasfmt
```

## Compiling

Alternatively, you can build from [source](https://github.com/0xdea/gasfmt):

```sh
git clone --depth 1 https://github.com/0xdea/gasfmt
cd gasfmt
cargo build --release --locked
```

## Usage

Format files in place, printing the path of each file that was changed:

```sh
gasfmt [FILES]...
```

Only list the files that need formatting, exiting with a failure status if there are any:

```sh
gasfmt --check [FILES]...
```

Without files, gasfmt formats standard input to standard output, so that it can be used as an editor filter.

## Examples

Format all assembly sources in the current directory:

```sh
gasfmt *.S
```

Check them, e.g., in a CI job:

```sh
gasfmt --check *.S
```

Format a file to standard output without modifying it:

```sh
gasfmt < listing1-1.S
```

## Zed

To format assembly sources on save in the [Zed](https://zed.dev/) editor, make sure that gasfmt is in your `PATH`
and add it as an external formatter to your `settings.json`:

```json
{
  "languages": {
    "Assembly": {
      "formatter": {
        "external": {
          "command": "gasfmt"
        }
      },
      "format_on_save": "on"
    }
  }
}
```

Zed passes the buffer to gasfmt on standard input and replaces it with what gasfmt writes to standard output.

## Compatibility

Tested on:

- Apple macOS Tahoe 26

## Credits

- The style is the one of [my code](https://github.com/0xdea/AoAA) for
  [The Art of ARM Assembly Language](https://artofarm.randallhyde.com/) by Randall Hyde.

## Changelog

- [CHANGELOG.md](https://github.com/0xdea/gasfmt/blob/master/CHANGELOG.md)

## TODO

- Support other comment styles and architectures.
