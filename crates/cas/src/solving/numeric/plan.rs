// SPDX-License-Identifier: AGPL-3.0-only
//! Stack-only bounds propagated through the complete scalar operation sequence.

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, Location};
use crate::number::Rational;

/// Bounds for the two canonical magnitudes of one rational.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Size {
    /// Numerator magnitude bits, including zero's zero-bit representation.
    numerator: u64,
    /// Positive denominator magnitude bits.
    denominator: u64,
}

impl Size {
    /// Inspect only cached magnitude lengths, validating the caller's smaller ceilings.
    pub(crate) fn of(value: &Rational, budget: &Budget, at: Location) -> Result<Self, Fault> {
        value.check(budget, at)?;
        let (numerator, denominator) = value.component_bits();
        budget.check(BudgetKind::IntermediateBits, numerator.max(denominator), at)?;
        Ok(Self {
            numerator,
            denominator,
        })
    }

    /// Bound a fixed zero or unit without allocating its representation.
    pub(crate) const fn constant(nonzero: bool) -> Self {
        Self {
            numerator: if nonzero { 1 } else { 0 },
            denominator: 1,
        }
    }

    /// Largest component bound.
    pub(crate) fn bits(self) -> u64 {
        self.numerator.max(self.denominator)
    }
}

/// Scalar formulas used solely to propagate bounds, never to compute values.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Scalar {
    /// Cross-products and one signed sum.
    Add,
    /// A negation followed by addition.
    Sub,
    /// Cross-cancellation and products.
    Mul,
    /// A reciprocal followed by multiplication.
    Div,
}

/// Conservative envelope; the caller still measures every scalar operation.
#[derive(Debug, Default)]
pub(crate) struct Plan {
    /// Primitive arithmetic calls, following the existing `Budget::arithmetic` unit.
    steps: u64,
    /// Limb work, including retained copies.
    work: u64,
    /// Prepaid upper bound for Euclidean iterations hidden in rational operations.
    gcd: u64,
    /// Largest bigint, including normalized long-division scratch.
    intermediate: u64,
    /// Conservative sum of scratch envelopes, larger than their simultaneous peak.
    bytes: u64,
}

/// Checked arithmetic on a resource prediction.
pub(crate) fn add(a: u64, b: u64, kind: BudgetKind, at: Location) -> Result<u64, Fault> {
    a.checked_add(b).ok_or_else(|| Fault::capacity(kind, at))
}

/// Checked multiplication on a resource prediction.
pub(crate) fn mul(a: u64, b: u64, kind: BudgetKind, at: Location) -> Result<u64, Fault> {
    a.checked_mul(b).ok_or_else(|| Fault::capacity(kind, at))
}

/// The design's conservative integer arithmetic cost `C(B)`.
pub(crate) fn cost(bits: u64, at: Location) -> Result<u64, Fault> {
    let limbs = add(bits, 63, BudgetKind::Work, at)? / 64;
    let extent = add(limbs.max(1), 1, BudgetKind::Work, at)?;
    mul(
        mul(extent, extent, BudgetKind::Work, at)?,
        8,
        BudgetKind::Work,
        at,
    )
}

impl Plan {
    /// Reserve an owned integer magnitude, including limb rounding and retained bytes.
    pub(crate) fn magnitude_copy(&mut self, bits: u64, at: Location) -> Result<(), Fault> {
        self.copy(
            Size {
                numerator: bits,
                denominator: 1,
            },
            at,
        )
    }

    /// Account for copies and scans, including full limbs and result storage.
    pub(crate) fn copy(&mut self, size: Size, at: Location) -> Result<(), Fault> {
        self.steps = add(self.steps, 1, BudgetKind::ArithmeticSteps, at)?;
        self.work = add(self.work, cost(size.bits(), at)?, BudgetKind::Work, at)?;
        self.storage(size.bits(), at)
    }

    /// Reserve all temporary magnitudes, rounding dependency limbs conservatively.
    fn storage(&mut self, bits: u64, at: Location) -> Result<(), Fault> {
        self.intermediate = self.intermediate.max(bits);
        let limb_bytes = mul(
            add(bits, 63, BudgetKind::StagedPayloadBytes, at)? / 64,
            8,
            BudgetKind::StagedPayloadBytes,
            at,
        )?;
        let bytes = add(
            mul(limb_bytes, 32, BudgetKind::StagedPayloadBytes, at)?,
            256,
            BudgetKind::StagedPayloadBytes,
            at,
        )?;
        self.bytes = add(self.bytes, bytes, BudgetKind::StagedPayloadBytes, at)?;
        Ok(())
    }

