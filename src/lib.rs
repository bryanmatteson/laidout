//! Standalone prepared-document layout kernel.

pub mod cost;
pub mod doc;
mod kernel;
mod prepare;
mod render;
#[allow(deprecated)]
pub mod table;
pub mod tags;
#[allow(deprecated)]
pub mod text;
#[allow(deprecated)]
pub mod tokens;

#[cfg(feature = "research")]
pub mod measure;

pub use cost::ConsumerCost;
#[allow(deprecated)]
pub use doc::{
    align, choice, concat, concat2, empty, flatten, group, hardline, join, line, nest, penalize,
    softline, tag, text, try_text, try_text_with, Doc, TagId, TextError, TextRun, WidthMode,
};
pub use kernel::{
    render_into, solve_into, ActiveAnnotations, AnnotationSpan, ExactSolveCapacity,
    FastSolveCapacity, IndentPolicy, LayoutRef, LayoutStrategy, LayoutVisitor, MaterializeCapacity,
    RenderCapacity, RenderError, RenderOptions, RenderWorkspace, RenderedRef, ReserveError,
    SolveCounter, SolveStats, SpanId, VisitCapacity, VisitError, WorkspaceMode, WorkspaceResource,
};
pub use prepare::{
    AnnotationId, CostComponent, PrepareError, PreparedBounds, PreparedDoc, PreparedResource,
};
pub use render::{render, AnnotatedRun, OwnedOutputResource, OwnedRenderError, Rendered};
pub use table::{table, Alignment, Column, Table, TableError};
pub use text::{from_text, from_text_with, IngestOptions};

#[cfg(feature = "research")]
pub mod research;
