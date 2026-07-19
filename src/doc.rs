//! Opaque, annotation-generic source documents.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use unicode_width::UnicodeWidthStr;

/// Compatibility annotation identifier used by the built-in helpers.
pub type TagId = u32;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum WidthMode {
    #[default]
    Narrow,
    Cjk,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextError {
    ControlCharacter { character: char, byte_offset: usize },
    DisplayWidthOverflow { byte_offset: usize },
}

impl fmt::Display for TextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ControlCharacter {
                character,
                byte_offset,
            } => write!(
                formatter,
                "control character {character:?} at UTF-8 byte offset {byte_offset}"
            ),
            Self::DisplayWidthOverflow { byte_offset } => write!(
                formatter,
                "text display width exceeds u32::MAX at UTF-8 byte offset {byte_offset}"
            ),
        }
    }
}

impl Error for TextError {}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TextRun {
    value: Arc<str>,
    columns: u32,
}

impl TextRun {
    pub(crate) fn try_new(value: &str, width_mode: WidthMode) -> Result<Self, TextError> {
        if let Some((byte_offset, character)) = value
            .char_indices()
            .find(|(_, character)| character.is_control())
        {
            return Err(TextError::ControlCharacter {
                character,
                byte_offset,
            });
        }
        Ok(Self {
            value: Arc::from(value),
            columns: measured_columns(value, width_mode, value.len())?,
        })
    }

