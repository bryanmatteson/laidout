//! Brute-force oracle: enumerate every choice assignment, render each fully,
//! keep the minimum-cost rendering. Exponential in the number of `Choice`
//! nodes — this is ground truth for the other engines, not an engine.

use std::rc::Rc;

use crate::cost::CostModel;
use crate::doc::{align, concat2, count_choices, nest, penalize, tag, Doc, TagId};
use crate::render::{
    materialize, out_cat, out_empty, out_newline, out_penalty, out_tagged, out_text, to_lines,
    AnnotationSpan, Out,
};

#[derive(Clone, Debug)]
pub struct Rendering<C> {
    pub cost: C,
    pub out: Rc<Out>,
    pub spans: Vec<AnnotationSpan>,
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
        Doc::Align(inner) => expansions(inner).into_iter().map(align).collect(),
        Doc::Tag(t, inner) => expansions(inner).into_iter().map(|d| tag(*t, d)).collect(),
        Doc::Penalty { amount, doc } => expansions(doc)
            .into_iter()
            .map(|d| penalize(*amount, d))
            .collect(),
        Doc::Choice(l, r) => {
            let mut out = expansions(l);
            out.extend(expansions(r));
            out
        }
    }
}

/// Render a choice-free document, accumulating cost with the model.
fn render_resolved<M: CostModel>(cm: &M, doc: &Rc<Doc>) -> Rendering<M::Cost> {
    let mut col: u32 = 0;
    let mut outputs = vec![out_empty()];

    enum Action {
        Visit(u32, Rc<Doc>),
        CloseTag(TagId),
    }

    let mut stack = vec![Action::Visit(0, doc.clone())];

    while let Some(action) = stack.pop() {
        let (indent, d) = match action {
            Action::CloseTag(tag) => {
                let inner = outputs.pop().expect("tag output frame exists");
                let tagged = out_tagged(tag, inner);
                let parent = outputs.last_mut().expect("parent output frame exists");
                *parent = out_cat(parent.clone(), tagged);
                continue;
            }
            Action::Visit(indent, d) => (indent, d),
        };

        match &*d {
            Doc::Empty => {}
            Doc::Text(run) => {
                col = col
                    .checked_add(run.columns())
                    .expect("brute-force layout column overflow");
                let parent = outputs.last_mut().expect("output frame exists");
                *parent = out_cat(parent.clone(), out_text(run.clone()));
            }
            Doc::Line { .. } => {
                col = indent;
                let parent = outputs.last_mut().expect("output frame exists");
                *parent = out_cat(parent.clone(), out_newline(indent));
            }
            Doc::Concat(a, b) => {
                stack.push(Action::Visit(indent, b.clone()));
                stack.push(Action::Visit(indent, a.clone()));
            }
            Doc::Nest(n, inner) => {
                stack.push(Action::Visit(
                    indent
                        .checked_add(u32::from(*n))
                        .expect("brute-force indentation overflow"),
                    inner.clone(),
                ));
            }
            Doc::Align(inner) => stack.push(Action::Visit(col, inner.clone())),
            Doc::Tag(tag, inner) => {
                outputs.push(out_empty());
                stack.push(Action::CloseTag(*tag));
                stack.push(Action::Visit(indent, inner.clone()));
            }
            Doc::Penalty { amount, doc } => {
                let parent = outputs.last_mut().expect("output frame exists");
                *parent = out_cat(parent.clone(), out_penalty(*amount));
                stack.push(Action::Visit(indent, doc.clone()));
            }
            Doc::Choice(..) => unreachable!("resolved documents contain no choices"),
        }
    }

    let out = outputs.pop().expect("root output frame exists");
    debug_assert!(outputs.is_empty());
    let rendered = materialize(cm, &out, crate::frontier::SolveStats::default());
    let cost = rendered.cost;
    let spans = rendered.spans;
    let lines = to_lines(&out);
    Rendering {
        cost,
        out,
        spans,
        lines,
    }
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
