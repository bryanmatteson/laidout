//! Greedy Wadler/Leijen baseline: at each `Choice`, take the preferred
//! branch when it fits, deciding by scanning the laid-out stream — the
//! candidate branch followed by the pending continuation — up to the next
//! break opportunity. This is the classical correct-but-not-optimal printer
//! the frontier engine is measured against.

use std::rc::Rc;

use crate::cost::{display_width, CostModel};
use crate::doc::{Doc, TagId};
use crate::render::{cost_of_lines, out_cat, out_empty, out_newline, out_text, to_lines, Out};

type Frame = (u32, Option<TagId>, Rc<Doc>);

/// True when the stream starting at `frames` reaches a break opportunity (any
/// `Line`) or its end without exceeding `width` from column `col`. `Choice`
/// nodes encountered during the scan are projected onto their preferred
/// branch.
fn fits(width: u32, mut col: u32, mut frames: Vec<Frame>) -> bool {
    while let Some((indent, tag, d)) = frames.pop() {
        match &*d {
            Doc::Empty => {}
            Doc::Text(s) => {
                col += display_width(s);
                if col > width {
                    return false;
                }
            }
            Doc::Line { .. } => return true,
            Doc::Concat(a, b) => {
                frames.push((indent, tag, b.clone()));
                frames.push((indent, tag, a.clone()));
            }
            Doc::Nest(n, inner) => frames.push((indent + u32::from(*n), tag, inner.clone())),
            Doc::Tag(t, inner) => frames.push((indent, Some(*t), inner.clone())),
            Doc::Choice(preferred, _) => frames.push((indent, tag, preferred.clone())),
        }
    }
    true
}

pub struct GreedyResult<C> {
    pub cost: C,
    pub out: Rc<Out>,
    pub lines: Vec<String>,
}

pub fn layout<M: CostModel>(cm: &M, doc: &Rc<Doc>, width: u32) -> GreedyResult<M::Cost> {
    let mut col: u32 = 0;
    let mut out = out_empty();
    let mut stack: Vec<Frame> = vec![(0, None, doc.clone())];

    while let Some((indent, tag, d)) = stack.pop() {
        match &*d {
            Doc::Empty => {}
            Doc::Text(s) => {
                col += display_width(s);
                out = out_cat(out, out_text(s.clone(), tag));
            }
            Doc::Line { .. } => {
                col = indent;
                out = out_cat(out, out_newline(indent));
            }
            Doc::Concat(a, b) => {
                stack.push((indent, tag, b.clone()));
                stack.push((indent, tag, a.clone()));
            }
            Doc::Nest(n, inner) => stack.push((indent + u32::from(*n), tag, inner.clone())),
            Doc::Tag(t, inner) => stack.push((indent, Some(*t), inner.clone())),
            Doc::Choice(preferred, alternative) => {
                let mut probe = stack.clone();
                probe.push((indent, tag, preferred.clone()));
                let chosen = if fits(width, col, probe) { preferred } else { alternative };
                stack.push((indent, tag, chosen.clone()));
            }
        }
    }

    let lines = to_lines(&out);
    let cost = cost_of_lines(cm, &lines);
    GreedyResult { cost, out, lines }
}
