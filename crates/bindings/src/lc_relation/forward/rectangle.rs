// SPDX-License-Identifier: AGPL-3.0-only
//! Positive rectangle extrema and the explicit product-to-element feasibility map.

use super::{
    AT, ExactConstant, LcQuantity, LcRelation, LcRelationError, LcValues, Rational, storage,
};
use crate::{Budget, LcInterval};
use std::cmp::Ordering;

/// Every admitted input is either one of these independent supports or positive energy.
#[derive(Debug)]
pub(super) struct Rectangle {
    /// Exact positive inductance interval.
    inductance: [Rational; 2],
    /// Exact positive capacitance interval.
    capacitance: [Rational; 2],
    /// Original finite initial-phase interval on the declared lift.
    phase: [Rational; 2],
    /// Complete positive rate image of the element product, with rational extrema.
    rate: [Rational; 2],
    /// One exact positive energy satisfying every supplied energy bound.
    energy: Rational,
}

impl Rectangle {
    /// Every supplied constraint must belong to the independently varying supported rectangle.
    pub(super) fn check(
        relation: &LcRelation,
        budget: &mut Budget,
    ) -> Result<Self, LcRelationError> {
        let slots = collect_slots(
            relation,
            [
                LcQuantity::Inductance,
                LcQuantity::Capacitance,
                LcQuantity::InitialPhase,
                LcQuantity::Energy,
            ],
            budget,
        )?;
        let [inductance, capacitance, phase, energy] = slots;
        let inductance = inductance.ok_or(LcRelationError::Projection)?;
        let capacitance = capacitance.ok_or(LcRelationError::Projection)?;
        let phase = phase.ok_or(LcRelationError::Projection)?;
        for lower in [&inductance[0], &capacitance[0]] {
            if lower.is_negative() || lower.is_zero() {
                return Err(LcRelationError::Projection);
            }
        }
        let product = [
            inductance[0].mul(&capacitance[0], budget)?,
            inductance[1].mul(&capacitance[1], budget)?,
        ];
        let rate = [
            inverse_root(&product[1], budget)?,
            inverse_root(&product[0], budget)?,
        ];
        let energy = energy.map_or_else(Rational::one, |range| range[1].clone());
        if energy.is_negative() || energy.is_zero() {
            return Err(LcRelationError::Projection);
        }
        Ok(Self {
            inductance,
            capacitance,
            phase,
            rate,
            energy,
        })
    }

    /// Positive multiplication and independent phase addition give the complete interval image.
    pub(super) fn lifted(
        &self,
        time: &[Rational; 2],
        budget: &mut Budget,
    ) -> Result<[Rational; 2], LcRelationError> {
        Ok([
            self.phase[0].add(&self.rate[0].mul(&time[0], budget)?, budget)?,
            self.phase[1].add(&self.rate[1].mul(&time[1], budget)?, budget)?,
        ])
    }

    /// A positive rate fixes P; the explicit element map stays inside the whole rectangle.
    pub(super) fn values(
        &self,
        rate: Rational,
        phase: Rational,
        budget: &mut Budget,
    ) -> Result<LcValues, LcRelationError> {
        for (value, range) in [(&rate, &self.rate), (&phase, &self.phase)] {
            if value.compare(&range[0], budget)? == Ordering::Less
                || value.compare(&range[1], budget)? == Ordering::Greater
            {
                return Err(LcRelationError::Containment);
            }
        }
        let product = Rational::one().div(&rate.pow(2, budget)?, budget)?;
        let proposed = product.div(&self.capacitance[1], budget)?;
        let inductance = if proposed.compare(&self.inductance[0], budget)? == Ordering::Less {
            self.inductance[0].clone()
        } else {
            proposed
        };
        let capacitance = product.div(&inductance, budget)?;
        let twice_energy = self
            .energy
            .mul(&Rational::from_ratio(2, 1, budget)?, budget)?;
        let voltage = root(twice_energy.div(&capacitance, budget)?, budget)?;
        let current = root(twice_energy.div(&inductance, budget)?, budget)?;
        Ok(LcValues {
            inductance: inductance.into(),
            capacitance: capacitance.into(),
            rate: rate.into(),
            initial_phase: Some(phase.into()),
            voltage,
            current,
            energy: self.energy.clone().into(),
            observations: Vec::new(),
        })
    }

    /// Conservative exact ownership includes every derived rate endpoint.
    pub(super) fn bytes(&self) -> u64 {
        self.inductance
            .iter()
            .chain(&self.capacitance)
            .chain(&self.phase)
            .chain(&self.rate)
            .chain(std::iter::once(&self.energy))
            .fold(256_u64, |bytes, value| {
                bytes.saturating_add(storage::rational(value))
            })
    }
}

/// Collect all permitted roles, intersecting every repeated original declaration.
pub(super) fn collect_slots<const N: usize>(
    relation: &LcRelation,
    accepted: [LcQuantity; N],
    budget: &mut Budget,
) -> Result<[Option<[Rational; 2]>; N], LcRelationError> {
    budget.work(1, AT)?;
    if relation.observations() != 0 {
        return Err(LcRelationError::Projection);
    }
    let mut slots: [Option<[Rational; 2]>; N] = std::array::from_fn(|_| None);
    for input in relation.inputs() {
        budget.work(1, AT)?;
        let slot = accepted
            .iter()
            .position(|role| *role == input.quantity())
            .ok_or(LcRelationError::Projection)?;
        let next = bounds(input.interval(), budget)?;
        slots[slot] = Some(match slots[slot].take() {
            Some(previous) => intersect(previous, &next, budget)?,
            None => next,
        });
    }
    Ok(slots)
}

/// This adapter refuses unbounded or nonrational endpoints instead of narrowing them.
pub(super) fn bounds(
    interval: &LcInterval,
    budget: &mut Budget,
) -> Result<[Rational; 2], LcRelationError> {
    budget.work(1, AT)?;
    let lower = interval
        .lower()
        .and_then(ExactConstant::as_rational)
        .ok_or(LcRelationError::Projection)?;
    let upper = interval
        .upper()
        .and_then(ExactConstant::as_rational)
        .ok_or(LcRelationError::Projection)?;
    lower.check(budget, AT)?;
    upper.check(budget, AT)?;
    Ok([lower.clone(), upper.clone()])
}

/// Repeated declarations mean intersection, never replacement by the last supplied interval.
pub(super) fn intersect(
    mut left: [Rational; 2],
    right: &[Rational; 2],
    budget: &mut Budget,
) -> Result<[Rational; 2], LcRelationError> {
    if right[0].compare(&left[0], budget)? == Ordering::Greater {
        left[0].clone_from(&right[0]);
    }
    if right[1].compare(&left[1], budget)? == Ordering::Less {
        left[1].clone_from(&right[1]);
    }
    if left[0].compare(&left[1], budget)? == Ordering::Greater {
        return Err(LcRelationError::Projection);
    }
    Ok(left)
}

/// Existing positive-root normalization must establish rational extrema for this closed adapter.
fn inverse_root(value: &Rational, budget: &mut Budget) -> Result<Rational, LcRelationError> {
    let root = root(value.clone(), budget)?;
    let rational = root.as_rational().ok_or(LcRelationError::Projection)?;
    Ok(Rational::one().div(rational, budget)?)
}

/// Positive exact square roots retain the CAS strict real domain.
fn root(value: Rational, budget: &mut Budget) -> Result<ExactConstant, LcRelationError> {
    Ok(ExactConstant::from(value)
        .positive()
        .ok_or(LcRelationError::Positivity)?
        .sqrt(budget)?)
}
