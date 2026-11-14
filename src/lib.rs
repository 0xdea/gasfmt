#![doc = env!("CARGO_PKG_DESCRIPTION")]
#![doc = ""]
#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![doc(html_logo_url = "https://raw.githubusercontent.com/0xdea/gasfmt/master/.img/logo.jpg")]

use crate::line::Parser;

mod line;
mod render;

/// The byte order mark that some editors write at the start of a file.
const BYTE_ORDER_MARK: char = '\u{feff}';

/// Formats the assembly `source` text.
///
/// The output always ends with a newline, unless it's empty or it holds
/// only a byte order mark. Formatting is idempotent: formatting the output
/// again returns it unchanged. A byte order mark at the start of `source`,
/// or after only blank lines, is kept in front of the output.
///
/// # Examples
///
/// Basic usage:
/// ```
/// let source = "_main: mov x0,#0   // status\n  bl _exit\n";
/// let formatted = "_main:\n\tmov x0, #0 // status\n\tbl  _exit\n";
///
/// assert_eq!(gasfmt::format(source), formatted);
/// ```
#[must_use]
pub fn format(source: &str) -> String {
    // The mark isn't part of the first line, which would not be recognized
    // with it in front (e.g., as a preprocessor directive).
    let text = strip_byte_order_mark(source);
    let formatted = render::render(&Parser::default().parse(text.unwrap_or(source)));

    if text.is_some() {
        format!("{BYTE_ORDER_MARK}{formatted}")
    } else {
        formatted
    }
}

