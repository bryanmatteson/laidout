//! Reusable prepared-document consumer kernel.

use std::collections::TryReserveError;
use std::fmt;
use std::num::NonZeroU32;
use std::ops::Range;

use crate::cost::{ConsumerCost, PreparedConsumerCostModel};
use crate::doc::FlatAlternative;
use crate::prepare::{
    then, AnnotationId, EdgeRange, FitStop, FitSummary, NodeId, PreparedData, PreparedDoc,
    PreparedNode, TextId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutStrategy {
    Fast,
    Exact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndentPolicy {
    Preserve,
    ClampToWidthMinusOne,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderOptions {
    width: NonZeroU32,
    newline_cost: u32,
    strategy: LayoutStrategy,
    indent_policy: IndentPolicy,
}

impl RenderOptions {
    pub const fn new(width: NonZeroU32) -> Self {
        Self {
            width,
            newline_cost: 1,
            strategy: LayoutStrategy::Exact,
            indent_policy: IndentPolicy::Preserve,
        }
    }

    pub const fn with_newline_cost(mut self, newline_cost: u32) -> Self {
        self.newline_cost = newline_cost;
        self
    }

    pub const fn with_strategy(mut self, strategy: LayoutStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    pub const fn with_indent_policy(mut self, policy: IndentPolicy) -> Self {
        self.indent_policy = policy;
        self
    }

    pub const fn width(&self) -> NonZeroU32 {
        self.width
    }
    pub const fn newline_cost(&self) -> u32 {
        self.newline_cost
    }
    pub const fn strategy(&self) -> LayoutStrategy {
        self.strategy
    }
    pub const fn indent_policy(&self) -> IndentPolicy {
        self.indent_policy
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SolveStats {
    pub work_items: u64,
    pub fit_checks: u64,
    pub memo_hits: u64,
    pub memo_misses: u64,
    pub candidates_generated: u64,
    pub candidates_pruned: u64,
    pub peak_frontier: usize,
    pub memo_entries: usize,
    pub plan_nodes: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SolveCounter {
    WorkItems,
    FitChecks,
    MemoHits,
    MemoMisses,
    CandidatesGenerated,
    CandidatesPruned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceMode {
    Growable,
    Fixed,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FastSolveCapacity {
    pub work_items: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExactSolveCapacity {
    pub memo_entries: usize,
    pub retained_candidates: usize,
    pub frontier_scratch: usize,
    pub work_items: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VisitCapacity {
    pub work_items: usize,
    pub annotation_depth: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MaterializeCapacity {
    pub output_bytes: usize,
    pub spans: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderCapacity {
    pub fast: FastSolveCapacity,
    pub exact: ExactSolveCapacity,
    pub plan_nodes: usize,
    pub visit: VisitCapacity,
    pub materialize: MaterializeCapacity,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WorkspaceResource {
    FastWorkItems,
    ExactWorkItems,
    MemoEntries,
    RetainedCandidates,
    FrontierScratch,
    PlanNodes,
    VisitWorkItems,
    AnnotationDepth,
    OutputBytes,
    Spans,
}

#[derive(Debug, thiserror::Error)]
pub enum ReserveError {
    #[error("workspace {resource:?} capacity {requested} is not representable")]
    CapacityOverflow {
        resource: WorkspaceResource,
        requested: usize,
    },
    #[error("failed to reserve {requested} entries for workspace {resource:?}")]
    Allocation {
        resource: WorkspaceResource,
        requested: usize,
        #[source]
        source: TryReserveError,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("workspace {resource:?} requirement {required:?} exceeds maximum {maximum}")]
    WorkspaceCapacityOverflow {
        resource: WorkspaceResource,
        required: Option<u128>,
        maximum: u128,
        stats: SolveStats,
    },
    #[error("fixed workspace {resource:?} capacity {capacity} is below required {required}")]
    WorkspaceExhausted {
        resource: WorkspaceResource,
        capacity: usize,
        required: usize,
        stats: SolveStats,
    },
    #[error("failed to grow workspace {resource:?} from {capacity} to {required}")]
    WorkspaceGrowthFailed {
        resource: WorkspaceResource,
        capacity: usize,
        required: usize,
        stats: SolveStats,
        #[source]
        source: TryReserveError,
    },
    #[error("solve statistic {counter:?} requirement {required} exceeds {maximum}")]
    StatisticsOverflow {
        counter: SolveCounter,
        required: u128,
        maximum: u64,
        stats: SolveStats,
    },
    #[error("prepared consumer cost domain invariant was violated")]
    CostDomainViolation,
    #[error("selected plan cost does not match its structural witness")]
    CostWitnessMismatch,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct PlanId(u32);

impl PlanId {
    fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug)]
enum PlanNode {
    Empty,
    Text(TextId),
    Newline {
        indent: u32,
    },
    Concat {
        left: PlanId,
        right: PlanId,
    },
    Annotate {
        annotation: AnnotationId,
        child: PlanId,
    },
    Penalty {
        amount: u32,
        child: PlanId,
    },
}

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
enum PlanKind {
    Empty,
    Text,
    Newline,
    Concat,
    Annotate,
    Penalty,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
struct PlanRecord {
    first: u32,
    second: u32,
    kind: PlanKind,
}

#[derive(Clone, Copy, Debug)]
enum PendingPlan {
    Existing(PlanId),
    Empty,
    Text(TextId),
    Newline {
        indent: u32,
    },
    Concat {
        left: PlanId,
        right: PlanId,
    },
    Annotate {
        annotation: AnnotationId,
        child: PlanId,
    },
    Penalty {
        amount: u32,
        child: PlanId,
    },
}

#[derive(Clone, Copy, Debug)]
struct Candidate {
    cost: ConsumerCost,
    last: u32,
    precedence: u64,
    plan: PlanId,
}

#[derive(Clone, Copy, Debug)]
struct ScratchCandidate {
    cost: ConsumerCost,
    last: u32,
    precedence: u64,
    plan: PendingPlan,
}

#[derive(Clone, Debug)]
struct FrontierSlot {
    candidate: ScratchCandidate,
    left: Option<u32>,
    right: Option<u32>,
    parent: Option<u32>,
    height: u16,
    precedence_previous: Option<u32>,
    precedence_next: Option<u32>,
    free_next: Option<u32>,
    live: bool,
}

impl FrontierSlot {
    fn new(candidate: ScratchCandidate) -> Self {
        Self {
            candidate,
            left: None,
            right: None,
            parent: None,
            height: 1,
            precedence_previous: None,
            precedence_next: None,
            free_next: None,
            live: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct MemoKey {
    node: u32,
    column: u32,
    indent: u32,
}

#[derive(Clone, Copy, Debug, Default)]
struct CandidateRange {
    start: u32,
    len: u32,
}

#[derive(Clone, Copy, Debug)]
enum ExactTask {
    Eval {
        node: NodeId,
        column: u32,
        indent: u32,
    },
    CompleteAlias {
        key: MemoKey,
    },
    CompleteUnion {
        key: MemoKey,
    },
    CompleteChoiceSet {
        key: MemoKey,
        result_start: usize,
    },
    CompleteAnnotate {
        key: MemoKey,
        annotation: AnnotationId,
    },
    CompletePenalty {
        key: MemoKey,
        amount: u32,
    },
    ContinueSequence {
        key: MemoKey,
        edges: EdgeRange,
        child_index: u32,
        indent: u32,
        fill: bool,
    },
    CompleteSequenceRight {
        key: MemoKey,
        edges: EdgeRange,
        child_index: u32,
        indent: u32,
        fill: bool,
        accumulated: CandidateRange,
        left_offset: u32,
        pending_start: usize,
    },
}

impl CandidateRange {
    fn indexes(self) -> Range<usize> {
        let start = self.start as usize;
        start..start + self.len as usize
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct MemoSlot {
    generation: u32,
    key: MemoKey,
    range: CandidateRange,
}

#[derive(Clone, Copy, Debug)]
enum Mode {
    Flat,
    Broken,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
enum FastTaskKind {
    EvalFlat,
    EvalBroken,
    FillBoundary,
    CompleteSeq,
    CompleteAnnotate,
    CompletePenalty,
}

#[derive(Clone, Copy, Debug)]
struct FastTask {
    continuation: FitSummary,
    first: u32,
    second: u32,
    kind: FastTaskKind,
}

#[derive(Clone, Copy, Debug)]
enum DecodedFastTask {
    Eval {
        node: NodeId,
        mode: Mode,
        indent: u32,
        continuation: FitSummary,
    },
    FillBoundary {
        next: NodeId,
        indent: u32,
    },
    CompleteSeq {
        count: usize,
    },
    CompleteAnnotate {
        annotation: AnnotationId,
    },
    CompletePenalty {
        amount: u32,
    },
}

impl FastTask {
    const EMPTY_CONTINUATION: FitSummary = FitSummary {
        columns: 0,
        stop: FitStop::End,
    };

    fn eval(node: NodeId, mode: Mode, indent: u32, continuation: FitSummary) -> Self {
        Self {
            continuation,
            first: node.0,
            second: indent,
            kind: match mode {
                Mode::Flat => FastTaskKind::EvalFlat,
                Mode::Broken => FastTaskKind::EvalBroken,
            },
        }
    }

    fn fill_boundary(next: NodeId, indent: u32) -> Self {
        Self {
            continuation: Self::EMPTY_CONTINUATION,
            first: next.0,
            second: indent,
            kind: FastTaskKind::FillBoundary,
        }
    }

    fn complete_seq(count: usize, stats: SolveStats) -> Result<Self, RenderError> {
        let count = u32::try_from(count).map_err(|_| RenderError::WorkspaceCapacityOverflow {
            resource: WorkspaceResource::FastWorkItems,
            required: Some(count as u128),
            maximum: u32::MAX as u128,
            stats,
        })?;
        Ok(Self {
            continuation: Self::EMPTY_CONTINUATION,
            first: count,
            second: 0,
            kind: FastTaskKind::CompleteSeq,
        })
    }

    fn complete_annotate(annotation: AnnotationId) -> Self {
        Self {
            continuation: Self::EMPTY_CONTINUATION,
            first: annotation.0,
            second: 0,
            kind: FastTaskKind::CompleteAnnotate,
        }
    }

    fn complete_penalty(amount: u32) -> Self {
        Self {
            continuation: Self::EMPTY_CONTINUATION,
            first: amount,
            second: 0,
            kind: FastTaskKind::CompletePenalty,
        }
    }

    fn decode(self) -> DecodedFastTask {
        match self.kind {
            FastTaskKind::EvalFlat => DecodedFastTask::Eval {
                node: NodeId(self.first),
                mode: Mode::Flat,
                indent: self.second,
                continuation: self.continuation,
            },
            FastTaskKind::EvalBroken => DecodedFastTask::Eval {
                node: NodeId(self.first),
                mode: Mode::Broken,
                indent: self.second,
                continuation: self.continuation,
            },
            FastTaskKind::FillBoundary => DecodedFastTask::FillBoundary {
                next: NodeId(self.first),
                indent: self.second,
            },
            FastTaskKind::CompleteSeq => DecodedFastTask::CompleteSeq {
                count: self.first as usize,
            },
            FastTaskKind::CompleteAnnotate => DecodedFastTask::CompleteAnnotate {
                annotation: AnnotationId(self.first),
            },
            FastTaskKind::CompletePenalty => {
                DecodedFastTask::CompletePenalty { amount: self.first }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum VisitWork {
    Plan(PlanId),
    ExitAnnotation { annotation: AnnotationId },
}

#[derive(Clone, Copy, Debug)]
struct OpenSpan {
    annotation: AnnotationId,
    span: SpanId,
}

pub struct RenderWorkspace {
    mode: WorkspaceMode,
    capacity: RenderCapacity,
    fast_work: Vec<FastTask>,
    fast_results: Vec<PlanId>,
    exact_work: Vec<ExactTask>,
    exact_results: Vec<CandidateRange>,
    exact_pending: Vec<ScratchCandidate>,
    memo_slots: Vec<MemoSlot>,
    memo_generation: u32,
    memo_len: usize,
    retained: Vec<Candidate>,
    scratch: Vec<FrontierSlot>,
    scratch_root: Option<u32>,
    scratch_head: Option<u32>,
    scratch_tail: Option<u32>,
    scratch_free: Option<u32>,
    scratch_live: usize,
    plans: Vec<PlanRecord>,
    visit_work: Vec<VisitWork>,
    annotations: Vec<AnnotationId>,
    open_spans: Vec<OpenSpan>,
    output: Vec<u8>,
    spans: Vec<AnnotationSpan>,
}

impl RenderWorkspace {
    pub fn growable() -> Self {
        Self {
            mode: WorkspaceMode::Growable,
            capacity: RenderCapacity::default(),
            fast_work: Vec::new(),
            fast_results: Vec::new(),
            exact_work: Vec::new(),
            exact_results: Vec::new(),
            exact_pending: Vec::new(),
            memo_slots: Vec::new(),
            memo_generation: 1,
            memo_len: 0,
            retained: Vec::new(),
            scratch: Vec::new(),
            scratch_root: None,
            scratch_head: None,
            scratch_tail: None,
            scratch_free: None,
            scratch_live: 0,
            plans: Vec::new(),
            visit_work: Vec::new(),
            annotations: Vec::new(),
            open_spans: Vec::new(),
            output: Vec::new(),
            spans: Vec::new(),
        }
    }

    #[cfg(feature = "research")]
    pub(crate) fn measure_footprint(&self) -> crate::measure::WorkspaceFootprint {
        use std::mem::size_of;

        use crate::measure::{RetainedFootprint, WorkspaceFootprint};

        const fn bytes(entries: usize, element: usize) -> u128 {
            entries as u128 * element as u128
        }

        let capacity = self.capacity;
        let fast_work_items = RetainedFootprint {
            logical_bytes: bytes(
                capacity.fast.work_items,
                size_of::<FastTask>() + size_of::<PlanId>(),
            ),
            actual_retained_bytes: bytes(self.fast_work.capacity(), size_of::<FastTask>())
                + bytes(self.fast_results.capacity(), size_of::<PlanId>()),
        };
        let exact_work_items = RetainedFootprint {
            logical_bytes: bytes(
                capacity.exact.work_items,
                size_of::<ExactTask>()
                    + size_of::<CandidateRange>()
                    + size_of::<ScratchCandidate>(),
            ),
            actual_retained_bytes: bytes(self.exact_work.capacity(), size_of::<ExactTask>())
                + bytes(self.exact_results.capacity(), size_of::<CandidateRange>())
                + bytes(self.exact_pending.capacity(), size_of::<ScratchCandidate>()),
        };
        let memo_entries = RetainedFootprint {
            logical_bytes: bytes(capacity.exact.memo_entries, size_of::<MemoSlot>()),
            actual_retained_bytes: bytes(self.memo_slots.capacity(), size_of::<MemoSlot>()),
        };
        let retained_candidates = RetainedFootprint {
            logical_bytes: bytes(capacity.exact.retained_candidates, size_of::<Candidate>()),
            actual_retained_bytes: bytes(self.retained.capacity(), size_of::<Candidate>()),
        };
        let frontier_scratch = RetainedFootprint {
            logical_bytes: bytes(capacity.exact.frontier_scratch, size_of::<FrontierSlot>()),
            actual_retained_bytes: bytes(self.scratch.capacity(), size_of::<FrontierSlot>()),
        };
        let plan_nodes = RetainedFootprint {
            logical_bytes: bytes(capacity.plan_nodes, size_of::<PlanRecord>()),
            actual_retained_bytes: bytes(self.plans.capacity(), size_of::<PlanRecord>()),
        };
        let visit_work_items = RetainedFootprint {
            logical_bytes: bytes(capacity.visit.work_items, size_of::<VisitWork>()),
            actual_retained_bytes: bytes(self.visit_work.capacity(), size_of::<VisitWork>()),
        };
        let annotation_depth = RetainedFootprint {
            logical_bytes: bytes(
                capacity.visit.annotation_depth,
                size_of::<AnnotationId>() + size_of::<OpenSpan>(),
            ),
            actual_retained_bytes: bytes(self.annotations.capacity(), size_of::<AnnotationId>())
                + bytes(self.open_spans.capacity(), size_of::<OpenSpan>()),
        };
        let output_bytes = RetainedFootprint {
            logical_bytes: capacity.materialize.output_bytes as u128,
            actual_retained_bytes: self.output.capacity() as u128,
        };
        let spans = RetainedFootprint {
            logical_bytes: bytes(capacity.materialize.spans, size_of::<AnnotationSpan>()),
            actual_retained_bytes: bytes(self.spans.capacity(), size_of::<AnnotationSpan>()),
        };
        let resources = [
            fast_work_items,
            exact_work_items,
            memo_entries,
            retained_candidates,
            frontier_scratch,
            plan_nodes,
            visit_work_items,
            annotation_depth,
            output_bytes,
            spans,
        ];
        WorkspaceFootprint {
            fast_work_items,
            exact_work_items,
            memo_entries,
            retained_candidates,
            frontier_scratch,
            plan_nodes,
            visit_work_items,
            annotation_depth,
            output_bytes,
            spans,
            logical_total: resources.iter().map(|value| value.logical_bytes).sum(),
            actual_retained_total: resources
                .iter()
                .map(|value| value.actual_retained_bytes)
                .sum(),
        }
    }

    pub fn fixed(capacity: RenderCapacity) -> Result<Self, ReserveError> {
        let mut workspace = Self::growable();
        workspace.reserve(capacity)?;
        workspace.mode = WorkspaceMode::Fixed;
        Ok(workspace)
    }

    pub const fn mode(&self) -> WorkspaceMode {
        self.mode
    }
    pub fn set_mode(&mut self, mode: WorkspaceMode) {
        self.mode = mode;
    }
    pub const fn capacity(&self) -> RenderCapacity {
        self.capacity
    }

    pub fn reserve(&mut self, requested: RenderCapacity) -> Result<(), ReserveError> {
        self.reset_operation();
        validate_capacity(requested)?;
        let memo_slots = memo_slots_for(requested.exact.memo_entries)?;

        // Stage every allocation before publishing any replacement. A failure in
        // the final resource therefore leaves both backing stores and logical
        // capacities exactly as they were at entry.
        let fast_work = stage_vec(
            &self.fast_work,
            requested.fast.work_items,
            WorkspaceResource::FastWorkItems,
        )?;
        let fast_results = stage_vec(
            &self.fast_results,
            requested.fast.work_items,
            WorkspaceResource::FastWorkItems,
        )?;
        let exact_work = stage_vec(
            &self.exact_work,
            requested.exact.work_items,
            WorkspaceResource::ExactWorkItems,
        )?;
        let exact_results = stage_vec(
            &self.exact_results,
            requested.exact.work_items,
            WorkspaceResource::ExactWorkItems,
        )?;
        let exact_pending = stage_vec(
            &self.exact_pending,
            requested.exact.work_items,
            WorkspaceResource::ExactWorkItems,
        )?;
        let memo = if self.memo_slots.len() < memo_slots {
            let mut replacement = Vec::new();
            replacement
                .try_reserve_exact(memo_slots)
                .map_err(|source| ReserveError::Allocation {
                    resource: WorkspaceResource::MemoEntries,
                    requested: requested.exact.memo_entries,
                    source,
                })?;
            replacement.resize(memo_slots, MemoSlot::default());
            Some(replacement)
        } else {
            None
        };
        let retained = stage_vec(
            &self.retained,
            requested.exact.retained_candidates,
            WorkspaceResource::RetainedCandidates,
        )?;
        let scratch = stage_vec(
            &self.scratch,
            requested.exact.frontier_scratch,
            WorkspaceResource::FrontierScratch,
        )?;
        let plans = stage_vec(
            &self.plans,
            requested.plan_nodes,
            WorkspaceResource::PlanNodes,
        )?;
        let visit_work = stage_vec(
            &self.visit_work,
            requested.visit.work_items,
            WorkspaceResource::VisitWorkItems,
        )?;
        let annotations = stage_vec(
            &self.annotations,
            requested.visit.annotation_depth,
            WorkspaceResource::AnnotationDepth,
        )?;
        let open_spans = stage_vec(
            &self.open_spans,
            requested.visit.annotation_depth,
            WorkspaceResource::AnnotationDepth,
        )?;
        let output = stage_vec(
            &self.output,
            requested.materialize.output_bytes,
            WorkspaceResource::OutputBytes,
        )?;
        let spans = stage_vec(
            &self.spans,
            requested.materialize.spans,
            WorkspaceResource::Spans,
        )?;

        publish_staged(&mut self.fast_work, fast_work);
        publish_staged(&mut self.fast_results, fast_results);
        publish_staged(&mut self.exact_work, exact_work);
        publish_staged(&mut self.exact_results, exact_results);
        publish_staged(&mut self.exact_pending, exact_pending);
        if let Some(memo) = memo {
            self.memo_slots = memo;
            self.memo_generation = 1;
            self.memo_len = 0;
        }
        publish_staged(&mut self.retained, retained);
        publish_staged(&mut self.scratch, scratch);
        publish_staged(&mut self.plans, plans);
        publish_staged(&mut self.visit_work, visit_work);
        publish_staged(&mut self.annotations, annotations);
        publish_staged(&mut self.open_spans, open_spans);
        publish_staged(&mut self.output, output);
        publish_staged(&mut self.spans, spans);
        self.capacity.fast.work_items =
            self.capacity.fast.work_items.max(requested.fast.work_items);
        self.capacity.exact.memo_entries = self
            .capacity
            .exact
            .memo_entries
            .max(requested.exact.memo_entries);
        self.capacity.exact.retained_candidates = self
            .capacity
            .exact
            .retained_candidates
            .max(requested.exact.retained_candidates);
        self.capacity.exact.frontier_scratch = self
            .capacity
            .exact
            .frontier_scratch
            .max(requested.exact.frontier_scratch);
        self.capacity.exact.work_items = self
            .capacity
            .exact
            .work_items
            .max(requested.exact.work_items);
        self.capacity.plan_nodes = self.capacity.plan_nodes.max(requested.plan_nodes);
        self.capacity.visit.work_items = self
            .capacity
            .visit
            .work_items
            .max(requested.visit.work_items);
        self.capacity.visit.annotation_depth = self
            .capacity
            .visit
            .annotation_depth
            .max(requested.visit.annotation_depth);
        self.capacity.materialize.output_bytes = self
            .capacity
            .materialize
            .output_bytes
            .max(requested.materialize.output_bytes);
        self.capacity.materialize.spans = self
            .capacity
            .materialize
            .spans
            .max(requested.materialize.spans);
        Ok(())
    }

    pub fn release_capacity(&mut self) {
        self.reset_operation();
        self.fast_work = Vec::new();
        self.fast_results = Vec::new();
        self.exact_work = Vec::new();
        self.exact_results = Vec::new();
        self.exact_pending = Vec::new();
        self.memo_slots = Vec::new();
        self.retained = Vec::new();
        self.scratch = Vec::new();
        self.scratch_root = None;
        self.scratch_head = None;
        self.scratch_tail = None;
        self.scratch_free = None;
        self.scratch_live = 0;
        self.plans = Vec::new();
        self.visit_work = Vec::new();
        self.annotations = Vec::new();
        self.open_spans = Vec::new();
        self.output = Vec::new();
        self.spans = Vec::new();
        self.capacity = RenderCapacity::default();
    }

    fn reset_operation(&mut self) {
        self.fast_work.clear();
        self.fast_results.clear();
        self.exact_work.clear();
        self.exact_results.clear();
        self.exact_pending.clear();
        self.retained.clear();
        self.scratch.clear();
        self.scratch_root = None;
        self.scratch_head = None;
        self.scratch_tail = None;
        self.scratch_free = None;
        self.scratch_live = 0;
        self.plans.clear();
        self.visit_work.clear();
        self.annotations.clear();
        self.open_spans.clear();
        self.output.clear();
        self.spans.clear();
        self.memo_len = 0;
        self.memo_generation = self.memo_generation.wrapping_add(1);
        if self.memo_generation == 0 {
            for slot in &mut self.memo_slots {
                slot.generation = 0;
            }
            self.memo_generation = 1;
        }
    }

    fn ensure<T>(
        mode: WorkspaceMode,
        resource: WorkspaceResource,
        logical: &mut usize,
        vector: &mut Vec<T>,
        required: usize,
        stats: SolveStats,
    ) -> Result<(), RenderError> {
        if required <= *logical && required <= vector.capacity() {
            return Ok(());
        }
        if mode == WorkspaceMode::Fixed {
            return Err(RenderError::WorkspaceExhausted {
                resource,
                capacity: *logical,
                required,
                stats,
            });
        }
        if required > vector.capacity() {
            vector
                .try_reserve_exact(required - vector.len())
                .map_err(|source| RenderError::WorkspaceGrowthFailed {
                    resource,
                    capacity: *logical,
                    required,
                    stats,
                    source,
                })?;
        }
        *logical = (*logical).max(required);
        Ok(())
    }

    fn ensure_plan(&mut self, additional: usize, stats: SolveStats) -> Result<(), RenderError> {
        let required = self.plans.len().checked_add(additional).ok_or(
            RenderError::WorkspaceCapacityOverflow {
                resource: WorkspaceResource::PlanNodes,
                required: None,
                maximum: u32::MAX as u128,
                stats,
            },
        )?;
        if required > u32::MAX as usize {
            return Err(RenderError::WorkspaceCapacityOverflow {
                resource: WorkspaceResource::PlanNodes,
                required: Some(required as u128),
                maximum: u32::MAX as u128,
                stats,
            });
        }
        Self::ensure(
            self.mode,
            WorkspaceResource::PlanNodes,
            &mut self.capacity.plan_nodes,
            &mut self.plans,
            required,
            stats,
        )
    }

    fn commit_plan(
        &mut self,
        pending: PendingPlan,
        stats: SolveStats,
    ) -> Result<PlanId, RenderError> {
        if let PendingPlan::Existing(plan) = pending {
            return Ok(plan);
        }
        self.ensure_plan(1, stats)?;
        let id = PlanId(u32::try_from(self.plans.len()).map_err(|_| {
            RenderError::WorkspaceCapacityOverflow {
                resource: WorkspaceResource::PlanNodes,
                required: Some(self.plans.len() as u128),
                maximum: u32::MAX as u128,
                stats,
            }
        })?);
        let record = match pending {
            PendingPlan::Existing(_) => unreachable!(),
            PendingPlan::Empty => PlanRecord {
                first: 0,
                second: 0,
                kind: PlanKind::Empty,
            },
            PendingPlan::Text(text) => PlanRecord {
                first: text.0,
                second: 0,
                kind: PlanKind::Text,
            },
            PendingPlan::Newline { indent } => PlanRecord {
                first: indent,
                second: 0,
                kind: PlanKind::Newline,
            },
            PendingPlan::Concat { left, right } => PlanRecord {
                first: left.0,
                second: right.0,
                kind: PlanKind::Concat,
            },
            PendingPlan::Annotate { annotation, child } => PlanRecord {
                first: annotation.0,
                second: child.0,
                kind: PlanKind::Annotate,
            },
            PendingPlan::Penalty { amount, child } => PlanRecord {
                first: amount,
                second: child.0,
                kind: PlanKind::Penalty,
            },
        };
        self.plans.push(record);
        Ok(id)
    }

    fn plan(&self, id: PlanId) -> PlanNode {
        let record = self.plans[id.index()];
        match record.kind {
            PlanKind::Empty => PlanNode::Empty,
            PlanKind::Text => PlanNode::Text(TextId(record.first)),
            PlanKind::Newline => PlanNode::Newline {
                indent: record.first,
            },
            PlanKind::Concat => PlanNode::Concat {
                left: PlanId(record.first),
                right: PlanId(record.second),
            },
            PlanKind::Annotate => PlanNode::Annotate {
                annotation: AnnotationId(record.first),
                child: PlanId(record.second),
            },
            PlanKind::Penalty => PlanNode::Penalty {
                amount: record.first,
                child: PlanId(record.second),
            },
        }
    }
}

fn validate_capacity(capacity: RenderCapacity) -> Result<(), ReserveError> {
    for (resource, requested) in [
        (
            WorkspaceResource::RetainedCandidates,
            capacity.exact.retained_candidates,
        ),
        (
            WorkspaceResource::FrontierScratch,
            capacity.exact.frontier_scratch,
        ),
        (WorkspaceResource::PlanNodes, capacity.plan_nodes),
        (WorkspaceResource::Spans, capacity.materialize.spans),
    ] {
        if requested > u32::MAX as usize {
            return Err(ReserveError::CapacityOverflow {
                resource,
                requested,
            });
        }
    }
    let _ = memo_slots_for(capacity.exact.memo_entries)?;
    Ok(())
}

fn stage_vec<T: Clone>(
    vector: &Vec<T>,
    requested: usize,
    resource: WorkspaceResource,
) -> Result<Option<Vec<T>>, ReserveError> {
    if vector.capacity() >= requested {
        return Ok(None);
    }
    let required = requested.max(vector.len());
    let mut replacement = Vec::new();
    replacement
        .try_reserve_exact(required)
        .map_err(|source| ReserveError::Allocation {
            resource,
            requested,
            source,
        })?;
    replacement.extend_from_slice(vector);
    Ok(Some(replacement))
}

fn publish_staged<T>(target: &mut Vec<T>, staged: Option<Vec<T>>) {
    if let Some(replacement) = staged {
        *target = replacement;
    }
}

fn memo_slots_for(entries: usize) -> Result<usize, ReserveError> {
    if entries == 0 {
        return Ok(0);
    }
    let mut slots = 8usize;
    while slots.checked_mul(7).ok_or(ReserveError::CapacityOverflow {
        resource: WorkspaceResource::MemoEntries,
        requested: entries,
    })? / 10
        < entries
    {
        slots = slots.checked_mul(2).ok_or(ReserveError::CapacityOverflow {
            resource: WorkspaceResource::MemoEntries,
            requested: entries,
        })?;
    }
    Ok(slots)
}

fn increment(stats: &mut SolveStats, counter: SolveCounter) -> Result<u64, RenderError> {
    let previous = match counter {
        SolveCounter::WorkItems => stats.work_items,
        SolveCounter::FitChecks => stats.fit_checks,
        SolveCounter::MemoHits => stats.memo_hits,
        SolveCounter::MemoMisses => stats.memo_misses,
        SolveCounter::CandidatesGenerated => stats.candidates_generated,
        SolveCounter::CandidatesPruned => stats.candidates_pruned,
    };
    let next = previous
        .checked_add(1)
        .ok_or(RenderError::StatisticsOverflow {
            counter,
            required: u128::from(previous) + 1,
            maximum: u64::MAX,
            stats: *stats,
        })?;
    match counter {
        SolveCounter::WorkItems => stats.work_items = next,
        SolveCounter::FitChecks => stats.fit_checks = next,
        SolveCounter::MemoHits => stats.memo_hits = next,
        SolveCounter::MemoMisses => stats.memo_misses = next,
        SolveCounter::CandidatesGenerated => stats.candidates_generated = next,
        SolveCounter::CandidatesPruned => stats.candidates_pruned = next,
    }
    Ok(previous)
}

fn add_pruned(stats: &mut SolveStats, amount: usize) -> Result<(), RenderError> {
    let amount = u64::try_from(amount).map_err(|_| RenderError::StatisticsOverflow {
        counter: SolveCounter::CandidatesPruned,
        required: amount as u128,
        maximum: u64::MAX,
        stats: *stats,
    })?;
    let required = u128::from(stats.candidates_pruned) + u128::from(amount);
    stats.candidates_pruned =
        stats
            .candidates_pruned
            .checked_add(amount)
            .ok_or(RenderError::StatisticsOverflow {
                counter: SolveCounter::CandidatesPruned,
                required,
                maximum: u64::MAX,
                stats: *stats,
            })?;
    Ok(())
}

fn effective_indent(options: RenderOptions, value: u32) -> u32 {
    match options.indent_policy {
        IndentPolicy::Preserve => value,
        IndentPolicy::ClampToWidthMinusOne => value.min(options.width.get() - 1),
    }
}

struct SolvedCore {
    root: PlanId,
    cost: ConsumerCost,
    stats: SolveStats,
    options: RenderOptions,
}

fn solve_core<A>(
    prepared: &PreparedDoc<A>,
    options: RenderOptions,
    workspace: &mut RenderWorkspace,
) -> Result<SolvedCore, RenderError> {
    workspace.reset_operation();
    let result = match options.strategy {
        LayoutStrategy::Fast => solve_fast(&prepared.0, options, workspace),
        LayoutStrategy::Exact => ExactSolver::new(&prepared.0, options, workspace).solve(),
    };
    if result.is_err() {
        workspace.reset_operation();
    }
    result
}

fn solve_fast<A>(
    data: &PreparedData<A>,
    options: RenderOptions,
    workspace: &mut RenderWorkspace,
) -> Result<SolvedCore, RenderError> {
    let mut stats = SolveStats::default();
    let model = PreparedConsumerCostModel::new(options.width.get(), options.newline_cost);
    let mut cost = ConsumerCost::zero();
    let mut column = 0u32;
    push_fast(
        workspace,
        FastTask::eval(
            data.root,
            Mode::Broken,
            0,
            FitSummary {
                columns: 0,
                stop: FitStop::End,
            },
        ),
        stats,
    )?;
    while let Some(task) = workspace.fast_work.pop() {
        match task.decode() {
            DecodedFastTask::Eval {
                node,
                mode,
                indent,
                continuation,
            } => {
                increment(&mut stats, SolveCounter::WorkItems)?;
                match data.nodes[node.index()] {
                    PreparedNode::Empty => {
                        let plan = workspace.commit_plan(PendingPlan::Empty, stats)?;
                        push_fast_result(workspace, plan, stats)?;
                    }
                    PreparedNode::Text(text) => {
                        let prepared = &data.texts[text.index()];
                        cost = cost
                            .checked_add(
                                model
                                    .text(column, prepared.display_width)
                                    .ok_or(RenderError::CostDomainViolation)?,
                            )
                            .ok_or(RenderError::CostDomainViolation)?;
                        column = column
                            .checked_add(prepared.display_width)
                            .ok_or(RenderError::CostDomainViolation)?;
                        let plan = workspace.commit_plan(PendingPlan::Text(text), stats)?;
                        push_fast_result(workspace, plan, stats)?;
                    }
                    PreparedNode::Break(flat) => match (mode, flat) {
                        (Mode::Flat, FlatAlternative::Space) => {
                            cost = cost
                                .checked_add(
                                    model
                                        .text(column, 1)
                                        .ok_or(RenderError::CostDomainViolation)?,
                                )
                                .ok_or(RenderError::CostDomainViolation)?;
                            column = column
                                .checked_add(1)
                                .ok_or(RenderError::CostDomainViolation)?;
                            let plan =
                                workspace.commit_plan(PendingPlan::Text(data.space_text), stats)?;
                            push_fast_result(workspace, plan, stats)?;
                        }
                        (Mode::Flat, FlatAlternative::Empty) => {
                            let plan = workspace.commit_plan(PendingPlan::Empty, stats)?;
                            push_fast_result(workspace, plan, stats)?;
                        }
                        (Mode::Broken, _) => emit_fast_newline(
                            workspace,
                            model,
                            &mut cost,
                            &mut column,
                            indent,
                            stats,
                        )?,
                    },
                    PreparedNode::HardLine => {
                        emit_fast_newline(workspace, model, &mut cost, &mut column, indent, stats)?
                    }
                    PreparedNode::Seq(range) => {
                        let children = data.children(range);
                        push_fast(
                            workspace,
                            FastTask::complete_seq(children.len(), stats)?,
                            stats,
                        )?;
                        let mut next = continuation;
                        for child in children.iter().rev() {
                            push_fast(
                                workspace,
                                FastTask::eval(*child, mode, indent, next),
                                stats,
                            )?;
                            let summary = data.fit_summaries[child.index()];
                            next = then(
                                if matches!(mode, Mode::Flat) {
                                    summary.flat
                                } else {
                                    summary.broken
                                },
                                next,
                            )
                            .ok_or(RenderError::CostDomainViolation)?;
                        }
                    }
                    PreparedNode::Group(child) => {
                        let flat = data.fit_summaries[child.index()].flat;
                        let selected =
                            if fits(flat, continuation, column, options.width.get(), &mut stats)? {
                                Mode::Flat
                            } else {
                                Mode::Broken
                            };
                        push_fast(
                            workspace,
                            FastTask::eval(child, selected, indent, continuation),
                            stats,
                        )?;
                    }
                    PreparedNode::Fill(range) => {
                        let children = data.children(range);
                        let count = if children.is_empty() {
                            0
                        } else {
                            children
                                .len()
                                .checked_mul(2)
                                .and_then(|value| value.checked_sub(1))
                                .ok_or(RenderError::WorkspaceCapacityOverflow {
                                    resource: WorkspaceResource::FastWorkItems,
                                    required: None,
                                    maximum: usize::MAX as u128,
                                    stats,
                                })?
                        };
                        push_fast(workspace, FastTask::complete_seq(count, stats)?, stats)?;
                        if let Some(last) = children.last() {
                            push_fast(
                                workspace,
                                FastTask::eval(*last, mode, indent, continuation),
                                stats,
                            )?;
                            for index in (0..children.len().saturating_sub(1)).rev() {
                                push_fast(
                                    workspace,
                                    FastTask::fill_boundary(children[index + 1], indent),
                                    stats,
                                )?;
                                push_fast(
                                    workspace,
                                    FastTask::eval(
                                        children[index],
                                        mode,
                                        indent,
                                        FitSummary {
                                            columns: 0,
                                            stop: FitStop::Break,
                                        },
                                    ),
                                    stats,
                                )?;
                            }
                        }
                    }
                    PreparedNode::Nest {
                        indent: amount,
                        child,
                    } => {
                        let nested = effective_indent(
                            options,
                            indent
                                .checked_add(amount)
                                .ok_or(RenderError::CostDomainViolation)?,
                        );
                        push_fast(
                            workspace,
                            FastTask::eval(child, mode, nested, continuation),
                            stats,
                        )?;
                    }
                    PreparedNode::Align(child) => {
                        push_fast(
                            workspace,
                            FastTask::eval(
                                child,
                                mode,
                                effective_indent(options, column),
                                continuation,
                            ),
                            stats,
                        )?;
                    }
                    PreparedNode::Choice {
                        preferred,
                        alternative,
                    } => {
                        let summary = data.fit_summaries[preferred.index()];
                        let candidate = if matches!(mode, Mode::Flat) {
                            summary.flat
                        } else {
                            summary.broken
                        };
                        let selected = if fits(
                            candidate,
                            continuation,
                            column,
                            options.width.get(),
                            &mut stats,
                        )? {
                            preferred
                        } else {
                            alternative
                        };
                        push_fast(
                            workspace,
                            FastTask::eval(selected, mode, indent, continuation),
                            stats,
                        )?;
                    }
                    PreparedNode::Annotate { annotation, child } => {
                        push_fast(workspace, FastTask::complete_annotate(annotation), stats)?;
                        push_fast(
                            workspace,
                            FastTask::eval(child, mode, indent, continuation),
                            stats,
                        )?;
                    }
                    PreparedNode::Penalty { amount, child } => {
                        cost = cost
                            .checked_add(model.penalty(amount))
                            .ok_or(RenderError::CostDomainViolation)?;
                        push_fast(workspace, FastTask::complete_penalty(amount), stats)?;
                        push_fast(
                            workspace,
                            FastTask::eval(child, mode, indent, continuation),
                            stats,
                        )?;
                    }
                }
            }
            DecodedFastTask::FillBoundary { next, indent } => {
                let candidate = data.fit_summaries[next.index()].flat;
                let after_space = column
                    .checked_add(1)
                    .ok_or(RenderError::CostDomainViolation)?;
                if fits(
                    candidate,
                    FitSummary {
                        columns: 0,
                        stop: FitStop::End,
                    },
                    after_space,
                    options.width.get(),
                    &mut stats,
                )? {
                    cost = cost
                        .checked_add(
                            model
                                .text(column, 1)
                                .ok_or(RenderError::CostDomainViolation)?,
                        )
                        .ok_or(RenderError::CostDomainViolation)?;
                    column = after_space;
                    let plan = workspace.commit_plan(PendingPlan::Text(data.space_text), stats)?;
                    push_fast_result(workspace, plan, stats)?;
                } else {
                    emit_fast_newline(workspace, model, &mut cost, &mut column, indent, stats)?;
                }
            }
            DecodedFastTask::CompleteSeq { count } => complete_fast_seq(workspace, count, stats)?,
            DecodedFastTask::CompleteAnnotate { annotation } => {
                let child = workspace
                    .fast_results
                    .pop()
                    .expect("annotation child result");
                let plan =
                    workspace.commit_plan(PendingPlan::Annotate { annotation, child }, stats)?;
                push_fast_result(workspace, plan, stats)?;
            }
            DecodedFastTask::CompletePenalty { amount } => {
                let child = workspace.fast_results.pop().expect("penalty child result");
                let plan = workspace.commit_plan(PendingPlan::Penalty { amount, child }, stats)?;
                push_fast_result(workspace, plan, stats)?;
            }
        }
    }
    let root = workspace.fast_results.pop().expect("fast root result");
    stats.plan_nodes = workspace.plans.len();
    Ok(SolvedCore {
        root,
        cost,
        stats,
        options,
    })
}

fn push_fast(
    workspace: &mut RenderWorkspace,
    task: FastTask,
    stats: SolveStats,
) -> Result<(), RenderError> {
    let required =
        workspace
            .fast_work
            .len()
            .checked_add(1)
            .ok_or(RenderError::WorkspaceCapacityOverflow {
                resource: WorkspaceResource::FastWorkItems,
                required: None,
                maximum: usize::MAX as u128,
                stats,
            })?;
    RenderWorkspace::ensure(
        workspace.mode,
        WorkspaceResource::FastWorkItems,
        &mut workspace.capacity.fast.work_items,
        &mut workspace.fast_work,
        required,
        stats,
    )?;
    workspace.fast_work.push(task);
    Ok(())
}

fn push_fast_result(
    workspace: &mut RenderWorkspace,
    plan: PlanId,
    stats: SolveStats,
) -> Result<(), RenderError> {
    let required = workspace.fast_results.len().checked_add(1).ok_or(
        RenderError::WorkspaceCapacityOverflow {
            resource: WorkspaceResource::FastWorkItems,
            required: None,
            maximum: usize::MAX as u128,
            stats,
        },
    )?;
    RenderWorkspace::ensure(
        workspace.mode,
        WorkspaceResource::FastWorkItems,
        &mut workspace.capacity.fast.work_items,
        &mut workspace.fast_results,
        required,
        stats,
    )?;
    workspace.fast_results.push(plan);
    Ok(())
}

fn complete_fast_seq(
    workspace: &mut RenderWorkspace,
    count: usize,
    stats: SolveStats,
) -> Result<(), RenderError> {
    if count == 0 {
        let empty = workspace.commit_plan(PendingPlan::Empty, stats)?;
        return push_fast_result(workspace, empty, stats);
    }
    let start = workspace.fast_results.len() - count;
    let mut root = workspace.fast_results[start];
    for index in start + 1..workspace.fast_results.len() {
        root = workspace.commit_plan(
            PendingPlan::Concat {
                left: root,
                right: workspace.fast_results[index],
            },
            stats,
        )?;
    }
    workspace.fast_results.truncate(start);
    push_fast_result(workspace, root, stats)
}

fn emit_fast_newline(
    workspace: &mut RenderWorkspace,
    model: PreparedConsumerCostModel,
    cost: &mut ConsumerCost,
    column: &mut u32,
    indent: u32,
    stats: SolveStats,
) -> Result<(), RenderError> {
    *cost = cost
        .checked_add(model.newline())
        .and_then(|value| value.checked_add(model.text(0, indent)?))
        .ok_or(RenderError::CostDomainViolation)?;
    *column = indent;
    let plan = workspace.commit_plan(PendingPlan::Newline { indent }, stats)?;
    push_fast_result(workspace, plan, stats)
}

fn fits(
    candidate: FitSummary,
    continuation: FitSummary,
    column: u32,
    width: u32,
    stats: &mut SolveStats,
) -> Result<bool, RenderError> {
    increment(stats, SolveCounter::FitChecks)?;
    if column > width {
        return Ok(false);
    }
    let remaining = width - column;
    if candidate.columns > remaining {
        return Ok(false);
    }
    match candidate.stop {
        FitStop::HardLine => Ok(false),
        FitStop::Break => Ok(true),
        FitStop::End => Ok(continuation.columns <= remaining - candidate.columns),
    }
}

struct ExactSolver<'a, A> {
    data: &'a PreparedData<A>,
    options: RenderOptions,
    model: PreparedConsumerCostModel,
    workspace: &'a mut RenderWorkspace,
    stats: SolveStats,
}

impl<'a, A> ExactSolver<'a, A> {
    fn new(
        data: &'a PreparedData<A>,
        options: RenderOptions,
        workspace: &'a mut RenderWorkspace,
    ) -> Self {
        Self {
            data,
            options,
            model: PreparedConsumerCostModel::new(options.width.get(), options.newline_cost),
            workspace,
            stats: SolveStats::default(),
        }
    }

    fn solve(mut self) -> Result<SolvedCore, RenderError> {
        self.push_exact_task(ExactTask::Eval {
            node: self.data.root,
            column: 0,
            indent: 0,
        })?;
        while let Some(task) = self.workspace.exact_work.pop() {
            self.execute_task(task)?;
        }
        let range = self
            .workspace
            .exact_results
            .pop()
            .expect("exact root result");
        debug_assert!(self.workspace.exact_results.is_empty());
        let best = range
            .indexes()
            .map(|index| self.workspace.retained[index])
            .min_by_key(|candidate| (candidate.cost, candidate.precedence))
            .expect("root frontier");
        self.stats.memo_entries = self.workspace.memo_len;
        self.stats.plan_nodes = self.workspace.plans.len();
        Ok(SolvedCore {
            root: best.plan,
            cost: best.cost,
            stats: self.stats,
            options: self.options,
        })
    }

    fn execute_task(&mut self, task: ExactTask) -> Result<(), RenderError> {
        match task {
            ExactTask::Eval {
                node,
                column,
                indent,
            } => self.begin_node(node, column, indent),
            ExactTask::CompleteAlias { key } => {
                let range = self.pop_exact_result();
                self.complete_node(key, range)
            }
            ExactTask::CompleteUnion { key } => {
                let alternative = self.pop_exact_result();
                let preferred = self.pop_exact_result();
                let range = self.union_ranges(preferred, alternative)?;
                self.complete_node(key, range)
            }
            ExactTask::CompleteChoiceSet { key, result_start } => {
                let result_end = self.workspace.exact_results.len();
                self.begin_builder();
                for result_index in result_start..result_end {
                    let range = self.workspace.exact_results[result_index];
                    for candidate_index in range.indexes() {
                        let candidate = self.workspace.retained[candidate_index];
                        self.present(ScratchCandidate {
                            cost: candidate.cost,
                            last: candidate.last,
                            precedence: 0,
                            plan: PendingPlan::Existing(candidate.plan),
                        })?;
                    }
                }
                let range = self.finish_builder()?;
                self.workspace.exact_results.truncate(result_start);
                self.complete_node(key, range)
            }
            ExactTask::CompleteAnnotate { key, annotation } => {
                let child = self.pop_exact_result();
                let range = self.wrap_range(
                    child,
                    |candidate| PendingPlan::Annotate {
                        annotation,
                        child: candidate.plan,
                    },
                    ConsumerCost::zero(),
                )?;
                self.complete_node(key, range)
            }
            ExactTask::CompletePenalty { key, amount } => {
                let child = self.pop_exact_result();
                let range = self.wrap_range(
                    child,
                    |candidate| PendingPlan::Penalty {
                        amount,
                        child: candidate.plan,
                    },
                    self.model.penalty(amount),
                )?;
                self.complete_node(key, range)
            }
            ExactTask::ContinueSequence {
                key,
                edges,
                child_index,
                indent,
                fill,
            } => {
                let mut accumulated = self.pop_exact_result();
                if child_index >= edges.len {
                    return self.complete_node(key, accumulated);
                }
                if fill {
                    accumulated = self.fill_boundary(accumulated, indent)?;
                }
                let left = self.workspace.retained[accumulated.start as usize];
                let child = self.data.children(edges)[child_index as usize];
                let pending_start = self.workspace.exact_pending.len();
                self.push_exact_task(ExactTask::CompleteSequenceRight {
                    key,
                    edges,
                    child_index,
                    indent,
                    fill,
                    accumulated,
                    left_offset: 0,
                    pending_start,
                })?;
                self.push_exact_task(ExactTask::Eval {
                    node: child,
                    column: left.last,
                    indent,
                })
            }
            ExactTask::CompleteSequenceRight {
                key,
                edges,
                child_index,
                indent,
                fill,
                accumulated,
                left_offset,
                pending_start,
            } => {
                let right = self.pop_exact_result();
                let left =
                    self.workspace.retained[accumulated.start as usize + left_offset as usize];
                for right_index in right.indexes() {
                    let candidate = self.workspace.retained[right_index];
                    self.push_pending(ScratchCandidate {
                        cost: left
                            .cost
                            .checked_add(candidate.cost)
                            .ok_or(RenderError::CostDomainViolation)?,
                        last: candidate.last,
                        precedence: 0,
                        plan: PendingPlan::Concat {
                            left: left.plan,
                            right: candidate.plan,
                        },
                    })?;
                }

                let next_left = left_offset + 1;
                if next_left < accumulated.len {
                    let next =
                        self.workspace.retained[accumulated.start as usize + next_left as usize];
                    let child = self.data.children(edges)[child_index as usize];
                    self.push_exact_task(ExactTask::CompleteSequenceRight {
                        key,
                        edges,
                        child_index,
                        indent,
                        fill,
                        accumulated,
                        left_offset: next_left,
                        pending_start,
                    })?;
                    self.push_exact_task(ExactTask::Eval {
                        node: child,
                        column: next.last,
                        indent,
                    })?;
                    return Ok(());
                }

                self.begin_builder();
                for index in pending_start..self.workspace.exact_pending.len() {
                    let candidate = self.workspace.exact_pending[index];
                    self.present(candidate)?;
                }
                self.workspace.exact_pending.truncate(pending_start);
                let combined = self.finish_builder()?;
                self.push_exact_task(ExactTask::ContinueSequence {
                    key,
                    edges,
                    child_index: child_index + 1,
                    indent,
                    fill,
                })?;
                self.push_exact_result(combined)
            }
        }
    }

    fn begin_node(&mut self, node: NodeId, column: u32, indent: u32) -> Result<(), RenderError> {
        increment(&mut self.stats, SolveCounter::WorkItems)?;
        let key = MemoKey {
            node: node.0,
            column,
            indent,
        };
        if let Some(range) = self.memo_get(key) {
            increment(&mut self.stats, SolveCounter::MemoHits)?;
            return self.push_exact_result(range);
        }
        increment(&mut self.stats, SolveCounter::MemoMisses)?;

        match self.data.nodes[node.index()] {
            PreparedNode::Empty => {
                let range = self.singleton(ConsumerCost::zero(), column, PendingPlan::Empty)?;
                self.complete_node(key, range)
            }
            PreparedNode::Text(text) => {
                let width = self.data.texts[text.index()].display_width;
                let range = self.singleton(
                    self.model
                        .text(column, width)
                        .ok_or(RenderError::CostDomainViolation)?,
                    column
                        .checked_add(width)
                        .ok_or(RenderError::CostDomainViolation)?,
                    PendingPlan::Text(text),
                )?;
                self.complete_node(key, range)
            }
            PreparedNode::Break(_) | PreparedNode::HardLine => {
                let cost = self
                    .model
                    .newline()
                    .checked_add(
                        self.model
                            .text(0, indent)
                            .ok_or(RenderError::CostDomainViolation)?,
                    )
                    .ok_or(RenderError::CostDomainViolation)?;
                let range = self.singleton(cost, indent, PendingPlan::Newline { indent })?;
                self.complete_node(key, range)
            }
            PreparedNode::Seq(edges) | PreparedNode::Fill(edges) => {
                let fill = matches!(self.data.nodes[node.index()], PreparedNode::Fill(_));
                let children = self.data.children(edges);
                let Some(first) = children.first().copied() else {
                    let range = self.singleton(ConsumerCost::zero(), column, PendingPlan::Empty)?;
                    return self.complete_node(key, range);
                };
                self.push_exact_task(ExactTask::ContinueSequence {
                    key,
                    edges,
                    child_index: 1,
                    indent,
                    fill,
                })?;
                self.push_exact_task(ExactTask::Eval {
                    node: first,
                    column,
                    indent,
                })
            }
            PreparedNode::Group(child) => {
                if let Some(flat) = self.data.flat_nodes[child.index()] {
                    self.push_exact_task(ExactTask::CompleteUnion { key })?;
                    self.push_exact_task(ExactTask::Eval {
                        node: child,
                        column,
                        indent,
                    })?;
                    self.push_exact_task(ExactTask::Eval {
                        node: flat,
                        column,
                        indent,
                    })
                } else {
                    self.push_exact_task(ExactTask::CompleteAlias { key })?;
                    self.push_exact_task(ExactTask::Eval {
                        node: child,
                        column,
                        indent,
                    })
                }
            }
            PreparedNode::Nest {
                indent: amount,
                child,
            } => {
                let nested = effective_indent(
                    self.options,
                    indent
                        .checked_add(amount)
                        .ok_or(RenderError::CostDomainViolation)?,
                );
                self.push_exact_task(ExactTask::CompleteAlias { key })?;
                self.push_exact_task(ExactTask::Eval {
                    node: child,
                    column,
                    indent: nested,
                })
            }
            PreparedNode::Align(child) => {
                self.push_exact_task(ExactTask::CompleteAlias { key })?;
                self.push_exact_task(ExactTask::Eval {
                    node: child,
                    column,
                    indent: effective_indent(self.options, column),
                })
            }
            PreparedNode::Choice {
                preferred,
                alternative,
            } => {
                if let Some(choice) = self.data.exact_choices[node.index()] {
                    for _ in 0..choice.skipped_choices {
                        increment(&mut self.stats, SolveCounter::WorkItems)?;
                        increment(&mut self.stats, SolveCounter::MemoMisses)?;
                    }
                    let result_start = self.workspace.exact_results.len();
                    self.push_exact_task(ExactTask::CompleteChoiceSet { key, result_start })?;
                    for alternative in self
                        .data
                        .exact_choice_alternatives(choice.alternatives)
                        .iter()
                        .rev()
                    {
                        self.push_exact_task(ExactTask::Eval {
                            node: *alternative,
                            column,
                            indent,
                        })?;
                    }
                    Ok(())
                } else {
                    self.push_exact_task(ExactTask::CompleteUnion { key })?;
                    self.push_exact_task(ExactTask::Eval {
                        node: alternative,
                        column,
                        indent,
                    })?;
                    self.push_exact_task(ExactTask::Eval {
                        node: preferred,
                        column,
                        indent,
                    })
                }
            }
            PreparedNode::Annotate { annotation, child } => {
                self.push_exact_task(ExactTask::CompleteAnnotate { key, annotation })?;
                self.push_exact_task(ExactTask::Eval {
                    node: child,
                    column,
                    indent,
                })
            }
            PreparedNode::Penalty { amount, child } => {
                self.push_exact_task(ExactTask::CompletePenalty { key, amount })?;
                self.push_exact_task(ExactTask::Eval {
                    node: child,
                    column,
                    indent,
                })
            }
        }
    }

    fn complete_node(&mut self, key: MemoKey, range: CandidateRange) -> Result<(), RenderError> {
        self.memo_insert(key, range)?;
        self.push_exact_result(range)
    }

    fn fill_boundary(
        &mut self,
        range: CandidateRange,
        indent: u32,
    ) -> Result<CandidateRange, RenderError> {
        let pending_start = self.workspace.exact_pending.len();
        for index in range.indexes() {
            let left = self.workspace.retained[index];
            self.begin_builder();
            let space_cost = self
                .model
                .text(left.last, 1)
                .ok_or(RenderError::CostDomainViolation)?;
            self.present(ScratchCandidate {
                cost: space_cost,
                last: left
                    .last
                    .checked_add(1)
                    .ok_or(RenderError::CostDomainViolation)?,
                precedence: 0,
                plan: PendingPlan::Text(self.data.space_text),
            })?;
            let newline_cost = self
                .model
                .newline()
                .checked_add(
                    self.model
                        .text(0, indent)
                        .ok_or(RenderError::CostDomainViolation)?,
                )
                .ok_or(RenderError::CostDomainViolation)?;
            self.present(ScratchCandidate {
                cost: newline_cost,
                last: indent,
                precedence: 0,
                plan: PendingPlan::Newline { indent },
            })?;
            let boundary = self.finish_builder()?;
            for boundary_index in boundary.indexes() {
                let right = self.workspace.retained[boundary_index];
                self.push_pending(ScratchCandidate {
                    cost: left
                        .cost
                        .checked_add(right.cost)
                        .ok_or(RenderError::CostDomainViolation)?,
                    last: right.last,
                    precedence: 0,
                    plan: PendingPlan::Concat {
                        left: left.plan,
                        right: right.plan,
                    },
                })?;
            }
        }
        self.begin_builder();
        for index in pending_start..self.workspace.exact_pending.len() {
            let candidate = self.workspace.exact_pending[index];
            self.present(candidate)?;
        }
        self.workspace.exact_pending.truncate(pending_start);
        self.finish_builder()
    }

    fn singleton(
        &mut self,
        cost: ConsumerCost,
        last: u32,
        plan: PendingPlan,
    ) -> Result<CandidateRange, RenderError> {
        self.begin_builder();
        self.present(ScratchCandidate {
            cost,
            last,
            precedence: 0,
            plan,
        })?;
        self.finish_builder()
    }

    fn union_ranges(
        &mut self,
        preferred: CandidateRange,
        alternative: CandidateRange,
    ) -> Result<CandidateRange, RenderError> {
        self.begin_builder();
        for range in [preferred, alternative] {
            for index in range.indexes() {
                let candidate = self.workspace.retained[index];
                self.present(ScratchCandidate {
                    cost: candidate.cost,
                    last: candidate.last,
                    precedence: 0,
                    plan: PendingPlan::Existing(candidate.plan),
                })?;
            }
        }
        self.finish_builder()
    }

    fn wrap_range(
        &mut self,
        range: CandidateRange,
        wrapper: impl Fn(Candidate) -> PendingPlan,
        additional: ConsumerCost,
    ) -> Result<CandidateRange, RenderError> {
        self.begin_builder();
        for index in range.indexes() {
            let candidate = self.workspace.retained[index];
            self.present(ScratchCandidate {
                cost: candidate
                    .cost
                    .checked_add(additional)
                    .ok_or(RenderError::CostDomainViolation)?,
                last: candidate.last,
                precedence: 0,
                plan: wrapper(candidate),
            })?;
        }
        self.finish_builder()
    }

    fn push_exact_task(&mut self, task: ExactTask) -> Result<(), RenderError> {
        let required = self.workspace.exact_work.len().checked_add(1).ok_or(
            RenderError::WorkspaceCapacityOverflow {
                resource: WorkspaceResource::ExactWorkItems,
                required: None,
                maximum: usize::MAX as u128,
                stats: self.stats,
            },
        )?;
        RenderWorkspace::ensure(
            self.workspace.mode,
            WorkspaceResource::ExactWorkItems,
            &mut self.workspace.capacity.exact.work_items,
            &mut self.workspace.exact_work,
            required,
            self.stats,
        )?;
        self.workspace.exact_work.push(task);
        Ok(())
    }

    fn push_exact_result(&mut self, range: CandidateRange) -> Result<(), RenderError> {
        let required = self.workspace.exact_results.len().checked_add(1).ok_or(
            RenderError::WorkspaceCapacityOverflow {
                resource: WorkspaceResource::ExactWorkItems,
                required: None,
                maximum: usize::MAX as u128,
                stats: self.stats,
            },
        )?;
        RenderWorkspace::ensure(
            self.workspace.mode,
            WorkspaceResource::ExactWorkItems,
            &mut self.workspace.capacity.exact.work_items,
            &mut self.workspace.exact_results,
            required,
            self.stats,
        )?;
        self.workspace.exact_results.push(range);
        Ok(())
    }

    fn pop_exact_result(&mut self) -> CandidateRange {
        self.workspace
            .exact_results
            .pop()
            .expect("exact continuation result")
    }

    fn push_pending(&mut self, candidate: ScratchCandidate) -> Result<(), RenderError> {
        let required = self.workspace.exact_pending.len().checked_add(1).ok_or(
            RenderError::WorkspaceCapacityOverflow {
                resource: WorkspaceResource::ExactWorkItems,
                required: None,
                maximum: usize::MAX as u128,
                stats: self.stats,
            },
        )?;
        RenderWorkspace::ensure(
            self.workspace.mode,
            WorkspaceResource::ExactWorkItems,
            &mut self.workspace.capacity.exact.work_items,
            &mut self.workspace.exact_pending,
            required,
            self.stats,
        )?;
        self.workspace.exact_pending.push(candidate);
        Ok(())
    }

    fn begin_builder(&mut self) {
        self.workspace.scratch.clear();
        self.workspace.scratch_root = None;
        self.workspace.scratch_head = None;
        self.workspace.scratch_tail = None;
        self.workspace.scratch_free = None;
        self.workspace.scratch_live = 0;
    }

    fn present(&mut self, mut candidate: ScratchCandidate) -> Result<(), RenderError> {
        let generated_before = self.stats.candidates_generated;
        let work_required =
            self.stats
                .work_items
                .checked_add(1)
                .ok_or(RenderError::StatisticsOverflow {
                    counter: SolveCounter::WorkItems,
                    required: u128::from(self.stats.work_items) + 1,
                    maximum: u64::MAX,
                    stats: self.stats,
                })?;
        let generated_required = self.stats.candidates_generated.checked_add(1).ok_or(
            RenderError::StatisticsOverflow {
                counter: SolveCounter::CandidatesGenerated,
                required: u128::from(self.stats.candidates_generated) + 1,
                maximum: u64::MAX,
                stats: self.stats,
            },
        )?;
        candidate.precedence = generated_before;
        self.stats.work_items = work_required;
        self.stats.candidates_generated = generated_required;
        let (same, predecessor, lower_bound) = frontier_neighbors(
            &self.workspace.scratch,
            self.workspace.scratch_root,
            candidate.last,
        );
        let dominated = same.is_some_and(|slot| {
            self.workspace.scratch[slot as usize].candidate.cost <= candidate.cost
        }) || predecessor.is_some_and(|slot| {
            self.workspace.scratch[slot as usize].candidate.cost < candidate.cost
        });
        if dominated {
            add_pruned(&mut self.stats, 1)?;
            return Ok(());
        }

        let mut removed = 0usize;
        let mut cursor = lower_bound;
        while let Some(slot) = cursor {
            let previous = self.workspace.scratch[slot as usize].candidate;
            if candidate.cost >= previous.cost {
                break;
            }
            removed = removed
                .checked_add(1)
                .ok_or(RenderError::WorkspaceCapacityOverflow {
                    resource: WorkspaceResource::FrontierScratch,
                    required: None,
                    maximum: u32::MAX as u128,
                    stats: self.stats,
                })?;
            cursor = frontier_successor(
                &self.workspace.scratch,
                self.workspace.scratch_root,
                previous.last,
            );
        }
        add_pruned(&mut self.stats, removed)?;

        if removed == 0 && self.workspace.scratch_free.is_none() {
            let required = self.workspace.scratch.len().checked_add(1).ok_or(
                RenderError::WorkspaceCapacityOverflow {
                    resource: WorkspaceResource::FrontierScratch,
                    required: None,
                    maximum: u32::MAX as u128,
                    stats: self.stats,
                },
            )?;
            if required > u32::MAX as usize {
                return Err(RenderError::WorkspaceCapacityOverflow {
                    resource: WorkspaceResource::FrontierScratch,
                    required: Some(required as u128),
                    maximum: u32::MAX as u128,
                    stats: self.stats,
                });
            }
            RenderWorkspace::ensure(
                self.workspace.mode,
                WorkspaceResource::FrontierScratch,
                &mut self.workspace.capacity.exact.frontier_scratch,
                &mut self.workspace.scratch,
                required,
                self.stats,
            )?;
        }

        for _ in 0..removed {
            let slot = frontier_lower_bound(
                &self.workspace.scratch,
                self.workspace.scratch_root,
                candidate.last,
            )
            .expect("preflighted dominated successor");
            let key = self.workspace.scratch[slot as usize].candidate.last;
            let (root, removed_slot) = frontier_remove(
                &mut self.workspace.scratch,
                self.workspace.scratch_root,
                key,
            );
            debug_assert_eq!(removed_slot, Some(slot));
            self.workspace.scratch_root = root;
            frontier_unlink_precedence(self.workspace, slot);
            self.workspace.scratch[slot as usize].live = false;
            self.workspace.scratch[slot as usize].free_next = self.workspace.scratch_free;
            self.workspace.scratch_free = Some(slot);
            self.workspace.scratch_live -= 1;
        }

        let slot = if let Some(slot) = self.workspace.scratch_free {
            self.workspace.scratch_free = self.workspace.scratch[slot as usize].free_next;
            self.workspace.scratch[slot as usize] = FrontierSlot::new(candidate);
            slot
        } else {
            let slot = u32::try_from(self.workspace.scratch.len()).map_err(|_| {
                RenderError::WorkspaceCapacityOverflow {
                    resource: WorkspaceResource::FrontierScratch,
                    required: Some(self.workspace.scratch.len() as u128 + 1),
                    maximum: u32::MAX as u128,
                    stats: self.stats,
                }
            })?;
            self.workspace.scratch.push(FrontierSlot::new(candidate));
            slot
        };
        frontier_append_precedence(self.workspace, slot);
        self.workspace.scratch_root = Some(frontier_insert(
            &mut self.workspace.scratch,
            self.workspace.scratch_root,
            slot,
        ));
        self.workspace.scratch_live += 1;
        Ok(())
    }

    fn finish_builder(&mut self) -> Result<CandidateRange, RenderError> {
        let mut additional_plans = 0usize;
        let mut cursor = self.workspace.scratch_head;
        while let Some(slot) = cursor {
            let value = &self.workspace.scratch[slot as usize];
            if !matches!(value.candidate.plan, PendingPlan::Existing(_)) {
                additional_plans = additional_plans.checked_add(1).ok_or(
                    RenderError::WorkspaceCapacityOverflow {
                        resource: WorkspaceResource::PlanNodes,
                        required: None,
                        maximum: u32::MAX as u128,
                        stats: self.stats,
                    },
                )?;
            }
            cursor = value.precedence_next;
        }
        self.workspace.ensure_plan(additional_plans, self.stats)?;
        let required = self
            .workspace
            .retained
            .len()
            .checked_add(self.workspace.scratch_live)
            .ok_or(RenderError::WorkspaceCapacityOverflow {
                resource: WorkspaceResource::RetainedCandidates,
                required: None,
                maximum: u32::MAX as u128,
                stats: self.stats,
            })?;
        if required > u32::MAX as usize {
            return Err(RenderError::WorkspaceCapacityOverflow {
                resource: WorkspaceResource::RetainedCandidates,
                required: Some(required as u128),
                maximum: u32::MAX as u128,
                stats: self.stats,
            });
        }
        RenderWorkspace::ensure(
            self.workspace.mode,
            WorkspaceResource::RetainedCandidates,
            &mut self.workspace.capacity.exact.retained_candidates,
            &mut self.workspace.retained,
            required,
            self.stats,
        )?;
        let start = self.workspace.retained.len();
        let mut cursor = self.workspace.scratch_head;
        while let Some(slot) = cursor {
            let value = &self.workspace.scratch[slot as usize];
            let candidate = value.candidate;
            cursor = value.precedence_next;
            let plan = self.workspace.commit_plan(candidate.plan, self.stats)?;
            self.workspace.retained.push(Candidate {
                cost: candidate.cost,
                last: candidate.last,
                precedence: candidate.precedence,
                plan,
            });
        }
        self.stats.peak_frontier = self.stats.peak_frontier.max(self.workspace.scratch_live);
        Ok(CandidateRange {
            start: start as u32,
            len: self.workspace.scratch_live as u32,
        })
    }

    fn memo_get(&self, key: MemoKey) -> Option<CandidateRange> {
        if self.workspace.memo_slots.is_empty() {
            return None;
        }
        let mut index = memo_hash(key) as usize & (self.workspace.memo_slots.len() - 1);
        loop {
            let slot = self.workspace.memo_slots[index];
            if slot.generation != self.workspace.memo_generation {
                return None;
            }
            if slot.key == key {
                return Some(slot.range);
            }
            index = (index + 1) & (self.workspace.memo_slots.len() - 1);
        }
    }

    fn memo_insert(&mut self, key: MemoKey, range: CandidateRange) -> Result<(), RenderError> {
        let required = self.workspace.memo_len.checked_add(1).ok_or(
            RenderError::WorkspaceCapacityOverflow {
                resource: WorkspaceResource::MemoEntries,
                required: None,
                maximum: usize::MAX as u128,
                stats: self.stats,
            },
        )?;
        if required > self.workspace.capacity.exact.memo_entries {
            if self.workspace.mode == WorkspaceMode::Fixed {
                return Err(RenderError::WorkspaceExhausted {
                    resource: WorkspaceResource::MemoEntries,
                    capacity: self.workspace.capacity.exact.memo_entries,
                    required,
                    stats: self.stats,
                });
            }
            self.grow_memo(required)?;
        }
        if self.workspace.memo_slots.is_empty() {
            self.grow_memo(required)?;
        }
        let mut index = memo_hash(key) as usize & (self.workspace.memo_slots.len() - 1);
        loop {
            if self.workspace.memo_slots[index].generation != self.workspace.memo_generation {
                self.workspace.memo_slots[index] = MemoSlot {
                    generation: self.workspace.memo_generation,
                    key,
                    range,
                };
                self.workspace.memo_len += 1;
                self.stats.memo_entries = self.workspace.memo_len;
                return Ok(());
            }
            if self.workspace.memo_slots[index].key == key {
                self.workspace.memo_slots[index].range = range;
                return Ok(());
            }
            index = (index + 1) & (self.workspace.memo_slots.len() - 1);
        }
    }

    fn grow_memo(&mut self, required: usize) -> Result<(), RenderError> {
        let slots =
            memo_slots_for(required).map_err(|_| RenderError::WorkspaceCapacityOverflow {
                resource: WorkspaceResource::MemoEntries,
                required: Some(required as u128),
                maximum: usize::MAX as u128,
                stats: self.stats,
            })?;
        if self.workspace.memo_slots.len() >= slots {
            self.workspace.capacity.exact.memo_entries = required;
            return Ok(());
        }
        let mut replacement = Vec::new();
        replacement.try_reserve_exact(slots).map_err(|source| {
            RenderError::WorkspaceGrowthFailed {
                resource: WorkspaceResource::MemoEntries,
                capacity: self.workspace.capacity.exact.memo_entries,
                required,
                stats: self.stats,
                source,
            }
        })?;
        replacement.resize(slots, MemoSlot::default());
        for slot in self
            .workspace
            .memo_slots
            .iter()
            .copied()
            .filter(|slot| slot.generation == self.workspace.memo_generation)
        {
            let mut index = memo_hash(slot.key) as usize & (slots - 1);
            while replacement[index].generation == self.workspace.memo_generation {
                index = (index + 1) & (slots - 1);
            }
            replacement[index] = slot;
        }
        self.workspace.memo_slots = replacement;
        self.workspace.capacity.exact.memo_entries = required;
        Ok(())
    }
}

fn frontier_height(slots: &[FrontierSlot], slot: Option<u32>) -> i32 {
    slot.map_or(0, |slot| i32::from(slots[slot as usize].height))
}

fn frontier_update_height(slots: &mut [FrontierSlot], slot: u32) {
    let left = frontier_height(slots, slots[slot as usize].left);
    let right = frontier_height(slots, slots[slot as usize].right);
    slots[slot as usize].height =
        u16::try_from(1 + left.max(right)).expect("AVL height is bounded by compact frontier size");
}

fn frontier_rotate_left(slots: &mut [FrontierSlot], root: u32) -> u32 {
    let pivot = slots[root as usize]
        .right
        .expect("left rotation has a right child");
    let parent = slots[root as usize].parent;
    let transfer = slots[pivot as usize].left;
    slots[root as usize].right = transfer;
    if let Some(transfer) = transfer {
        slots[transfer as usize].parent = Some(root);
    }
    slots[pivot as usize].left = Some(root);
    slots[pivot as usize].parent = parent;
    slots[root as usize].parent = Some(pivot);
    frontier_update_height(slots, root);
    frontier_update_height(slots, pivot);
    pivot
}

fn frontier_rotate_right(slots: &mut [FrontierSlot], root: u32) -> u32 {
    let pivot = slots[root as usize]
        .left
        .expect("right rotation has a left child");
    let parent = slots[root as usize].parent;
    let transfer = slots[pivot as usize].right;
    slots[root as usize].left = transfer;
    if let Some(transfer) = transfer {
        slots[transfer as usize].parent = Some(root);
    }
    slots[pivot as usize].right = Some(root);
    slots[pivot as usize].parent = parent;
    slots[root as usize].parent = Some(pivot);
    frontier_update_height(slots, root);
    frontier_update_height(slots, pivot);
    pivot
}

fn frontier_rebalance(slots: &mut [FrontierSlot], root: u32) -> u32 {
    frontier_update_height(slots, root);
    let left = slots[root as usize].left;
    let right = slots[root as usize].right;
    let balance = frontier_height(slots, left) - frontier_height(slots, right);
    if balance > 1 {
        let left = left.expect("left-heavy AVL node has a left child");
        if frontier_height(slots, slots[left as usize].left)
            < frontier_height(slots, slots[left as usize].right)
        {
            let replacement = frontier_rotate_left(slots, left);
            slots[root as usize].left = Some(replacement);
            slots[replacement as usize].parent = Some(root);
        }
        return frontier_rotate_right(slots, root);
    }
    if balance < -1 {
        let right = right.expect("right-heavy AVL node has a right child");
        if frontier_height(slots, slots[right as usize].right)
            < frontier_height(slots, slots[right as usize].left)
        {
            let replacement = frontier_rotate_right(slots, right);
            slots[root as usize].right = Some(replacement);
            slots[replacement as usize].parent = Some(root);
        }
        return frontier_rotate_left(slots, root);
    }
    root
}

fn frontier_insert(slots: &mut [FrontierSlot], root: Option<u32>, inserted: u32) -> u32 {
    let Some(root) = root else {
        slots[inserted as usize].parent = None;
        return inserted;
    };
    let root_key = slots[root as usize].candidate.last;
    let inserted_key = slots[inserted as usize].candidate.last;
    if inserted_key < root_key {
        let child = frontier_insert(slots, slots[root as usize].left, inserted);
        slots[root as usize].left = Some(child);
        slots[child as usize].parent = Some(root);
    } else {
        debug_assert!(inserted_key > root_key);
        let child = frontier_insert(slots, slots[root as usize].right, inserted);
        slots[root as usize].right = Some(child);
        slots[child as usize].parent = Some(root);
    }
    let parent = slots[root as usize].parent;
    let root = frontier_rebalance(slots, root);
    slots[root as usize].parent = parent;
    root
}

fn frontier_detach_min(slots: &mut [FrontierSlot], root: u32) -> (Option<u32>, u32) {
    let parent = slots[root as usize].parent;
    let Some(left) = slots[root as usize].left else {
        let remainder = slots[root as usize].right;
        if let Some(remainder) = remainder {
            slots[remainder as usize].parent = parent;
        }
        slots[root as usize].right = None;
        slots[root as usize].parent = None;
        return (remainder, root);
    };
    let (replacement, minimum) = frontier_detach_min(slots, left);
    slots[root as usize].left = replacement;
    if let Some(replacement) = replacement {
        slots[replacement as usize].parent = Some(root);
    }
    let root = frontier_rebalance(slots, root);
    slots[root as usize].parent = parent;
    (Some(root), minimum)
}

fn frontier_remove(
    slots: &mut [FrontierSlot],
    root: Option<u32>,
    key: u32,
) -> (Option<u32>, Option<u32>) {
    let Some(root) = root else {
        return (None, None);
    };
    let parent = slots[root as usize].parent;
    let root_key = slots[root as usize].candidate.last;
    if key < root_key {
        let (replacement, removed) = frontier_remove(slots, slots[root as usize].left, key);
        if removed.is_none() {
            return (Some(root), None);
        }
        slots[root as usize].left = replacement;
        if let Some(replacement) = replacement {
            slots[replacement as usize].parent = Some(root);
        }
        let root = frontier_rebalance(slots, root);
        slots[root as usize].parent = parent;
        return (Some(root), removed);
    }
    if key > root_key {
        let (replacement, removed) = frontier_remove(slots, slots[root as usize].right, key);
        if removed.is_none() {
            return (Some(root), None);
        }
        slots[root as usize].right = replacement;
        if let Some(replacement) = replacement {
            slots[replacement as usize].parent = Some(root);
        }
        let root = frontier_rebalance(slots, root);
        slots[root as usize].parent = parent;
        return (Some(root), removed);
    }

    let left = slots[root as usize].left;
    let right = slots[root as usize].right;
    let replacement = match (left, right) {
        (None, replacement) | (replacement, None) => {
            if let Some(replacement) = replacement {
                slots[replacement as usize].parent = parent;
            }
            replacement
        }
        (Some(left), Some(right)) => {
            let (new_right, successor) = frontier_detach_min(slots, right);
            slots[successor as usize].left = Some(left);
            slots[left as usize].parent = Some(successor);
            slots[successor as usize].right = new_right;
            if let Some(new_right) = new_right {
                slots[new_right as usize].parent = Some(successor);
            }
            slots[successor as usize].parent = parent;
            let successor = frontier_rebalance(slots, successor);
            slots[successor as usize].parent = parent;
            Some(successor)
        }
    };
    slots[root as usize].left = None;
    slots[root as usize].right = None;
    slots[root as usize].parent = None;
    slots[root as usize].height = 1;
    (replacement, Some(root))
}

fn frontier_neighbors(
    slots: &[FrontierSlot],
    root: Option<u32>,
    key: u32,
) -> (Option<u32>, Option<u32>, Option<u32>) {
    let mut current = root;
    let mut same = None;
    let mut predecessor = None;
    let mut lower_bound = None;
    while let Some(slot) = current {
        let slot_key = slots[slot as usize].candidate.last;
        if key < slot_key {
            lower_bound = Some(slot);
            current = slots[slot as usize].left;
        } else if key > slot_key {
            predecessor = Some(slot);
            current = slots[slot as usize].right;
        } else {
            same = Some(slot);
            lower_bound = Some(slot);
            break;
        }
    }
    (same, predecessor, lower_bound)
}

fn frontier_lower_bound(slots: &[FrontierSlot], root: Option<u32>, key: u32) -> Option<u32> {
    frontier_neighbors(slots, root, key).2
}

fn frontier_successor(slots: &[FrontierSlot], root: Option<u32>, key: u32) -> Option<u32> {
    let mut current = root;
    let mut successor = None;
    while let Some(slot) = current {
        if slots[slot as usize].candidate.last > key {
            successor = Some(slot);
            current = slots[slot as usize].left;
        } else {
            current = slots[slot as usize].right;
        }
    }
    successor
}

fn frontier_unlink_precedence(workspace: &mut RenderWorkspace, slot: u32) {
    let previous = workspace.scratch[slot as usize].precedence_previous;
    let next = workspace.scratch[slot as usize].precedence_next;
    if let Some(previous) = previous {
        workspace.scratch[previous as usize].precedence_next = next;
    } else {
        workspace.scratch_head = next;
    }
    if let Some(next) = next {
        workspace.scratch[next as usize].precedence_previous = previous;
    } else {
        workspace.scratch_tail = previous;
    }
    workspace.scratch[slot as usize].precedence_previous = None;
    workspace.scratch[slot as usize].precedence_next = None;
}

fn frontier_append_precedence(workspace: &mut RenderWorkspace, slot: u32) {
    let previous = workspace.scratch_tail;
    workspace.scratch[slot as usize].precedence_previous = previous;
    workspace.scratch[slot as usize].precedence_next = None;
    if let Some(previous) = previous {
        workspace.scratch[previous as usize].precedence_next = Some(slot);
    } else {
        workspace.scratch_head = Some(slot);
    }
    workspace.scratch_tail = Some(slot);
}

fn memo_hash(key: MemoKey) -> u64 {
    fn mix(value: u64) -> u64 {
        let mut value = value.wrapping_add(0x9e3779b97f4a7c15);
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^ (value >> 31)
    }
    mix(((u64::from(key.node) << 32) | u64::from(key.column)) ^ mix(u64::from(key.indent)))
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SpanId(u32);

impl SpanId {
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnnotationSpan {
    pub annotation: AnnotationId,
    pub range: Range<usize>,
    pub parent: Option<SpanId>,
}

pub struct ActiveAnnotations<'a, A> {
    ids: &'a [AnnotationId],
    data: &'a PreparedData<A>,
}

impl<'a, A> ActiveAnnotations<'a, A> {
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (AnnotationId, &'a A)> + '_ {
        self.ids
            .iter()
            .copied()
            .map(|id| (id, self.data.annotations[id.0 as usize].as_ref()))
    }

    pub fn innermost(&self) -> Option<(AnnotationId, &'a A)> {
        let id = *self.ids.last()?;
        Some((id, self.data.annotations[id.0 as usize].as_ref()))
    }
}

pub trait LayoutVisitor<A> {
    type Error;
    fn enter_annotation(&mut self, _id: AnnotationId, _annotation: &A) -> Result<(), Self::Error> {
        Ok(())
    }
    fn exit_annotation(&mut self, _id: AnnotationId, _annotation: &A) -> Result<(), Self::Error> {
        Ok(())
    }
    fn penalty(&mut self, _amount: u32) -> Result<(), Self::Error> {
        Ok(())
    }
    fn text(
        &mut self,
        text: &str,
        annotations: ActiveAnnotations<'_, A>,
    ) -> Result<(), Self::Error>;
    fn newline(
        &mut self,
        indent: u32,
        annotations: ActiveAnnotations<'_, A>,
    ) -> Result<(), Self::Error>;
}

#[derive(Debug)]
pub enum VisitError<E> {
    Workspace(RenderError),
    Visitor(E),
}

impl<E: fmt::Display> fmt::Display for VisitError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workspace(error) => error.fmt(formatter),
            Self::Visitor(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for VisitError<E> {}

pub struct LayoutRef<'p, 'w, A> {
    prepared: &'p PreparedDoc<A>,
    workspace: &'w mut RenderWorkspace,
    core: SolvedCore,
    active: bool,
}

impl<A> LayoutRef<'_, '_, A> {
    pub const fn cost(&self) -> ConsumerCost {
        self.core.cost
    }
    pub const fn stats(&self) -> SolveStats {
        self.core.stats
    }

    #[allow(clippy::result_large_err)]
    pub fn visit<V: LayoutVisitor<A>>(
        mut self,
        visitor: &mut V,
    ) -> Result<(), VisitError<V::Error>> {
        let result = visit_plan(&self.prepared.0, self.workspace, &self.core, visitor);
        self.workspace.reset_operation();
        self.active = false;
        result
    }
}

impl<A> Drop for LayoutRef<'_, '_, A> {
    fn drop(&mut self) {
        if self.active {
            self.workspace.reset_operation();
        }
    }
}

pub fn solve_into<'p, 'w, A>(
    prepared: &'p PreparedDoc<A>,
    options: RenderOptions,
    workspace: &'w mut RenderWorkspace,
) -> Result<LayoutRef<'p, 'w, A>, RenderError> {
    let core = solve_core(prepared, options, workspace)?;
    Ok(LayoutRef {
        prepared,
        workspace,
        core,
        active: true,
    })
}

#[allow(clippy::result_large_err)]
fn visit_plan<A, V: LayoutVisitor<A>>(
    data: &PreparedData<A>,
    workspace: &mut RenderWorkspace,
    core: &SolvedCore,
    visitor: &mut V,
) -> Result<(), VisitError<V::Error>> {
    workspace.visit_work.clear();
    workspace.annotations.clear();
    push_visit(workspace, VisitWork::Plan(core.root), core.stats).map_err(VisitError::Workspace)?;
    let model = PreparedConsumerCostModel::new(core.options.width.get(), core.options.newline_cost);
    let mut cost = ConsumerCost::zero();
    let mut column = 0u32;
    while let Some(work) = workspace.visit_work.pop() {
        match work {
            VisitWork::Plan(id) => match workspace.plan(id) {
                PlanNode::Empty => {}
                PlanNode::Text(text) => {
                    let text = &data.texts[text.index()];
                    cost = cost
                        .checked_add(
                            model
                                .text(column, text.display_width)
                                .ok_or(VisitError::Workspace(RenderError::CostDomainViolation))?,
                        )
                        .ok_or(VisitError::Workspace(RenderError::CostDomainViolation))?;
                    column = column
                        .checked_add(text.display_width)
                        .ok_or(VisitError::Workspace(RenderError::CostDomainViolation))?;
                    if !text.value.is_empty() {
                        visitor
                            .text(
                                &text.value,
                                ActiveAnnotations {
                                    ids: &workspace.annotations,
                                    data,
                                },
                            )
                            .map_err(VisitError::Visitor)?;
                    }
                }
                PlanNode::Newline { indent } => {
                    cost = cost
                        .checked_add(model.newline())
                        .and_then(|value| value.checked_add(model.text(0, indent)?))
                        .ok_or(VisitError::Workspace(RenderError::CostDomainViolation))?;
                    column = indent;
                    visitor
                        .newline(
                            indent,
                            ActiveAnnotations {
                                ids: &workspace.annotations,
                                data,
                            },
                        )
                        .map_err(VisitError::Visitor)?;
                }
                PlanNode::Concat { left, right } => {
                    push_visit(workspace, VisitWork::Plan(right), core.stats)
                        .map_err(VisitError::Workspace)?;
                    push_visit(workspace, VisitWork::Plan(left), core.stats)
                        .map_err(VisitError::Workspace)?;
                }
                PlanNode::Annotate { annotation, child } => {
                    visitor
                        .enter_annotation(annotation, &data.annotations[annotation.0 as usize])
                        .map_err(VisitError::Visitor)?;
                    push_annotation(workspace, annotation, core.stats)
                        .map_err(VisitError::Workspace)?;
                    push_visit(
                        workspace,
                        VisitWork::ExitAnnotation { annotation },
                        core.stats,
                    )
                    .map_err(VisitError::Workspace)?;
                    push_visit(workspace, VisitWork::Plan(child), core.stats)
                        .map_err(VisitError::Workspace)?;
                }
                PlanNode::Penalty { amount, child } => {
                    cost = cost
                        .checked_add(model.penalty(amount))
                        .ok_or(VisitError::Workspace(RenderError::CostDomainViolation))?;
                    visitor.penalty(amount).map_err(VisitError::Visitor)?;
                    push_visit(workspace, VisitWork::Plan(child), core.stats)
                        .map_err(VisitError::Workspace)?;
                }
            },
            VisitWork::ExitAnnotation { annotation } => {
                let popped = workspace.annotations.pop();
                debug_assert_eq!(popped, Some(annotation));
                visitor
                    .exit_annotation(annotation, &data.annotations[annotation.0 as usize])
                    .map_err(VisitError::Visitor)?;
            }
        }
    }
    if cost != core.cost {
        return Err(VisitError::Workspace(RenderError::CostWitnessMismatch));
    }
    Ok(())
}

fn push_visit(
    workspace: &mut RenderWorkspace,
    work: VisitWork,
    stats: SolveStats,
) -> Result<(), RenderError> {
    let required = workspace.visit_work.len().checked_add(1).ok_or(
        RenderError::WorkspaceCapacityOverflow {
            resource: WorkspaceResource::VisitWorkItems,
            required: None,
            maximum: usize::MAX as u128,
            stats,
        },
    )?;
    RenderWorkspace::ensure(
        workspace.mode,
        WorkspaceResource::VisitWorkItems,
        &mut workspace.capacity.visit.work_items,
        &mut workspace.visit_work,
        required,
        stats,
    )?;
    workspace.visit_work.push(work);
    Ok(())
}

fn push_annotation(
    workspace: &mut RenderWorkspace,
    annotation: AnnotationId,
    stats: SolveStats,
) -> Result<(), RenderError> {
    let required = workspace.annotations.len().checked_add(1).ok_or(
        RenderError::WorkspaceCapacityOverflow {
            resource: WorkspaceResource::AnnotationDepth,
            required: None,
            maximum: usize::MAX as u128,
            stats,
        },
    )?;
    RenderWorkspace::ensure(
        workspace.mode,
        WorkspaceResource::AnnotationDepth,
        &mut workspace.capacity.visit.annotation_depth,
        &mut workspace.annotations,
        required,
        stats,
    )?;
    workspace.annotations.push(annotation);
    Ok(())
}

pub struct RenderedRef<'p, 'w, A> {
    prepared: &'p PreparedDoc<A>,
    workspace: &'w mut RenderWorkspace,
    core: SolvedCore,
    active: bool,
}

impl<A> RenderedRef<'_, '_, A> {
    pub fn text(&self) -> &str {
        std::str::from_utf8(&self.workspace.output).expect("prepared text remains UTF-8")
    }
    pub fn spans(&self) -> &[AnnotationSpan] {
        &self.workspace.spans
    }
    pub fn resolved_spans(&self) -> ResolvedSpans<'_, A> {
        ResolvedSpans {
            spans: self.workspace.spans.iter(),
            data: &self.prepared.0,
        }
    }
    pub const fn cost(&self) -> ConsumerCost {
        self.core.cost
    }
    pub const fn stats(&self) -> SolveStats {
        self.core.stats
    }
}

impl<A> Drop for RenderedRef<'_, '_, A> {
    fn drop(&mut self) {
        if self.active {
            self.workspace.reset_operation();
            self.active = false;
        }
    }
}

pub struct ResolvedSpans<'a, A> {
    spans: std::slice::Iter<'a, AnnotationSpan>,
    data: &'a PreparedData<A>,
}

impl<'a, A> Iterator for ResolvedSpans<'a, A> {
    type Item = (&'a AnnotationSpan, &'a A);
    fn next(&mut self) -> Option<Self::Item> {
        let span = self.spans.next()?;
        Some((
            span,
            self.data.annotations[span.annotation.0 as usize].as_ref(),
        ))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.spans.size_hint()
    }
}

impl<A> ExactSizeIterator for ResolvedSpans<'_, A> {}

pub fn render_into<'p, 'w, A>(
    prepared: &'p PreparedDoc<A>,
    options: RenderOptions,
    workspace: &'w mut RenderWorkspace,
) -> Result<RenderedRef<'p, 'w, A>, RenderError> {
    let core = solve_core(prepared, options, workspace)?;
    if let Err(error) = materialize(&prepared.0, workspace, &core) {
        workspace.reset_operation();
        return Err(error);
    }
    Ok(RenderedRef {
        prepared,
        workspace,
        core,
        active: true,
    })
}

fn materialize<A>(
    data: &PreparedData<A>,
    workspace: &mut RenderWorkspace,
    core: &SolvedCore,
) -> Result<(), RenderError> {
    workspace.visit_work.clear();
    workspace.annotations.clear();
    workspace.open_spans.clear();
    workspace.output.clear();
    workspace.spans.clear();
    push_visit(workspace, VisitWork::Plan(core.root), core.stats)?;
    let model = PreparedConsumerCostModel::new(core.options.width.get(), core.options.newline_cost);
    let mut cost = ConsumerCost::zero();
    let mut column = 0u32;
    while let Some(work) = workspace.visit_work.pop() {
        match work {
            VisitWork::Plan(id) => match workspace.plan(id) {
                PlanNode::Empty => {}
                PlanNode::Text(text) => {
                    let text = &data.texts[text.index()];
                    cost = cost
                        .checked_add(
                            model
                                .text(column, text.display_width)
                                .ok_or(RenderError::CostDomainViolation)?,
                        )
                        .ok_or(RenderError::CostDomainViolation)?;
                    column = column
                        .checked_add(text.display_width)
                        .ok_or(RenderError::CostDomainViolation)?;
                    append_output(workspace, text.value.as_bytes(), core.stats)?;
                }
                PlanNode::Newline { indent } => {
                    cost = cost
                        .checked_add(model.newline())
                        .and_then(|value| value.checked_add(model.text(0, indent)?))
                        .ok_or(RenderError::CostDomainViolation)?;
                    column = indent;
                    append_output(workspace, b"\n", core.stats)?;
                    for _ in 0..indent {
                        append_output(workspace, b" ", core.stats)?;
                    }
                }
                PlanNode::Concat { left, right } => {
                    push_visit(workspace, VisitWork::Plan(right), core.stats)?;
                    push_visit(workspace, VisitWork::Plan(left), core.stats)?;
                }
                PlanNode::Annotate { annotation, child } => {
                    let required = workspace.spans.len().checked_add(1).ok_or(
                        RenderError::WorkspaceCapacityOverflow {
                            resource: WorkspaceResource::Spans,
                            required: None,
                            maximum: u32::MAX as u128,
                            stats: core.stats,
                        },
                    )?;
                    RenderWorkspace::ensure(
                        workspace.mode,
                        WorkspaceResource::Spans,
                        &mut workspace.capacity.materialize.spans,
                        &mut workspace.spans,
                        required,
                        core.stats,
                    )?;
                    let span = SpanId(u32::try_from(workspace.spans.len()).map_err(|_| {
                        RenderError::WorkspaceCapacityOverflow {
                            resource: WorkspaceResource::Spans,
                            required: Some(workspace.spans.len() as u128),
                            maximum: u32::MAX as u128,
                            stats: core.stats,
                        }
                    })?);
                    let parent = workspace.open_spans.last().map(|open| open.span);
                    workspace.spans.push(AnnotationSpan {
                        annotation,
                        range: workspace.output.len()..workspace.output.len(),
                        parent,
                    });
                    push_open_span(workspace, OpenSpan { annotation, span }, core.stats)?;
                    push_visit(
                        workspace,
                        VisitWork::ExitAnnotation { annotation },
                        core.stats,
                    )?;
                    push_visit(workspace, VisitWork::Plan(child), core.stats)?;
                }
                PlanNode::Penalty { amount, child } => {
                    cost = cost
                        .checked_add(model.penalty(amount))
                        .ok_or(RenderError::CostDomainViolation)?;
                    push_visit(workspace, VisitWork::Plan(child), core.stats)?;
                }
            },
            VisitWork::ExitAnnotation { annotation } => {
                let open = workspace
                    .open_spans
                    .pop()
                    .expect("open materialized annotation");
                debug_assert_eq!(open.annotation, annotation);
                workspace.spans[open.span.index()].range.end = workspace.output.len();
            }
        }
    }
    if cost != core.cost {
        return Err(RenderError::CostWitnessMismatch);
    }
    Ok(())
}

fn append_output(
    workspace: &mut RenderWorkspace,
    bytes: &[u8],
    stats: SolveStats,
) -> Result<(), RenderError> {
    let required = workspace.output.len().checked_add(bytes.len()).ok_or(
        RenderError::WorkspaceCapacityOverflow {
            resource: WorkspaceResource::OutputBytes,
            required: None,
            maximum: usize::MAX as u128,
            stats,
        },
    )?;
    RenderWorkspace::ensure(
        workspace.mode,
        WorkspaceResource::OutputBytes,
        &mut workspace.capacity.materialize.output_bytes,
        &mut workspace.output,
        required,
        stats,
    )?;
    workspace.output.extend_from_slice(bytes);
    Ok(())
}

fn push_open_span(
    workspace: &mut RenderWorkspace,
    open: OpenSpan,
    stats: SolveStats,
) -> Result<(), RenderError> {
    let required = workspace.open_spans.len().checked_add(1).ok_or(
        RenderError::WorkspaceCapacityOverflow {
            resource: WorkspaceResource::AnnotationDepth,
            required: None,
            maximum: usize::MAX as u128,
            stats,
        },
    )?;
    RenderWorkspace::ensure(
        workspace.mode,
        WorkspaceResource::AnnotationDepth,
        &mut workspace.capacity.visit.annotation_depth,
        &mut workspace.open_spans,
        required,
        stats,
    )?;
    workspace.open_spans.push(open);
    Ok(())
}

pub(crate) fn seed_capacity(
    bounds: crate::prepare::PreparedBounds,
    strategy: LayoutStrategy,
) -> RenderCapacity {
    let mut capacity = RenderCapacity {
        plan_nodes: bounds.single_plan_nodes_upper(),
        visit: VisitCapacity {
            work_items: bounds.visit_work_items(),
            annotation_depth: bounds.annotation_depth(),
        },
        materialize: MaterializeCapacity {
            output_bytes: bounds.output_bytes_upper(),
            spans: bounds.spans_upper(),
        },
        ..RenderCapacity::default()
    };
    match strategy {
        LayoutStrategy::Fast => capacity.fast.work_items = bounds.fast_work_items(),
        LayoutStrategy::Exact => {
            capacity.exact.work_items = bounds.fast_work_items();
            capacity.exact.memo_entries = 1;
            capacity.exact.retained_candidates = 1;
            capacity.exact.frontier_scratch = 1;
        }
    }
    capacity
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::Doc;

    fn scratch_candidate(last: u32, overflow: u128, burden: u64) -> ScratchCandidate {
        ScratchCandidate {
            cost: ConsumerCost { overflow, burden },
            last,
            precedence: 0,
            plan: PendingPlan::Empty,
        }
    }

    fn assert_avl(workspace: &RenderWorkspace) {
        fn walk(
            slots: &[FrontierSlot],
            slot: Option<u32>,
            parent: Option<u32>,
            minimum: Option<u32>,
            maximum: Option<u32>,
        ) -> (usize, i32) {
            let Some(slot) = slot else {
                return (0, 0);
            };
            let value = &slots[slot as usize];
            assert!(value.live);
            assert_eq!(value.parent, parent);
            let key = value.candidate.last;
            assert!(minimum.is_none_or(|minimum| minimum < key));
            assert!(maximum.is_none_or(|maximum| key < maximum));
            let (left_count, left_height) = walk(slots, value.left, Some(slot), minimum, Some(key));
            let (right_count, right_height) =
                walk(slots, value.right, Some(slot), Some(key), maximum);
            assert!((left_height - right_height).abs() <= 1);
            let height = 1 + left_height.max(right_height);
            assert_eq!(i32::from(value.height), height);
            (1 + left_count + right_count, height)
        }

        let (tree_count, _) = walk(&workspace.scratch, workspace.scratch_root, None, None, None);
        assert_eq!(tree_count, workspace.scratch_live);

        let mut list_count = 0usize;
        let mut previous = None;
        let mut cursor = workspace.scratch_head;
        while let Some(slot) = cursor {
            let value = &workspace.scratch[slot as usize];
            assert!(value.live);
            assert_eq!(value.precedence_previous, previous);
            previous = Some(slot);
            cursor = value.precedence_next;
            list_count += 1;
        }
        assert_eq!(previous, workspace.scratch_tail);
        assert_eq!(list_count, workspace.scratch_live);
    }

    #[test]
    fn intrusive_avl_matches_the_linear_dominance_oracle() {
        let prepared = Doc::<u32>::empty().prepare().unwrap();
        let options = RenderOptions::new(NonZeroU32::new(80).unwrap());
        let mut workspace = RenderWorkspace::growable();
        let mut solver = ExactSolver::new(&prepared.0, options, &mut workspace);
        solver.begin_builder();

        let mut reference = Vec::<ScratchCandidate>::new();
        let mut state = 0x91e1_0da5_c79e_7b1du64;
        for precedence in 0..10_000u64 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let mut candidate = scratch_candidate(
                ((state >> 17) & 127) as u32,
                u128::from((state >> 29) & 255),
                (state >> 41) & 63,
            );
            candidate.precedence = precedence;
            let dominated = reference.iter().any(|previous| {
                (previous.last == candidate.last && previous.cost <= candidate.cost)
                    || (previous.last < candidate.last && previous.cost < candidate.cost)
            });
            if !dominated {
                reference.retain(|previous| {
                    !(candidate.last <= previous.last && candidate.cost < previous.cost)
                });
                reference.push(candidate);
            }

            solver.present(candidate).unwrap();
            assert_avl(solver.workspace);
            let mut actual = Vec::new();
            let mut cursor = solver.workspace.scratch_head;
            while let Some(slot) = cursor {
                let value = &solver.workspace.scratch[slot as usize];
                actual.push((
                    value.candidate.last,
                    value.candidate.cost,
                    value.candidate.precedence,
                ));
                cursor = value.precedence_next;
            }
            let expected = reference
                .iter()
                .map(|candidate| (candidate.last, candidate.cost, candidate.precedence))
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn intrusive_avl_deletion_covers_leaf_single_and_double_child_shapes() {
        fn validate(slots: &[FrontierSlot], root: Option<u32>, parent: Option<u32>) -> i32 {
            let Some(root) = root else {
                return 0;
            };
            let slot = &slots[root as usize];
            assert_eq!(slot.parent, parent);
            let left = validate(slots, slot.left, Some(root));
            let right = validate(slots, slot.right, Some(root));
            assert!((left - right).abs() <= 1);
            assert_eq!(i32::from(slot.height), 1 + left.max(right));
            1 + left.max(right)
        }

        let mut slots = Vec::new();
        let mut root = None;
        for key in [40, 20, 60, 10, 30, 50, 70, 25, 35, 45, 55] {
            let slot = slots.len() as u32;
            slots.push(FrontierSlot::new(scratch_candidate(key, 0, 0)));
            root = Some(frontier_insert(&mut slots, root, slot));
        }
        for key in [10, 30, 60, 40, 55, 20, 25, 35, 45, 50, 70] {
            let (replacement, removed) = frontier_remove(&mut slots, root, key);
            slots[removed.expect("key exists") as usize].live = false;
            root = replacement;
            if let Some(root) = root {
                slots[root as usize].parent = None;
            }
            validate(&slots, root, None);
        }
        assert!(root.is_none());
    }

    #[test]
    fn every_semantic_counter_fails_before_the_first_invalid_increment() {
        for counter in [
            SolveCounter::WorkItems,
            SolveCounter::FitChecks,
            SolveCounter::MemoHits,
            SolveCounter::MemoMisses,
            SolveCounter::CandidatesGenerated,
            SolveCounter::CandidatesPruned,
        ] {
            let mut stats = SolveStats::default();
            match counter {
                SolveCounter::WorkItems => stats.work_items = u64::MAX,
                SolveCounter::FitChecks => stats.fit_checks = u64::MAX,
                SolveCounter::MemoHits => stats.memo_hits = u64::MAX,
                SolveCounter::MemoMisses => stats.memo_misses = u64::MAX,
                SolveCounter::CandidatesGenerated => stats.candidates_generated = u64::MAX,
                SolveCounter::CandidatesPruned => stats.candidates_pruned = u64::MAX,
            }
            let before = stats;
            let error = increment(&mut stats, counter).unwrap_err();
            assert!(matches!(
                error,
                RenderError::StatisticsOverflow {
                    counter: actual,
                    required,
                    maximum: u64::MAX,
                    stats: snapshot,
                } if actual == counter
                    && required == u128::from(u64::MAX) + 1
                    && snapshot == before
            ));
            assert_eq!(stats, before);
        }

        let mut stats = SolveStats {
            candidates_pruned: u64::MAX,
            ..SolveStats::default()
        };
        let before = stats;
        assert!(matches!(
            add_pruned(&mut stats, 1),
            Err(RenderError::StatisticsOverflow {
                counter: SolveCounter::CandidatesPruned,
                required,
                stats: snapshot,
                ..
            }) if required == u128::from(u64::MAX) + 1 && snapshot == before
        ));
        assert_eq!(stats, before);
    }

    #[test]
    fn exact_candidate_preflight_is_atomic_and_work_items_win_the_tie() {
        let prepared = Doc::<u32>::empty().prepare().unwrap();
        let options = RenderOptions::new(NonZeroU32::new(80).unwrap());
        let mut workspace = RenderWorkspace::growable();
        let mut solver = ExactSolver::new(&prepared.0, options, &mut workspace);
        solver.begin_builder();
        solver.stats.work_items = u64::MAX;
        solver.stats.candidates_generated = u64::MAX;
        let before = solver.stats;
        assert!(matches!(
            solver.present(scratch_candidate(0, 0, 0)),
            Err(RenderError::StatisticsOverflow {
                counter: SolveCounter::WorkItems,
                stats: snapshot,
                ..
            }) if snapshot == before
        ));
        assert_eq!(solver.stats, before);
        assert_eq!(solver.workspace.scratch_live, 0);

        solver.stats.work_items = 0;
        let before = solver.stats;
        assert!(matches!(
            solver.present(scratch_candidate(0, 0, 0)),
            Err(RenderError::StatisticsOverflow {
                counter: SolveCounter::CandidatesGenerated,
                stats: snapshot,
                ..
            }) if snapshot == before
        ));
        assert_eq!(solver.stats, before);
        assert_eq!(solver.workspace.scratch_live, 0);
    }

    #[test]
    fn common_fast_and_plan_arena_records_stay_compact() {
        use std::mem::size_of;

        assert_eq!(size_of::<FastTask>(), 20);
        assert_eq!(size_of::<PlanRecord>(), 9);
        assert!(size_of::<PlanRecord>() < size_of::<PlanNode>());
    }
}
