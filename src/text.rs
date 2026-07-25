//! Ingest unstructured text into the document algebra.
//!
//! Each inter-token whitespace run is a choice between preserving the run
//! and breaking the line. Physical newlines remain hard boundaries, and
//! leading indentation is preserved as a classified text run. [`Doc::align`]
//! makes reflowed lines return to the source line's content column.

use crate::doc::{measured_columns, Doc, TagId, TextError, WidthMode};
use crate::tags;
use std::mem;
use std::num::NonZeroU8;

/// Policy for normalizing raw text into measured document runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IngestOptions {
    /// Unicode display-width policy for emitted text runs.
    pub width_mode: WidthMode,
    /// Distance between tab stops while normalizing source text.
    pub tab_width: NonZeroU8,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self {
            width_mode: WidthMode::Narrow,
            tab_width: NonZeroU8::new(8).expect("eight is nonzero"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
/// Semantic class assigned to a run produced by text ingestion.
pub enum TextKind {
    /// An alphanumeric or identifier-like run.
    Word,
    /// One punctuation character.
    Symbol,
    /// Horizontal space within a source line.
    Whitespace,
    /// Leading horizontal space on a source line.
    Indent,
    /// A reflow opportunity introduced for horizontal whitespace.
    Newline,
}

impl TextKind {
    /// Map this class to the corresponding built-in numeric tag.
    pub const fn built_in_tag(self) -> TagId {
        match self {
            Self::Word => tags::WORD,
            Self::Symbol => tags::SYMBOL,
            Self::Whitespace => tags::WHITESPACE,
            Self::Indent => tags::INDENT,
            Self::Newline => tags::NEWLINE,
        }
    }
}

fn is_word_char(ch: char, previous: Option<char>) -> bool {
    ch.is_alphanumeric()
        || ch == '_'
        || ((ch == '\'' || ch == '\u{2019}') && previous.is_some_and(char::is_alphabetic))
}

fn classified<A>(
    kind: TextKind,
    value: &str,
    width_mode: WidthMode,
    annotation: &mut impl FnMut(TextKind) -> A,
) -> Result<Doc<A>, TextError> {
    Ok(Doc::annotate(
        annotation(kind),
        Doc::try_text_with(value, width_mode)?,
    ))
}

fn flush<A>(
    docs: &mut Vec<Doc<A>>,
    buffer: &mut String,
    kind: Option<TextKind>,
    width_mode: WidthMode,
    annotation: &mut impl FnMut(TextKind) -> A,
) -> Result<(), TextError> {
    if let Some(kind) = kind {
        if !buffer.is_empty() {
            let value = mem::take(buffer);
            let run = classified(kind, &value, width_mode, annotation)?;
            docs.push(if kind == TextKind::Whitespace {
                Doc::choice(
                    run,
                    Doc::annotate(annotation(TextKind::Newline), Doc::line()),
                )
            } else {
                run
            });
        }
    }
    Ok(())
}

fn content_doc<A>(
    content: &str,
    width_mode: WidthMode,
    annotation: &mut impl FnMut(TextKind) -> A,
) -> Result<Doc<A>, TextError> {
    let mut docs = Vec::new();
    let mut buffer = String::new();
    let mut kind = None;
    let mut previous = None;

    for ch in content.chars() {
        let next_kind = if ch == ' ' {
            TextKind::Whitespace
        } else if is_word_char(ch, previous) {
            TextKind::Word
        } else {
            TextKind::Symbol
        };

        // Symbols are deliberately one run per character, matching the Go
        // lexer and making punctuation independently taggable.
        if kind != Some(next_kind) || next_kind == TextKind::Symbol {
            flush(&mut docs, &mut buffer, kind, width_mode, annotation)?;
            kind = Some(next_kind);
        }
        buffer.push(ch);
        if next_kind == TextKind::Symbol {
            flush(&mut docs, &mut buffer, kind, width_mode, annotation)?;
            kind = None;
        }
        previous = Some(ch);
    }
    flush(&mut docs, &mut buffer, kind, width_mode, annotation)?;
    Ok(Doc::concat(docs))
}

fn line_doc<A>(
    source_line: &str,
    width_mode: WidthMode,
    annotation: &mut impl FnMut(TextKind) -> A,
) -> Result<Doc<A>, TextError> {
    let indent_end = source_line
        .char_indices()
        .find_map(|(index, ch)| (ch != ' ').then_some(index))
        .unwrap_or(source_line.len());
    let (leading, content) = source_line.split_at(indent_end);
    if leading.is_empty() {
        Ok(Doc::align(content_doc(content, width_mode, annotation)?))
    } else {
        Ok(Doc::concat([
            Doc::annotate(
                annotation(TextKind::Indent),
                Doc::try_text_with(leading, width_mode)?,
            ),
            Doc::align(content_doc(content, width_mode, annotation)?),
        ]))
    }
}

fn normalize(input: &str, options: IngestOptions) -> Result<String, TextError> {
    let mut normalized = String::with_capacity(input.len());
    let mut source_line = String::new();
    let mut source_columns = 0u32;
    let mut chars = input.char_indices().peekable();

    while let Some((byte_offset, character)) = chars.next() {
        match character {
            '\r' => {
                if chars.peek().is_some_and(|(_, next)| *next == '\n') {
                    chars.next();
                }
                normalized.push('\n');
                source_line.clear();
                source_columns = 0;
            }
            '\n' => {
                normalized.push('\n');
                source_line.clear();
                source_columns = 0;
            }
            '\t' => {
                let tab_width = u32::from(options.tab_width.get());
                let spaces = tab_width - (source_columns % tab_width);
                source_columns = source_columns
                    .checked_add(spaces)
                    .ok_or(TextError::DisplayWidthOverflow { byte_offset })?;
                for _ in 0..spaces {
                    normalized.push(' ');
                    source_line.push(' ');
                }
            }
            _ if character.is_control() => {
                return Err(TextError::ControlCharacter {
                    character,
                    byte_offset,
                });
            }
            _ => {
                normalized.push(character);
                source_line.push(character);
                source_columns = measured_columns(&source_line, options.width_mode, byte_offset)?;
            }
        }
    }

    Ok(normalized)
}

/// Convert arbitrary text into a reflowable document.
///
/// Wide layouts reproduce normalized input: carriage returns become line
/// feeds and tabs expand to configured source-line tab stops. Narrow layouts
/// replace horizontal whitespace with a break when the selected layout wraps.
pub fn from_text(input: &str) -> Result<Doc, TextError> {
    from_text_with(input, IngestOptions::default())
}

/// Convert arbitrary text under an explicit width and tab-stop policy.
pub fn from_text_with(input: &str, options: IngestOptions) -> Result<Doc, TextError> {
    from_text_with_annotations(input, options, TextKind::built_in_tag)
}

/// Convert arbitrary text while choosing the annotation type for classified runs.
///
/// The callback is invoked for every emitted semantic run. It need not return
/// values implementing `Eq` or `Hash`.
pub fn from_text_with_annotations<A>(
    input: &str,
    options: IngestOptions,
    mut annotation: impl FnMut(TextKind) -> A,
) -> Result<Doc<A>, TextError> {
    if input.is_empty() {
        return Ok(Doc::empty());
    }

    let normalized = normalize(input, options)?;
    let mut docs = Vec::new();
    for (index, source_line) in normalized.split('\n').enumerate() {
        if index > 0 {
            docs.push(Doc::hard_line());
        }
        docs.push(line_doc(source_line, options.width_mode, &mut annotation)?);
    }
    Ok(Doc::concat(docs))
}
