// SPDX-License-Identifier: AGPL-3.0-only
//! Typed numerical API views, distinct from exact decimal wire quantities.

/// Typed percentile interval; its point estimate need not lie inside the interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConfidenceInterval<Unit> {
    /// Exact requested binary64 confidence, not its rounded display.
    pub confidence: f64,
    /// Lower endpoint in the named unit.
    pub lower: Unit,
    /// Upper endpoint in the same unit.
    pub upper: Unit,
}

/// Host point estimate, bootstrap standard error and conditional interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Estimate<Unit> {
    /// Unrounded host point estimate.
    pub point: Unit,
    /// Unrounded bootstrap standard error in the same unit.
    pub standard_error: Unit,
    /// Conditional percentile interval, not a numerical-error enclosure.
    pub interval: ConfidenceInterval<Unit>,
}

/// Reciprocal throughput view without an invented reciprocal standard error.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rate<Unit> {
    /// Positive exact per-item work count.
    pub numerator: u64,
    /// Correctly rounded binary64 view of the exact numerator/time ratio.
    pub point: Unit,
    /// Correctly rounded binary64 views of reversed reciprocal endpoints.
    pub interval: ConfidenceInterval<Unit>,
}

/// Define a unit wrapper without importing engine dimensional/certification semantics.
macro_rules! unit {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct $name(pub f64);
    };
}

unit!(
    SecondsPerItem,
    "Host wall-time seconds per representative aggregate item."
);
unit!(
    ItemsPerSecond,
    "Aggregate representative items per wall-time second."
);
unit!(
    ElementsPerSecond,
    "Explicit scalar elements processed per wall-time second."
);
unit!(
    VectorOperationsPerSecond,
    "Logical vector operations per second, not inspected hardware instructions."
);
unit!(
    FloatingOperationsPerSecond,
    "Conventional recipe floating operations per wall-time second."
);
unit!(
    LogicalBytesPerSecond,
    "Logical recipe bytes per second, not physical memory-bus traffic."
);

impl crate::Measurement {
    /// Unrounded complete host time estimates with typed seconds-per-item units.
    #[must_use]
    pub fn time_estimates(&self) -> [Estimate<SecondsPerItem>; 5] {
        let confidence = f64::from_bits(self.analysis.confidence.source_bits.0);
        self.analysis.estimates.each_ref().map(|estimate| Estimate {
            point: SecondsPerItem(f64::from_bits(estimate.point.source_bits.0)),
            standard_error: SecondsPerItem(f64::from_bits(estimate.standard_error.source_bits.0)),
            interval: ConfidenceInterval {
                confidence,
                lower: SecondsPerItem(f64::from_bits(estimate.lower.source_bits.0)),
                upper: SecondsPerItem(f64::from_bits(estimate.upper.source_bits.0)),
            },
        })
    }

    /// Typed item throughput; exact decimal endpoints remain in the raw rate record.
    #[must_use]
    pub fn items_per_second(&self) -> Option<Rate<ItemsPerSecond>> {
        self.rate(0, ItemsPerSecond)
    }
    /// Typed scalar-element throughput.
    #[must_use]
    pub fn elements_per_second(&self) -> Option<Rate<ElementsPerSecond>> {
        self.rate(1, ElementsPerSecond)
    }
    /// Typed logical vector throughput, absent for scalar/backend recipes.
    #[must_use]
    pub fn vector_operations_per_second(&self) -> Option<Rate<VectorOperationsPerSecond>> {
        self.rate(2, VectorOperationsPerSecond)
    }
    /// Typed conventional arithmetic throughput, absent without a recipe ledger.
    #[must_use]
    pub fn floating_operations_per_second(&self) -> Option<Rate<FloatingOperationsPerSecond>> {
        self.rate(3, FloatingOperationsPerSecond)
    }
    /// Typed logical byte throughput, absent for register arithmetic.
    #[must_use]
    pub fn logical_bytes_per_second(&self) -> Option<Rate<LogicalBytesPerSecond>> {
        self.rate(4, LogicalBytesPerSecond)
    }

    /// Construct a typed rate from preserved correctly rounded raw views.
    fn rate<Unit>(&self, index: usize, wrap: impl Fn(f64) -> Unit) -> Option<Rate<Unit>> {
        let rate = self.rates.get(index)?.as_ref()?;
        Some(Rate {
            numerator: rate.decimal.numerator,
            point: wrap(f64::from_bits(rate.source_bits[0]?.0)),
            interval: ConfidenceInterval {
                confidence: f64::from_bits(self.analysis.confidence.source_bits.0),
                lower: wrap(f64::from_bits(rate.source_bits[1]?.0)),
                upper: wrap(f64::from_bits(rate.source_bits[2]?.0)),
            },
        })
    }
}