/// Splits the leading byte order mark off `source`, returning what follows
/// it, or `None` if there isn't one.
///
/// Blank lines before the mark are dropped by formatting, which would make
/// it the first character for the next pass, so they're skipped here.
/// Indentation in front of it makes it a part of its line instead.
fn strip_byte_order_mark(source: &str) -> Option<&str> {
    let (blank, text) = source.split_once(BYTE_ORDER_MARK)?;
    let is_leading = blank.chars().all(char::is_whitespace)
        && (blank.is_empty() || blank.ends_with(['\n', '\r']));

    is_leading.then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operands_are_aligned_within_a_block() {
        assert_eq!(
            format("\tadrl x0, fmt\n\tbl _printf\n\tret\n\n\tbl _exit\n"),
            "\tadrl x0, fmt\n\tbl   _printf\n\tret\n\n\tbl _exit\n",
            "heads should be padded to the widest one that has operands"
        );
    }

    #[test]
    fn operand_less_statements_do_not_widen_the_block() {
        assert_eq!(
            format("\tstr x0, [fp]\n\tget_errno\n\tmov w2, w0\n"),
            "\tstr x0, [fp]\n\tget_errno\n\tmov w2, w0\n",
            "a statement without operands should not pad the others"
        );
    }

    #[test]
    fn assignments_are_aligned_at_the_equals_sign() {
        assert_eq!(
            format("maxlen = 0x100\ntrue=1\nfalse =  0\n"),
            "maxlen = 0x100\ntrue   = 1\nfalse  = 0\n",
            "symbols should be padded to the widest one"
        );
    }

    #[test]
    fn trailing_comments_are_aligned_past_the_longest_statement() {
        assert_eq!(
            format("\tsub sp, sp, #0x40 // room\n\tadrl x1, saveLR\n\tstr lr, [x1] // save\n"),
            "\tsub  sp, sp, #0x40 // room\n\tadrl x1, saveLR\n\tstr  lr, [x1]      // save\n",
            "comments should start one space past the longest statement"
        );
    }

    #[test]
    fn labels_and_blank_lines_end_a_block() {
        assert_eq!(
            format("\tb done\nloop:\n\tadrl x0, fmt\n\n\tb loop\n"),
            "\tb done\nloop:\n\tadrl x0, fmt\n\n\tb loop\n",
            "alignment should not carry over a label or a blank line"
        );
    }

    #[test]
    fn comment_lines_do_not_end_a_block() {
        assert_eq!(
            format("\tadrl x0, fmt\n\t// call\n// it\n\tbl _printf\n"),
            "\tadrl x0, fmt\n\t// call\n// it\n\tbl   _printf\n",
            "statements around a comment line should align together"
        );
    }

    #[test]
    fn whitespace_is_cleaned_up() {
        assert_eq!(
            format("\r\n  ret  \r\n\r\n\r\n    ret"),
            "\tret\n\n\tret\n",
            "indentation, line endings and blank lines should be normalized"
        );
        assert_eq!(format(" \n\n"), "", "blank input should format to nothing");
    }

    #[test]
    fn formatting_is_idempotent() {
        let source =
            "a: mov x0,x1 // c\n  .macro m a,b\n add \\a ,\\b\n .endm\nx=1\n /* k */ ret\n";
        let formatted = format(source);

        assert_ne!(formatted, source, "the source should need formatting");
        assert_eq!(
            format(&formatted),
            formatted,
            "a second pass should change nothing"
        );
    }

    #[test]
    fn character_constants_are_kept_untouched() {
        let source = "  cmp w0,#'a'// one ,two\n  .byte ',' ,' '  ,'\\n'\n  .ifc 'x  y' ,''// k\n";
        let formatted =
            "\tcmp   w0, #'a'       // one ,two\n\t.byte ',', ' ', '\\n'\n  .ifc 'x  y' ,''// k\n";

        assert_eq!(
            format(source),
            formatted,
            "closed constants should be formatted, other quotes passed through"
        );
        assert_eq!(
            format("\t.balign 4\n\t.byte' '\n"),
            "\t.balign 4\n\t.byte   ' '\n",
            "padding should not end up inside a constant"
        );
        assert_eq!(
            format(formatted),
            formatted,
            "a second pass should change nothing"
        );
    }

    #[test]
    fn hash_after_a_label_is_kept_on_its_line() {
        let source = "lab: #define X  1\n  mov x0,x1\nlab: #error \"open\n  mov x0,x1\n";
        let formatted = "lab: #define X  1\n\tmov x0, x1\nlab: #error \"open\n\tmov x0, x1\n";

        assert_eq!(
            format(source),
            formatted,
            "only the statements after the labels should change"
        );
        assert_eq!(
            format(formatted),
            formatted,
            "a second pass should change nothing"
        );
    }

    #[test]
    fn string_comparisons_keep_their_operands() {
        assert_eq!(
            format("  .ifc   a  b,a b  // c\n  nop\n  .endif\n"),
            "\t.ifc a  b,a b // c\n\tnop\n\t.endif\n",
            "only the whitespace around the operands should change"
        );
    }

    #[test]
    fn byte_order_mark_is_kept_in_front_of_the_first_line() {
        assert_eq!(
            format("\u{feff}#define V 7\n  mov x0,#V\n"),
            "\u{feff}#define V 7\n\tmov x0, #V\n",
            "the mark should not be taken for a part of the first line"
        );
        assert_eq!(
            format("\u{feff}  ret\n"),
            "\u{feff}\tret\n",
            "the first line should be formatted as any other"
        );
        assert_eq!(format("\u{feff} \n"), "\u{feff}", "the mark should be kept");
        assert_eq!(
            format("\r\n \n\u{feff}#define V 7\n"),
            "\u{feff}#define V 7\n",
            "a mark that ends up first should not change on a second pass"
        );

        let formatted = "\t\u{feff}x: =1\n";
        assert_eq!(
            format("\n  \u{feff}x:  =1\n"),
            formatted,
            "indentation in front of the mark should make it a part of the line"
        );
        assert_eq!(
            format(formatted),
            formatted,
            "a second pass should change nothing"
        );
    }

    #[test]
    fn lone_carriage_returns_end_lines() {
        assert_eq!(
            format("_f:\r  mov x0,x1\r  ret\r"),
            "_f:\n\tmov x0, x1\n\tret\n",
            "lines that end with a carriage return should not be merged"
        );

        let formatted = "a: /* c */\n\n/* d */\n";
        assert_eq!(
            format("a: /* c */\r\r\n/* d */\r"),
            formatted,
            "a line passed through should not keep a part of its line ending"
        );
        assert_eq!(
            format(formatted),
            formatted,
            "a second pass should change nothing"
        );
    }

    #[test]
    fn multiline_constructs_are_passed_through() {
        let source = "  .ascii \"a\n  b ,c\"\n  b la\\\nbel\n  mov x0,x1\n";
        let formatted = "  .ascii \"a\n  b ,c\"\n  b la\\\nbel\n\tmov x0, x1\n";

        assert_eq!(
            format(source),
            formatted,
            "only the statement after the literal and the continuation should change"
        );
        assert_eq!(
            format(formatted),
            formatted,
            "a second pass should change nothing"
        );
    }
}
