//! Shared output events and the high-level consumer rendering facade.
//!
//! Engines preserve annotations and penalties structurally in [`Out`]. One
//! iterative materialization walk produces UTF-8 text, nested byte spans, and
//! reconstructed cost, keeping the consumer path independent of rope shape.

use std::ops::Range;
use std::rc::Rc;

use crate::cost::{ConsumerCost, ConsumerCostModel, CostModel, LawfulCostModel};
use crate::doc::{Doc, TagId, TextRun};
use crate::frontier::{best_with_limits, RenderError, SolveLimits, SolveStats};

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Out {
    Empty,
    Text(TextRun),
    Newline(u32),
    Cat(Rc<Out>, Rc<Out>),
    Tagged(TagId, Rc<Out>),
    Penalty(u32),
}

pub fn out_empty() -> Rc<Out> {
    Rc::new(Out::Empty)
}

pub fn out_text(run: TextRun) -> Rc<Out> {
    Rc::new(Out::Text(run))
}

pub fn out_newline(indent: u32) -> Rc<Out> {
    Rc::new(Out::Newline(indent))
}

pub fn out_cat(a: Rc<Out>, b: Rc<Out>) -> Rc<Out> {
    match (&*a, &*b) {
        (Out::Empty, _) => b,
        (_, Out::Empty) => a,
        _ => Rc::new(Out::Cat(a, b)),
    }
}

pub fn out_tagged(tag: TagId, out: Rc<Out>) -> Rc<Out> {
    Rc::new(Out::Tagged(tag, out))
}

pub fn out_penalty(amount: u32) -> Rc<Out> {
    Rc::new(Out::Penalty(amount))
}

/// One lossless annotation in document preorder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnnotationSpan {
    pub tag: TagId,
    pub range: Range<usize>,
    pub parent: Option<usize>,
}

/// A rendered text slice plus its active outer-to-inner annotation stack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnnotatedRun<'a> {
    pub text: &'a str,
    pub range: Range<usize>,
    pub tags: Vec<TagId>,
}

/// Consumer-owned rendering independent of the source document and solver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rendered<C> {
    pub text: String,
    pub spans: Vec<AnnotationSpan>,
    pub cost: C,
    pub stats: SolveStats,
}

impl<C> Rendered<C> {
    /// Split rendered text at annotation boundaries.
    ///
    /// Runs are nonempty and cover the text exactly once. Tag IDs are ordered
    /// from the outermost active annotation to the innermost.
    pub fn annotated_runs(&self) -> Vec<AnnotatedRun<'_>> {
        if self.text.is_empty() {
            return Vec::new();
        }

        let mut boundaries = Vec::with_capacity(self.spans.len() * 2 + 2);
        boundaries.push(0);
        boundaries.push(self.text.len());
        for span in &self.spans {
            boundaries.push(span.range.start);
            boundaries.push(span.range.end);
        }
        boundaries.sort_unstable();
        boundaries.dedup();

        boundaries
            .windows(2)
            .filter_map(|boundary| {
                let range = boundary[0]..boundary[1];
                if range.is_empty() {
                    return None;
                }
                let tags = self
                    .spans
                    .iter()
                    .filter(|span| span.range.start <= range.start && range.end <= span.range.end)
                    .map(|span| span.tag)
                    .collect();
                Some(AnnotatedRun {
                    text: &self.text[range.clone()],
                    range,
                    tags,
                })
            })
            .collect()
    }
}

/// Options for the built-in exact consumer renderer.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderOptions {
    width: u32,
    newline_cost: u32,
    limits: SolveLimits,
}

impl RenderOptions {
    pub fn new(width: u32) -> Self {
        Self {
            width,
            newline_cost: 1,
            limits: SolveLimits::default(),
        }
    }

    pub fn with_newline_cost(mut self, newline_cost: u32) -> Self {
        self.newline_cost = newline_cost;
        self
    }

