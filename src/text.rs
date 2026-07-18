//! Ingest unstructured text into the document algebra.
//!
//! Each inter-token whitespace run is a choice between preserving the run
//! and breaking the line. Physical newlines remain hard boundaries, and
//! leading indentation is preserved as a classified text run. [`align`]
//! makes reflowed lines return to the source line's content column.

use std::mem;
use std::rc::Rc;

use crate::doc::{align, choice, concat, hardline, line, tag, text, Doc, TagId};
use crate::tags;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Word,
    Symbol,
    Whitespace,
}

impl Kind {
    fn tag(self) -> TagId {
        match self {
            Kind::Word => tags::WORD,
            Kind::Symbol => tags::SYMBOL,
            Kind::Whitespace => tags::WHITESPACE,
        }
    }
}

fn is_word_char(ch: char, previous: Option<char>) -> bool {
    ch.is_alphanumeric()
        || ch == '_'
        || ((ch == '\'' || ch == '\u{2019}') && previous.is_some_and(char::is_alphabetic))
}

fn classified(kind: Kind, value: &str) -> Rc<Doc> {
    tag(kind.tag(), text(value))
}

fn flush(docs: &mut Vec<Rc<Doc>>, buffer: &mut String, kind: Option<Kind>) {
    if let Some(kind) = kind {
        if !buffer.is_empty() {
            let value = mem::take(buffer);
            let run = classified(kind, &value);
            docs.push(if kind == Kind::Whitespace {
                choice(run, tag(tags::NEWLINE, line()))
            } else {
                run
            });
        }
    }
}

fn content_doc(content: &str) -> Rc<Doc> {
    let mut docs = Vec::new();
    let mut buffer = String::new();
    let mut kind = None;
    let mut previous = None;

    for ch in content.chars() {
        let next_kind = if ch == ' ' || ch == '\t' {
            Kind::Whitespace
        } else if is_word_char(ch, previous) {
            Kind::Word
        } else {
            Kind::Symbol
        };

        // Symbols are deliberately one run per character, matching the Go
        // lexer and making punctuation independently taggable.
        if kind != Some(next_kind) || next_kind == Kind::Symbol {
            flush(&mut docs, &mut buffer, kind);
            kind = Some(next_kind);
        }
        buffer.push(ch);
        if next_kind == Kind::Symbol {
            flush(&mut docs, &mut buffer, kind);
            kind = None;
        }
        previous = Some(ch);
    }
    flush(&mut docs, &mut buffer, kind);
    concat(docs)
}

fn line_doc(source_line: &str) -> Rc<Doc> {
    let indent_end = source_line
        .char_indices()
        .find_map(|(index, ch)| (ch != ' ' && ch != '\t').then_some(index))
        .unwrap_or(source_line.len());
    let (leading, content) = source_line.split_at(indent_end);
    if leading.is_empty() {
        align(content_doc(content))
    } else {
        concat([
            tag(tags::INDENT, text(leading)),
            align(content_doc(content)),
        ])
    }
}

/// Convert arbitrary text into a reflowable document.
///
/// Wide layouts reproduce the input byte-for-byte except that CRLF line
/// endings are normalized to LF. Narrow layouts may replace horizontal
/// whitespace inside a physical line with a line break. Empty lines and a
/// trailing newline are preserved.
pub fn from_text(input: &str) -> Rc<Doc> {
    if input.is_empty() {
        return crate::doc::empty();
    }

    let normalized = input.replace("\r\n", "\n");
    let mut docs = Vec::new();
    for (index, source_line) in normalized.split('\n').enumerate() {
        if index > 0 {
            docs.push(hardline());
        }
        docs.push(line_doc(source_line));
    }
    concat(docs)
}
