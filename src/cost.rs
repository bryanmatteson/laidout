//! Checked compact consumer costs and optional unbounded research costs.

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// Lexicographically ordered production cost.
///
/// Overflow is minimized before layout burden.
pub struct ConsumerCost {
    pub(crate) overflow: u128,
    pub(crate) burden: u64,
}

impl ConsumerCost {
    /// Returns the squared horizontal-overflow component.
    pub const fn overflow(self) -> u128 {
        self.overflow
    }

    /// Returns the newline-and-penalty burden component.
    pub const fn burden(self) -> u64 {
        self.burden
    }

    pub(crate) const fn zero() -> Self {
        Self {
            overflow: 0,
            burden: 0,
        }
    }

    pub(crate) fn checked_add(self, other: Self) -> Option<Self> {
        Some(Self {
            overflow: self.overflow.checked_add(other.overflow)?,
            burden: self.burden.checked_add(other.burden)?,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PreparedConsumerCostModel {
    width: u32,
    newline_cost: u32,
}

impl PreparedConsumerCostModel {
    pub(crate) const fn new(width: u32, newline_cost: u32) -> Self {
        Self {
            width,
            newline_cost,
        }
    }

    fn over(self, column: u32) -> u128 {
        u128::from(column.saturating_sub(self.width))
    }

    pub(crate) fn text(self, column: u32, width: u32) -> Option<ConsumerCost> {
        let end_column = column.checked_add(width)?;
        let start = self.over(column);
        let end = self.over(end_column);
        let start_squared = start.checked_mul(start)?;
        let end_squared = end.checked_mul(end)?;
        Some(ConsumerCost {
            overflow: end_squared.checked_sub(start_squared)?,
            burden: 0,
        })
    }

    pub(crate) fn newline(self) -> ConsumerCost {
        ConsumerCost {
            overflow: 0,
            burden: u64::from(self.newline_cost),
        }
    }

    pub(crate) fn penalty(self, amount: u32) -> ConsumerCost {
        ConsumerCost {
            overflow: 0,
            burden: u64::from(amount),
        }
    }
}

/// Terminal display width using the default narrow Unicode policy.
pub fn display_width(value: &str) -> u32 {
    u32::try_from(unicode_width::UnicodeWidthStr::width(value))
        .expect("text display width exceeds u32::MAX")
}

#[cfg(test)]
mod tests {
    use super::{ConsumerCost, PreparedConsumerCostModel};

    #[test]
    fn every_compact_cost_operation_is_exact_at_its_accepted_boundary() {
        let almost_maximum = ConsumerCost {
            overflow: u128::MAX - 1,
            burden: u64::MAX - 1,
        };
        assert_eq!(
            almost_maximum
                .checked_add(ConsumerCost {
                    overflow: 1,
                    burden: 1,
                })
                .unwrap(),
            ConsumerCost {
                overflow: u128::MAX,
                burden: u64::MAX,
            }
        );
        assert!(ConsumerCost {
            overflow: u128::MAX,
            burden: 0,
        }
        .checked_add(ConsumerCost {
            overflow: 1,
            burden: 0,
        })
        .is_none());
        assert!(ConsumerCost {
            overflow: 0,
            burden: u64::MAX,
        }
        .checked_add(ConsumerCost {
            overflow: 0,
            burden: 1,
        })
        .is_none());

        let model = PreparedConsumerCostModel::new(0, u32::MAX);
        let maximum_line = model.text(0, u32::MAX).unwrap();
        assert_eq!(
            maximum_line.overflow,
            u128::from(u32::MAX) * u128::from(u32::MAX)
        );
        assert_eq!(model.newline().burden, u64::from(u32::MAX));
        assert_eq!(model.penalty(u32::MAX).burden, u64::from(u32::MAX));
        assert!(model.text(1, u32::MAX).is_none());
    }

    #[test]
    fn secondary_saturation_would_destroy_preferred_strict_order() {
        let preferred = (0u128, 1u64);
        let later = (0u128, 0u64);
        assert!(later < preferred);
        let suffix = u64::MAX;
        assert_eq!(
            preferred.1.saturating_add(suffix),
            later.1.saturating_add(suffix)
        );
        assert!(ConsumerCost {
            overflow: preferred.0,
            burden: preferred.1,
        }
        .checked_add(ConsumerCost {
            overflow: 0,
            burden: suffix,
        })
        .is_none());
    }
}

#[cfg(feature = "research")]
/// Arbitrary-precision cost models and laws used by research engines.
pub mod research {
    use std::fmt::Debug;

    use num_bigint::BigUint;

    /// Defines the cost algebra used by an allocating research engine.
    pub trait CostModel {
        /// Ordered cost value produced by the model.
        type Cost: Clone + Ord + Debug;

        /// Returns the additive identity.
        fn zero(&self) -> Self::Cost;
        /// Adds two independent cost contributions.
        fn add(&self, left: &Self::Cost, right: &Self::Cost) -> Self::Cost;
        /// Costs a text run beginning at `column`.
        fn text(&self, column: u32, width: u32) -> Self::Cost;
        /// Costs one newline.
        fn newline(&self) -> Self::Cost;
        /// Costs an explicit document penalty.
        fn penalty(&self, amount: u32) -> Self::Cost;
    }

    mod sealed {
        pub trait Sealed {}
    }

    /// Sealed marker for models that satisfy the frontier pruning laws.
    pub trait LawfulCostModel: CostModel + sealed::Sealed {}

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    /// Arbitrary-precision counterpart to [`super::ConsumerCost`].
    pub struct ResearchConsumerCost {
        /// Squared horizontal-overflow component.
        pub overflow: BigUint,
        /// Newline-and-penalty burden component.
        pub burden: BigUint,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    /// Arbitrary-precision version of the production consumer model.
    pub struct ResearchConsumerCostModel {
        width: u32,
        newline_cost: u32,
    }

    impl ResearchConsumerCostModel {
        /// Creates a model for the requested page width.
        pub const fn new(width: u32) -> Self {
            Self {
                width,
                newline_cost: 1,
            }
        }

        /// Sets the burden charged for each newline.
        pub const fn with_newline_cost(mut self, newline_cost: u32) -> Self {
            self.newline_cost = newline_cost;
            self
        }

        fn over(self, column: u32) -> u64 {
            u64::from(column.saturating_sub(self.width))
        }
    }

    impl CostModel for ResearchConsumerCostModel {
        type Cost = ResearchConsumerCost;

        fn zero(&self) -> Self::Cost {
            ResearchConsumerCost {
                overflow: BigUint::from(0u8),
                burden: BigUint::from(0u8),
            }
        }

        fn add(&self, left: &Self::Cost, right: &Self::Cost) -> Self::Cost {
            ResearchConsumerCost {
                overflow: &left.overflow + &right.overflow,
                burden: &left.burden + &right.burden,
            }
        }

        fn text(&self, column: u32, width: u32) -> Self::Cost {
            let start = self.over(column);
            let end = self.over(column.checked_add(width).expect("research column overflow"));
            ResearchConsumerCost {
                overflow: BigUint::from(end * end - start * start),
                burden: BigUint::from(0u8),
            }
        }

        fn newline(&self) -> Self::Cost {
            ResearchConsumerCost {
                overflow: BigUint::from(0u8),
                burden: BigUint::from(self.newline_cost),
            }
        }

        fn penalty(&self, amount: u32) -> Self::Cost {
            ResearchConsumerCost {
                overflow: BigUint::from(0u8),
                burden: BigUint::from(amount),
            }
        }
    }

    impl sealed::Sealed for ResearchConsumerCostModel {}
    impl LawfulCostModel for ResearchConsumerCostModel {}

    #[derive(Clone, Copy, Debug)]
    /// Research model that minimizes overflow and then final height.
    pub struct OverflowThenHeight {
        /// Target page width in display columns.
        pub width: u32,
    }

    impl CostModel for OverflowThenHeight {
        type Cost = (BigUint, BigUint);

        fn zero(&self) -> Self::Cost {
            (BigUint::from(0u8), BigUint::from(0u8))
        }

        fn add(&self, left: &Self::Cost, right: &Self::Cost) -> Self::Cost {
            (&left.0 + &right.0, &left.1 + &right.1)
        }

        fn text(&self, column: u32, width: u32) -> Self::Cost {
            let over = |value: u32| u64::from(value.saturating_sub(self.width));
            let start = over(column);
            let end = over(column.checked_add(width).expect("research column overflow"));
            (BigUint::from(end * end - start * start), BigUint::from(0u8))
        }

        fn newline(&self) -> Self::Cost {
            (BigUint::from(0u8), BigUint::from(1u8))
        }

        fn penalty(&self, _amount: u32) -> Self::Cost {
            (BigUint::from(0u8), BigUint::from(0u8))
        }
    }

    impl sealed::Sealed for OverflowThenHeight {}
    impl LawfulCostModel for OverflowThenHeight {}
}
