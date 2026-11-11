//! Rendering of classified lines, with column alignment.

use crate::line::{Line, Statement};

/// Indentation of an indented line.
const INDENT: &str = "\t";

impl Line {
    /// Returns whether the line can be part of an alignment block.
    ///
    /// A block is a run of statements, possibly with comment lines among
    /// them; any other line ends it.
    const fn is_aligned(&self) -> bool {
        matches!(self, Self::Comment { .. } | Self::Statement(_))
    }

    /// Returns the statement held by the line, if any.
    const fn statement(&self) -> Option<&Statement> {
        if let Self::Statement(statement) = self {
            Some(statement)
        } else {
            None
        }
    }
}

impl Statement {
    /// Renders the statement without indentation and trailing comment, with
    /// its head padded to `head_width` characters when a tail follows.
    fn code(&self, head_width: usize) -> String {
        if self.tail.is_empty() {
            self.head.clone()
        } else {
            format!("{:<head_width$} {}", self.head, self.tail)
        }
    }

    /// Renders the whole statement, with its head padded to `head_width`
    /// characters and its code to `code_width` when a comment follows.
    fn render(&self, head_width: usize, code_width: usize) -> String {
        let indent = indent(self.indented);
        let code = self.code(head_width);
        let Some(comment) = &self.comment else {
            return format!("{indent}{code}");
        };

        format!("{indent}{code:<code_width$} {comment}")
    }
}

/// Renders `lines` as formatted source text.
pub fn render(lines: &[Line]) -> String {
    let mut output = String::new();
    for block in lines.chunk_by(|prev, next| prev.is_aligned() && next.is_aligned()) {
        render_block(block, &mut output);
    }

    output
}

/// Renders the lines of `block` into `output`, aligning the operands and the
/// trailing comments of its statements.
fn render_block(block: &[Line], output: &mut String) {
    let statements = || block.iter().filter_map(Line::statement);
    let head_width = statements()
        .filter(|statement| !statement.tail.is_empty())
        .map(|statement| statement.head.chars().count())
        .max()
        .unwrap_or_default();
    let code_width = statements()
        .map(|statement| statement.code(head_width).chars().count())
        .max()
        .unwrap_or_default();

    for line in block {
        let text = match line {
            Line::Blank => String::new(),
            Line::Comment { indented, text } => format!("{}{text}", indent(*indented)),
            Line::Label {
                name,
                comment: None,
            } => format!("{name}:"),
            Line::Label {
                name,
                comment: Some(comment),
            } => format!("{name}: {comment}"),
            Line::Preprocessor(text) | Line::Verbatim(text) => text.clone(),
            Line::Statement(statement) => statement.render(head_width, code_width),
        };
        output.push_str(&text);
        output.push('\n');
    }
}

/// Returns the indentation of a line that is `indented` or not.
const fn indent(indented: bool) -> &'static str {
    if indented { INDENT } else { "" }
}
