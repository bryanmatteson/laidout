//! Parametric cost models, in the style of "A Pretty Expressive Printer"
//! (Porncharoenwase, Nguyen, Torlak; OOPSLA 2023).
//!
//! A cost model assigns a cost to placing a text run at a column and to
//! taking a line break; total cost is the sum over a rendering. The critical
//! contract is *incrementality*: placing one run of width `a + b` at column
//! `c` must cost exactly `text(c, a) + text(c + a, b)`, so that engines that
//! see text in different-sized pieces agree on cost.

use std::fmt::Debug;

use num_bigint::BigUint;
use unicode_width::UnicodeWidthStr;

/// Additive cost interface used by all rendering engines.
///
/// Arbitrary implementations are accepted by the brute-force and greedy
/// engines, which do not use algebraic laws to discard layouts. The exact
/// frontier accepts only [`LawfulCostModel`], whose implementations are sealed
/// to the models proved in `docs/cost-model-proofs.md`.
pub trait CostModel {
    type Cost: Clone + Ord + Debug;

    fn zero(&self) -> Self::Cost;
    fn add(&self, a: &Self::Cost, b: &Self::Cost) -> Self::Cost;
    /// Cost of placing `width` display columns of text starting at `col`.
    fn text(&self, col: u32, width: u32) -> Self::Cost;
    /// Cost of one line break.
    fn newline(&self) -> Self::Cost;
    /// Cost of an explicit branch-local stylistic penalty.
    fn penalty(&self, amount: u32) -> Self::Cost;
}

mod sealed {
    pub trait Sealed {}
}

/// A cost model whose laws are established for exact Pareto-frontier pruning.
///
/// The laws are:
///
/// - `zero` is a two-sided identity for associative `add`;
/// - `add` is monotone in both arguments;
/// - `text` is incremental under splitting;
/// - `text(c, width)` is nondecreasing in `c`;
/// - primitive text, newline, and penalty costs are nonnegative; and
/// - penalty cost depends only on its amount.
///
/// This trait is sealed because Rust trait bounds cannot express or verify
/// those semantic laws for arbitrary downstream implementations. See the
/// machine-checked obligations and mathematical derivation in
/// `docs/cost-model-proofs.md`.
///
/// ```compile_fail
/// use laidout::{CostModel, LawfulCostModel};
///
/// #[derive(Debug)]
/// struct External;
///
/// impl CostModel for External {
///     type Cost = u64;
///     fn zero(&self) -> u64 { 0 }
///     fn add(&self, a: &u64, b: &u64) -> u64 { a + b }
///     fn text(&self, _col: u32, width: u32) -> u64 { u64::from(width) }
///     fn newline(&self) -> u64 { 1 }
///     fn penalty(&self, amount: u32) -> u64 { u64::from(amount) }
/// }
///
/// impl LawfulCostModel for External {}
/// ```
pub trait LawfulCostModel: CostModel + sealed::Sealed {}

/// Default model: lexicographic (squared overflow area, line breaks).
///
/// Overflow past `width` is squared per line, which telescopes under the
/// incrementality contract: `text(c, w)` contributes
/// `over(c + w)^2 - over(c)^2`, so a full line ending at column `e` costs
/// exactly `over(e)^2` regardless of how the line was split into runs.
/// Height is a strictly weaker criterion, so the printer never trades
/// overflow for fewer lines.
#[derive(Clone, Copy, Debug)]
pub struct OverflowThenHeight {
    pub width: u32,
}

impl OverflowThenHeight {
    fn over(&self, col: u32) -> u64 {
        u64::from(col.saturating_sub(self.width))
    }
}

impl CostModel for OverflowThenHeight {
    type Cost = (BigUint, u32);

    fn zero(&self) -> Self::Cost {
        (BigUint::from(0u8), 0)
    }

    fn add(&self, a: &Self::Cost, b: &Self::Cost) -> Self::Cost {
        (&a.0 + &b.0, a.1.saturating_add(b.1))
    }

    fn text(&self, col: u32, width: u32) -> Self::Cost {
        let start = self.over(col);
        let end = self.over(col.checked_add(width).expect("cost-model column overflow"));
        (BigUint::from(end * end - start * start), 0)
    }

