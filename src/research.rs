//! Allocating compatibility engines for arbitrary cost models.
//!
//! This module is separate from the compact prepared consumer
//! kernel. It favors a small, inspectable oracle implementation over bounded
//! storage and is available only with the `research` feature.

use std::ops::Range;
use std::rc::Rc;

pub use crate::cost::research::{
    CostModel, LawfulCostModel, OverflowThenHeight, ResearchConsumerCost, ResearchConsumerCostModel,
};
pub use crate::doc::count_choices;
use crate::doc::{Doc, Node, TagId, TextRun};

#[derive(Clone, Debug)]
/// Persistent output tree used by allocating research engines.
pub enum Out {
    /// Empty output.
    Empty,
    /// Measured text.
    Text(TextRun),
    /// A newline followed by indentation.
    Newline {
        /// Indentation columns emitted after the newline.
        indent: u32,
    },
    /// Concatenated output.
    Cat(Rc<Out>, Rc<Out>),
    /// Tagged child output.
    Tagged {
        /// Numeric tag applied to the child output.
        tag: TagId,
        /// Tagged output.
        child: Rc<Out>,
    },
    /// Penalized child output.
    Penalty {
        /// Additional consumer cost.
        amount: u32,
        /// Penalized output.
        child: Rc<Out>,
    },
}

/// Constructs empty research output.
pub fn out_empty() -> Rc<Out> {
    Rc::new(Out::Empty)
}

/// Constructs research text output.
pub fn out_text(text: TextRun) -> Rc<Out> {
    Rc::new(Out::Text(text))
}

/// Constructs a research newline.
pub fn out_newline(indent: u32) -> Rc<Out> {
    Rc::new(Out::Newline { indent })
}

/// Concatenates two research outputs, eliminating empty identities.
pub fn out_cat(left: Rc<Out>, right: Rc<Out>) -> Rc<Out> {
    match (&*left, &*right) {
        (Out::Empty, _) => right,
        (_, Out::Empty) => left,
        _ => Rc::new(Out::Cat(left, right)),
    }
}

/// Wraps research output in a numeric tag.
pub fn out_tagged(tag: TagId, child: Rc<Out>) -> Rc<Out> {
    Rc::new(Out::Tagged { tag, child })
}

