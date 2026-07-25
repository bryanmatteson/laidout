//! Prepared-document layout with application-owned annotations and reusable
//! allocation control.
//!
//! # Example
//!
//! ```
//! use std::num::NonZeroU32;
//!
//! use laidout::{render, Doc, RenderOptions};
//!
//! let doc = Doc::<u32>::group(Doc::concat([
//!     Doc::text("status"),
//!     Doc::line(),
//!     Doc::text("ready"),
//! ]));
//! let options = RenderOptions::new(NonZeroU32::new(12).unwrap());
//! let output = render(&doc, options)?;
//! assert_eq!(output.text(), "status ready");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
#![forbid(unsafe_code)]
#![warn(missing_docs)]

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
    softline, tag, text, try_text, try_text_with, Doc, TagId, TextError, TextErrorKind, TextRun,
    WidthMode,
};
pub use kernel::{
    render_into, solve_into, ActiveAnnotations, AnnotationSpan, ExactSolveCapacity,
    FastSolveCapacity, IndentPolicy, LayoutRef, LayoutStrategy, LayoutVisitor, MaterializeCapacity,
    RenderCapacity, RenderError, RenderErrorKind, RenderOptions, RenderWorkspace, RenderedRef,
    ReserveError, ReserveErrorKind, SolveCounter, SolveStats, SpanId, VisitCapacity, VisitError,
    WorkspaceMode, WorkspaceResource,
};
pub use prepare::{
    AnnotationId, CostComponent, PrepareError, PrepareErrorKind, PreparedBounds, PreparedDoc,
    PreparedResource,
};
pub use render::{
    render, AnnotatedRun, OwnedOutputResource, OwnedRenderError, OwnedRenderErrorKind, Rendered,
};
pub use table::{table, Alignment, Column, Table, TableError, TableErrorKind};
pub use text::{from_text, from_text_with, from_text_with_annotations, IngestOptions, TextKind};

pub use render::{
    render_prepared, render_prepared_into_owned, RenderSummary, RenderedParts, Renderer, WriteError,
};

/// Common document construction and rendering types.
///
/// Import this module when a formatter only needs the primary document,
/// options, workspace, and rendering entry points.
pub mod prelude {
    pub use crate::{
        render, render_into, Doc, LayoutStrategy, RenderOptions, RenderWorkspace, Renderer,
    };
}

#[cfg(feature = "research")]
pub mod research;
