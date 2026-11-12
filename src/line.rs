//! Classification of assembly source lines.

/// Directives that emit data, which are indented like instructions.
const DATA_DIRECTIVES: [&str; 31] = [
    ".2byte",
    ".4byte",
    ".8byte",
    ".ascii",
    ".asciz",
    ".byte",
    ".double",
    ".dword",
    ".fill",
    ".float",
    ".hword",
    ".inst",
    ".int",
    ".long",
    ".octa",
    ".quad",
    ".short",
    ".single",
    ".skip",
    ".sleb128",
    ".space",
    ".string",
    ".string16",
    ".string32",
    ".string64",
    ".string8",
    ".uleb128",
    ".word",
    ".xword",
    ".zero",
    ".incbin",
];

/// A classified source line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Line {
    /// An empty line.
    Blank,
    /// A line holding only a comment.
    Comment {
        /// Whether the comment was indented in the source.
        indented: bool,
        /// The comment text, starting with its `//` sigil.
        text: String,
    },
    /// A label definition, without its trailing colon.
    Label {
        /// The label name.
        name: String,
        /// The trailing comment, if any.
        comment: Option<String>,
    },
    /// A C preprocessor directive, emitted as written at column 0.
    Preprocessor(String),
    /// An instruction, a directive, a macro invocation, or an assignment.
    Statement(Statement),
    /// A line that is passed through unchanged.
    Verbatim(String),
}

/// An instruction, a directive, a macro invocation, or an assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Statement {
    /// Whether the statement is indented in the output.
    pub indented: bool,
    /// The mnemonic, directive, or macro name, or the symbol being assigned.
    pub head: String,
    /// The normalized operands, or `= expression` for an assignment.
    pub tail: String,
    /// The trailing comment, if any.
    pub comment: Option<String>,
}

/// Lexical context of the scanner within a line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Context {
    /// Inside a `/* */` block comment.
    Block,
    /// Plain code.
    Code,
    /// Inside a string literal.
    Text,
}

/// What the scanner found in a line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Scan {
    /// Byte offset of the `//` comment, if any.
    comment: Option<usize>,
    /// Whether the line holds constructs that are passed through unchanged.
    verbatim: bool,
    /// Whether a block comment is still open at the end of the line.
    in_block: bool,
}

/// Line classifier, carrying the state that spans several lines.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Parser {
    /// Whether the previous line left a block comment open.
    in_block: bool,
    /// Whether the previous preprocessor line ended with a backslash.
    in_continuation: bool,
    /// Nesting depth of `.macro` definitions.
    macro_depth: usize,
}

impl Parser {
    /// Classifies every line of `source`.
    ///
    /// Runs of blank lines are collapsed into one, and blank lines at either
    /// end are dropped.
    pub fn parse(mut self, source: &str) -> Vec<Line> {
        let mut lines = Vec::new();
        for raw in source.lines() {
            self.parse_line(raw, &mut lines);
        }
        if lines.last() == Some(&Line::Blank) {
            lines.pop();
        }

        lines
    }

    /// Classifies the `raw` line and appends the result to `lines`.
    fn parse_line(&mut self, raw: &str, lines: &mut Vec<Line>) {
        let trimmed = raw.trim();
        if self.in_continuation {
            self.in_continuation = trimmed.ends_with('\\');
            lines.push(Line::Verbatim(raw.to_owned()));
            return;
        }

        let scan = scan(raw, self.in_block);
        let was_in_block = self.in_block;
        self.in_block = scan.in_block;
        if !was_in_block && trimmed.starts_with('#') {
            self.in_continuation = trimmed.ends_with('\\');
            lines.push(Line::Preprocessor(trimmed.to_owned()));
            return;
        }
        if scan.verbatim {
            lines.push(Line::Verbatim(raw.to_owned()));
            return;
        }

        // The offset comes from `char_indices`, so the split can't fail.
        let (code, comment) = scan
            .comment
            .and_then(|offset| raw.split_at_checked(offset))
            .map_or((raw, None), |(code, comment)| {
                (code, Some(comment.trim_end().to_owned()))
            });
        let mut indented = raw.starts_with(char::is_whitespace);
        let mut code = code.trim();

        while let Some((name, rest)) = split_label(code) {
            if rest.is_empty() {
                let name = name.to_owned();
                lines.push(Line::Label { name, comment });
                return;
            }
            // Code sharing a line with its label moves to a line of its own.
            let name = name.to_owned();
            lines.push(Line::Label {
                name,
                comment: None,
            });
            indented = true;
            code = rest;
        }

        if !code.is_empty() {
            let (head, tail) = split_statement(code);
            let indented = self.indent(&head, &tail, indented);
            lines.push(Line::Statement(Statement {
                indented,
                head,
                tail,
                comment,
            }));
        } else if let Some(text) = comment {
            lines.push(Line::Comment { indented, text });
        } else if lines.last().is_some_and(|last| *last != Line::Blank) {
            lines.push(Line::Blank);
        } else {
            // A blank line at the start or after another one is dropped.
        }
    }

