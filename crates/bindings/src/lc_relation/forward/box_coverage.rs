// SPDX-License-Identifier: AGPL-3.0-only
//! Original positive box forward coverage for a signed finite time query.

use super::{AT, LcQuantity, LcRelation, LcRelationError, Rational, rectangle, storage};
use crate::{Budget, LcInput};
use std::cmp::Ordering;

/// Conditional positive-oscillation outer coverage, without an image converse.
#[derive(Debug)]
pub struct LcBoxForwardCover<'a> {
    /// Every independent original physical constraint remains in this borrowed owner.
    relation: &'a LcRelation,
    /// The exact signed query relative to the relation's declared epoch.
    time: &'a LcInput,
    /// Exact positive root bracket checked against the full original product support.
    bracket: [Rational; 2],
    /// Derived positive inverse-root outer interval.
    rate: [Rational; 2],
    /// The exact rational outer interval of lifted phase.
    lifted: [Rational; 2],
}

impl LcBoxForwardCover<'_> {
    /// The original relation is borrowed, including repeated input positions.
    #[must_use]
    pub const fn relation(&self) -> &LcRelation {
        self.relation
    }

    /// The signed original query remains distinct from observed timestamps.
    #[must_use]
    pub const fn time(&self) -> &LcInput {
        self.time
    }

    /// Checked positive square-root bracket `r,s`, not a rounded root value.
    #[must_use]
    pub const fn bracket(&self) -> &[Rational; 2] {
        &self.bracket
    }

    /// Safe inverse-root outer interval `[1/s,1/r]`.
    #[must_use]
    pub const fn rate(&self) -> &[Rational; 2] {
        &self.rate
    }

    /// Coverage of every original lifted phase for this signed query.
    #[must_use]
    pub const fn lifted(&self) -> &[Rational; 2] {
        &self.lifted
    }

    /// Conservative retained exact result record, excluding borrowed originals.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        (size_of::<Self>() as u64).saturating_add(rational_bytes(
            self.bracket.iter().chain(&self.rate).chain(&self.lifted),
        ))
    }
}

impl LcRelation {
    /// Check positive-oscillation rectangular coverage without a rational exact root.
    ///
    /// Every supplied constraint must be a finite rational L, C or initial-phase
    /// support. Repeated positions are intersected before product bounds. The
    /// caller supplies a rational positive root bracket; squared comparisons
    /// authenticate it against the complete positive element product interval.
    /// The four rate-time products are all retained in the extremum check, so
    /// negative and zero-crossing query times are admitted. Exact rest uses its
    /// separate recipe; this cover is conditional on positive oscillation.
    /// # Errors
    /// Refuses observations, missing or extra source roles, unbounded/irrational
    /// supports, nonpositive elements, invalid brackets and exhausted resources.
    pub fn box_forward_cover<'a>(
        &'a self,
        time: &'a LcInput,
        bracket: [Rational; 2],
        budget: &mut Budget,
    ) -> Result<LcBoxForwardCover<'a>, LcRelationError> {
        let [inductance, capacitance, phase] = supports(self, budget)?;
        if time.quantity() != LcQuantity::Time(0) {
            return Err(LcRelationError::Meaning);
        }
        let times = rectangle::bounds(time.interval(), budget)?;
        let live = rational_bytes(
            inductance
                .iter()
                .chain(&capacitance)
                .chain(&phase)
                .chain(&times)
                .chain(&bracket),
        );
        let (rate, lifted) = budget.with_retained_payload(live, AT, |budget| {
            cover_bounds(&inductance, &capacitance, &phase, &times, &bracket, budget)
        })?;
        let result = LcBoxForwardCover {
            relation: self,
            time,
            bracket,
            rate,
            lifted,
        };
        storage::retained(result.retained_payload_bytes(), budget)?;
        Ok(result)
    }
}

/// Bound the original product first, then all signed rate-time corners.
fn cover_bounds(
    inductance: &[Rational; 2],
    capacitance: &[Rational; 2],
    phase: &[Rational; 2],
    times: &[Rational; 2],
    bracket: &[Rational; 2],
    budget: &mut Budget,
) -> Result<([Rational; 2], [Rational; 2]), LcRelationError> {
    let product = [
        inductance[0].mul(&capacitance[0], budget)?,
        inductance[1].mul(&capacitance[1], budget)?,
    ];
    let live = rational_bytes(product.iter());
    budget.with_retained_payload(live, AT, |budget| {
        if bracket[0].is_negative()
            || bracket[0].is_zero()
            || bracket[0].compare(&bracket[1], budget)? == Ordering::Greater
            || bracket[0].pow(2, budget)?.compare(&product[0], budget)? == Ordering::Greater
            || bracket[1].pow(2, budget)?.compare(&product[1], budget)? == Ordering::Less
        {
            return Err(LcRelationError::Projection);
        }
        let rate = [
            Rational::one().div(&bracket[1], budget)?,
            Rational::one().div(&bracket[0], budget)?,
        ];
        let live = rational_bytes(rate.iter());
        budget.with_retained_payload(live, AT, |budget| {
            let products = four_products(&rate, times, budget)?;
            let live = rational_bytes(products.iter());
            let lifted = budget.with_retained_payload(live, AT, |budget| {
                let (minimum, maximum) = extrema(&products, budget)?;
                Ok::<_, LcRelationError>([
                    phase[0].add(minimum, budget)?,
                    phase[1].add(maximum, budget)?,
                ])
            })?;
            Ok((rate, lifted))
        })
    })
}

/// Existing conservative rational-storage tariff, accumulated without overflow.
fn rational_bytes<'a>(values: impl Iterator<Item = &'a Rational>) -> u64 {
    values.fold(0_u64, |bytes, value| {
        bytes.saturating_add(storage::rational(value))
    })
}

/// Intersect repeated original positions; an extra role cannot disappear.
fn supports(
    relation: &LcRelation,
    budget: &mut Budget,
) -> Result<[[Rational; 2]; 3], LcRelationError> {
    let slots = rectangle::collect_slots(
        relation,
        [
            LcQuantity::Inductance,
            LcQuantity::Capacitance,
            LcQuantity::InitialPhase,
        ],
        budget,
    )?;
    let [Some(inductance), Some(capacitance), Some(phase)] = slots else {
        return Err(LcRelationError::Projection);
    };
    if [&inductance[0], &capacitance[0]]
        .into_iter()
        .any(|lower| lower.is_negative() || lower.is_zero())
    {
        return Err(LcRelationError::Projection);
    }
    Ok([inductance, capacitance, phase])
}

/// Signed time requires all four endpoint products, including crossed corners.
fn four_products(
    rate: &[Rational; 2],
    time: &[Rational; 2],
    budget: &mut Budget,
) -> Result<[Rational; 4], LcRelationError> {
    Ok([
        rate[0].mul(&time[0], budget)?,
        rate[0].mul(&time[1], budget)?,
        rate[1].mul(&time[0], budget)?,
        rate[1].mul(&time[1], budget)?,
    ])
}

/// Compare exact products; order is not inferred from the signs of time endpoints.
fn extrema<'a>(
    products: &'a [Rational; 4],
    budget: &mut Budget,
) -> Result<(&'a Rational, &'a Rational), LcRelationError> {
    let mut minimum = &products[0];
    let mut maximum = &products[0];
    for value in &products[1..] {
        if value.compare(minimum, budget)? == Ordering::Less {
            minimum = value;
        }
        if value.compare(maximum, budget)? == Ordering::Greater {
            maximum = value;
        }
    }
    Ok((minimum, maximum))
}
