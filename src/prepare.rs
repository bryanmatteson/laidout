//! Dense prepared document representation and conservative domain bounds.

use std::collections::HashMap;
use std::sync::Arc;

use crate::doc::{Doc, FlatAlternative, Node, TextRun};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct NodeId(pub(crate) u32);

impl NodeId {
    pub(crate) fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct TextId(pub(crate) u32);

impl TextId {
    pub(crate) fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// Result-local identifier for one prepared annotation value.
pub struct AnnotationId(pub(crate) u32);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct EdgeRange {
    pub(crate) start: u32,
    pub(crate) len: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExactChoice {
    pub(crate) alternatives: EdgeRange,
    pub(crate) skipped_choices: u32,
}

struct ExactChoices {
    records: Box<[Option<ExactChoice>]>,
    edges: Box<[NodeId]>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum PreparedNode {
    Empty,
    Text(TextId),
    Break(FlatAlternative),
    HardLine,
    Seq(EdgeRange),
    Group(NodeId),
    Fill(EdgeRange),
    Nest {
        indent: u32,
        child: NodeId,
    },
    Align(NodeId),
    Choice {
        preferred: NodeId,
        alternative: NodeId,
    },
    Annotate {
        annotation: AnnotationId,
        child: NodeId,
    },
    Penalty {
        amount: u32,
        child: NodeId,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedText {
    pub(crate) value: Arc<str>,
    pub(crate) byte_len: usize,
    pub(crate) display_width: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ModeFitSummary {
    pub(crate) flat: FitSummary,
    pub(crate) broken: FitSummary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FitSummary {
    pub(crate) columns: u32,
    pub(crate) stop: FitStop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FitStop {
    End,
    Break,
    HardLine,
}

impl FitSummary {
    const fn end(columns: u32) -> Self {
        Self {
            columns,
            stop: FitStop::End,
        }
    }

    const fn broken() -> Self {
        Self {
            columns: 0,
            stop: FitStop::Break,
        }
    }

    const fn hard_line() -> Self {
        Self {
            columns: 0,
            stop: FitStop::HardLine,
        }
    }
}

pub(crate) fn then(left: FitSummary, right: FitSummary) -> Option<FitSummary> {
    if left.stop != FitStop::End {
        return Some(left);
    }
    Some(FitSummary {
        columns: left.columns.checked_add(right.columns)?,
        stop: right.stop,
    })
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
/// Conservative, width-independent bounds computed during preparation.
pub struct PreparedBounds {
    output_bytes_upper: usize,
    line_count_upper: usize,
    spans_upper: usize,
    annotation_depth: usize,
    fast_work_items: usize,
    solve_work_items_upper: Option<u128>,
    fast_fit_checks_upper: Option<u128>,
    single_plan_nodes_upper: usize,
    visit_work_items: usize,
    candidate_emissions_upper: Option<u128>,
    overflow_cost_upper: u128,
    burden_cost_upper: u64,
}

impl PreparedBounds {
    /// Maximum materialized UTF-8 bytes for one candidate.
    pub const fn output_bytes_upper(self) -> usize {
        self.output_bytes_upper
    }
    /// Maximum materialized line count for one candidate.
    pub const fn line_count_upper(self) -> usize {
        self.line_count_upper
    }
    /// Maximum materialized annotation spans for one candidate.
    pub const fn spans_upper(self) -> usize {
        self.spans_upper
    }
    /// Maximum simultaneously active annotations.
    pub const fn annotation_depth(self) -> usize {
        self.annotation_depth
    }
    /// Exact peak Fast traversal stack requirement.
    pub const fn fast_work_items(self) -> usize {
        self.fast_work_items
    }
    /// Conservative semantic solve-work upper bound when representable.
    pub const fn solve_work_items_upper(self) -> Option<u128> {
        self.solve_work_items_upper
    }
    /// Conservative Fast decision upper bound when representable.
    pub const fn fast_fit_checks_upper(self) -> Option<u128> {
        self.fast_fit_checks_upper
    }
    /// Maximum selected-layout nodes in one candidate.
    pub const fn single_plan_nodes_upper(self) -> usize {
        self.single_plan_nodes_upper
    }
    /// Exact selected-layout visitor stack requirement.
    pub const fn visit_work_items(self) -> usize {
        self.visit_work_items
    }
    /// Conservative Exact candidate-emission upper bound when representable.
    pub const fn candidate_emissions_upper(self) -> Option<u128> {
        self.candidate_emissions_upper
    }
    /// Maximum squared-overflow cost across complete candidate output.
    pub const fn overflow_cost_upper(self) -> u128 {
        self.overflow_cost_upper
    }
    /// Maximum newline-and-penalty burden across complete candidate output.
    pub const fn burden_cost_upper(self) -> u64 {
        self.burden_cost_upper
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
/// Prepared representation resource that exceeded its production domain.
pub enum PreparedResource {
    /// Prepared nodes.
    Nodes,
    /// Child edges.
    Edges,
    /// Interned text records.
    Texts,
    /// Annotation handles.
    Annotations,
    /// UTF-8 text payload bytes.
    TextBytes,
    /// Display-column sums.
    DisplayColumns,
    /// Materialized output bytes.
    OutputBytes,
    /// Materialized line count.
    LineCount,
    /// Structural indentation.
    Indentation,
    /// Simultaneously active annotations.
    AnnotationDepth,
    /// Materialized span occurrences.
    SpanOccurrences,
    /// Selected-layout node identifiers.
    PlanNodes,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
/// Production cost component checked during preparation.
pub enum CostComponent {
    /// Squared horizontal overflow.
    Overflow,
    /// Newline-and-penalty burden.
    Burden,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
/// Failure while preparing a source document for repeated solving.
pub enum PrepareError {
    #[error("prepared {resource:?} representation requires {required:?}, maximum {maximum}")]
    /// A prepared representation bound cannot be represented.
    RepresentationExceeded {
        /// Resource whose bound failed.
        resource: PreparedResource,
        /// Exact requirement, or `None` if its calculation overflowed.
        required: Option<u128>,
        /// Largest representable value.
        maximum: u128,
    },
    #[error("prepared {component:?} cost requires {required:?}, maximum {maximum}")]
    /// A complete candidate can exceed the checked production cost domain.
    CostDomainExceeded {
        /// Cost component whose bound failed.
        component: CostComponent,
        /// Exact requirement, or `None` if its calculation overflowed.
        required: Option<u128>,
        /// Largest representable value.
        maximum: u128,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
/// Stable category for a [`PrepareError`].
pub enum PrepareErrorKind {
    /// Prepared representation was not representable.
    RepresentationExceeded,
    /// Candidate cost was not representable.
    CostDomainExceeded,
}

impl PrepareError {
    /// Returns the stable error category.
    pub const fn kind(&self) -> PrepareErrorKind {
        match self {
            Self::RepresentationExceeded { .. } => PrepareErrorKind::RepresentationExceeded,
            Self::CostDomainExceeded { .. } => PrepareErrorKind::CostDomainExceeded,
        }
    }
}

/// Immutable dense representation prepared for repeated layout.
pub struct PreparedDoc<A = u32>(pub(crate) Arc<PreparedData<A>>);

impl<A> Clone for PreparedDoc<A> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<A> PreparedDoc<A> {
    /// Returns the conservative bounds computed during preparation.
    pub fn bounds(&self) -> PreparedBounds {
        self.0.bounds
    }

    /// Returns the number of retained annotation values.
    pub fn annotation_count(&self) -> usize {
        self.0.annotations.len()
    }
}

pub(crate) struct PreparedData<A> {
    pub(crate) nodes: Box<[PreparedNode]>,
    pub(crate) edges: Box<[NodeId]>,
    pub(crate) texts: Box<[PreparedText]>,
    pub(crate) annotations: Box<[Arc<A>]>,
    pub(crate) fit_summaries: Box<[ModeFitSummary]>,
    pub(crate) flat_nodes: Box<[Option<NodeId>]>,
    pub(crate) exact_choices: Box<[Option<ExactChoice>]>,
    pub(crate) exact_choice_edges: Box<[NodeId]>,
    pub(crate) root: NodeId,
    pub(crate) space_text: TextId,
    pub(crate) bounds: PreparedBounds,
}

impl<A> PreparedData<A> {
    pub(crate) fn children(&self, range: EdgeRange) -> &[NodeId] {
        let start = range.start as usize;
        let end = start
            .checked_add(range.len as usize)
            .expect("prepared edge range checked during preparation");
        &self.edges[start..end]
    }

    pub(crate) fn exact_choice_alternatives(&self, range: EdgeRange) -> &[NodeId] {
        let start = range.start as usize;
        let end = start
            .checked_add(range.len as usize)
            .expect("prepared exact-choice range checked during preparation");
        &self.exact_choice_edges[start..end]
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum NodeKey {
    Empty,
    Text(TextId),
    Break(FlatAlternative),
    HardLine,
    Seq(Box<[NodeId]>),
    Group(NodeId),
    Fill(Box<[NodeId]>),
    Nest {
        indent: u32,
        child: NodeId,
    },
    Align(NodeId),
    Choice {
        preferred: NodeId,
        alternative: NodeId,
    },
    Annotate {
        annotation: AnnotationId,
        child: NodeId,
    },
    Penalty {
        amount: u32,
        child: NodeId,
    },
}

#[derive(Clone, Copy, Debug)]
struct BoundCalc {
    text_bytes: Option<u128>,
    display_columns: Option<u128>,
    newlines: Option<u128>,
    penalty: Option<u128>,
    spans: Option<u128>,
    nest: Option<u128>,
    annotation_depth: Option<u128>,
    plan_nodes: Option<u128>,
    derivations: Option<u128>,
    interpretations: Option<u128>,
    emissions: Option<u128>,
    fit_checks: Option<u128>,
    fast_work: Option<u128>,
}

impl Default for BoundCalc {
    fn default() -> Self {
        Self {
            text_bytes: Some(0),
            display_columns: Some(0),
            newlines: Some(0),
            penalty: Some(0),
            spans: Some(0),
            nest: Some(0),
            annotation_depth: Some(0),
            plan_nodes: Some(0),
            derivations: None,
            interpretations: None,
            emissions: None,
            fit_checks: None,
            fast_work: Some(0),
        }
    }
}

impl BoundCalc {
    fn leaf() -> Self {
        Self {
            plan_nodes: Some(1),
            derivations: Some(1),
            interpretations: Some(1),
            emissions: Some(1),
            fit_checks: Some(0),
            fast_work: Some(1),
            ..Self::default()
        }
    }

    fn unary(child: Self) -> Self {
        Self {
            derivations: child.derivations,
            interpretations: checked_add(Some(1), child.interpretations),
            emissions: checked_add(child.emissions, child.derivations),
            fast_work: checked_add(child.fast_work, Some(1)),
            ..child
        }
    }

    fn maximum(left: Self, right: Self) -> Self {
        Self {
            text_bytes: option_max(left.text_bytes, right.text_bytes),
            display_columns: option_max(left.display_columns, right.display_columns),
            newlines: option_max(left.newlines, right.newlines),
            penalty: option_max(left.penalty, right.penalty),
            spans: option_max(left.spans, right.spans),
            nest: option_max(left.nest, right.nest),
            annotation_depth: option_max(left.annotation_depth, right.annotation_depth),
            plan_nodes: option_max(left.plan_nodes, right.plan_nodes),
            derivations: checked_add(left.derivations, right.derivations),
            interpretations: checked_add(
                Some(1),
                checked_add(left.interpretations, right.interpretations),
            ),
            emissions: checked_add(
                checked_add(left.emissions, right.emissions),
                checked_add(left.derivations, right.derivations),
            ),
            fit_checks: checked_add(Some(1), option_max(left.fit_checks, right.fit_checks)),
            fast_work: checked_add(option_max(left.fast_work, right.fast_work), Some(1)),
        }
    }
}

fn checked_add(left: Option<u128>, right: Option<u128>) -> Option<u128> {
    left?.checked_add(right?)
}

fn checked_mul(left: Option<u128>, right: Option<u128>) -> Option<u128> {
    left?.checked_mul(right?)
}

fn option_max(left: Option<u128>, right: Option<u128>) -> Option<u128> {
    Some(left?.max(right?))
}

struct Builder<A> {
    nodes: Vec<PreparedNode>,
    edges: Vec<NodeId>,
    texts: Vec<PreparedText>,
    annotations: Vec<Arc<A>>,
    summaries: Vec<ModeFitSummary>,
    flat_nodes: Vec<Option<NodeId>>,
    bounds: Vec<BoundCalc>,
    text_ids: HashMap<(Arc<str>, u32), TextId>,
    annotation_ids: HashMap<usize, AnnotationId>,
    node_ids: HashMap<NodeKey, NodeId>,
    source_ids: HashMap<usize, Option<NodeId>>,
    source_flat: HashMap<usize, Option<NodeId>>,
    space_text: TextId,
}

#[derive(Clone, Copy)]
struct BoundLimits {
    text_bytes: u128,
    display_columns: u128,
    indentation: u128,
    newlines: u128,
    spans: u128,
    annotation_depth: u128,
    plan_nodes: u128,
    output_bytes: u128,
    line_count: u128,
    overflow_cost: u128,
    burden_cost: u128,
}

impl BoundLimits {
    const PRODUCTION: Self = Self {
        text_bytes: usize::MAX as u128,
        display_columns: u32::MAX as u128,
        indentation: u32::MAX as u128,
        newlines: usize::MAX as u128,
        spans: (usize::MAX as u32) as u128,
        annotation_depth: usize::MAX as u128,
        plan_nodes: (usize::MAX as u32) as u128,
        output_bytes: usize::MAX as u128,
        line_count: usize::MAX as u128,
        overflow_cost: u128::MAX,
        burden_cost: u64::MAX as u128,
    };
}

impl<A> Builder<A> {
    fn new() -> Result<Self, PrepareError> {
        let mut builder = Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            texts: Vec::new(),
            annotations: Vec::new(),
            summaries: Vec::new(),
            flat_nodes: Vec::new(),
            bounds: Vec::new(),
            text_ids: HashMap::new(),
            annotation_ids: HashMap::new(),
            node_ids: HashMap::new(),
            source_ids: HashMap::new(),
            source_flat: HashMap::new(),
            space_text: TextId(0),
        };
        builder.space_text = builder.intern_text(&TextRun::trusted_ascii(" "))?;
        Ok(builder)
    }

    fn intern_text(&mut self, run: &TextRun) -> Result<TextId, PrepareError> {
        let key = (run.value_arc(), run.columns());
        if let Some(id) = self.text_ids.get(&key) {
            return Ok(*id);
        }
        let id = compact_id(self.texts.len(), PreparedResource::Texts).map(TextId)?;
        self.texts.push(PreparedText {
            value: key.0.clone(),
            byte_len: key.0.len(),
            display_width: key.1,
        });
        self.text_ids.insert(key, id);
        Ok(id)
    }

    fn intern_annotation(&mut self, value: &Arc<A>) -> Result<AnnotationId, PrepareError> {
        let identity = Arc::as_ptr(value) as usize;
        if let Some(id) = self.annotation_ids.get(&identity) {
            return Ok(*id);
        }
        let id =
            compact_id(self.annotations.len(), PreparedResource::Annotations).map(AnnotationId)?;
        self.annotations.push(value.clone());
        self.annotation_ids.insert(identity, id);
        Ok(id)
    }

    fn source_id(&self, doc: &Doc<A>) -> NodeId {
        self.source_ids[&(Arc::as_ptr(doc.root()) as usize)].expect("source child prepared first")
    }

    fn source_flat(&self, doc: &Doc<A>) -> Option<NodeId> {
        self.source_flat[&(Arc::as_ptr(doc.root()) as usize)]
    }

    fn intern_node(&mut self, key: NodeKey) -> Result<NodeId, PrepareError> {
        if let Some(id) = self.node_ids.get(&key) {
            return Ok(*id);
        }
        let id = compact_id(self.nodes.len(), PreparedResource::Nodes).map(NodeId)?;
        let node = match &key {
            NodeKey::Empty => PreparedNode::Empty,
            NodeKey::Text(text) => PreparedNode::Text(*text),
            NodeKey::Break(flat) => PreparedNode::Break(*flat),
            NodeKey::HardLine => PreparedNode::HardLine,
            NodeKey::Seq(children) => PreparedNode::Seq(self.push_edges(children)?),
            NodeKey::Group(child) => PreparedNode::Group(*child),
            NodeKey::Fill(children) => PreparedNode::Fill(self.push_edges(children)?),
            NodeKey::Nest { indent, child } => PreparedNode::Nest {
                indent: *indent,
                child: *child,
            },
            NodeKey::Align(child) => PreparedNode::Align(*child),
            NodeKey::Choice {
                preferred,
                alternative,
            } => PreparedNode::Choice {
                preferred: *preferred,
                alternative: *alternative,
            },
            NodeKey::Annotate { annotation, child } => PreparedNode::Annotate {
                annotation: *annotation,
                child: *child,
            },
            NodeKey::Penalty { amount, child } => PreparedNode::Penalty {
                amount: *amount,
                child: *child,
            },
        };
        let summary = self.summary(node)?;
        let bounds = self.bound(node);
        self.nodes.push(node);
        self.summaries.push(summary);
        self.flat_nodes.push(None);
        self.bounds.push(bounds);
        self.node_ids.insert(key, id);
        Ok(id)
    }

    fn push_edges(&mut self, children: &[NodeId]) -> Result<EdgeRange, PrepareError> {
        let start = compact_id(self.edges.len(), PreparedResource::Edges)?;
        let len = compact_id(children.len(), PreparedResource::Edges)?;
        let required = self
            .edges
            .len()
            .checked_add(children.len())
            .ok_or_else(|| representation(PreparedResource::Edges, None, u32::MAX as u128))?;
        if required > u32::MAX as usize {
            return Err(representation(
                PreparedResource::Edges,
                Some(required as u128),
                u32::MAX as u128,
            ));
        }
        self.edges.extend_from_slice(children);
        Ok(EdgeRange { start, len })
    }

    fn children(&self, range: EdgeRange) -> &[NodeId] {
        let start = range.start as usize;
        &self.edges[start..start + range.len as usize]
    }

    fn summary(&self, node: PreparedNode) -> Result<ModeFitSummary, PrepareError> {
        let (flat, broken) = match node {
            PreparedNode::Empty => (FitSummary::end(0), FitSummary::end(0)),
            PreparedNode::Text(text) => {
                let width = self.texts[text.index()].display_width;
                (FitSummary::end(width), FitSummary::end(width))
            }
            PreparedNode::Break(FlatAlternative::Space) => {
                (FitSummary::end(1), FitSummary::broken())
            }
            PreparedNode::Break(FlatAlternative::Empty) => {
                (FitSummary::end(0), FitSummary::broken())
            }
            PreparedNode::HardLine => (FitSummary::hard_line(), FitSummary::hard_line()),
            PreparedNode::Seq(range) => (
                self.fold_summary(self.children(range), true)?,
                self.fold_summary(self.children(range), false)?,
            ),
            PreparedNode::Group(child) => {
                let summary = self.summaries[child.index()].flat;
                (summary, summary)
            }
            PreparedNode::Fill(range) => (
                self.fill_summary(self.children(range), true)?,
                self.fill_summary(self.children(range), false)?,
            ),
            PreparedNode::Nest { child, .. }
            | PreparedNode::Align(child)
            | PreparedNode::Annotate { child, .. }
            | PreparedNode::Penalty { child, .. } => {
                let summary = self.summaries[child.index()];
                (summary.flat, summary.broken)
            }
            PreparedNode::Choice { preferred, .. } => {
                let summary = self.summaries[preferred.index()];
                (summary.flat, summary.broken)
            }
        };
        Ok(ModeFitSummary { flat, broken })
    }

    fn fold_summary(&self, children: &[NodeId], flat: bool) -> Result<FitSummary, PrepareError> {
        let mut total = FitSummary::end(0);
        for child in children {
            let summary = self.summaries[child.index()];
            total =
                then(total, if flat { summary.flat } else { summary.broken }).ok_or_else(|| {
                    representation(PreparedResource::DisplayColumns, None, u32::MAX as u128)
                })?;
        }
        Ok(total)
    }

    fn fill_summary(&self, children: &[NodeId], flat: bool) -> Result<FitSummary, PrepareError> {
        let Some((first, rest)) = children.split_first() else {
            return Ok(FitSummary::end(0));
        };
        let first = self.summaries[first.index()];
        let mut total = if flat { first.flat } else { first.broken };
        if !rest.is_empty() {
            total = then(total, FitSummary::broken()).ok_or_else(|| {
                representation(PreparedResource::DisplayColumns, None, u32::MAX as u128)
            })?;
        }
        Ok(total)
    }

    fn bound(&self, node: PreparedNode) -> BoundCalc {
        match node {
            PreparedNode::Empty => BoundCalc::leaf(),
            PreparedNode::Text(text) => {
                let text = &self.texts[text.index()];
                BoundCalc {
                    text_bytes: Some(text.byte_len as u128),
                    display_columns: Some(u128::from(text.display_width)),
                    ..BoundCalc::leaf()
                }
            }
            PreparedNode::Break(_) | PreparedNode::HardLine => BoundCalc {
                newlines: Some(1),
                ..BoundCalc::leaf()
            },
            PreparedNode::Seq(range) => self.sequence_bound(self.children(range), false),
            PreparedNode::Group(child) => {
                let broken = self.bounds[child.index()];
                let flat = self.flat_nodes[child.index()]
                    .map(|id| self.bounds[id.index()])
                    .unwrap_or(broken);
                BoundCalc::maximum(flat, broken)
            }
            PreparedNode::Fill(range) => self.sequence_bound(self.children(range), true),
            PreparedNode::Nest { indent, child } => {
                let mut value = BoundCalc::unary(self.bounds[child.index()]);
                value.nest = checked_add(value.nest, Some(u128::from(indent)));
                value
            }
            PreparedNode::Align(child) => BoundCalc::unary(self.bounds[child.index()]),
            PreparedNode::Choice {
                preferred,
                alternative,
            } => BoundCalc::maximum(
                self.bounds[preferred.index()],
                self.bounds[alternative.index()],
            ),
            PreparedNode::Annotate { child, .. } => {
                let mut value = BoundCalc::unary(self.bounds[child.index()]);
                value.spans = checked_add(value.spans, Some(1));
                value.annotation_depth = checked_add(value.annotation_depth, Some(1));
                value.plan_nodes = checked_add(value.plan_nodes, Some(1));
                value
            }
            PreparedNode::Penalty { amount, child } => {
                let mut value = BoundCalc::unary(self.bounds[child.index()]);
                value.penalty = checked_add(value.penalty, Some(u128::from(amount)));
                value.plan_nodes = checked_add(value.plan_nodes, Some(1));
                value
            }
        }
    }

    fn sequence_bound(&self, children: &[NodeId], fill: bool) -> BoundCalc {
        let mut result = BoundCalc::leaf();
        result.plan_nodes = Some(0);
        result.emissions = Some(0);
        for (index, child) in children.iter().enumerate() {
            if fill && index != 0 {
                let space = BoundCalc {
                    text_bytes: Some(1),
                    display_columns: Some(1),
                    ..BoundCalc::leaf()
                };
                let newline = BoundCalc {
                    newlines: Some(1),
                    ..BoundCalc::leaf()
                };
                result = sequence_step(result, BoundCalc::maximum(space, newline));
            }
            result = sequence_step(result, self.bounds[child.index()]);
        }
        result.plan_nodes = checked_add(
            result.plan_nodes,
            Some(children.len().saturating_sub(1) as u128),
        );
        result.fast_work = checked_add(result.fast_work, Some(children.len() as u128));
        result
    }

    fn build_exact_choices(&self) -> Result<ExactChoices, PrepareError> {
        let mut incoming = vec![0usize; self.nodes.len()];
        let mut choice_parents = vec![0usize; self.nodes.len()];
        for node in &self.nodes {
            match *node {
                PreparedNode::Seq(range) | PreparedNode::Fill(range) => {
                    for child in self.children(range) {
                        incoming[child.index()] =
                            incoming[child.index()].checked_add(1).ok_or_else(|| {
                                representation(PreparedResource::Edges, None, u32::MAX as u128)
                            })?;
                    }
                }
                PreparedNode::Group(child)
                | PreparedNode::Nest { child, .. }
                | PreparedNode::Align(child)
                | PreparedNode::Annotate { child, .. }
                | PreparedNode::Penalty { child, .. } => {
                    incoming[child.index()] =
                        incoming[child.index()].checked_add(1).ok_or_else(|| {
                            representation(PreparedResource::Edges, None, u32::MAX as u128)
                        })?;
                }
                PreparedNode::Choice {
                    preferred,
                    alternative,
                } => {
                    for child in [preferred, alternative] {
                        incoming[child.index()] =
                            incoming[child.index()].checked_add(1).ok_or_else(|| {
                                representation(PreparedResource::Edges, None, u32::MAX as u128)
                            })?;
                        choice_parents[child.index()] = choice_parents[child.index()]
                            .checked_add(1)
                            .ok_or_else(|| {
                                representation(PreparedResource::Edges, None, u32::MAX as u128)
                            })?;
                    }
                }
                PreparedNode::Empty
                | PreparedNode::Text(_)
                | PreparedNode::Break(_)
                | PreparedNode::HardLine => {}
            }
        }

        let mut records = vec![None; self.nodes.len()];
        let mut edges = Vec::new();
        for root_index in 0..self.nodes.len() {
            if !matches!(self.nodes[root_index], PreparedNode::Choice { .. })
                || (incoming[root_index] == 1 && choice_parents[root_index] != 0)
            {
                continue;
            }
            let root = NodeId(u32::try_from(root_index).map_err(|_| {
                representation(
                    PreparedResource::Nodes,
                    Some(root_index as u128),
                    u32::MAX as u128,
                )
            })?);
            let mut alternatives = Vec::new();
            let mut work = vec![root];
            let mut choices = 0usize;
            while let Some(node) = work.pop() {
                let flatten = node == root
                    || (incoming[node.index()] == 1 && choice_parents[node.index()] == 1);
                match self.nodes[node.index()] {
                    PreparedNode::Choice {
                        preferred,
                        alternative,
                    } if flatten => {
                        choices = choices.checked_add(1).ok_or_else(|| {
                            representation(PreparedResource::Nodes, None, u32::MAX as u128)
                        })?;
                        work.push(alternative);
                        work.push(preferred);
                    }
                    _ => alternatives.push(node),
                }
            }
            if choices <= 1 {
                continue;
            }
            let start = compact_id(edges.len(), PreparedResource::Edges)?;
            let len = compact_id(alternatives.len(), PreparedResource::Edges)?;
            let required = edges
                .len()
                .checked_add(alternatives.len())
                .ok_or_else(|| representation(PreparedResource::Edges, None, u32::MAX as u128))?;
            if required > u32::MAX as usize {
                return Err(representation(
                    PreparedResource::Edges,
                    Some(required as u128),
                    u32::MAX as u128,
                ));
            }
            edges.extend(alternatives);
            records[root_index] = Some(ExactChoice {
                alternatives: EdgeRange { start, len },
                skipped_choices: u32::try_from(choices - 1).map_err(|_| {
                    representation(
                        PreparedResource::Nodes,
                        Some(choices as u128),
                        u32::MAX as u128,
                    )
                })?,
            });
        }
        Ok(ExactChoices {
            records: records.into_boxed_slice(),
            edges: edges.into_boxed_slice(),
        })
    }

    fn prepare(self, root: &Doc<A>) -> Result<PreparedDoc<A>, PrepareError> {
        self.prepare_with_limits(root, BoundLimits::PRODUCTION)
    }

    fn prepare_with_limits(
        mut self,
        root: &Doc<A>,
        limits: BoundLimits,
    ) -> Result<PreparedDoc<A>, PrepareError> {
        enum Work<A> {
            Enter(Arc<Node<A>>),
            Exit(Arc<Node<A>>),
        }
        let mut work = vec![Work::Enter(root.root().clone())];
        while let Some(item) = work.pop() {
            match item {
                Work::Enter(node) => {
                    let pointer = Arc::as_ptr(&node) as usize;
                    if self.source_ids.contains_key(&pointer) {
                        continue;
                    }
                    self.source_ids.insert(pointer, None);
                    work.push(Work::Exit(node.clone()));
                    push_node_children(node.as_ref(), |child| {
                        work.push(Work::Enter(child.root().clone()))
                    });
                }
                Work::Exit(node) => {
                    let pointer = Arc::as_ptr(&node) as usize;
                    let (id, flat) = self.build_source(node.as_ref())?;
                    self.source_ids.insert(pointer, Some(id));
                    self.source_flat.insert(pointer, flat);
                    if self.flat_nodes[id.index()].is_none() {
                        self.flat_nodes[id.index()] = flat;
                    }
                }
            }
        }
        let root = self.source_ids[&(Arc::as_ptr(root.root()) as usize)].expect("prepared root");
        let bounds = self.finish_bounds_with_limits(root, limits)?;
        let exact = self.build_exact_choices()?;
        Ok(PreparedDoc(Arc::new(PreparedData {
            nodes: self.nodes.into_boxed_slice(),
            edges: self.edges.into_boxed_slice(),
            texts: self.texts.into_boxed_slice(),
            annotations: self.annotations.into_boxed_slice(),
            fit_summaries: self.summaries.into_boxed_slice(),
            flat_nodes: self.flat_nodes.into_boxed_slice(),
            exact_choices: exact.records,
            exact_choice_edges: exact.edges,
            root,
            space_text: self.space_text,
            bounds,
        })))
    }

    fn build_source(&mut self, node: &Node<A>) -> Result<(NodeId, Option<NodeId>), PrepareError> {
        let result = match node {
            Node::Empty => {
                let id = self.intern_node(NodeKey::Empty)?;
                (id, Some(id))
            }
            Node::Text(run) => {
                let text = self.intern_text(run)?;
                let id = self.intern_node(NodeKey::Text(text))?;
                (id, Some(id))
            }
            Node::Break(flat) => {
                let flat_id = match flat {
                    FlatAlternative::Space => self.intern_node(NodeKey::Text(self.space_text))?,
                    FlatAlternative::Empty => self.intern_node(NodeKey::Empty)?,
                };
                (self.intern_node(NodeKey::Break(*flat))?, Some(flat_id))
            }
            Node::HardLine => (self.intern_node(NodeKey::HardLine)?, None),
            Node::Seq(children) => {
                let ids = children
                    .iter()
                    .map(|child| self.source_id(child))
                    .collect::<Vec<_>>();
                let flat = children
                    .iter()
                    .map(|child| self.source_flat(child))
                    .collect::<Option<Vec<_>>>()
                    .map(|children| self.intern_node(NodeKey::Seq(children.into_boxed_slice())))
                    .transpose()?;
                (
                    self.intern_node(NodeKey::Seq(ids.into_boxed_slice()))?,
                    flat,
                )
            }
            Node::Group(child) => {
                let flat = self.source_flat(child);
                (
                    self.intern_node(NodeKey::Group(self.source_id(child)))?,
                    flat,
                )
            }
            Node::Fill(children) => {
                let ids = children
                    .iter()
                    .map(|child| self.source_id(child))
                    .collect::<Vec<_>>();
                let flat = children
                    .iter()
                    .map(|child| self.source_flat(child))
                    .collect::<Option<Vec<_>>>()
                    .map(|children| self.intern_node(NodeKey::Fill(children.into_boxed_slice())))
                    .transpose()?;
                (
                    self.intern_node(NodeKey::Fill(ids.into_boxed_slice()))?,
                    flat,
                )
            }
            Node::Nest { indent, child } => {
                let flat = self
                    .source_flat(child)
                    .map(|child| {
                        self.intern_node(NodeKey::Nest {
                            indent: *indent,
                            child,
                        })
                    })
                    .transpose()?;
                (
                    self.intern_node(NodeKey::Nest {
                        indent: *indent,
                        child: self.source_id(child),
                    })?,
                    flat,
                )
            }
            Node::Align(child) => {
                let flat = self
                    .source_flat(child)
                    .map(|child| self.intern_node(NodeKey::Align(child)))
                    .transpose()?;
                (
                    self.intern_node(NodeKey::Align(self.source_id(child)))?,
                    flat,
                )
            }
            Node::Choice {
                preferred,
                alternative,
            } => (
                self.intern_node(NodeKey::Choice {
                    preferred: self.source_id(preferred),
                    alternative: self.source_id(alternative),
                })?,
                self.source_flat(preferred),
            ),
            Node::Annotate { annotation, child } => {
                let annotation = self.intern_annotation(annotation)?;
                let flat = self
                    .source_flat(child)
                    .map(|child| self.intern_node(NodeKey::Annotate { annotation, child }))
                    .transpose()?;
                (
                    self.intern_node(NodeKey::Annotate {
                        annotation,
                        child: self.source_id(child),
                    })?,
                    flat,
                )
            }
            Node::Penalty { amount, child } => {
                let flat = self
                    .source_flat(child)
                    .map(|child| {
                        self.intern_node(NodeKey::Penalty {
                            amount: *amount,
                            child,
                        })
                    })
                    .transpose()?;
                (
                    self.intern_node(NodeKey::Penalty {
                        amount: *amount,
                        child: self.source_id(child),
                    })?,
                    flat,
                )
            }
        };
        Ok(result)
    }

    fn finish_bounds_with_limits(
        &self,
        root: NodeId,
        limits: BoundLimits,
    ) -> Result<PreparedBounds, PrepareError> {
        let value = self.bounds[root.index()];
        let text_bytes = required_bound(
            value.text_bytes,
            PreparedResource::TextBytes,
            limits.text_bytes,
        )?;
        let display_columns = required_bound(
            value.display_columns,
            PreparedResource::DisplayColumns,
            limits.display_columns,
        )?;
        let nest = required_bound(
            value.nest,
            PreparedResource::Indentation,
            limits.indentation,
        )?;
        let newlines =
            required_bound(value.newlines, PreparedResource::LineCount, limits.newlines)?;
        let spans = required_bound(value.spans, PreparedResource::SpanOccurrences, limits.spans)?;
        let annotation_depth = required_bound(
            value.annotation_depth,
            PreparedResource::AnnotationDepth,
            limits.annotation_depth,
        )?;
        let plan_nodes = required_bound(
            value.plan_nodes,
            PreparedResource::PlanNodes,
            limits.plan_nodes,
        )?;
        let fast_work = required_bound(
            value.fast_work,
            PreparedResource::PlanNodes,
            usize::MAX as u128,
        )?;
        let penalty = value
            .penalty
            .ok_or_else(|| cost_error(CostComponent::Burden, None, limits.burden_cost))?;

        let column = display_columns.checked_add(nest).ok_or_else(|| {
            representation(PreparedResource::Indentation, None, limits.indentation)
        })?;
        require_at_most(
            PreparedResource::DisplayColumns,
            column,
            limits.display_columns,
        )?;
        let line_count = newlines
            .checked_add(1)
            .ok_or_else(|| representation(PreparedResource::LineCount, None, limits.line_count))?;
        let output = newlines
            .checked_mul(column.checked_add(1).ok_or_else(|| {
                representation(PreparedResource::OutputBytes, None, limits.output_bytes)
            })?)
            .and_then(|extra| text_bytes.checked_add(extra))
            .ok_or_else(|| {
                representation(PreparedResource::OutputBytes, None, limits.output_bytes)
            })?;
        require_at_most(PreparedResource::OutputBytes, output, limits.output_bytes)?;
        require_at_most(PreparedResource::LineCount, line_count, limits.line_count)?;
        require_at_most(PreparedResource::SpanOccurrences, spans, limits.spans)?;
        require_at_most(
            PreparedResource::AnnotationDepth,
            annotation_depth,
            limits.annotation_depth,
        )?;
        require_at_most(PreparedResource::PlanNodes, plan_nodes, limits.plan_nodes)?;
        let burden = newlines
            .checked_mul(u128::from(u32::MAX))
            .and_then(|cost| cost.checked_add(penalty))
            .ok_or_else(|| cost_error(CostComponent::Burden, None, limits.burden_cost))?;
        if burden > limits.burden_cost {
            return Err(cost_error(
                CostComponent::Burden,
                Some(burden),
                limits.burden_cost,
            ));
        }
        let overflow = line_count
            .checked_mul(column)
            .and_then(|cost| cost.checked_mul(column))
            .ok_or_else(|| cost_error(CostComponent::Overflow, None, limits.overflow_cost))?;
        if overflow > limits.overflow_cost {
            return Err(cost_error(
                CostComponent::Overflow,
                Some(overflow),
                limits.overflow_cost,
            ));
        }
        Ok(PreparedBounds {
            output_bytes_upper: output as usize,
            line_count_upper: line_count as usize,
            spans_upper: spans as usize,
            annotation_depth: annotation_depth as usize,
            fast_work_items: fast_work as usize,
            solve_work_items_upper: checked_add(value.interpretations, value.emissions),
            fast_fit_checks_upper: value.fit_checks,
            single_plan_nodes_upper: plan_nodes as usize,
            visit_work_items: plan_nodes as usize,
            candidate_emissions_upper: value.emissions,
            overflow_cost_upper: overflow,
            burden_cost_upper: burden as u64,
        })
    }
}

fn sequence_step(left: BoundCalc, right: BoundCalc) -> BoundCalc {
    let derivations = checked_mul(left.derivations, right.derivations);
    let interpretations = checked_add(
        left.interpretations,
        checked_mul(left.derivations, right.interpretations),
    );
    let emissions = checked_add(
        left.emissions,
        checked_add(
            checked_mul(left.derivations, right.emissions),
            checked_mul(left.derivations, right.derivations),
        ),
    );
    BoundCalc {
        text_bytes: checked_add(left.text_bytes, right.text_bytes),
        display_columns: checked_add(left.display_columns, right.display_columns),
        newlines: checked_add(left.newlines, right.newlines),
        penalty: checked_add(left.penalty, right.penalty),
        spans: checked_add(left.spans, right.spans),
        nest: checked_add(left.nest, right.nest),
        annotation_depth: option_max(left.annotation_depth, right.annotation_depth),
        plan_nodes: checked_add(checked_add(left.plan_nodes, right.plan_nodes), Some(1)),
        derivations,
        interpretations,
        emissions,
        fit_checks: checked_add(left.fit_checks, right.fit_checks),
        fast_work: checked_add(option_max(left.fast_work, right.fast_work), Some(1)),
    }
}

fn required_bound(
    value: Option<u128>,
    resource: PreparedResource,
    maximum: u128,
) -> Result<u128, PrepareError> {
    value.ok_or_else(|| representation(resource, None, maximum))
}

fn compact_id(length: usize, resource: PreparedResource) -> Result<u32, PrepareError> {
    u32::try_from(length)
        .map_err(|_| representation(resource, Some(length as u128), u32::MAX as u128))
}

fn representation(
    resource: PreparedResource,
    required: Option<u128>,
    maximum: u128,
) -> PrepareError {
    PrepareError::RepresentationExceeded {
        resource,
        required,
        maximum,
    }
}

fn cost_error(component: CostComponent, required: Option<u128>, maximum: u128) -> PrepareError {
    PrepareError::CostDomainExceeded {
        component,
        required,
        maximum,
    }
}

fn require_at_most(
    resource: PreparedResource,
    required: u128,
    maximum: u128,
) -> Result<(), PrepareError> {
    if required > maximum {
        Err(representation(resource, Some(required), maximum))
    } else {
        Ok(())
    }
}

fn push_node_children<'a, A>(node: &'a Node<A>, mut push: impl FnMut(&'a Doc<A>)) {
    match node {
        Node::Seq(children) | Node::Fill(children) => {
            for child in children.iter().rev() {
                push(child);
            }
        }
        Node::Group(child)
        | Node::Nest { child, .. }
        | Node::Align(child)
        | Node::Annotate { child, .. }
        | Node::Penalty { child, .. } => push(child),
        Node::Choice {
            preferred,
            alternative,
        } => {
            push(alternative);
            push(preferred);
        }
        Node::Empty | Node::Text(_) | Node::Break(_) | Node::HardLine => {}
    }
}

impl<A> Doc<A> {
    /// Converts this source graph into an immutable prepared representation.
    ///
    /// Annotation handles are interned by identity, so `A` needs no equality,
    /// hashing, or clone implementation.
    pub fn prepare(&self) -> Result<PreparedDoc<A>, PrepareError> {
        Builder::new()?.prepare(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepare_with_limits(
        doc: &Doc<u32>,
        limits: BoundLimits,
    ) -> Result<PreparedDoc<u32>, PrepareError> {
        Builder::new()?.prepare_with_limits(doc, limits)
    }

    fn shared_fill(mut doc: Doc<u32>, depth: usize) -> Doc<u32> {
        for _ in 0..depth {
            doc = Doc::fill([doc.clone(), doc]);
        }
        doc
    }

    #[test]
    fn reduced_cost_domains_reject_the_first_invalid_component() {
        let overflow = Doc::<u32>::text("xxxxx");
        let error = match prepare_with_limits(
            &overflow,
            BoundLimits {
                overflow_cost: 24,
                ..BoundLimits::PRODUCTION
            },
        ) {
            Ok(_) => panic!("reduced overflow domain unexpectedly accepted the document"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            PrepareError::CostDomainExceeded {
                component: CostComponent::Overflow,
                required: Some(25),
                maximum: 24,
            }
        );

        let burden = shared_fill(Doc::penalize(10, Doc::text("")), 4);
        let error = match prepare_with_limits(
            &burden,
            BoundLimits {
                burden_cost: 159,
                ..BoundLimits::PRODUCTION
            },
        ) {
            Ok(_) => panic!("reduced burden domain unexpectedly accepted the document"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            PrepareError::CostDomainExceeded {
                component: CostComponent::Burden,
                required: Some(required),
                maximum: 159,
            } if required > 159
        ));
    }

    #[test]
    fn compact_shared_dags_cannot_hide_expanded_representation_bounds() {
        let output = shared_fill(Doc::<u32>::text("x"), 3);
        let error = match prepare_with_limits(
            &output,
            BoundLimits {
                output_bytes: 7,
                ..BoundLimits::PRODUCTION
            },
        ) {
            Ok(_) => panic!("reduced output domain unexpectedly accepted the document"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            PrepareError::RepresentationExceeded {
                resource: PreparedResource::OutputBytes,
                required: Some(required),
                maximum: 7,
            } if required > 7
        ));

        let spans = shared_fill(Doc::annotate(1, Doc::empty()), 2);
        assert!(matches!(
            prepare_with_limits(
                &spans,
                BoundLimits {
                    spans: 3,
                    ..BoundLimits::PRODUCTION
                },
            ),
            Err(PrepareError::RepresentationExceeded {
                resource: PreparedResource::SpanOccurrences,
                required: Some(required),
                maximum: 3,
            }) if required > 3
        ));

        let plans = shared_fill(Doc::<u32>::text("x"), 3);
        assert!(matches!(
            prepare_with_limits(
                &plans,
                BoundLimits {
                    plan_nodes: 20,
                    ..BoundLimits::PRODUCTION
                },
            ),
            Err(PrepareError::RepresentationExceeded {
                resource: PreparedResource::PlanNodes,
                required: Some(required),
                maximum: 20,
            }) if required > 20
        ));
    }

    #[test]
    fn advisory_work_overflow_does_not_reject_a_fast_usable_document() {
        let mut doc = Doc::<u32>::text("x");
        for _ in 0..128 {
            doc = Doc::choice(doc.clone(), doc);
        }
        let prepared = doc.prepare().unwrap();
        assert_eq!(prepared.bounds().candidate_emissions_upper(), None);
        assert_eq!(prepared.bounds().solve_work_items_upper(), None);
    }

    #[test]
    fn exact_choice_fusion_is_linear_and_stops_at_shared_subgraphs() {
        let alternative = |index: usize| {
            Doc::penalize(
                u32::try_from(index).unwrap(),
                Doc::<u32>::text("x".repeat(128 - index)),
            )
        };
        let chain = (1..128).fold(alternative(0), |doc, index| {
            Doc::choice(doc, alternative(index))
        });
        let prepared = chain.prepare().unwrap();
        let choice = prepared.0.exact_choices[prepared.0.root.index()].unwrap();
        assert_eq!(choice.alternatives.len, 128);
        assert_eq!(choice.skipped_choices, 126);
        assert_eq!(prepared.0.exact_choice_edges.len(), 128);
        assert_eq!(
            prepared
                .0
                .exact_choices
                .iter()
                .filter(|choice| choice.is_some())
                .count(),
            1
        );

        let shared = Doc::choice(Doc::<u32>::text("a"), Doc::text("b"));
        let root = Doc::choice(shared.clone(), Doc::choice(shared, Doc::text("c")));
        let prepared = root.prepare().unwrap();
        let root_choice = prepared.0.exact_choices[prepared.0.root.index()].unwrap();
        assert_eq!(root_choice.alternatives.len, 3);
        assert_eq!(
            prepared
                .0
                .exact_choices
                .iter()
                .filter(|choice| choice.is_some())
                .count(),
            1
        );
    }
}

#[cfg(not(any(target_pointer_width = "32", target_pointer_width = "64")))]
compile_error!("laidout's prepared consumer kernel supports only 32-bit and 64-bit pointers");
