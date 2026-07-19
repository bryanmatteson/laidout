//! Exact memoized preference-aware Pareto-frontier search.
//!
//! `solve(doc, col, indent)` returns layouts of `doc` starting at `col`,
//! pruned by `(cost, last-column)` dominance while retaining equal-cost states
//! needed to preserve left-branch ties. Documents are structurally
//! interned, and memo keys contain only document identity, column, and
//! indentation. Tags and penalties remain ordinary structural output events.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::cost::LawfulCostModel;
use crate::doc::{Doc, TagId};
use crate::render::{out_cat, out_empty, out_newline, out_penalty, out_tagged, out_text, Out};

#[derive(Clone, Debug)]
pub struct Cand<C> {
    pub cost: C,
    pub last: u32,
    pub out: Rc<Out>,
}

/// Deterministic work counters for an exact solve.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SolveStats {
    /// Every subproblem invocation, including memo hits.
    pub solve_calls: u64,
    /// Subproblem invocations satisfied from a completed memo entry.
    pub memo_hits: u64,
    /// Subproblem invocations that required evaluation.
    pub memo_misses: u64,
    /// Candidates emitted by a node before that node's pruning pass.
    pub candidates_generated: u64,
    /// Emitted candidates absent from the retained preference-aware frontier.
    pub candidates_pruned: u64,
    /// Largest retained frontier for one completed subproblem.
    pub peak_frontier: usize,
    /// Completed memo entries retained at success or failure.
    pub memo_entries: usize,
    /// Structurally distinct documents interned at success or failure.
    pub interned_documents: usize,
}

/// Deterministic exact-solver resource limits. `None` means unlimited.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SolveLimits {
    /// Maximum permitted solve invocations, including memo hits.
    pub max_solve_calls: Option<u64>,
    /// Maximum permitted node-emitted candidates before pruning.
    pub max_candidates: Option<u64>,
    /// Maximum permitted completed memo entries.
    pub max_memo_entries: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolveLimitKind {
    SolveCalls,
    Candidates,
    MemoEntries,
}

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RenderError {
    LimitExceeded {
        kind: SolveLimitKind,
        configured: u64,
        stats: SolveStats,
    },
    ColumnOverflow {
        column: u32,
        width: u32,
        stats: SolveStats,
    },
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExceeded {
                kind,
                configured,
                stats,
            } => write!(
                f,
                "exact render exceeded {kind:?} limit {configured}; current statistics: {stats:?}"
            ),
            Self::ColumnOverflow {
                column,
                width,
                stats,
            } => write!(
                f,
                "exact render column overflow while adding width {width} at column {column}; current statistics: {stats:?}"
            ),
        }
    }
}

impl Error for RenderError {}

