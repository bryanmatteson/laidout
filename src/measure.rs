//! Allocation-free retained-footprint telemetry for the prepared consumer kernel.
//!
//! This module is research-only because physical layout sizes are diagnostic,
//! not semantic API. Values are byte counts and never expose payloads or
//! mutable storage.

use std::mem::size_of;
use std::sync::Arc;

use crate::prepare::{ExactChoice, ModeFitSummary, NodeId, PreparedNode, PreparedText};
use crate::{PreparedDoc, RenderWorkspace};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
/// Byte footprint of every retained prepared-document table.
pub struct PreparedFootprint {
    /// Prepared node bytes.
    pub nodes: u128,
    /// Child-edge bytes.
    pub edges: u128,
    /// Text-record bytes.
    pub text_records: u128,
    /// UTF-8 payload bytes.
    pub text_bytes: u128,
    /// Annotation handle bytes.
    pub annotations: u128,
    /// Fast fit-summary bytes.
    pub fit_summaries: u128,
    /// Flat-projection table bytes.
    pub flat_nodes: u128,
    /// Exact-choice record bytes.
    pub exact_choice_records: u128,
    /// Exact-choice edge bytes.
    pub exact_choice_edges: u128,
    /// Sum of all prepared bytes.
    pub total: u128,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
/// Logical and allocator-retained bytes for one workspace resource.
pub struct RetainedFootprint {
    /// Bytes implied by public logical capacity.
    pub logical_bytes: u128,
    /// Bytes retained by backing allocations.
    pub actual_retained_bytes: u128,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
/// Per-resource byte footprint of a render workspace.
pub struct WorkspaceFootprint {
    /// Fast solver stacks.
    pub fast_work_items: RetainedFootprint,
    /// Exact solver stacks.
    pub exact_work_items: RetainedFootprint,
    /// Exact memo table.
    pub memo_entries: RetainedFootprint,
    /// Retained exact candidates.
    pub retained_candidates: RetainedFootprint,
    /// Frontier scratch candidates.
    pub frontier_scratch: RetainedFootprint,
    /// Materialized selected-layout nodes.
    pub plan_nodes: RetainedFootprint,
    /// Visitor traversal stack.
    pub visit_work_items: RetainedFootprint,
    /// Active annotation stacks.
    pub annotation_depth: RetainedFootprint,
    /// Materialized UTF-8 storage.
    pub output_bytes: RetainedFootprint,
    /// Materialized annotation spans.
    pub spans: RetainedFootprint,
    /// Sum of public logical bytes.
    pub logical_total: u128,
    /// Sum of backing allocation bytes.
    pub actual_retained_total: u128,
}

const fn bytes(entries: usize, element: usize) -> u128 {
    entries as u128 * element as u128
}

/// Measures the retained tables in a prepared document.
pub fn prepared_footprint<A>(prepared: &PreparedDoc<A>) -> PreparedFootprint {
    let data = &prepared.0;
    let nodes = bytes(data.nodes.len(), size_of::<PreparedNode>());
    let edges = bytes(data.edges.len(), size_of::<NodeId>());
    let text_records = bytes(data.texts.len(), size_of::<PreparedText>());
    let text_bytes = data.texts.iter().map(|text| text.value.len() as u128).sum();
    let annotations = bytes(data.annotations.len(), size_of::<Arc<A>>());
    let fit_summaries = bytes(data.fit_summaries.len(), size_of::<ModeFitSummary>());
    let flat_nodes = bytes(data.flat_nodes.len(), size_of::<Option<NodeId>>());
    let exact_choice_records = bytes(data.exact_choices.len(), size_of::<Option<ExactChoice>>());
    let exact_choice_edges = bytes(data.exact_choice_edges.len(), size_of::<NodeId>());
    PreparedFootprint {
        nodes,
        edges,
        text_records,
        text_bytes,
        annotations,
        fit_summaries,
        flat_nodes,
        exact_choice_records,
        exact_choice_edges,
        total: nodes
            + edges
            + text_records
            + text_bytes
            + annotations
            + fit_summaries
            + flat_nodes
            + exact_choice_records
            + exact_choice_edges,
    }
}

/// Measures logical and backing storage retained by a workspace.
pub fn workspace_footprint(workspace: &RenderWorkspace) -> WorkspaceFootprint {
    workspace.measure_footprint()
}
