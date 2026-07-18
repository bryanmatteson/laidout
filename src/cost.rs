//! Parametric cost models, in the style of "A Pretty Expressive Printer"
//! (Porncharoenwase, Nguyen, Torlak; OOPSLA 2023).
//!
//! A cost model assigns a cost to placing a text run at a column and to
//! taking a line break; total cost is the sum over a rendering. The critical
//! contract is *incrementality*: placing one run of width `a + b` at column
//! `c` must cost exactly `text(c, a) + text(c + a, b)`, so that engines that
//! see text in different-sized pieces agree on cost.

use std::fmt::Debug;

use unicode_width::UnicodeWidthStr;

pub trait CostModel {
    type Cost: Clone + Ord + Debug;

    fn zero(&self) -> Self::Cost;
    fn add(&self, a: &Self::Cost, b: &Self::Cost) -> Self::Cost;
    /// Cost of placing `width` display columns of text starting at `col`.
    fn text(&self, col: u32, width: u32) -> Self::Cost;
    /// Cost of one line break.
    fn newline(&self) -> Self::Cost;
}

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
    type Cost = (u64, u32);

    fn zero(&self) -> Self::Cost {
        (0, 0)
    }

    fn add(&self, a: &Self::Cost, b: &Self::Cost) -> Self::Cost {
        (a.0 + b.0, a.1 + b.1)
    }

    fn text(&self, col: u32, width: u32) -> Self::Cost {
        let start = self.over(col);
        let end = self.over(col + width);
        (end * end - start * start, 0)
    }

    fn newline(&self) -> Self::Cost {
        (0, 1)
    }
}

/// Terminal display width of a text run according to Unicode width rules.
pub fn display_width(s: &str) -> u32 {
    u32::try_from(UnicodeWidthStr::width(s)).expect("text display width exceeds u32::MAX")
}

#[cfg(test)]
mod tests {
    use super::display_width;

    #[test]
    fn display_width_counts_terminal_columns() {
        assert_eq!(display_width("ascii"), 5);
        assert_eq!(display_width("界"), 2);
        assert_eq!(display_width("e\u{301}"), 1);
    }
}