    pub fn with_limits(mut self, limits: SolveLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn newline_cost(&self) -> u32 {
        self.newline_cost
    }

    pub fn limits(&self) -> SolveLimits {
        self.limits
    }
}

/// Render with the practical built-in cost model and exact frontier solver.
pub fn render(
    doc: &Rc<Doc>,
    options: &RenderOptions,
) -> Result<Rendered<ConsumerCost>, RenderError> {
    let model = ConsumerCostModel::new(options.width).with_newline_cost(options.newline_cost);
    render_with(doc, &model, options.limits)
}

/// Render exactly with a cost model whose frontier laws are proved.
pub fn render_with<M: LawfulCostModel>(
    doc: &Rc<Doc>,
    model: &M,
    limits: SolveLimits,
) -> Result<Rendered<M::Cost>, RenderError> {
    let solved = best_with_limits(model, doc, limits)?;
    let rendered = materialize(model, &solved.candidate.out, solved.stats);
    debug_assert_eq!(solved.candidate.cost, rendered.cost);
    Ok(rendered)
}

enum Walk<'a> {
    Visit(&'a Out),
    CloseTag(usize),
}

pub(crate) fn materialize<M: CostModel>(cm: &M, out: &Out, stats: SolveStats) -> Rendered<M::Cost> {
    let mut text = String::new();
    let mut spans: Vec<AnnotationSpan> = Vec::new();
    let mut open_tags: Vec<usize> = Vec::new();
    let mut cost = cm.zero();
    let mut col = 0u32;
    let mut stack = vec![Walk::Visit(out)];

    while let Some(action) = stack.pop() {
        match action {
            Walk::Visit(Out::Empty) => {}
            Walk::Visit(Out::Text(run)) => {
                cost = cm.add(&cost, &cm.text(col, run.columns()));
                col = col
                    .checked_add(run.columns())
                    .expect("verified output column overflow");
                text.push_str(run.value());
            }
            Walk::Visit(Out::Newline(indent)) => {
                cost = cm.add(&cost, &cm.newline());
                cost = cm.add(&cost, &cm.text(0, *indent));
                text.push('\n');
                for _ in 0..*indent {
                    text.push(' ');
                }
                col = *indent;
            }
            Walk::Visit(Out::Cat(left, right)) => {
                stack.push(Walk::Visit(right));
                stack.push(Walk::Visit(left));
            }
            Walk::Visit(Out::Tagged(tag, inner)) => {
                let index = spans.len();
                spans.push(AnnotationSpan {
                    tag: *tag,
                    range: text.len()..text.len(),
                    parent: open_tags.last().copied(),
                });
                open_tags.push(index);
                stack.push(Walk::CloseTag(index));
                stack.push(Walk::Visit(inner));
            }
            Walk::Visit(Out::Penalty(amount)) => {
                cost = cm.add(&cost, &cm.penalty(*amount));
            }
            Walk::CloseTag(index) => {
                let popped = open_tags.pop();
                debug_assert_eq!(popped, Some(index));
                spans[index].range.end = text.len();
            }
        }
    }

    debug_assert!(open_tags.is_empty());
    debug_assert!(spans.iter().all(|span| {
        span.range.start <= span.range.end
            && span.range.end <= text.len()
            && text.is_char_boundary(span.range.start)
            && text.is_char_boundary(span.range.end)
            && span.parent.is_none_or(|parent| {
                parent < spans.len()
                    && spans[parent].range.start <= span.range.start
                    && span.range.end <= spans[parent].range.end
            })
    }));

    Rendered {
        text,
        spans,
        cost,
        stats,
    }
}

/// Canonical reconstruction of every semantic cost event in an output rope.
pub fn cost_of_out<M: CostModel>(cm: &M, out: &Out) -> M::Cost {
    materialize(cm, out, SolveStats::default()).cost
}

/// Flatten a rope into lines. Each line already includes its indentation.
pub fn to_lines(out: &Out) -> Vec<String> {
    to_string(out).split('\n').map(str::to_owned).collect()
}

pub fn to_string(out: &Out) -> String {
    let mut text = String::new();
    let mut stack = vec![out];
    while let Some(event) = stack.pop() {
        match event {
            Out::Empty | Out::Penalty(_) => {}
            Out::Text(run) => text.push_str(run.value()),
            Out::Newline(indent) => {
                text.push('\n');
                for _ in 0..*indent {
                    text.push(' ');
                }
            }
            Out::Cat(left, right) => {
                stack.push(right);
                stack.push(left);
            }
            Out::Tagged(_, inner) => stack.push(inner),
        }
    }
    text
}

/// The whitespace-insensitive content of a rendering: every engine and every
/// layout of the same document must agree on this.
pub fn words(s: &str) -> Vec<String> {
    s.split_whitespace().map(str::to_owned).collect()
}
