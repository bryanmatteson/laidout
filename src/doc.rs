//! The document algebra.
//!
//! `Choice` is the primitive; `group` is sugar built with `flatten`. A `Line`
//! node always renders as a break in every engine — its `flat` field exists
//! only so `flatten` knows what the flat projection of that break is (space,
//! nothing, or impossible).

use std::rc::Rc;

/// Semantic tag identifier. Orthogonal to layout: tags never influence
/// measurement or cost, they only annotate output spans.
pub type TagId = u32;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Doc {
    Empty,
    /// Text without line breaks.
    Text(Rc<str>),
    /// A line break. `flat` is the flat projection used by `flatten`:
    /// `Some(" ")` for `line`, `Some("")` for `softline`, `None` for
    /// `hardline` (no flat form exists).
    Line {
        flat: Option<Rc<str>>,
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
}

pub fn empty() -> Rc<Doc> {
    Rc::new(Doc::Empty)
}

/// Panics if `s` contains a line feed; line structure is expressed with
/// `line`/`softline`/`hardline`, never embedded in text.
pub fn text(s: impl AsRef<str>) -> Rc<Doc> {
    let s = s.as_ref();
    assert!(!s.contains('\n'), "text must not contain line feeds: {s:?}");
    Rc::new(Doc::Text(Rc::from(s)))
}

pub fn line() -> Rc<Doc> {
    Rc::new(Doc::Line {
        flat: Some(Rc::from(" ")),
    })
}

pub fn softline() -> Rc<Doc> {
    Rc::new(Doc::Line {
        flat: Some(Rc::from("")),
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
        Doc::Line { flat: Some(s) } => Some(Rc::new(Doc::Text(s.clone()))),
        Doc::Line { flat: None } => None,
        Doc::Concat(a, b) => Some(concat2(flatten(a)?, flatten(b)?)),
        // Indentation only manifests after breaks; a flat form has none.
        Doc::Nest(_, inner) => flatten(inner),
        Doc::Align(inner) => flatten(inner).map(align),
        Doc::Choice(preferred, _) => flatten(preferred),
        Doc::Tag(t, inner) => Some(tag(*t, flatten(inner)?)),
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
        Doc::Nest(_, inner) | Doc::Align(inner) | Doc::Tag(_, inner) => count_choices(inner),
        Doc::Choice(l, r) => 1 + count_choices(l) + count_choices(r),
    }
}
