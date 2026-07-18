//! Brute-force oracle: enumerate every choice assignment, render each fully,
//! keep the minimum-cost rendering. Exponential in the number of `Choice`
//! nodes — this is ground truth for the other engines, not an engine.

use std::rc::Rc;

use crate::cost::{display_width, CostModel};
use crate::doc::{concat2, count_choices, nest, tag, Doc};

#[derive(Clone, Debug)]
pub struct Rendering<C> {
    pub cost: C,
    pub lines: Vec<String>,
}

impl<C> Rendering<C> {
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }
}

/// Every choice-free resolution of `doc`, in left-preferring order.
fn expansions(doc: &Rc<Doc>) -> Vec<Rc<Doc>> {
    match &**doc {
        Doc::Empty | Doc::Text(_) | Doc::Line { .. } => vec![doc.clone()],
        Doc::Concat(a, b) => {
            let left = expansions(a);
            let right = expansions(b);
            let mut out = Vec::with_capacity(left.len() * right.len());
            for l in &left {
                for r in &right {
                    out.push(concat2(l.clone(), r.clone()));
                }
            }
            out
        }
        Doc::Nest(n, inner) => expansions(inner).into_iter().map(|d| nest(*n, d)).collect(),
        Doc::Tag(t, inner) => expansions(inner).into_iter().map(|d| tag(*t, d)).collect(),
        Doc::Choice(l, r) => {
            let mut out = expansions(l);
            out.extend(expansions(r));
            out
        }
    }
}

/// Render a choice-free document, accumulating cost with the model.
fn render_resolved<M: CostModel>(cm: &M, doc: &Rc<Doc>) -> Rendering<M::Cost> {
    let mut lines = vec![String::new()];
    let mut cost = cm.zero();
    let mut col: u32 = 0;
    let mut stack: Vec<(u32, Rc<Doc>)> = vec![(0, doc.clone())];

    while let Some((indent, d)) = stack.pop() {
        match &*d {
            Doc::Empty => {}
            Doc::Text(s) => {
                let w = display_width(s);
                cost = cm.add(&cost, &cm.text(col, w));
                col += w;
                lines.last_mut().unwrap().push_str(s);
            }
            Doc::Line { .. } => {
                cost = cm.add(&cost, &cm.newline());
                cost = cm.add(&cost, &cm.text(0, indent));
                col = indent;
                let mut line = String::new();
                for _ in 0..indent {
                    line.push(' ');
                }
                lines.push(line);
            }
            Doc::Concat(a, b) => {
                stack.push((indent, b.clone()));
                stack.push((indent, a.clone()));
            }
            Doc::Nest(n, inner) => stack.push((indent + u32::from(*n), inner.clone())),
            Doc::Tag(_, inner) => stack.push((indent, inner.clone())),
            Doc::Choice(..) => unreachable!("resolved documents contain no choices"),
        }
    }

    Rendering { cost, lines }
}

/// Minimum-cost rendering over all layouts. Ties keep the earliest
/// (left-preferring) layout. Panics when the document has more than
/// `max_choices` choice nodes.
pub fn best<M: CostModel>(cm: &M, doc: &Rc<Doc>, max_choices: usize) -> Rendering<M::Cost> {
    let n = count_choices(doc);
    assert!(
        n <= max_choices,
        "brute-force oracle refused: {n} choices exceeds cap {max_choices}"
    );

    let mut best: Option<Rendering<M::Cost>> = None;
    for resolved in expansions(doc) {
        let rendering = render_resolved(cm, &resolved);
        match &best {
            Some(b) if b.cost <= rendering.cost => {}
            _ => best = Some(rendering),
        }
    }
    best.expect("a document always has at least one layout")
}
