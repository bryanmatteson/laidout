//! The optimal engine: memoized Pareto-frontier search.
//!
//! `solve(doc, col, indent)` returns the frontier of layouts of `doc`
//! starting at `col`: candidates carrying (cost, last-line column, output
//! rope), pruned so that a candidate ending further right must be strictly
//! cheaper. Documents are structurally interned, then subproblems are
//! memoized on (interned document, column, indentation, active tag), so
//! independently rebuilt but equal subtrees share results. The interplay of
//! memoization and pruning is what keeps the search polynomial in practice
//! for group-shaped documents.
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

#[derive(Clone, Eq, Hash, PartialEq)]
struct Key {
    doc_id: usize,
    col: u32,
    indent: u32,
    tag: Option<TagId>,
}

pub struct Engine<'a, M: CostModel> {
    cm: &'a M,
    doc_ids: HashMap<Rc<Doc>, usize>,
    next_doc_id: usize,
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
        Engine {
            cm,
            doc_ids: HashMap::new(),
            next_doc_id: 0,
            memo: HashMap::new(),
        }
    }

    fn intern(&mut self, doc: &Rc<Doc>) -> usize {
        if let Some(id) = self.doc_ids.get(doc) {
            return *id;
        }
        let id = self.next_doc_id;
        self.next_doc_id += 1;
        self.doc_ids.insert(doc.clone(), id);
        id
    }

    pub fn solve(
        &mut self,
        doc: &Rc<Doc>,
        col: u32,
        indent: u32,
        tag: Option<TagId>,
    ) -> Rc<Vec<Cand<M::Cost>>> {
        let key = Key {
            doc_id: self.intern(doc),
            col,
            indent,
            tag,
        };
        if let Some(hit) = self.memo.get(&key) {
            return hit.clone();
        }

        let cands = match &**doc {
            Doc::Empty => vec![Cand {
                cost: self.cm.zero(),
                last: col,
                out: out_empty(),
            }],
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
                vec![Cand {
                    cost,
                    last: indent,
                    out: out_newline(indent),
                }]
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
                self.solve(&inner, col, indent + u32::from(*n), tag)
                    .as_ref()
                    .clone()
            }
            Doc::Align(inner) => {
                let inner = inner.clone();
                self.solve(&inner, col, col, tag).as_ref().clone()
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

#[cfg(test)]
mod tests {
    use crate::cost::OverflowThenHeight;
    use crate::doc::{concat2, line, text};

    use super::Engine;

    #[test]
    fn structurally_equal_rebuilt_subtrees_share_memo_entries() {
        let left = concat2(text("same"), line());
        let rebuilt = concat2(text("same"), line());
        assert!(!std::rc::Rc::ptr_eq(&left, &rebuilt));

        let cm = OverflowThenHeight { width: 20 };
        let mut engine = Engine::new(&cm);
        engine.solve(&left, 0, 0, None);
        let after_first = engine.memo.len();
        let docs_after_first = engine.doc_ids.len();
        engine.solve(&rebuilt, 0, 0, None);

        assert_eq!(engine.memo.len(), after_first);
        assert_eq!(engine.doc_ids.len(), docs_after_first);
    }
}