#[derive(Clone, Debug)]
pub struct Solved<C> {
    pub candidate: Cand<C>,
    pub stats: SolveStats,
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct Key {
    doc_id: usize,
    col: u32,
    indent: u32,
}

#[derive(Clone)]
struct DocKey(Rc<Doc>);

impl PartialEq for DocKey {
    fn eq(&self, other: &Self) -> bool {
        if Rc::ptr_eq(&self.0, &other.0) {
            return true;
        }
        let mut stack = vec![(self.0.as_ref(), other.0.as_ref())];
        while let Some((left, right)) = stack.pop() {
            match (left, right) {
                (Doc::Empty, Doc::Empty) => {}
                (Doc::Text(left), Doc::Text(right)) if left == right => {}
                (Doc::Line { flat: left }, Doc::Line { flat: right }) if left == right => {}
                (Doc::Concat(la, lb), Doc::Concat(ra, rb))
                | (Doc::Choice(la, lb), Doc::Choice(ra, rb)) => {
                    stack.push((lb, rb));
                    stack.push((la, ra));
                }
                (Doc::Nest(ln, left), Doc::Nest(rn, right)) if ln == rn => {
                    stack.push((left, right));
                }
                (Doc::Align(left), Doc::Align(right)) => stack.push((left, right)),
                (Doc::Tag(lt, left), Doc::Tag(rt, right)) if lt == rt => {
                    stack.push((left, right));
                }
                (
                    Doc::Penalty {
                        amount: la,
                        doc: left,
                    },
                    Doc::Penalty {
                        amount: ra,
                        doc: right,
                    },
                ) if la == ra => stack.push((left, right)),
                _ => return false,
            }
        }
        true
    }
}

impl Eq for DocKey {}

impl Hash for DocKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut stack = vec![self.0.as_ref()];
        while let Some(doc) = stack.pop() {
            match doc {
                Doc::Empty => 0u8.hash(state),
                Doc::Text(run) => {
                    1u8.hash(state);
                    run.hash(state);
                }
                Doc::Line { flat } => {
                    2u8.hash(state);
                    flat.hash(state);
                }
                Doc::Concat(left, right) => {
                    3u8.hash(state);
                    stack.push(right);
                    stack.push(left);
                }
                Doc::Nest(amount, inner) => {
                    4u8.hash(state);
                    amount.hash(state);
                    stack.push(inner);
                }
                Doc::Align(inner) => {
                    5u8.hash(state);
                    stack.push(inner);
                }
                Doc::Choice(left, right) => {
                    6u8.hash(state);
                    stack.push(right);
                    stack.push(left);
                }
                Doc::Tag(tag, inner) => {
                    7u8.hash(state);
                    tag.hash(state);
                    stack.push(inner);
                }
                Doc::Penalty { amount, doc } => {
                    8u8.hash(state);
                    amount.hash(state);
                    stack.push(doc);
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Unary {
    Pass,
    Tag(TagId),
    Penalty(u32),
}

enum WorkFor<C> {
    Eval {
        doc: Rc<Doc>,
        col: u32,
        indent: u32,
    },
    CompleteUnary {
        key: Key,
        child: Key,
        kind: Unary,
    },
    AfterConcatLeft {
        key: Key,
        left: Key,
        right: Rc<Doc>,
        indent: u32,
    },
    CompleteConcat {
        key: Key,
        left: Rc<Vec<Cand<C>>>,
        right: Rc<Doc>,
        indent: u32,
    },
    CompleteChoice {
        key: Key,
        preferred: Key,
        alternative: Key,
    },
}

pub struct Engine<'a, M: LawfulCostModel> {
    cm: &'a M,
    limits: SolveLimits,
    stats: SolveStats,
    doc_ids: HashMap<DocKey, usize>,
    next_doc_id: usize,
    memo: HashMap<Key, Rc<Vec<Cand<M::Cost>>>>,
}

impl<'a, M: LawfulCostModel> Engine<'a, M> {
    pub fn new(cm: &'a M) -> Self {
        Self::with_limits(cm, SolveLimits::default())
    }

    pub fn with_limits(cm: &'a M, limits: SolveLimits) -> Self {
        Self {
            cm,
            limits,
            stats: SolveStats::default(),
            doc_ids: HashMap::new(),
            next_doc_id: 0,
            memo: HashMap::new(),
        }
    }

    pub fn stats(&self) -> SolveStats {
        self.stats
    }

    fn limit_error(&self, kind: SolveLimitKind, configured: u64) -> RenderError {
        RenderError::LimitExceeded {
            kind,
            configured,
            stats: self.stats,
        }
    }

    fn add_columns(&self, column: u32, width: u32) -> Result<u32, RenderError> {
        column
            .checked_add(width)
            .ok_or(RenderError::ColumnOverflow {
                column,
                width,
                stats: self.stats,
            })
    }

    fn record_solve_call(&mut self) -> Result<(), RenderError> {
        self.stats.solve_calls = self.stats.solve_calls.saturating_add(1);
        if let Some(limit) = self.limits.max_solve_calls {
            if self.stats.solve_calls > limit {
                return Err(self.limit_error(SolveLimitKind::SolveCalls, limit));
            }
        }
        Ok(())
    }

    fn record_candidate(&mut self) -> Result<(), RenderError> {
        self.stats.candidates_generated = self.stats.candidates_generated.saturating_add(1);
        if let Some(limit) = self.limits.max_candidates {
            if self.stats.candidates_generated > limit {
                return Err(self.limit_error(SolveLimitKind::Candidates, limit));
            }
        }
        Ok(())
    }

    fn intern(&mut self, doc: &Rc<Doc>) -> usize {
        let lookup = DocKey(doc.clone());
        if let Some(id) = self.doc_ids.get(&lookup) {
            return *id;
        }
        let id = self.next_doc_id;
        self.next_doc_id += 1;
        self.doc_ids.insert(lookup, id);
        self.stats.interned_documents = self.doc_ids.len();
        id
    }

    fn key(&mut self, doc: &Rc<Doc>, col: u32, indent: u32) -> Key {
        Key {
            doc_id: self.intern(doc),
            col,
            indent,
        }
    }

    fn complete(&mut self, key: Key, candidates: Vec<Cand<M::Cost>>) -> Result<(), RenderError> {
        let before = candidates.len();
        let mut kept: Vec<Cand<M::Cost>> = Vec::with_capacity(before);
        for candidate in candidates {
            // Keep generation order so equal total costs preserve the
            // document algebra's left/preferred bias. A strictly cheaper
            // candidate ending no farther right dominates. At the exact same
            // last column, equality also dominates because the earlier
            // candidate has identical continuation behavior.
            let dominated = kept.iter().any(|previous| {
                (previous.last == candidate.last && previous.cost <= candidate.cost)
                    || (previous.last < candidate.last && previous.cost < candidate.cost)
            });
            if dominated {
                continue;
            }

            kept.retain(|previous| {
                !(candidate.last <= previous.last && candidate.cost < previous.cost)
            });
            kept.push(candidate);
        }
        self.stats.candidates_pruned = self
            .stats
            .candidates_pruned
            .saturating_add(u64::try_from(before - kept.len()).unwrap_or(u64::MAX));
        self.stats.peak_frontier = self.stats.peak_frontier.max(kept.len());

        if let Some(limit) = self.limits.max_memo_entries {
            if self.memo.len() >= limit {
                return Err(self.limit_error(
                    SolveLimitKind::MemoEntries,
                    u64::try_from(limit).unwrap_or(u64::MAX),
                ));
            }
        }
        self.memo.insert(key, Rc::new(kept));
        self.stats.memo_entries = self.memo.len();
        Ok(())
    }

    /// Solve one exact subproblem, accumulating deterministic engine stats.
    pub fn solve(
        &mut self,
        doc: &Rc<Doc>,
        col: u32,
        indent: u32,
    ) -> Result<Rc<Vec<Cand<M::Cost>>>, RenderError> {
        let root = self.key(doc, col, indent);
        let mut work = vec![WorkFor::Eval {
            doc: doc.clone(),
            col,
            indent,
        }];

        while let Some(item) = work.pop() {
            match item {
                WorkFor::Eval { doc, col, indent } => {
                    self.record_solve_call()?;
                    let key = self.key(&doc, col, indent);
                    if self.memo.contains_key(&key) {
                        self.stats.memo_hits = self.stats.memo_hits.saturating_add(1);
                        continue;
                    }
                    self.stats.memo_misses = self.stats.memo_misses.saturating_add(1);

                    match &*doc {
                        Doc::Empty => {
                            self.record_candidate()?;
                            self.complete(
                                key,
                                vec![Cand {
                                    cost: self.cm.zero(),
                                    last: col,
                                    out: out_empty(),
                                }],
                            )?;
                        }
                        Doc::Text(run) => {
                            self.record_candidate()?;
                            let last = self.add_columns(col, run.columns())?;
                            self.complete(
                                key,
                                vec![Cand {
                                    cost: self.cm.text(col, run.columns()),
                                    last,
                                    out: out_text(run.clone()),
                                }],
                            )?;
                        }
                        Doc::Line { .. } => {
                            self.record_candidate()?;
                            let cost = self.cm.add(&self.cm.newline(), &self.cm.text(0, indent));
                            self.complete(
                                key,
                                vec![Cand {
                                    cost,
                                    last: indent,
                                    out: out_newline(indent),
                                }],
                            )?;
                        }
                        Doc::Concat(left, right) => {
                            let left_key = self.key(left, col, indent);
                            work.push(WorkFor::AfterConcatLeft {
                                key,
                                left: left_key,
                                right: right.clone(),
                                indent,
                            });
                            work.push(WorkFor::Eval {
                                doc: left.clone(),
                                col,
                                indent,
                            });
                        }
                        Doc::Nest(amount, inner) => {
                            let child_indent = self.add_columns(indent, u32::from(*amount))?;
                            let child = self.key(inner, col, child_indent);
                            work.push(WorkFor::CompleteUnary {
                                key,
                                child,
                                kind: Unary::Pass,
                            });
                            work.push(WorkFor::Eval {
                                doc: inner.clone(),
                                col,
                                indent: child_indent,
                            });
                        }
                        Doc::Align(inner) => {
                            let child = self.key(inner, col, col);
                            work.push(WorkFor::CompleteUnary {
                                key,
                                child,
                                kind: Unary::Pass,
                            });
                            work.push(WorkFor::Eval {
                                doc: inner.clone(),
                                col,
                                indent: col,
                            });
                        }
                        Doc::Choice(preferred, alternative) => {
                            let preferred_key = self.key(preferred, col, indent);
                            let alternative_key = self.key(alternative, col, indent);
                            work.push(WorkFor::CompleteChoice {
                                key,
                                preferred: preferred_key,
                                alternative: alternative_key,
                            });
                            work.push(WorkFor::Eval {
                                doc: alternative.clone(),
                                col,
                                indent,
                            });
                            work.push(WorkFor::Eval {
                                doc: preferred.clone(),
                                col,
                                indent,
                            });
                        }
                        Doc::Tag(tag, inner) => {
                            let child = self.key(inner, col, indent);
                            work.push(WorkFor::CompleteUnary {
                                key,
                                child,
                                kind: Unary::Tag(*tag),
                            });
                            work.push(WorkFor::Eval {
                                doc: inner.clone(),
                                col,
                                indent,
                            });
                        }
                        Doc::Penalty { amount, doc } => {
                            let child = self.key(doc, col, indent);
                            work.push(WorkFor::CompleteUnary {
                                key,
                                child,
                                kind: Unary::Penalty(*amount),
                            });
                            work.push(WorkFor::Eval {
                                doc: doc.clone(),
                                col,
                                indent,
                            });
                        }
                    }
                }
                WorkFor::CompleteUnary { key, child, kind } => {
                    let child = self
                        .memo
                        .get(&child)
                        .expect("completed unary child is memoized")
                        .clone();
                    let mut candidates = Vec::with_capacity(child.len());
                    for candidate in child.iter() {
                        self.record_candidate()?;
                        let (cost, out) = match kind {
                            Unary::Pass => (candidate.cost.clone(), candidate.out.clone()),
                            Unary::Tag(tag) => (
                                candidate.cost.clone(),
                                out_tagged(tag, candidate.out.clone()),
                            ),
                            Unary::Penalty(amount) => (
                                self.cm.add(&self.cm.penalty(amount), &candidate.cost),
                                out_cat(out_penalty(amount), candidate.out.clone()),
                            ),
                        };
                        candidates.push(Cand {
                            cost,
                            last: candidate.last,
                            out,
                        });
                    }
                    self.complete(key, candidates)?;
                }
                WorkFor::AfterConcatLeft {
                    key,
                    left,
                    right,
                    indent,
                } => {
                    let left = self
                        .memo
                        .get(&left)
                        .expect("completed concat left child is memoized")
                        .clone();
                    work.push(WorkFor::CompleteConcat {
                        key,
                        left: left.clone(),
                        right: right.clone(),
                        indent,
                    });
                    for candidate in left.iter().rev() {
                        work.push(WorkFor::Eval {
                            doc: right.clone(),
                            col: candidate.last,
                            indent,
                        });
                    }
                }
                WorkFor::CompleteConcat {
                    key,
                    left,
                    right,
                    indent,
                } => {
                    let mut combined = Vec::new();
                    for left_candidate in left.iter() {
                        let right_key = self.key(&right, left_candidate.last, indent);
                        let right_candidates = self
                            .memo
                            .get(&right_key)
                            .expect("completed concat right child is memoized")
                            .clone();
                        for right_candidate in right_candidates.iter() {
                            self.record_candidate()?;
                            combined.push(Cand {
                                cost: self.cm.add(&left_candidate.cost, &right_candidate.cost),
                                last: right_candidate.last,
                                out: out_cat(
                                    left_candidate.out.clone(),
                                    right_candidate.out.clone(),
                                ),
                            });
                        }
                    }
                    self.complete(key, combined)?;
                }
                WorkFor::CompleteChoice {
                    key,
                    preferred,
                    alternative,
                } => {
                    let preferred = self
                        .memo
                        .get(&preferred)
                        .expect("completed preferred child is memoized")
                        .clone();
                    let alternative = self
                        .memo
                        .get(&alternative)
                        .expect("completed alternative child is memoized")
                        .clone();
                    let mut candidates = Vec::with_capacity(preferred.len() + alternative.len());
                    for candidate in preferred.iter().chain(alternative.iter()) {
                        self.record_candidate()?;
                        candidates.push(candidate.clone());
                    }
                    self.complete(key, candidates)?;
                }
            }
        }

        Ok(self
            .memo
            .get(&root)
            .expect("completed root is memoized")
            .clone())
    }
}

/// Exact minimum-cost layout with deterministic resource limits.
pub fn best_with_limits<M: LawfulCostModel>(
    cm: &M,
    doc: &Rc<Doc>,
    limits: SolveLimits,
) -> Result<Solved<M::Cost>, RenderError> {
    let mut engine = Engine::with_limits(cm, limits);
    let frontier = engine.solve(doc, 0, 0)?;
    let mut candidates = frontier.iter();
    let mut candidate = candidates
        .next()
        .expect("a document always has at least one layout")
        .clone();
    for next in candidates {
        if next.cost < candidate.cost {
            candidate = next.clone();
        }
    }
    Ok(Solved {
        candidate,
        stats: engine.stats(),
    })
}

/// Exact minimum-cost layout with unlimited deterministic work.
pub fn best<M: LawfulCostModel>(cm: &M, doc: &Rc<Doc>) -> Cand<M::Cost> {
    best_with_limits(cm, doc, SolveLimits::default())
        .expect("unlimited exact solving cannot exceed a resource limit")
        .candidate
}

#[cfg(test)]
mod tests {
    use crate::cost::OverflowThenHeight;
    use crate::doc::{concat2, line, text};
    use crate::table::{table, Column};

    use super::{Engine, RenderError, SolveLimitKind, SolveLimits};

    #[test]
    fn structurally_equal_rebuilt_subtrees_share_memo_entries() {
        let left = concat2(text("same"), line());
        let rebuilt = concat2(text("same"), line());
        assert!(!std::rc::Rc::ptr_eq(&left, &rebuilt));

        let cm = OverflowThenHeight { width: 20 };
        let mut engine = Engine::new(&cm);
        engine.solve(&left, 0, 0).unwrap();
        let after_first = engine.memo.len();
        let docs_after_first = engine.doc_ids.len();
        engine.solve(&rebuilt, 0, 0).unwrap();

        assert_eq!(engine.memo.len(), after_first);
        assert_eq!(engine.doc_ids.len(), docs_after_first);
    }

    #[test]
    fn rebuilt_equal_tables_share_interned_documents_and_memo_entries() {
        let build = || {
            table([Column::new(), Column::new()])
                .row([text("left"), text("right")])
                .row([text("up"), text("down")])
                .build()
                .unwrap()
        };
        let left = build();
        let rebuilt = build();
        assert!(!std::rc::Rc::ptr_eq(&left, &rebuilt));
        assert_eq!(left, rebuilt);

        let cm = OverflowThenHeight { width: 20 };
        let mut engine = Engine::new(&cm);
        engine.solve(&left, 0, 0).unwrap();
        let after_first = engine.memo.len();
        let docs_after_first = engine.doc_ids.len();
        engine.solve(&rebuilt, 0, 0).unwrap();

        assert_eq!(engine.memo.len(), after_first);
        assert_eq!(engine.doc_ids.len(), docs_after_first);
    }

    #[test]
    fn limit_abort_never_caches_the_incomplete_root() {
        let doc = concat2(text("left"), text("right"));
        let cm = OverflowThenHeight { width: 20 };
        let mut engine = Engine::with_limits(
            &cm,
            SolveLimits {
                max_candidates: Some(1),
                ..SolveLimits::default()
            },
        );
        let root = engine.key(&doc, 0, 0);

        let error = engine.solve(&doc, 0, 0).unwrap_err();
        assert!(matches!(
            error,
            RenderError::LimitExceeded {
                kind: SolveLimitKind::Candidates,
                configured: 1,
                ..
            }
        ));
        assert!(!engine.memo.contains_key(&root));
        assert_eq!(engine.stats.memo_entries, 1);
    }

    #[test]
    fn column_overflow_is_typed_instead_of_clamped() {
        let doc = text("xx");
        let cm = OverflowThenHeight { width: u32::MAX };
        let mut engine = Engine::new(&cm);

        assert!(matches!(
            engine.solve(&doc, u32::MAX - 1, 0),
            Err(RenderError::ColumnOverflow {
                column,
                width: 2,
                ..
            }) if column == u32::MAX - 1
        ));
    }
}