    pub(crate) fn trusted_ascii(value: &'static str) -> Self {
        Self {
            value: Arc::from(value),
            columns: u32::try_from(value.len()).expect("trusted text run fits u32"),
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn columns(&self) -> u32 {
        self.columns
    }

    pub(crate) fn value_arc(&self) -> Arc<str> {
        self.value.clone()
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
    u32::try_from(width).map_err(|_| TextError::DisplayWidthOverflow { byte_offset })
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum FlatAlternative {
    Space,
    Empty,
}

pub(crate) enum Node<A> {
    Empty,
    Text(TextRun),
    Break(FlatAlternative),
    HardLine,
    Seq(Box<[Doc<A>]>),
    Group(Doc<A>),
    Fill(Box<[Doc<A>]>),
    Nest {
        indent: u32,
        child: Doc<A>,
    },
    Align(Doc<A>),
    Choice {
        preferred: Doc<A>,
        alternative: Doc<A>,
    },
    Annotate {
        annotation: Arc<A>,
        child: Doc<A>,
    },
    Penalty {
        amount: u32,
        child: Doc<A>,
    },
}

/// Immutable source document. `Clone` clones one `Arc` handle.
pub struct Doc<A = u32>(pub(crate) Option<Arc<Node<A>>>);

impl<A> Clone for Doc<A> {
    fn clone(&self) -> Self {
        Self(Some(self.root().clone()))
    }
}

impl<A> Doc<A> {
    pub(crate) fn root(&self) -> &Arc<Node<A>> {
        self.0
            .as_ref()
            .expect("document root is present outside destruction")
    }

    fn from_node(node: Node<A>) -> Self {
        Self(Some(Arc::new(node)))
    }

    pub fn empty() -> Self {
        Self::from_node(Node::Empty)
    }

    pub fn text(value: impl AsRef<str>) -> Self {
        Self::try_text(value).unwrap_or_else(|error| panic!("{error}"))
    }

    pub fn try_text(value: impl AsRef<str>) -> Result<Self, TextError> {
        Self::try_text_with(value, WidthMode::Narrow)
    }

    pub fn try_text_with(value: impl AsRef<str>, width_mode: WidthMode) -> Result<Self, TextError> {
        Ok(Self::from_node(Node::Text(TextRun::try_new(
            value.as_ref(),
            width_mode,
        )?)))
    }

    pub fn line() -> Self {
        Self::from_node(Node::Break(FlatAlternative::Space))
    }

    pub fn soft_line() -> Self {
        Self::from_node(Node::Break(FlatAlternative::Empty))
    }

    pub fn hard_line() -> Self {
        Self::from_node(Node::HardLine)
    }

    pub fn concat(docs: impl IntoIterator<Item = Self>) -> Self {
        let mut children = Vec::new();
        for doc in docs {
            let flatten = match doc.root().as_ref() {
                Node::Empty => continue,
                Node::Seq(items) => Some(items.to_vec()),
                _ => None,
            };
            if let Some(items) = flatten {
                children.extend(items);
            } else {
                children.push(doc);
            }
        }
        match children.len() {
            0 => Self::empty(),
            1 => children.pop().expect("one child"),
            _ => Self::from_node(Node::Seq(children.into_boxed_slice())),
        }
    }

    pub fn join(separator: Self, docs: impl IntoIterator<Item = Self>) -> Self {
        let mut interleaved = Vec::new();
        for (index, doc) in docs.into_iter().enumerate() {
            if index != 0 {
                interleaved.push(separator.clone());
            }
            interleaved.push(doc);
        }
        Self::concat(interleaved)
    }

    pub fn group(doc: Self) -> Self {
        Self::from_node(Node::Group(doc))
    }

    pub fn fill(docs: impl IntoIterator<Item = Self>) -> Self {
        Self::from_node(Node::Fill(
            docs.into_iter().collect::<Vec<_>>().into_boxed_slice(),
        ))
    }

    pub fn nest(indent: u32, doc: Self) -> Self {
        Self::from_node(Node::Nest { indent, child: doc })
    }

    pub fn align(doc: Self) -> Self {
        Self::from_node(Node::Align(doc))
    }

    pub fn choice(preferred: Self, alternative: Self) -> Self {
        Self::from_node(Node::Choice {
            preferred,
            alternative,
        })
    }

    pub fn annotate(annotation: A, doc: Self) -> Self {
        Self::annotate_shared(Arc::new(annotation), doc)
    }

    pub fn annotate_shared(annotation: Arc<A>, doc: Self) -> Self {
        Self::from_node(Node::Annotate {
            annotation,
            child: doc,
        })
    }

    pub fn penalize(amount: u32, doc: Self) -> Self {
        if amount == 0 {
            doc
        } else {
            Self::from_node(Node::Penalty { amount, child: doc })
        }
    }

    pub fn flatten(&self) -> Option<Self> {
        enum Work<A> {
            Enter(Doc<A>),
            Seq(usize),
            Fill(usize),
            Align,
            Annotate(Arc<A>),
            Penalty(u32),
        }

        let mut work = vec![Work::Enter(self.clone())];
        let mut results: Vec<Option<Doc<A>>> = Vec::new();
        while let Some(item) = work.pop() {
            match item {
                Work::Enter(doc) => match doc.root().as_ref() {
                    Node::Empty | Node::Text(_) => results.push(Some(doc)),
                    Node::Break(FlatAlternative::Space) => results.push(Some(Doc::from_node(
                        Node::Text(TextRun::trusted_ascii(" ")),
                    ))),
                    Node::Break(FlatAlternative::Empty) => results.push(Some(Doc::empty())),
                    Node::HardLine => results.push(None),
                    Node::Seq(children) => {
                        work.push(Work::Seq(children.len()));
                        for child in children.iter().rev() {
                            work.push(Work::Enter(child.clone()));
                        }
                    }
                    Node::Group(child) | Node::Nest { child, .. } => {
                        work.push(Work::Enter(child.clone()));
                    }
                    Node::Fill(children) => {
                        work.push(Work::Fill(children.len()));
                        for child in children.iter().rev() {
                            work.push(Work::Enter(child.clone()));
                        }
                    }
                    Node::Align(child) => {
                        work.push(Work::Align);
                        work.push(Work::Enter(child.clone()));
                    }
                    Node::Choice { preferred, .. } => {
                        work.push(Work::Enter(preferred.clone()));
                    }
                    Node::Annotate { annotation, child } => {
                        work.push(Work::Annotate(annotation.clone()));
                        work.push(Work::Enter(child.clone()));
                    }
                    Node::Penalty { amount, child } => {
                        work.push(Work::Penalty(*amount));
                        work.push(Work::Enter(child.clone()));
                    }
                },
                Work::Seq(count) => {
                    let start = results.len() - count;
                    let values = results.drain(start..).collect::<Option<Vec<_>>>();
                    results.push(values.map(Doc::concat));
                }
                Work::Fill(count) => {
                    let start = results.len() - count;
                    let values = results.drain(start..).collect::<Option<Vec<_>>>();
                    results.push(values.map(Doc::fill));
                }
                Work::Align => {
                    let value = results.pop().expect("flatten result");
                    results.push(value.map(Doc::align));
                }
                Work::Annotate(annotation) => {
                    let value = results.pop().expect("flatten result");
                    results.push(value.map(|doc| Doc::annotate_shared(annotation, doc)));
                }
                Work::Penalty(amount) => {
                    let value = results.pop().expect("flatten result");
                    results.push(value.map(|doc| Doc::penalize(amount, doc)));
                }
            }
        }
        results.pop().expect("root flatten result")
    }
}

impl<A> Drop for Doc<A> {
    fn drop(&mut self) {
        let Some(root) = self.0.take() else {
            return;
        };
        let mut current = Some(root);
        let mut pending = Vec::new();
        while let Some(node) = current.take().or_else(|| pending.pop()) {
            let Some(node) = Arc::into_inner(node) else {
                continue;
            };
            let mut children = Vec::new();
            match node {
                Node::Seq(mut docs) | Node::Fill(mut docs) => {
                    for doc in docs.iter_mut() {
                        if let Some(child) = doc.0.take() {
                            children.push(child);
                        }
                    }
                }
                Node::Group(mut child)
                | Node::Align(mut child)
                | Node::Nest { mut child, .. }
                | Node::Penalty { mut child, .. }
                | Node::Annotate { mut child, .. } => {
                    if let Some(child) = child.0.take() {
                        children.push(child);
                    }
                }
                Node::Choice {
                    mut preferred,
                    mut alternative,
                } => {
                    if let Some(child) = preferred.0.take() {
                        children.push(child);
                    }
                    if let Some(child) = alternative.0.take() {
                        children.push(child);
                    }
                }
                Node::Empty | Node::Text(_) | Node::Break(_) | Node::HardLine => {}
            }
            if let Some(first) = children.pop() {
                pending.extend(children);
                current = Some(first);
            }
        }
    }
}

fn node_pair<A>(left: &Doc<A>, right: &Doc<A>) -> (usize, usize) {
    (
        Arc::as_ptr(left.root()) as usize,
        Arc::as_ptr(right.root()) as usize,
    )
}

impl<A: PartialEq> PartialEq for Doc<A> {
    fn eq(&self, other: &Self) -> bool {
        if Arc::ptr_eq(self.root(), other.root()) {
            return true;
        }
        let mut stack = vec![(self, other)];
        let mut visited = HashSet::new();
        while let Some((left, right)) = stack.pop() {
            if Arc::ptr_eq(left.root(), right.root()) || !visited.insert(node_pair(left, right)) {
                continue;
            }
            match (left.root().as_ref(), right.root().as_ref()) {
                (Node::Empty, Node::Empty) | (Node::HardLine, Node::HardLine) => {}
                (Node::Text(left), Node::Text(right)) if left == right => {}
                (Node::Break(left), Node::Break(right)) if left == right => {}
                (Node::Seq(left), Node::Seq(right)) | (Node::Fill(left), Node::Fill(right))
                    if left.len() == right.len() =>
                {
                    stack.extend(left.iter().zip(right.iter()));
                }
                (Node::Group(left), Node::Group(right))
                | (Node::Align(left), Node::Align(right)) => stack.push((left, right)),
                (
                    Node::Nest {
                        indent: left_indent,
                        child: left,
                    },
                    Node::Nest {
                        indent: right_indent,
                        child: right,
                    },
                ) if left_indent == right_indent => stack.push((left, right)),
                (
                    Node::Choice {
                        preferred: left_preferred,
                        alternative: left_alternative,
                    },
                    Node::Choice {
                        preferred: right_preferred,
                        alternative: right_alternative,
                    },
                ) => {
                    stack.push((left_alternative, right_alternative));
                    stack.push((left_preferred, right_preferred));
                }
                (
                    Node::Annotate {
                        annotation: left_annotation,
                        child: left,
                    },
                    Node::Annotate {
                        annotation: right_annotation,
                        child: right,
                    },
                ) if left_annotation == right_annotation => stack.push((left, right)),
                (
                    Node::Penalty {
                        amount: left_amount,
                        child: left,
                    },
                    Node::Penalty {
                        amount: right_amount,
                        child: right,
                    },
                ) if left_amount == right_amount => stack.push((left, right)),
                _ => return false,
            }
        }
        true
    }
}

impl<A: Eq> Eq for Doc<A> {}

struct Fnv(u64);

impl Fnv {
    fn byte(&mut self, byte: u8) {
        self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }

    fn u32(&mut self, value: u32) {
        for byte in value.to_le_bytes() {
            self.byte(byte);
        }
    }

    fn u64(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.byte(byte);
        }
    }
}

impl Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.byte(*byte);
        }
    }
}