    /// Decides whether the statement made of `head` and `tail` is indented,
    /// given whether it was `indented` in the source.
    fn indent(&mut self, head: &str, tail: &str, indented: bool) -> bool {
        if head.eq_ignore_ascii_case(".macro") {
            self.macro_depth = self.macro_depth.saturating_add(1);
            return false;
        }
        if head.eq_ignore_ascii_case(".endm") || head.eq_ignore_ascii_case(".endmacro") {
            self.macro_depth = self.macro_depth.saturating_sub(1);
            return false;
        }
        if self.macro_depth > 0 {
            return false;
        }

        // Assignments and non-data directives are found both at column 0 and
        // inside function bodies, so they stay where the author put them.
        let is_assignment = tail.starts_with('=');
        let is_directive = head.starts_with('.');
        let is_data = DATA_DIRECTIVES
            .iter()
            .any(|directive| head.eq_ignore_ascii_case(directive));
        if is_assignment || (is_directive && !is_data) {
            indented
        } else {
            true
        }
    }
}

/// Scans `line` for a `//` comment and for constructs that are passed
/// through unchanged (`/* */` comments and `;` separators), starting inside
/// a block comment if `in_block` is set.
fn scan(line: &str, in_block: bool) -> Scan {
    let mut context = if in_block {
        Context::Block
    } else {
        Context::Code
    };
    let mut verbatim = in_block;
    let mut chars = line.char_indices().peekable();

    while let Some((offset, ch)) = chars.next() {
        let next = chars.peek().map(|&(_, next)| next);
        match (context, ch, next) {
            (Context::Code, '/', Some('/')) => {
                return Scan {
                    comment: Some(offset),
                    verbatim,
                    in_block: false,
                };
            }
            (Context::Code, '/', Some('*')) => {
                chars.next();
                verbatim = true;
                context = Context::Block;
            }
            (Context::Code, ';', _) => verbatim = true,
            (Context::Code, '"', _) => context = Context::Text,
            (Context::Text, '"', _) => context = Context::Code,
            (Context::Block, '*', Some('/')) => {
                chars.next();
                context = Context::Code;
            }
            // Skip the character of a character constant or escape sequence.
            (Context::Code, '\'', _) | (Context::Text, '\\', _) => {
                chars.next();
            }
            (Context::Block | Context::Code | Context::Text, ..) => {}
        }
    }

    Scan {
        comment: None,
        verbatim,
        in_block: context == Context::Block,
    }
}

/// Returns whether `ch` can be part of a symbol name (or of a macro argument
/// reference, which starts with a backslash).
const fn is_symbol_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '$' | '\\')
}

/// Splits `code` after its leading symbol, returning the symbol and the rest.
fn split_symbol(code: &str) -> (&str, &str) {
    let offset = code.find(|ch| !is_symbol_char(ch)).unwrap_or(code.len());

    // The offset comes from `find`, so the split can't fail.
    code.split_at_checked(offset).unwrap_or((code, ""))
}

/// Splits a leading label off `code`, returning its name and the rest.
fn split_label(code: &str) -> Option<(&str, &str)> {
    let (name, rest) = split_symbol(code);
    let rest = rest.strip_prefix(':')?;

    (!name.is_empty()).then(|| (name, rest.trim_start()))
}

/// Splits the statement in `code` into its head and its normalized tail.
fn split_statement(code: &str) -> (String, String) {
    let (symbol, rest) = split_symbol(code);
    if !symbol.is_empty()
        && let Some(expression) = rest.trim_start().strip_prefix('=')
        && !expression.starts_with('=')
    {
        let tail = format!("= {}", normalize(expression));
        return (symbol.to_owned(), tail.trim_end().to_owned());
    }

    // A string literal can follow the head without whitespace, so the head
    // also ends at a quote. A statement that starts with one (e.g., a quoted
    // symbol name) can't be split safely, and is kept whole.
    let head_len = match code.find(|ch: char| ch.is_whitespace() || ch == '"') {
        Some(0) | None => code.len(),
        Some(offset) => offset,
    };
    // The length comes from `find`, so the split can't fail.
    let (head, operands) = code.split_at_checked(head_len).unwrap_or((code, ""));

    (head.to_owned(), normalize(operands))
}

