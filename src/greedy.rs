//! Greedy Wadler/Leijen baseline: at each `Choice`, take the preferred
//! branch when it fits, deciding by scanning the laid-out stream — the
//! candidate branch followed by the pending continuation — up to the next
//! break opportunity. This is the classical correct-but-not-optimal printer
//! the frontier engine is measured against.

use std::rc::Rc;

use crate::cost::CostModel;
use crate::doc::{Doc, TagId};
use crate::render::{
    materialize, out_cat, out_empty, out_newline, out_penalty, out_tagged, out_text, to_lines,
    AnnotationSpan, Out,
};

type Frame = (u32, Rc<Doc>);

/// True when the stream starting at `frames` reaches a break opportunity (any
/// `Line`) or its end without exceeding `width` from column `col`. `Choice`
/// nodes encountered during the scan are projected onto their preferred
/// branch.
fn fits(width: u32, mut col: u32, mut frames: Vec<Frame>) -> bool {
    while let Some((indent, d)) = frames.pop() {
        match &*d {
            Doc::Empty => {}
            Doc::Text(run) => {
                col = col
                    .checked_add(run.columns())
                    .expect("greedy fit column overflow");
                if col > width {
                    return false;
                }
            }
            Doc::Line { .. } => return true,
            Doc::Concat(a, b) => {
                frames.push((indent, b.clone()));
                frames.push((indent, a.clone()));
            }
            Doc::Nest(n, inner) => frames.push((
                indent
                    .checked_add(u32::from(*n))
                    .expect("greedy fit indentation overflow"),
                inner.clone(),
            )),
            Doc::Align(inner) => frames.push((col, inner.clone())),
            Doc::Tag(_, inner) | Doc::Penalty { doc: inner, .. } => {
                frames.push((indent, inner.clone()));
            }
            Doc::Choice(preferred, _) => frames.push((indent, preferred.clone())),
        }
    }
    true
}

pub struct GreedyResult<C> {
    pub cost: C,
    pub out: Rc<Out>,
    pub spans: Vec<AnnotationSpan>,
    pub lines: Vec<String>,
}

pub fn layout<M: CostModel>(cm: &M, doc: &Rc<Doc>, width: u32) -> GreedyResult<M::Cost> {
    let mut col: u32 = 0;
    let mut outputs = vec![out_empty()];

    enum Action {
        Visit(Frame),
        CloseTag(TagId),
    }

    let mut stack = vec![Action::Visit((0, doc.clone()))];

    while let Some(action) = stack.pop() {
        let (indent, d) = match action {
            Action::CloseTag(tag) => {
                let inner = outputs.pop().expect("tag output frame exists");
                let tagged = out_tagged(tag, inner);
                let parent = outputs.last_mut().expect("parent output frame exists");
                *parent = out_cat(parent.clone(), tagged);
                continue;
            }
            Action::Visit(frame) => frame,
        };

        match &*d {
            Doc::Empty => {}
            Doc::Text(run) => {
                col = col
                    .checked_add(run.columns())
                    .expect("greedy layout column overflow");
                let parent = outputs.last_mut().expect("output frame exists");
                *parent = out_cat(parent.clone(), out_text(run.clone()));
            }
            Doc::Line { .. } => {
                col = indent;
                let parent = outputs.last_mut().expect("output frame exists");
                *parent = out_cat(parent.clone(), out_newline(indent));
            }
            Doc::Concat(a, b) => {
                stack.push(Action::Visit((indent, b.clone())));
                stack.push(Action::Visit((indent, a.clone())));
            }
            Doc::Nest(n, inner) => {
                stack.push(Action::Visit((
                    indent
                        .checked_add(u32::from(*n))
                        .expect("greedy layout indentation overflow"),
                    inner.clone(),
                )));
            }
            Doc::Align(inner) => stack.push(Action::Visit((col, inner.clone()))),
            Doc::Tag(tag, inner) => {
                outputs.push(out_empty());
                stack.push(Action::CloseTag(*tag));
                stack.push(Action::Visit((indent, inner.clone())));
            }
            Doc::Penalty { amount, doc } => {
                let parent = outputs.last_mut().expect("output frame exists");
                *parent = out_cat(parent.clone(), out_penalty(*amount));
                stack.push(Action::Visit((indent, doc.clone())));
            }
            Doc::Choice(preferred, alternative) => {
                let mut probe = stack
                    .iter()
                    .filter_map(|action| match action {
                        Action::Visit(frame) => Some(frame.clone()),
                        Action::CloseTag(_) => None,
                    })
                    .collect::<Vec<_>>();
                probe.push((indent, preferred.clone()));
                let chosen = if fits(width, col, probe) {
                    preferred
                } else {
                    alternative
                };
                stack.push(Action::Visit((indent, chosen.clone())));
            }
        }
    }

    let out = outputs.pop().expect("root output frame exists");
    debug_assert!(outputs.is_empty());
    let lines = to_lines(&out);
    let rendered = materialize(cm, &out, crate::frontier::SolveStats::default());
    GreedyResult {
        cost: rendered.cost,
        spans: rendered.spans,
        out,
        lines,
    }
}
