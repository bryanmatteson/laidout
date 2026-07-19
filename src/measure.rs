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
pub struct PreparedFootprint {
    pub nodes: u128,
    pub edges: u128,
    pub text_records: u128,
    pub text_bytes: u128,
    pub annotations: u128,
    pub fit_summaries: u128,
    pub flat_nodes: u128,
    pub exact_choice_records: u128,
    pub exact_choice_edges: u128,
    pub total: u128,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RetainedFootprint {
    pub logical_bytes: u128,
    pub actual_retained_bytes: u128,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkspaceFootprint {
    pub fast_work_items: RetainedFootprint,
    pub exact_work_items: RetainedFootprint,
    pub memo_entries: RetainedFootprint,
    pub retained_candidates: RetainedFootprint,
    pub frontier_scratch: RetainedFootprint,
    pub plan_nodes: RetainedFootprint,
    pub visit_work_items: RetainedFootprint,
    pub annotation_depth: RetainedFootprint,
    pub output_bytes: RetainedFootprint,
    pub spans: RetainedFootprint,
    pub logical_total: u128,
    pub actual_retained_total: u128,
}

const fn bytes(entries: usize, element: usize) -> u128 {
    entries as u128 * element as u128
}

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

pub fn workspace_footprint(workspace: &RenderWorkspace) -> WorkspaceFootprint {
    workspace.measure_footprint()
}