    fn newline(&self) -> Self::Cost {
        (BigUint::from(0u8), 1)
    }

    fn penalty(&self, _amount: u32) -> Self::Cost {
        (BigUint::from(0u8), 0)
    }
}

impl sealed::Sealed for OverflowThenHeight {}
impl LawfulCostModel for OverflowThenHeight {}

/// Cost used by the high-level consumer renderer.
///
/// Overflow is lexicographically dominant. `burden` combines line breaks and
/// explicit branch-local penalties in documented burden units.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ConsumerCost {
    /// Exact, unbounded squared-overflow area.
    pub overflow: BigUint,
    pub burden: u64,
}

/// Built-in practical cost model used by [`crate::render()`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConsumerCostModel {
    width: u32,
    newline_cost: u32,
}

impl ConsumerCostModel {
    pub fn new(width: u32) -> Self {
        Self {
            width,
            newline_cost: 1,
        }
    }

    pub fn with_newline_cost(mut self, newline_cost: u32) -> Self {
        self.newline_cost = newline_cost;
        self
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn newline_cost(&self) -> u32 {
        self.newline_cost
    }

    fn over(&self, col: u32) -> u64 {
        u64::from(col.saturating_sub(self.width))
    }
}

impl CostModel for ConsumerCostModel {
    type Cost = ConsumerCost;

    fn zero(&self) -> Self::Cost {
        ConsumerCost {
            overflow: BigUint::from(0u8),
            burden: 0,
        }
    }

    fn add(&self, a: &Self::Cost, b: &Self::Cost) -> Self::Cost {
        ConsumerCost {
            overflow: &a.overflow + &b.overflow,
            burden: a.burden.saturating_add(b.burden),
        }
    }

    fn text(&self, col: u32, width: u32) -> Self::Cost {
        let start = self.over(col);
        let end = self.over(col.checked_add(width).expect("cost-model column overflow"));
        ConsumerCost {
            overflow: BigUint::from(end * end - start * start),
            burden: 0,
        }
    }

    fn newline(&self) -> Self::Cost {
        ConsumerCost {
            overflow: BigUint::from(0u8),
            burden: u64::from(self.newline_cost),
        }
    }

    fn penalty(&self, amount: u32) -> Self::Cost {
        ConsumerCost {
            overflow: BigUint::from(0u8),
            burden: u64::from(amount),
        }
    }
}

impl sealed::Sealed for ConsumerCostModel {}
impl LawfulCostModel for ConsumerCostModel {}

/// Terminal display width of a text run according to Unicode width rules.
pub fn display_width(s: &str) -> u32 {
    u32::try_from(UnicodeWidthStr::width(s)).expect("text display width exceeds u32::MAX")
}

#[cfg(test)]
mod tests {
    use num_bigint::BigUint;

    use super::{display_width, ConsumerCost, ConsumerCostModel, CostModel};

    #[test]
    fn display_width_counts_terminal_columns() {
        assert_eq!(display_width("ascii"), 5);
        assert_eq!(display_width("界"), 2);
        assert_eq!(display_width("e\u{301}"), 1);
    }

    #[test]
    fn dominant_accumulation_preserves_lexicographic_order_past_u64_max() {
        let model = ConsumerCostModel::new(80);
        let less = ConsumerCost {
            overflow: BigUint::from(0u8),
            burden: u64::MAX,
        };
        let greater = ConsumerCost {
            overflow: BigUint::from(1u8),
            burden: 0,
        };
        let increment = ConsumerCost {
            overflow: BigUint::from(u64::MAX),
            burden: 0,
        };

        assert!(less < greater);
        assert!(model.add(&less, &increment) < model.add(&greater, &increment));

        let research = super::OverflowThenHeight { width: 80 };
        let less = (BigUint::from(0u8), u32::MAX);
        let greater = (BigUint::from(1u8), 0);
        let increment = (BigUint::from(u64::MAX), 0);
        assert!(less < greater);
        assert!(research.add(&less, &increment) < research.add(&greater, &increment));
    }
}
