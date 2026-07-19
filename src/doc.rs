//! The document algebra.
//!
//! `Choice` is the primitive; `group` is sugar built with `flatten`. A `Line`
//! node always renders as a break in every engine — its `flat` field exists
//! only so `flatten` knows what the flat projection of that break is (space,
//! nothing, or impossible).

use std::error::Error;
use std::fmt;
use std::rc::Rc;

use unicode_width::UnicodeWidthStr;

/// Semantic tag identifier. Orthogonal to layout: tags never influence
/// measurement or cost, they only annotate output spans.
pub type TagId = u32;

/// Terminal-column policy used when a text run is constructed.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum WidthMode {
    /// Unicode width with East Asian ambiguous characters treated as narrow.
    #[default]
    Narrow,
    /// Unicode width with East Asian ambiguous characters treated as wide.
    Cjk,
}

/// Failure to turn input text into measured document runs.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextError {
    /// A terminal control character appeared where document text was expected.
    ControlCharacter { character: char, byte_offset: usize },
    /// The measured display width cannot be represented by the document model.
    DisplayWidthOverflow { byte_offset: usize },
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ControlCharacter {
                character,
                byte_offset,
            } => write!(
                f,
                "control character {character:?} at UTF-8 byte offset {byte_offset}"
            ),
            Self::DisplayWidthOverflow { byte_offset } => write!(
                f,
                "text display width exceeds u32::MAX at UTF-8 byte offset {byte_offset}"
            ),
        }
    }
}

impl Error for TextError {}

/// Immutable text plus the authoritative number of terminal columns it uses.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TextRun {
    value: Rc<str>,
    columns: u32,
}

impl TextRun {
    fn try_new(value: &str, width_mode: WidthMode) -> Result<Self, TextError> {
        if let Some((byte_offset, character)) = value
            .char_indices()
            .find(|(_, character)| character.is_control())
        {
            return Err(TextError::ControlCharacter {
                character,
                byte_offset,
            });
        }
        let columns = measured_columns(value, width_mode, value.len())?;
        Ok(Self {
            value: Rc::from(value),
            columns,
        })
    }

    fn trusted_ascii(value: &'static str) -> Self {
        Self {
            value: Rc::from(value),
            columns: u32::try_from(value.len()).expect("trusted text run fits in u32"),
        }
    }

    /// The original UTF-8 text.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// The display width fixed when this run was constructed.
    pub fn columns(&self) -> u32 {
        self.columns
    }
}

pub(crate) fn measured_columns(
    value: &str,
    width_mode: WidthMode,
    byte_offset: usize,
) -> Result<u32, TextError> {
    let width = match width_mode {
        WidthMode::Narrow => UnicodeWidthStr::width(value),
        WidthMode::Cjk => UnicodeWidthStr::width_cjk(value),
    };
    columns_from_width(width, byte_offset)
}

fn columns_from_width(width: usize, byte_offset: usize) -> Result<u32, TextError> {
    u32::try_from(width).map_err(|_| TextError::DisplayWidthOverflow { byte_offset })
}

/// Fallible constructor for untrusted single-line text.
pub fn try_text(value: impl AsRef<str>) -> Result<Rc<Doc>, TextError> {
    try_text_with(value, WidthMode::Narrow)
}

/// Fallible constructor for untrusted text under an explicit width policy.
pub fn try_text_with(value: impl AsRef<str>, width_mode: WidthMode) -> Result<Rc<Doc>, TextError> {
    Ok(Rc::new(Doc::Text(TextRun::try_new(
        value.as_ref(),
        width_mode,
    )?)))
}

#[non_exhaustive]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Doc {
    Empty,
    /// Validated text with an authoritative display width.
    Text(TextRun),
    /// A line break. `flat` is the flat projection used by `flatten`:
    /// `Some(" ")` for `line`, `Some("")` for `softline`, `None` for
    /// `hardline` (no flat form exists).
    Line {
        flat: Option<TextRun>,
    },
    Concat(Rc<Doc>, Rc<Doc>),
    /// Adds to the indentation applied after line breaks inside.
    Nest(u16, Rc<Doc>),
    /// Sets indentation after line breaks inside to the current column.
    ///
    /// This is the classic `align` combinator. Unlike [`nest`], its
    /// indentation is determined when layout reaches the node rather than
    /// when the document is constructed.
    Align(Rc<Doc>),
    /// Layout alternative. Engines prefer the left branch on cost ties.
    Choice(Rc<Doc>, Rc<Doc>),
    Tag(TagId, Rc<Doc>),
    /// Add branch-local cost without occupying terminal columns.
    Penalty {
        amount: u32,
        doc: Rc<Doc>,
    },
}