/// Wraps research output in a penalty.
pub fn out_penalty(amount: u32, child: Rc<Out>) -> Rc<Out> {
    Rc::new(Out::Penalty { amount, child })
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Materialized numeric-tag span produced by a research engine.
pub struct AnnotationSpan {
    /// Numeric tag.
    pub tag: TagId,
    /// UTF-8 byte range.
    pub range: Range<usize>,
    /// Parent span index, when nested.
    pub parent: Option<usize>,
}

#[derive(Clone, Debug)]
/// Materialized research output and its cost.
pub struct Rendered<C> {
    /// Rendered UTF-8 text.
    pub text: String,
    /// Structural tag spans.
    pub spans: Vec<AnnotationSpan>,
    /// Model-specific cost.
    pub cost: C,
}

#[derive(Clone)]
struct State<C> {
    cost: C,
    column: u32,
    out: Rc<Out>,
}

fn append<M: CostModel>(model: &M, left: State<M::Cost>, right: State<M::Cost>) -> State<M::Cost> {
    State {
        cost: model.add(&left.cost, &right.cost),
        column: right.column,
        out: out_cat(left.out, right.out),
    }
}

fn apply<M: CostModel>(
    model: &M,
    doc: &Doc<TagId>,
    state: State<M::Cost>,
    indent: u32,
) -> Vec<State<M::Cost>> {
    match doc.root().as_ref() {
        Node::Empty => vec![state],
        Node::Text(text) => {
            let end = state
                .column
                .checked_add(text.columns())
                .expect("research layout column overflow");
            let piece = State {
                cost: model.text(state.column, text.columns()),
                column: end,
                out: out_text(text.clone()),
            };
            vec![append(model, state, piece)]
        }
        Node::Break(_) | Node::HardLine => {
            let newline = State {
                cost: model.add(&model.newline(), &model.text(0, indent)),
                column: indent,
                out: out_newline(indent),
            };
            vec![append(model, state, newline)]
        }
        Node::Seq(children) => apply_sequence(model, children, vec![state], indent, false),
        Node::Group(child) => {
            let mut layouts = Vec::new();
            if let Some(flat) = child.flatten() {
                layouts.extend(apply(model, &flat, state.clone(), indent));
            }
            layouts.extend(apply(model, child, state, indent));
            layouts
        }
        Node::Fill(children) => apply_sequence(model, children, vec![state], indent, true),
        Node::Nest {
            indent: amount,
            child,
        } => apply(
            model,
            child,
            state,
            indent
                .checked_add(*amount)
                .expect("research indentation overflow"),
        ),
        Node::Align(child) => {
            let aligned = state.column;
            apply(model, child, state, aligned)
        }
        Node::Choice {
            preferred,
            alternative,
        } => {
            let mut layouts = apply(model, preferred, state.clone(), indent);
            layouts.extend(apply(model, alternative, state, indent));
            layouts
        }
        Node::Annotate { annotation, child } => {
            let local = State {
                cost: model.zero(),
                column: state.column,
                out: out_empty(),
            };
            apply(model, child, local, indent)
                .into_iter()
                .map(|mut child| {
                    child.out = out_tagged(**annotation, child.out);
                    append(model, state.clone(), child)
                })
                .collect()
        }
        Node::Penalty { amount, child } => {
            let local = State {
                cost: model.penalty(*amount),
                column: state.column,
                out: out_penalty(*amount, out_empty()),
            };
            apply(model, child, local, indent)
                .into_iter()
                .map(|child| append(model, state.clone(), child))
                .collect()
        }
    }
}

fn apply_sequence<M: CostModel>(
    model: &M,
    children: &[Doc<TagId>],
    mut states: Vec<State<M::Cost>>,
    indent: u32,
    fill: bool,
) -> Vec<State<M::Cost>> {
    for (index, child) in children.iter().enumerate() {
        if fill && index != 0 {
            let mut with_boundary = Vec::with_capacity(states.len().saturating_mul(2));
            for state in states {
                let space_end = state
                    .column
                    .checked_add(1)
                    .expect("research fill column overflow");
                with_boundary.push(append(
                    model,
                    state.clone(),
                    State {
                        cost: model.text(state.column, 1),
                        column: space_end,
                        out: out_text(TextRun::trusted_ascii(" ")),
                    },
                ));
                with_boundary.push(append(
                    model,
                    state,
                    State {
                        cost: model.add(&model.newline(), &model.text(0, indent)),
                        column: indent,
                        out: out_newline(indent),
                    },
                ));
            }
            states = with_boundary;
        }
        let mut next = Vec::new();
        for state in states {
            next.extend(apply(model, child, state, indent));
        }
        states = next;
    }
    states
}

fn all_layouts<M: CostModel>(model: &M, doc: &Doc<TagId>) -> Vec<State<M::Cost>> {
    apply(
        model,
        doc,
        State {
            cost: model.zero(),
            column: 0,
            out: out_empty(),
        },
        0,
    )
}

fn materialize<C>(cost: C, out: Rc<Out>) -> Rendered<C> {
    enum Work {
        Node(Rc<Out>),
        ExitTag(usize),
    }

    let mut text = String::new();
    let mut spans = Vec::<AnnotationSpan>::new();
    let mut open = Vec::<usize>::new();
    let mut work = vec![Work::Node(out)];
    while let Some(item) = work.pop() {
        match item {
            Work::Node(out) => match &*out {
                Out::Empty => {}
                Out::Text(run) => text.push_str(run.value()),
                Out::Newline { indent } => {
                    text.push('\n');
                    for _ in 0..*indent {
                        text.push(' ');
                    }
                }
                Out::Cat(left, right) => {
                    work.push(Work::Node(right.clone()));
                    work.push(Work::Node(left.clone()));
                }
                Out::Tagged { tag, child } => {
                    let span = spans.len();
                    spans.push(AnnotationSpan {
                        tag: *tag,
                        range: text.len()..text.len(),
                        parent: open.last().copied(),
                    });
                    open.push(span);
                    work.push(Work::ExitTag(span));
                    work.push(Work::Node(child.clone()));
                }
                Out::Penalty { child, .. } => work.push(Work::Node(child.clone())),
            },
            Work::ExitTag(span) => {
                assert_eq!(open.pop(), Some(span));
                spans[span].range.end = text.len();
            }
        }
    }
    Rendered { text, spans, cost }
}

/// Recomputes the model cost of a research output tree.
pub fn cost_of_out<M: CostModel>(model: &M, out: &Rc<Out>) -> M::Cost {
    let mut cost = model.zero();
    let mut column = 0u32;
    let mut work = vec![out.clone()];
    while let Some(out) = work.pop() {
        match &*out {
            Out::Empty => {}
            Out::Text(text) => {
                cost = model.add(&cost, &model.text(column, text.columns()));
                column = column
                    .checked_add(text.columns())
                    .expect("research output column overflow");
            }
            Out::Newline { indent } => {
                cost = model.add(&cost, &model.newline());
                cost = model.add(&cost, &model.text(0, *indent));
                column = *indent;
            }
            Out::Cat(left, right) => {
                work.push(right.clone());
                work.push(left.clone());
            }
            Out::Tagged { child, .. } => work.push(child.clone()),
            Out::Penalty { amount, child } => {
                cost = model.add(&cost, &model.penalty(*amount));
                work.push(child.clone());
            }
        }
    }
    cost
}

/// Materializes a research output tree as text.
pub fn to_string(out: &Rc<Out>) -> String {
    materialize((), out.clone()).text
}

/// Materializes and splits a research output tree into lines.
pub fn to_lines(out: &Rc<Out>) -> Vec<String> {
    to_string(out).split('\n').map(str::to_owned).collect()
}

/// Exhaustive allocating oracle.
pub mod brute {
    use super::*;

    #[derive(Clone, Debug)]
    /// Best exhaustive rendering and its structural witness.
    pub struct Rendering<C> {
        /// Model-specific cost.
        pub cost: C,
        /// Persistent output witness.
        pub out: Rc<Out>,
        /// Materialized tag spans.
        pub spans: Vec<AnnotationSpan>,
        /// Materialized output lines.
        pub lines: Vec<String>,
    }

    impl<C> Rendering<C> {
        /// Joins output lines with newline characters.
        pub fn text(&self) -> String {
            self.lines.join("\n")
        }
    }

    /// Enumerates every layout and returns the minimum-cost rendering.
    pub fn best<M: CostModel>(
        model: &M,
        doc: &Doc<TagId>,
        max_choices: usize,
    ) -> Rendering<M::Cost> {
        let choices = crate::doc::count_choices(doc);
        assert!(
            choices <= max_choices,
            "brute-force oracle refused: {choices} choices exceeds cap {max_choices}"
        );
        let state = all_layouts(model, doc)
            .into_iter()
            .min_by(|left, right| left.cost.cmp(&right.cost))
            .expect("a document always has one layout");
        let out = state.out;
        let rendered = materialize(state.cost.clone(), out.clone());
        Rendering {
            cost: state.cost,
            spans: rendered.spans,
            lines: rendered.text.split('\n').map(str::to_owned).collect(),
            out,
        }
    }
}

/// First-fitting allocating compatibility engine.
pub mod greedy {
    use super::*;

    #[derive(Clone, Debug)]
    /// Greedy rendering and its structural witness.
    pub struct GreedyResult<C> {
        /// Model-specific cost.
        pub cost: C,
        /// Persistent output witness.
        pub out: Rc<Out>,
        /// Materialized tag spans.
        pub spans: Vec<AnnotationSpan>,
        /// Materialized output lines.
        pub lines: Vec<String>,
    }

    fn fits_width(out: &Rc<Out>, width: u32) -> bool {
        let mut column = 0u32;
        let mut work = vec![out.clone()];
        while let Some(out) = work.pop() {
            match &*out {
                Out::Empty | Out::Penalty { .. } => {
                    if let Out::Penalty { child, .. } = &*out {
                        work.push(child.clone());
                    }
                }
                Out::Text(text) => {
                    column = match column.checked_add(text.columns()) {
                        Some(column) if column <= width => column,
                        _ => return false,
                    };
                }
                Out::Newline { indent } => column = *indent,
                Out::Cat(left, right) => {
                    work.push(right.clone());
                    work.push(left.clone());
                }
                Out::Tagged { child, .. } => work.push(child.clone()),
            }
        }
        true
    }

    /// Selects the first enumerated layout that fits `width`.
    pub fn layout<M: CostModel>(model: &M, doc: &Doc<TagId>, width: u32) -> GreedyResult<M::Cost> {
        let layouts = all_layouts(model, doc);
        let state = layouts
            .iter()
            .find(|state| fits_width(&state.out, width))
            .cloned()
            .unwrap_or_else(|| layouts[0].clone());
        let out = state.out;
        let rendered = materialize(state.cost.clone(), out.clone());
        GreedyResult {
            cost: state.cost,
            spans: rendered.spans,
            lines: rendered.text.split('\n').map(str::to_owned).collect(),
            out,
        }
    }
}

/// Sealed lawful-model frontier compatibility engine.
pub mod frontier {
    use super::*;

    #[derive(Clone, Debug)]
    /// Winning frontier candidate.
    pub struct Cand<C> {
        /// Model-specific cost.
        pub cost: C,
        /// Final display column.
        pub last: u32,
        /// Persistent output witness.
        pub out: Rc<Out>,
    }

    /// Returns the minimum-cost lawful candidate.
    pub fn best<M: LawfulCostModel>(model: &M, doc: &Doc<TagId>) -> Cand<M::Cost> {
        let state = all_layouts(model, doc)
            .into_iter()
            .min_by(|left, right| left.cost.cmp(&right.cost))
            .expect("a document always has one layout");
        Cand {
            cost: state.cost,
            last: state.column,
            out: state.out,
        }
    }
}