impl<A: Hash> Hash for Doc<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        enum Work<A> {
            Enter(Doc<A>),
            Exit(Doc<A>),
        }
        let mut work = vec![Work::Enter(self.clone())];
        let mut hashes = HashMap::<usize, u64>::new();
        while let Some(item) = work.pop() {
            match item {
                Work::Enter(doc) => {
                    let pointer = Arc::as_ptr(doc.root()) as usize;
                    if hashes.contains_key(&pointer) {
                        continue;
                    }
                    work.push(Work::Exit(doc.clone()));
                    push_children(&doc, |child| work.push(Work::Enter(child.clone())));
                }
                Work::Exit(doc) => {
                    let mut hash = Fnv(0xcbf29ce484222325);
                    match doc.root().as_ref() {
                        Node::Empty => hash.byte(0),
                        Node::Text(text) => {
                            hash.byte(1);
                            hash.write(text.value().as_bytes());
                            hash.u32(text.columns());
                        }
                        Node::Break(flat) => {
                            hash.byte(2);
                            hash.byte(match flat {
                                FlatAlternative::Space => 1,
                                FlatAlternative::Empty => 0,
                            });
                        }
                        Node::HardLine => hash.byte(3),
                        Node::Seq(children) | Node::Fill(children) => {
                            hash.byte(if matches!(doc.root().as_ref(), Node::Seq(_)) {
                                4
                            } else {
                                6
                            });
                            hash.u64(children.len() as u64);
                            for child in children.iter() {
                                hash.u64(hashes[&(Arc::as_ptr(child.root()) as usize)]);
                            }
                        }
                        Node::Group(child) | Node::Align(child) => {
                            hash.byte(if matches!(doc.root().as_ref(), Node::Group(_)) {
                                5
                            } else {
                                8
                            });
                            hash.u64(hashes[&(Arc::as_ptr(child.root()) as usize)]);
                        }
                        Node::Nest { indent, child } => {
                            hash.byte(7);
                            hash.u32(*indent);
                            hash.u64(hashes[&(Arc::as_ptr(child.root()) as usize)]);
                        }
                        Node::Choice {
                            preferred,
                            alternative,
                        } => {
                            hash.byte(9);
                            hash.u64(hashes[&(Arc::as_ptr(preferred.root()) as usize)]);
                            hash.u64(hashes[&(Arc::as_ptr(alternative.root()) as usize)]);
                        }
                        Node::Annotate { annotation, child } => {
                            hash.byte(10);
                            annotation.hash(&mut hash);
                            hash.u64(hashes[&(Arc::as_ptr(child.root()) as usize)]);
                        }
                        Node::Penalty { amount, child } => {
                            hash.byte(11);
                            hash.u32(*amount);
                            hash.u64(hashes[&(Arc::as_ptr(child.root()) as usize)]);
                        }
                    }
                    hashes.insert(Arc::as_ptr(doc.root()) as usize, hash.finish());
                }
            }
        }
        state.write_u64(hashes[&(Arc::as_ptr(self.root()) as usize)]);
    }
}

