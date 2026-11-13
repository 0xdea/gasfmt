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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Context {
    /// Inside a `/* */` block comment.
    Block,
    /// Plain code.
    #[default]
    Code,
    /// Inside a string literal.
    Text,
}

/// What a line ending with a backslash continues on the next line.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Continuation {
    /// The line doesn't end with a backslash.
    #[default]
    None,
    /// A C preprocessor directive.
    Preprocessor,
    /// A statement or a comment.
    Statement,
}

/// What the scanner found in a line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Scan {
    /// Byte offset of the `//` comment, if any.
    comment: Option<usize>,
    /// Whether the line holds constructs that are passed through unchanged.
    verbatim: bool,
    /// The lexical context at the end of the line.
    context: Context,
}

/// Line classifier, carrying the state that spans several lines.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Parser {
    /// The lexical context in which the previous line ended.
    context: Context,
    /// What the previous line continues on the current one.
    continuation: Continuation,
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
        let continues = trimmed.ends_with('\\');

        let previous = self.continuation;
        let started = self.context;
        let is_preprocessor = match previous {
            Continuation::None => started == Context::Code && trimmed.starts_with('#'),
            Continuation::Preprocessor => true,
            Continuation::Statement => false,
        };

        // The context is tracked on every line, even one passed through, so
        // that a string or a comment opened here is known to the next line.
        let scan = scan(raw, started);
        self.context = scan.context;

        if is_preprocessor {
            if continues {
                self.continuation = Continuation::Preprocessor;
            } else {
                self.continuation = Continuation::None;
                // When the directive ends, the context is reset to code,
                // so a string literal that was left open in it doesn't
                // affect the lines that follow.
                if self.context == Context::Text {
                    self.context = Context::Code;
                }
            }
            lines.push(if previous == Continuation::None {
                // Only the first line of a directive is safe to move.
                Line::Preprocessor(trimmed.to_owned())
            } else {
                Line::Verbatim(raw.to_owned())
            });
            return;
        }

        // The continuation is recorded before any line is passed through,
        // since such a line (e.g., `label: #x \`) can end with a backslash too.
        self.continuation = if continues {
            Continuation::Statement
        } else {
            Continuation::None
        };

        // On a line of its own, a hash that follows a label would be taken
        // for the start of a preprocessor directive by the next pass.
        if previous == Continuation::None
            && started == Context::Code
            && skip_labels(trimmed).starts_with('#')
        {
            // As for a directive, a stray quote in it doesn't affect the
            // lines that follow.
            if !continues && self.context == Context::Text {
                self.context = Context::Code;
            }
            lines.push(Line::Verbatim(raw.to_owned()));
            return;
        }

        // Joining a continued line with the next one can't be done safely,
        // since the backslash may even split a symbol.
        if continues || previous == Continuation::Statement || scan.verbatim {
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

        // A label at the start of a line is always kept on its own line.
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

        // A statement at the start of a line is always indented, except for
        // assignments and non-data directives, which are kept where the author put them.
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
/// through unchanged (`/* */` comments, `;` separators, string literals
/// spanning several lines, and quotes that aren't part of a closed character
/// constant), starting in the `context` left open by the previous line.
///
/// The scan stops at a quote that isn't part of a closed character constant,
/// so that nothing after it can affect the lines that follow.
fn scan(line: &str, context: Context) -> Scan {
    let mut context = context;
    let mut verbatim = context != Context::Code;
    let mut chars = line.char_indices();

    while let Some((offset, ch)) = chars.next() {
        let next = chars.as_str().chars().next();
        match (context, ch, next) {
            (Context::Code, '/', Some('/')) => {
                return Scan {
                    comment: Some(offset),
                    verbatim,
                    context,
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
            (Context::Code, '\'', _) => {
                let Some(len) = constant_len(chars.as_str()) else {
                    // Any other use of the quote (e.g., the strings of `.ifc`,
                    // or a constant without its closing quote) can't be told
                    // apart, and neither can what follows it on the line.
                    return Scan {
                        comment: None,
                        verbatim: true,
                        context,
                    };
                };
                for _ in 0..len {
                    chars.next();
                }
            }
            // Skip the character of an escape sequence.
            (Context::Text, '\\', _) => {
                chars.next();
            }
            (Context::Block | Context::Code | Context::Text, ..) => {}
        }
    }

    Scan {
        comment: None,
        verbatim: verbatim || context != Context::Code,
        context,
    }
}

/// Returns the number of characters at the start of `code`, which follows an
/// opening quote, that complete a character constant: a character, which may
/// be escaped with a backslash, and the closing quote (as in `'a'` or
/// `'\n'`).
///
/// Returns `None` if the quote doesn't open such a constant.
fn constant_len(code: &str) -> Option<usize> {
    let mut chars = code.chars();
    let len = if chars.next()? == '\\' {
        chars.next()?;
        3
    } else {
        2
    };

    (chars.next()? == '\'').then_some(len)
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

/// Returns what follows the leading labels of `code`.
fn skip_labels(mut code: &str) -> &str {
    while let Some((_, rest)) = split_label(code) {
        code = rest;
    }

    code
}

/// Splits the statement in `code` into its head and its normalized tail.
///
/// The operands of `.ifc` and `.ifnc` are only trimmed.
fn split_statement(code: &str) -> (String, String) {
    let (symbol, rest) = split_symbol(code);
    if !symbol.is_empty()
        && let Some(expression) = rest.trim_start().strip_prefix('=')
        && !expression.starts_with('=')
    {
        let tail = format!("= {}", normalize(expression));
        return (symbol.to_owned(), tail.trim_end().to_owned());
    }

    // A string literal or a character constant can follow the head without
    // whitespace, so the head also ends at a quote. A statement that starts
    // with one (e.g., a quoted symbol name) can't be split safely, and is
    // kept whole.
    let head_len = match code.find(|ch: char| ch.is_whitespace() || matches!(ch, '"' | '\'')) {
        Some(0) | None => code.len(),
        Some(offset) => offset,
    };
    // The length comes from `find`, so the split can't fail.
    let (head, operands) = code.split_at_checked(head_len).unwrap_or((code, ""));

    // Some assemblers compare the strings of these directives as written,
    // whitespace included, so only what surrounds them is cleaned up.
    let is_comparison = [".ifc", ".ifnc"]
        .iter()
        .any(|directive| head.eq_ignore_ascii_case(directive));
    let tail = if is_comparison {
        operands.trim().to_owned()
    } else {
        normalize(operands)
    };

    (head.to_owned(), tail)
}

/// Normalizes the whitespace of `operands`: runs are collapsed into a single
/// space, and commas are followed, but not preceded, by one. String literals
/// and character constants are left untouched.
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
            // Copy the rest of a character constant. Lines with any other
            // use of the quote don't get here, since `scan` flags them.
            (Context::Code, '\'') => {
                if let Some(len) = constant_len(chars.as_str()) {
                    normalized.extend(chars.by_ref().take(len));
                }
            }
            // Copy the character of an escape sequence.
            (Context::Text, '\\') => normalized.extend(chars.next()),
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
    fn normalize_handles_the_closing_quote_of_character_constants() {
        assert_eq!(
            normalize("',',1 , 'a' , 2,'b',3 ,'\\'' ,4"),
            "',', 1, 'a', 2, 'b', 3, '\\'', 4",
            "separators after a closing quote should be normalized"
        );
        assert_eq!(
            normalize(r#"'"' ,"a  ,b""#),
            r#"'"', "a  ,b""#,
            "a literal after a quote constant should be kept as written"
        );
    }

    #[test]
    fn code_after_closing_quote_is_recognized() {
        assert_eq!(
            parse("\tmov w0,#'a'// first ,one\n\tmov w0,#'\\''// second"),
            [
                statement(true, "mov", "w0, #'a'", Some("// first ,one")),
                statement(true, "mov", "w0, #'\\''", Some("// second")),
            ],
            "the closing quote should not hide the comment sigil"
        );
        assert_eq!(
            parse("\t.ascii 'x'\"a  ,b\"\n  ret"),
            [
                statement(true, ".ascii", "'x'\"a  ,b\"", None),
                statement(true, "ret", "", None),
            ],
            "the closing quote should not hide the start of a literal"
        );
        assert_eq!(
            parse("  mov w0,#'a';nop  ,x"),
            [Line::Verbatim("  mov w0,#'a';nop  ,x".to_owned())],
            "the closing quote should not hide the separator"
        );
    }

    #[test]
    fn unclosed_quote_is_passed_through() {
        for source in [
            "  .ifc '\\arg',''// a ,b  c",
            "  .ifc 'x  y','x  y'",
            "  cmp w0,#'\\ch'// q ,r",
            "  .byte 'a,1 , 'b , 2",
            "  .byte ' ",
        ] {
            assert_eq!(
                parse(source),
                [Line::Verbatim(source.to_owned())],
                "a quote that isn't a closed constant should keep the line as written"
            );
        }
    }

    #[test]
    fn unclosed_quote_does_not_affect_the_following_lines() {
        for source in [
            "  .byte '\"",
            "  .byte '\\\"",
            "  mov w0, #'/*",
            "  .ifc 'ab','c\"'",
        ] {
            assert_eq!(
                parse(&format!("{source}\n  mov x0 ,x1")),
                [
                    Line::Verbatim(source.to_owned()),
                    statement(true, "mov", "x0, x1", None),
                ],
                "nothing after the quote should open a literal or a comment"
            );
        }
    }

    #[test]
    fn constant_glued_to_its_directive_is_kept_untouched() {
        assert_eq!(
            parse("\t.byte' ' ,'a'"),
            [statement(true, ".byte", "' ', 'a'", None)],
            "the directive should end where the constant starts"
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
    fn hash_after_a_label_is_passed_through() {
        for source in ["lab: #define X  1", "a: b:#x ,y // c"] {
            assert_eq!(
                parse(source),
                [Line::Verbatim(source.to_owned())],
                "a hash moved to its own line would turn into a directive"
            );
        }
        assert_eq!(
            parse("lab: mov x0 ,#1"),
            [
                Line::Label {
                    name: "lab".to_owned(),
                    comment: None,
                },
                statement(true, "mov", "x0, #1", None),
            ],
            "other code should still move off the line of its label"
        );
    }

    #[test]
    fn open_string_does_not_outlive_a_hash_after_a_label() {
        assert_eq!(
            parse("lab: #error \"open\n  mov x0 ,x1"),
            [
                Line::Verbatim("lab: #error \"open".to_owned()),
                statement(true, "mov", "x0, x1", None),
            ],
            "a stray quote should not affect the lines that follow"
        );
        assert_eq!(
            parse("lab: #x /* open\n  mov x0 ,x1 */\n  mov x0 ,x1"),
            [
                Line::Verbatim("lab: #x /* open".to_owned()),
                Line::Verbatim("  mov x0 ,x1 */".to_owned()),
                statement(true, "mov", "x0, x1", None),
            ],
            "a block comment should still continue on the lines that follow"
        );
    }

    #[test]
    fn string_comparison_operands_are_kept_untouched() {
        assert_eq!(
            parse("  .ifc   a  b ,a b  // c\nlab: .IFNC \\x  y,z"),
            [
                statement(true, ".ifc", "a  b ,a b", Some("// c")),
                Line::Label {
                    name: "lab".to_owned(),
                    comment: None,
                },
                statement(true, ".IFNC", "\\x  y,z", None),
            ],
            "whitespace inside the compared strings should not be normalized"
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
    fn multiline_string_is_passed_through() {
        assert_eq!(
            parse("\t.ascii \"hello\n   world  ,x\"\n  ret"),
            [
                Line::Verbatim("\t.ascii \"hello".to_owned()),
                Line::Verbatim("   world  ,x\"".to_owned()),
                statement(true, "ret", "", None),
            ],
            "only the lines holding the literal should be kept as written"
        );
    }

    #[test]
    fn backslash_continuation_is_passed_through() {
        assert_eq!(
            parse("\tb lab\\\nel\n  ret"),
            [
                Line::Verbatim("\tb lab\\".to_owned()),
                Line::Verbatim("el".to_owned()),
                statement(true, "ret", "", None),
            ],
            "a continued statement should be kept as written on both lines"
        );
        assert_eq!(
            parse("\t.ascii \"a\\\nb\"\n  ret"),
            [
                Line::Verbatim("\t.ascii \"a\\".to_owned()),
                Line::Verbatim("b\"".to_owned()),
                statement(true, "ret", "", None),
            ],
            "a literal closed on the continuation line should not stay open"
        );
    }

    #[test]
    fn open_string_does_not_outlive_a_preprocessor_directive() {
        assert_eq!(
            parse("#error don't \"x\n  mov x0,x1"),
            [
                Line::Preprocessor("#error don't \"x".to_owned()),
                statement(true, "mov", "x0, x1", None),
            ],
            "a stray quote in a directive should not affect the next line"
        );
        assert_eq!(
            parse("#define S \"a \\\n b\"\n  ret"),
            [
                Line::Preprocessor("#define S \"a \\".to_owned()),
                Line::Verbatim(" b\"".to_owned()),
                statement(true, "ret", "", None),
            ],
            "a literal can span the continuation lines of a directive"
        );
    }

    #[test]
    fn escaped_quote_character_constant_is_not_a_string() {
        assert_eq!(
            parse("\tmov w0,#'\\\"' // quote\n  ret"),
            [
                statement(true, "mov", "w0, #'\\\"'", Some("// quote")),
                statement(true, "ret", "", None),
            ],
            "the quote of the constant should not open a literal"
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
