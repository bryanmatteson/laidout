//! Research pretty-printer: parametric-cost optimal layout.
//!
//! Three implementations of the same layout problem, kept honest against
//! each other:
//!
//! - [`brute`]: exponential enumeration of every layout — ground truth.
//! - [`greedy`]: the classical Wadler/Leijen first-fit printer — baseline.
//! - [`frontier`]: memoized Pareto-frontier search — the engine under study.
//!
//! Costs are parametric ([`cost::CostModel`]); the default,
//! [`cost::OverflowThenHeight`], minimizes squared overflow past the target
//! width, then line count. Documents carry semantic tags that never affect
//! layout ([`doc::Doc::Tag`]).

pub mod brute;
pub mod corpus;
pub mod cost;
pub mod doc;
pub mod frontier;
pub mod greedy;
pub mod render;

pub use cost::{CostModel, OverflowThenHeight};
pub use doc::{
    choice, concat, concat2, count_choices, empty, flatten, group, hardline, join, line, nest,
    softline, tag, text, Doc, TagId,
};
pub use render::{to_lines, to_string, words};