fn push_children<'a, A>(doc: &'a Doc<A>, mut push: impl FnMut(&'a Doc<A>)) {
    match doc.root().as_ref() {
        Node::Seq(children) | Node::Fill(children) => {
            for child in children.iter().rev() {
                push(child);
            }
        }
        Node::Group(child)
        | Node::Align(child)
        | Node::Nest { child, .. }
        | Node::Annotate { child, .. }
        | Node::Penalty { child, .. } => push(child),
        Node::Choice {
            preferred,
            alternative,
        } => {
            push(alternative);
            push(preferred);
        }
        Node::Empty | Node::Text(_) | Node::Break(_) | Node::HardLine => {}
    }
}

impl<A: fmt::Debug> fmt::Debug for Doc<A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        enum Work<A> {
            Node(Doc<A>),
            Text(&'static str),
        }
        let mut seen = HashMap::<usize, usize>::new();
        let mut next = 0usize;
        let mut work = vec![Work::Node(self.clone())];
        while let Some(item) = work.pop() {
            match item {
                Work::Text(text) => formatter.write_str(text)?,
                Work::Node(doc) => {
                    let pointer = Arc::as_ptr(doc.root()) as usize;
                    if let Some(index) = seen.get(&pointer) {
                        write!(formatter, "#{index}")?;
                        continue;
                    }
                    let index = next;
                    next += 1;
                    seen.insert(pointer, index);
                    match doc.root().as_ref() {
                        Node::Empty => formatter.write_str("empty()")?,
                        Node::Text(text) => write!(formatter, "text({:?})", text.value())?,
                        Node::Break(FlatAlternative::Space) => formatter.write_str("line()")?,
                        Node::Break(FlatAlternative::Empty) => {
                            formatter.write_str("soft_line()")?
                        }
                        Node::HardLine => formatter.write_str("hard_line()")?,
                        Node::Seq(children) | Node::Fill(children) => {
                            formatter.write_str(
                                if matches!(doc.root().as_ref(), Node::Seq(_)) {
                                    "concat(["
                                } else {
                                    "fill(["
                                },
                            )?;
                            work.push(Work::Text("])"));
                            for (child_index, child) in children.iter().enumerate().rev() {
                                work.push(Work::Node(child.clone()));
                                if child_index != 0 {
                                    work.push(Work::Text(", "));
                                }
                            }
                        }
                        Node::Group(child) | Node::Align(child) => {
                            formatter.write_str(
                                if matches!(doc.root().as_ref(), Node::Group(_)) {
                                    "group("
                                } else {
                                    "align("
                                },
                            )?;
                            work.push(Work::Text(")"));
                            work.push(Work::Node(child.clone()));
                        }
                        Node::Nest { indent, child } => {
                            write!(formatter, "nest({indent}, ")?;
                            work.push(Work::Text(")"));
                            work.push(Work::Node(child.clone()));
                        }
                        Node::Choice {
                            preferred,
                            alternative,
                        } => {
                            formatter.write_str("choice(")?;
                            work.push(Work::Text(")"));
                            work.push(Work::Node(alternative.clone()));
                            work.push(Work::Text(", "));
                            work.push(Work::Node(preferred.clone()));
                        }
                        Node::Annotate { annotation, child } => {
                            write!(formatter, "annotate({annotation:?}, ")?;
                            work.push(Work::Text(")"));
                            work.push(Work::Node(child.clone()));
                        }
                        Node::Penalty { amount, child } => {
                            write!(formatter, "penalize({amount}, ")?;
                            work.push(Work::Text(")"));
                            work.push(Work::Node(child.clone()));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[deprecated(since = "0.2.0", note = "use Doc::empty")]
pub fn empty() -> Doc<u32> {
    Doc::empty()
}
#[deprecated(since = "0.2.0", note = "use Doc::text")]
pub fn text(value: impl AsRef<str>) -> Doc<u32> {
    Doc::text(value)
}
#[deprecated(since = "0.2.0", note = "use Doc::try_text")]
pub fn try_text(value: impl AsRef<str>) -> Result<Doc<u32>, TextError> {
    Doc::try_text(value)
}
#[deprecated(since = "0.2.0", note = "use Doc::try_text_with")]
pub fn try_text_with(value: impl AsRef<str>, mode: WidthMode) -> Result<Doc<u32>, TextError> {
    Doc::try_text_with(value, mode)
}
#[deprecated(since = "0.2.0", note = "use Doc::line")]
pub fn line() -> Doc<u32> {
    Doc::line()
}
#[deprecated(since = "0.2.0", note = "use Doc::soft_line")]
pub fn softline() -> Doc<u32> {
    Doc::soft_line()
}
#[deprecated(since = "0.2.0", note = "use Doc::hard_line")]
pub fn hardline() -> Doc<u32> {
    Doc::hard_line()
}
#[deprecated(since = "0.2.0", note = "use Doc::concat")]
pub fn concat(docs: impl IntoIterator<Item = Doc<u32>>) -> Doc<u32> {
    Doc::concat(docs)
}
#[deprecated(since = "0.2.0", note = "use Doc::concat")]
pub fn concat2(left: Doc<u32>, right: Doc<u32>) -> Doc<u32> {
    Doc::concat([left, right])
}
#[deprecated(since = "0.2.0", note = "use Doc::join")]
pub fn join(separator: Doc<u32>, docs: impl IntoIterator<Item = Doc<u32>>) -> Doc<u32> {
    Doc::join(separator, docs)
}
#[deprecated(since = "0.2.0", note = "use Doc::group")]
pub fn group(doc: Doc<u32>) -> Doc<u32> {
    Doc::group(doc)
}
#[deprecated(since = "0.2.0", note = "use Doc::nest")]
pub fn nest(indent: u16, doc: Doc<u32>) -> Doc<u32> {
    Doc::nest(u32::from(indent), doc)
}
#[deprecated(since = "0.2.0", note = "use Doc::align")]
pub fn align(doc: Doc<u32>) -> Doc<u32> {
    Doc::align(doc)
}
#[deprecated(since = "0.2.0", note = "use Doc::choice")]
pub fn choice(preferred: Doc<u32>, alternative: Doc<u32>) -> Doc<u32> {
    Doc::choice(preferred, alternative)
}
#[deprecated(since = "0.2.0", note = "use Doc::annotate")]
pub fn tag(annotation: TagId, doc: Doc<u32>) -> Doc<u32> {
    Doc::annotate(annotation, doc)
}
#[deprecated(since = "0.2.0", note = "use Doc::penalize")]
pub fn penalize(amount: u32, doc: Doc<u32>) -> Doc<u32> {
    Doc::penalize(amount, doc)
}
#[deprecated(since = "0.2.0", note = "use Doc::flatten")]
pub fn flatten(doc: &Doc<u32>) -> Option<Doc<u32>> {
    doc.flatten()
}

#[cfg(feature = "research")]
pub fn count_choices<A>(doc: &Doc<A>) -> usize {
    let mut count = 0usize;
    let mut work = vec![doc];
    while let Some(doc) = work.pop() {
        match doc.root().as_ref() {
            Node::Group(_) | Node::Choice { .. } => {
                count = count
                    .checked_add(1)
                    .expect("choice count exceeds usize::MAX");
            }
            Node::Fill(children) => {
                count = count
                    .checked_add(children.len().saturating_sub(1))
                    .expect("choice count exceeds usize::MAX");
            }
            _ => {}
        }
        push_children(doc, |child| work.push(child));
    }
    count
}
