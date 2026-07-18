//! The optimal engine: memoized Pareto-frontier search.
//!
//! `solve(doc, col, indent)` returns the frontier of layouts of `doc`
//! starting at `col`: candidates carrying (cost, last-line column, output
//! rope), pruned so that a candidate ending further right must be strictly
//! cheaper. Subproblems are memoized on (node identity, column, indentation,
//! active tag); the interplay of memoization and pruning is what keeps the
//! search polynomial in practice for group-shaped documents.
//!
//! Ties prefer earlier (left/preferred) candidates, matching the brute-force
//! oracle's left bias, so `best` is deterministic and oracle-comparable on
//! cost.

use std::collections::HashMap;
use std::rc::Rc;

use crate::cost::{display_width, CostModel};
use crate::doc::{Doc, TagId};
use crate::render::{out_cat, out_empty, out_newline, out_text, Out};

#[derive(Clone, Debug)]
pub struct Cand<C> {
    pub cost: C,
    pub last: u32,
    pub out: Rc<Out>,
}

type Key = (usize, u32, u32, Option<TagId>);

pub struct Engine<'a, M: CostModel> {
    cm: &'a M,
    memo: HashMap<Key, Rc<Vec<Cand<M::Cost>>>>,
}

/// Keep the Pareto frontier: after a stable sort by (last, cost), a
/// candidate survives only when strictly cheaper than every survivor ending
/// at or left of it. Stability preserves the left bias on exact ties.
fn prune<C: Clone + Ord>(mut cands: Vec<Cand<C>>) -> Vec<Cand<C>> {
    cands.sort_by(|a, b| a.last.cmp(&b.last).then_with(|| a.cost.cmp(&b.cost)));
    let mut kept: Vec<Cand<C>> = Vec::with_capacity(cands.len());
    for c in cands {
        match kept.last() {
            Some(prev) if c.cost >= prev.cost => {}
            _ => kept.push(c),
        }
    }
    kept
}

impl<'a, M: CostModel> Engine<'a, M> {
    pub fn new(cm: &'a M) -> Self {
        Engine { cm, memo: HashMap::new() }
    }

    pub fn solve(
        &mut self,
        doc: &Rc<Doc>,
        col: u32,
        indent: u32,
        tag: Option<TagId>,
    ) -> Rc<Vec<Cand<M::Cost>>> {
        let key: Key = (Rc::as_ptr(doc) as usize, col, indent, tag);
        if let Some(hit) = self.memo.get(&key) {
            return hit.clone();
        }

        let cands = match &**doc {
            Doc::Empty => vec![Cand { cost: self.cm.zero(), last: col, out: out_empty() }],
            Doc::Text(s) => {
                let w = display_width(s);
                vec![Cand {
                    cost: self.cm.text(col, w),
                    last: col + w,
                    out: out_text(s.clone(), tag),
                }]
            }
            Doc::Line { .. } => {
                let cost = self.cm.add(&self.cm.newline(), &self.cm.text(0, indent));
                vec![Cand { cost, last: indent, out: out_newline(indent) }]
            }
            Doc::Concat(a, b) => {
                let left = self.solve(a, col, indent, tag);
                let mut combined = Vec::new();
                for ca in left.iter() {
                    let right = self.solve(b, ca.last, indent, tag);
                    for cb in right.iter() {
                        combined.push(Cand {
                            cost: self.cm.add(&ca.cost, &cb.cost),
                            last: cb.last,
                            out: out_cat(ca.out.clone(), cb.out.clone()),
                        });
                    }
                }
                prune(combined)
            }
            Doc::Nest(n, inner) => {
                let inner = inner.clone();
                self.solve(&inner, col, indent + u32::from(*n), tag).as_ref().clone()
            }
            Doc::Tag(t, inner) => {
                let (t, inner) = (*t, inner.clone());
                self.solve(&inner, col, indent, Some(t)).as_ref().clone()
            }
            Doc::Choice(preferred, alternative) => {
                let (preferred, alternative) = (preferred.clone(), alternative.clone());
                let mut cands: Vec<Cand<M::Cost>> =
                    self.solve(&preferred, col, indent, tag).as_ref().clone();
                cands.extend(self.solve(&alternative, col, indent, tag).iter().cloned());
                prune(cands)
            }
        };

        let rc = Rc::new(cands);
        self.memo.insert(key, rc.clone());
        rc
    }
}

/// The minimum-cost layout of `doc` under the cost model.
pub fn best<M: CostModel>(cm: &M, doc: &Rc<Doc>) -> Cand<M::Cost> {
    let mut engine = Engine::new(cm);
    let frontier = engine.solve(doc, 0, 0, None);
    frontier
        .iter()
        .min_by(|a, b| a.cost.cmp(&b.cost))
        .expect("a document always has at least one layout")
        .clone()
}
