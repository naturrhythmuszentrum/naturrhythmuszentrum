// SPDX-License-Identifier: AGPL-3.0-only
//! Exact integer square-root proposals and independently squared rational bounds.

use super::{
    BigUint, Budget, BudgetKind, Fault, InvalidCode, Location, One, Rational, ToPrimitive, Zero,
    charge, product,
};

/// Square-root arithmetic has its own diagnostic boundary.
const AT: Location = Location::new("rational square root");

/// Construct the nonnegative integer floor root by bounded bit trials.
fn floor_root(value: &BigUint, budget: &mut Budget) -> Result<BigUint, Fault> {
    let bits = value.bits();
    budget.check(
        BudgetKind::IntermediateBits,
        bits.saturating_add(bits % 2),
        AT,
    )?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        bits.div_ceil(8).saturating_mul(16).saturating_add(128),
        AT,
    )?;
    let mut root = BigUint::zero();
    for bit in (0..bits.div_ceil(2)).rev() {
        charge(bits, bits, budget, AT)?;
        let shift =
            usize::try_from(bit).map_err(|_| Fault::capacity(BudgetKind::IntermediateBits, AT))?;
        let candidate = &root | (BigUint::one() << shift);
        let square = product(&candidate, &candidate, budget, AT)?;
        if square <= *value {
            root = candidate;
        }
    }
    Ok(root)
}

impl Rational {
    /// Recognize the canonical literal half without performing or charging arithmetic.
    pub(crate) fn is_half(&self) -> bool {
        let numerator_one = self.numerator().is_one();
        let denominator_two = self.denominator().to_u64() == Some(2);
        let nonnegative = !self.is_negative();
        nonnegative && numerator_one && denominator_two
    }

    /// Return a rational square root only after both canonical components square back exactly.
    pub(crate) fn square_root_exact(&self, budget: &mut Budget) -> Result<Option<Self>, Fault> {
        self.check(budget, AT)?;
        if self.is_negative() {
            return Err(Fault::invalid(InvalidCode::RefutedDomain, AT));
        }
        let numerator = floor_root(self.numerator(), budget)?;
        let denominator = floor_root(self.denominator(), budget)?;
        let exact_numerator = product(&numerator, &numerator, budget, AT)? == *self.numerator();
        let exact_denominator =
            product(&denominator, &denominator, budget, AT)? == *self.denominator();
        if exact_numerator && exact_denominator {
            Ok(Some(Self::new(false, &numerator, &denominator, budget)?))
        } else {
            Ok(None)
        }
    }

    /// Certify rational bounds by squaring them against the original nonnegative rational.
    pub(crate) fn square_root_bounds(&self, budget: &mut Budget) -> Result<(Self, Self), Fault> {
        self.check(budget, AT)?;
        if self.is_negative() {
            return Err(Fault::invalid(InvalidCode::RefutedDomain, AT));
        }
        let scaled = product(self.numerator(), self.denominator(), budget, AT)?;
        let floor = floor_root(&scaled, budget)?;
        let lower = Self::new(false, &floor, self.denominator(), budget)?;
        let square = lower.mul(&lower, budget)?;
        if square == *self {
            return Ok((lower.clone(), lower));
        }
        charge(floor.bits(), 1, budget, AT)?;
        let ceiling = &floor + BigUint::one();
        let upper = Self::new(false, &ceiling, self.denominator(), budget)?;
        let lower_valid = square.compare(self, budget)? == std::cmp::Ordering::Less;
        let upper_valid =
            upper.mul(&upper, budget)?.compare(self, budget)? == std::cmp::Ordering::Greater;
        if lower_valid && upper_valid {
            Ok((lower, upper))
        } else {
            Err(Fault::invalid(InvalidCode::EndpointMismatch, AT))
        }
    }
}