/// Normalizes the whitespace of `operands`: runs are collapsed into a single
/// space, and commas are followed, but not preceded, by one. String literals
/// are left untouched.
fn normalize(operands: &str) -> String {
    let mut normalized = String::with_capacity(operands.len());
    let mut context = Context::Code;
    let mut needs_space = false;
    let mut chars = operands.trim().chars();

    while let Some(ch) = chars.next() {
        if context == Context::Code && ch.is_whitespace() {
            needs_space = true;
            continue;
        }
        if context == Context::Code && ch == ',' {
            normalized.push(',');
            needs_space = true;
            continue;
        }
        if needs_space {
            normalized.push(' ');
            needs_space = false;
        }
        normalized.push(ch);

        match (context, ch) {
            (Context::Code, '"') => context = Context::Text,
            (Context::Text, '"') => context = Context::Code,
            // Copy the character of a character constant or escape sequence.
            (Context::Code, '\'') | (Context::Text, '\\') => normalized.extend(chars.next()),
            (Context::Block | Context::Code | Context::Text, _) => {}
        }
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Classifies `source` with a fresh parser.
    fn parse(source: &str) -> Vec<Line> {
        Parser::default().parse(source)
    }

    /// Builds the statement line expected from a test.
    fn statement(indented: bool, head: &str, tail: &str, comment: Option<&str>) -> Line {
        Line::Statement(Statement {
            indented,
            head: head.to_owned(),
            tail: tail.to_owned(),
            comment: comment.map(str::to_owned),
        })
    }

    #[test]
    fn normalize_fixes_comma_and_whitespace_runs() {
        assert_eq!(
            normalize("x0 ,x1,\t[sp,  #0x10]!"),
            "x0, x1, [sp, #0x10]!",
            "commas should be followed, not preceded, by one space"
        );
    }

    #[test]
    fn normalize_keeps_string_literals_untouched() {
        assert_eq!(
            normalize(r#""a ,b  \" ,c"  ,  ','"#),
            r#""a ,b  \" ,c", ','"#,
            "whitespace and commas in literals should be kept as written"
        );
    }

    #[test]
    fn comment_sigil_inside_string_is_not_a_comment() {
        assert_eq!(
            parse("\t.asciz \"http://x\" // url"),
            [statement(true, ".asciz", "\"http://x\"", Some("// url"))],
            "only the sigil outside the literal should start the comment"
        );
    }

    #[test]
    fn string_glued_to_its_directive_is_kept_untouched() {
        assert_eq!(
            parse("\t.ascii\"a  ,b\" // text"),
            [statement(true, ".ascii", "\"a  ,b\"", Some("// text"))],
            "the directive should end where the literal starts"
        );
    }

    #[test]
    fn statement_starting_with_a_quote_is_kept_whole() {
        assert_eq!(
            parse("\"my  label\": ret  ,x"),
            [statement(true, "\"my  label\": ret  ,x", "", None)],
            "a statement that can't be split safely should not be normalized"
        );
    }

    #[test]
    fn assignment_is_split_at_the_equals_sign() {
        assert_eq!(
            parse("maxlen=0x100\n.if a == b"),
            [
                statement(false, "maxlen", "= 0x100", None),
                statement(false, ".if", "a == b", None),
            ],
            "an assignment should be told apart from a comparison"
        );
    }

    #[test]
    fn label_sharing_a_line_with_code_is_split() {
        assert_eq!(
            parse("0: ret // done"),
            [
                Line::Label {
                    name: "0".to_owned(),
                    comment: None
                },
                statement(true, "ret", "", Some("// done")),
            ],
            "the label and its code should end up on separate lines"
        );
    }

    #[test]
    fn instructions_are_indented_except_in_macro_bodies() {
        assert_eq!(
            parse("ret\n  .macro leave\n  mov sp, fp\n  .endm\nret"),
            [
                statement(true, "ret", "", None),
                statement(false, ".macro", "leave", None),
                statement(false, "mov", "sp, fp", None),
                statement(false, ".endm", "", None),
                statement(true, "ret", "", None),
            ],
            "macro definitions should sit at column 0"
        );
    }

    #[test]
    fn directives_keep_their_indentation_unless_they_emit_data() {
        assert_eq!(
            parse(".balign 4\n  .balign 4\n.word 1"),
            [
                statement(false, ".balign", "4", None),
                statement(true, ".balign", "4", None),
                statement(true, ".word", "1", None),
            ],
            "only data directives should be forced to indent"
        );
    }

    #[test]
    fn unsupported_constructs_are_passed_through() {
        let source = "  mov x0,x1 ; nop\n /* a\n\n  b */  ret\n#define A \\\n   1 ,2";
        assert_eq!(
            parse(source),
            [
                Line::Verbatim("  mov x0,x1 ; nop".to_owned()),
                Line::Verbatim(" /* a".to_owned()),
                Line::Verbatim(String::new()),
                Line::Verbatim("  b */  ret".to_owned()),
                Line::Preprocessor("#define A \\".to_owned()),
                Line::Verbatim("   1 ,2".to_owned()),
            ],
            "separators, block comments and continuations should be kept"
        );
    }

    #[test]
    fn blank_lines_are_collapsed_and_trimmed() {
        assert_eq!(
            parse("\n\nret\n \n\t\nret\n\n"),
            [
                statement(true, "ret", "", None),
                Line::Blank,
                statement(true, "ret", "", None),
            ],
            "a single blank line should survive between the statements"
        );
    }
}