    /// Propagate raw products and sums before any scalar dependency is called.
    pub(crate) fn scalar(
        &mut self,
        operation: Scalar,
        left: Size,
        right: Size,
        at: Location,
    ) -> Result<Size, Fault> {
        let mut rhs = right;
        if matches!(operation, Scalar::Div) {
            std::mem::swap(&mut rhs.numerator, &mut rhs.denominator);
        }
        let sum = |a, b| add(a, b, BudgetKind::IntermediateBits, at);
        let denominator = sum(left.denominator, rhs.denominator)?;
        let numerator = if matches!(operation, Scalar::Add | Scalar::Sub) {
            sum(
                sum(left.numerator, rhs.denominator)?.max(sum(rhs.numerator, left.denominator)?),
                1,
            )?
        } else {
            sum(left.numerator, rhs.numerator)?
        };
        let result = Size {
            numerator,
            denominator,
        };
        self.rational(left.bits().max(right.bits()), result.bits(), at)?;
        if matches!(operation, Scalar::Sub | Scalar::Div) {
            self.copy(right, at)?;
        }
        Ok(result)
    }

    /// The rational envelope bounds all gcds, quotients, products and scans.
    fn rational(&mut self, input: u64, raw: u64, at: Location) -> Result<(), Fault> {
        let bound = add(
            mul(input, 2, BudgetKind::IntermediateBits, at)?,
            1,
            BudgetKind::IntermediateBits,
            at,
        )?
        .max(raw);
        // `Integer::div_rem` may normalize a multi-limb divisor by up to 63 bits.
        let scratch = if bound > 32 {
            add(bound, 63, BudgetKind::IntermediateBits, at)?
        } else {
            bound
        };
        let scaled = mul(input, 8, BudgetKind::ArithmeticSteps, at)?;
        let calls = add(scaled, 32, BudgetKind::ArithmeticSteps, at)?;
        self.steps = add(self.steps, calls, BudgetKind::ArithmeticSteps, at)?;
        let work = mul(calls, cost(scratch, at)?, BudgetKind::Work, at)?;
        self.work = add(self.work, work, BudgetKind::Work, at)?;
        let remainders = add(scaled, 3, BudgetKind::GcdSteps, at)?;
        self.gcd = add(self.gcd, remainders, BudgetKind::GcdSteps, at)?;
        self.storage(scratch, at)
    }

    /// Check the entire sequence, then retain its conservative gcd reservation.
    /// No bigint allocation or scalar arithmetic precedes these checks.
    pub(crate) fn reserve(&self, budget: &mut Budget, at: Location) -> Result<(), Fault> {
        budget.check(BudgetKind::IntermediateBits, self.intermediate, at)?;
        usize::try_from(self.bytes)
            .map_err(|_| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?;
        budget.check(BudgetKind::StagedPayloadBytes, self.bytes, at)?;
        let usage = budget.usage();
        budget.check(
            BudgetKind::ArithmeticSteps,
            add(
                usage.arithmetic_steps,
                self.steps,
                BudgetKind::ArithmeticSteps,
                at,
            )?,
            at,
        )?;
        budget.check(
            BudgetKind::GcdSteps,
            add(usage.gcd_steps, self.gcd, BudgetKind::GcdSteps, at)?,
            at,
        )?;
        let work = add(self.work, u64::from(self.gcd != 0), BudgetKind::Work, at)?;
        budget.check(
            BudgetKind::Work,
            add(usage.work, work, BudgetKind::Work, at)?,
            at,
        )?;
        if self.gcd != 0 {
            budget.solving(BudgetKind::GcdSteps, self.gcd, at)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Plan, Scalar, Size, cost};
    use crate::budget::BudgetKind;
    use crate::error::{Fault, Location};

    /// Machine-capacity overflow is caught in stack-only predictions before allocation.
    /// ´test:unit:numeric-bound-capacity´
    #[test]
    fn numeric_bound_capacity() {
        let at = Location::new("numeric capacity test");
        assert_eq!(
            cost(u64::MAX, at),
            Err(Fault::capacity(BudgetKind::Work, at))
        );
        let huge = Size {
            numerator: u64::MAX,
            denominator: 1,
        };
        let mut plan = Plan::default();
        assert_eq!(
            plan.scalar(Scalar::Mul, huge, huge, at).err(),
            Some(Fault::capacity(BudgetKind::IntermediateBits, at))
        );
        assert_eq!(plan.steps, 0);
        assert_eq!(plan.work, 0);
        assert_eq!(plan.gcd, 0);
    }
}
