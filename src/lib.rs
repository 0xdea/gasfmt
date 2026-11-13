#![doc = env!("CARGO_PKG_DESCRIPTION")]
#![doc = ""]
#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![doc(html_logo_url = "https://raw.githubusercontent.com/0xdea/gasfmt/master/.img/logo.jpg")]

use crate::line::Parser;

mod line;
mod render;

/// Formats the assembly `source` text.
///
/// The output always ends with a newline, unless it's empty. Formatting is
/// idempotent: formatting the output again returns it unchanged.
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
    render::render(&Parser::default().parse(source))
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