pub fn empty() -> Rc<Doc> {
    Rc::new(Doc::Empty)
}

/// Convenience constructor for trusted literals.
///
/// Panics with the corresponding [`TextError`] when `value` contains a
/// control character or cannot be measured.
pub fn text(value: impl AsRef<str>) -> Rc<Doc> {
    try_text(value).unwrap_or_else(|error| panic!("{error}"))
}

pub fn line() -> Rc<Doc> {
    Rc::new(Doc::Line {
        flat: Some(TextRun::trusted_ascii(" ")),
    })
}

pub fn softline() -> Rc<Doc> {
    Rc::new(Doc::Line {
        flat: Some(TextRun::trusted_ascii("")),
    })
}

pub fn hardline() -> Rc<Doc> {
    Rc::new(Doc::Line { flat: None })
}

pub fn concat2(a: Rc<Doc>, b: Rc<Doc>) -> Rc<Doc> {
    Rc::new(Doc::Concat(a, b))
}

pub fn concat(docs: impl IntoIterator<Item = Rc<Doc>>) -> Rc<Doc> {
    let mut iter = docs.into_iter();
    let first = match iter.next() {
        Some(d) => d,
        None => return empty(),
    };
    iter.fold(first, concat2)
}

pub fn nest(n: u16, d: Rc<Doc>) -> Rc<Doc> {
    Rc::new(Doc::Nest(n, d))
}

pub fn align(d: Rc<Doc>) -> Rc<Doc> {
    Rc::new(Doc::Align(d))
}

pub fn choice(preferred: Rc<Doc>, alternative: Rc<Doc>) -> Rc<Doc> {
    Rc::new(Doc::Choice(preferred, alternative))
}

pub fn tag(t: TagId, d: Rc<Doc>) -> Rc<Doc> {
    Rc::new(Doc::Tag(t, d))
}

/// Add branch-local burden to `doc` without changing its rendered text.
pub fn penalize(amount: u32, doc: Rc<Doc>) -> Rc<Doc> {
    if amount == 0 {
        doc
    } else {
        Rc::new(Doc::Penalty { amount, doc })
    }
}

pub fn join(sep: Rc<Doc>, docs: impl IntoIterator<Item = Rc<Doc>>) -> Rc<Doc> {
    let mut out: Option<Rc<Doc>> = None;
    for d in docs {
        out = Some(match out {
            None => d,
            Some(acc) => concat2(concat2(acc, sep.clone()), d),
        });
    }
    out.unwrap_or_else(empty)
}

/// The flat projection: every `line` a space, every `softline` nothing.
/// Returns `None` when the document contains a `hardline` (no flat form).
pub fn flatten(d: &Rc<Doc>) -> Option<Rc<Doc>> {
    match &**d {
        Doc::Empty | Doc::Text(_) => Some(d.clone()),
        Doc::Line { flat: Some(run) } => Some(Rc::new(Doc::Text(run.clone()))),
        Doc::Line { flat: None } => None,
        Doc::Concat(a, b) => Some(concat2(flatten(a)?, flatten(b)?)),
        // Indentation only manifests after breaks; a flat form has none.
        Doc::Nest(_, inner) => flatten(inner),
        Doc::Align(inner) => flatten(inner).map(align),
        Doc::Choice(preferred, _) => flatten(preferred),
        Doc::Tag(t, inner) => Some(tag(*t, flatten(inner)?)),
        Doc::Penalty { amount, doc } => Some(penalize(*amount, flatten(doc)?)),
    }
}

/// `group(d)`: prefer the flat projection of `d`, else `d` broken.
/// A document with no flat form is returned unchanged.
pub fn group(d: Rc<Doc>) -> Rc<Doc> {
    match flatten(&d) {
        Some(flat) => choice(flat, d),
        None => d,
    }
}

/// Number of `Choice` nodes; the brute-force oracle is exponential in this.
pub fn count_choices(d: &Doc) -> usize {
    match d {
        Doc::Empty | Doc::Text(_) | Doc::Line { .. } => 0,
        Doc::Concat(a, b) => count_choices(a) + count_choices(b),
        Doc::Nest(_, inner)
        | Doc::Align(inner)
        | Doc::Tag(_, inner)
        | Doc::Penalty { doc: inner, .. } => count_choices(inner),
        Doc::Choice(l, r) => 1 + count_choices(l) + count_choices(r),
    }
}

#[cfg(test)]
mod tests {
    use super::{columns_from_width, TextError};

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn width_overflow_is_a_typed_error_with_its_source_offset() {
        let width = usize::try_from(u64::from(u32::MAX) + 1).unwrap();
        assert_eq!(
            columns_from_width(width, 17),
            Err(TextError::DisplayWidthOverflow { byte_offset: 17 })
        );
    }
}
