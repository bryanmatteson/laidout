//! Research pretty-printer: parametric-cost optimal layout.
//!
//! Three implementations of the same layout problem, kept honest against
//! each other:
//!
//! - [`brute`]: exponential enumeration of every layout — ground truth.
//! - [`greedy`]: the classical Wadler/Leijen first-fit printer — baseline.
//! - [`frontier`]: memoized Pareto-frontier search — the engine under study.
//!
//! Non-pruning engines accept parametric [`cost::CostModel`] implementations;
//! exact search accepts only the sealed [`cost::LawfulCostModel`] set. The
//! high-level [`render()`] facade uses [`cost::ConsumerCostModel`], while
//! [`cost::OverflowThenHeight`] remains the research baseline. Documents carry
//! stored terminal widths, structural semantic tags, and branch-local
//! penalties; tags never affect layout ([`doc::Doc::Tag`]).

pub mod brute;
pub mod corpus;
pub mod cost;
pub mod doc;
pub mod frontier;
pub mod greedy;
pub mod measure;
pub mod render;
pub mod table;
pub mod tags;
pub mod text;
pub mod tokens;

pub use cost::{ConsumerCost, ConsumerCostModel, CostModel, LawfulCostModel, OverflowThenHeight};
pub use doc::{
    align, choice, concat, concat2, count_choices, empty, flatten, group, hardline, join, line,
    nest, penalize, softline, tag, text, try_text, try_text_with, Doc, TagId, TextError, TextRun,
    WidthMode,
};
pub use frontier::{RenderError, SolveLimitKind, SolveLimits, SolveStats};
pub use render::{
    render, render_with, to_lines, to_string, words, AnnotatedRun, AnnotationSpan, RenderOptions,
    Rendered,
};
pub use table::{table, Alignment, Column, Table, TableError};
pub use text::{from_text, from_text_with, IngestOptions};
