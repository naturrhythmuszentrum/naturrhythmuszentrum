// SPDX-License-Identifier: AGPL-3.0-only
//! Exact original rows for the timing, clock and phase branch.

use super::super::PiAffine;
use super::{
    AT, PI_AT, TimingClockPhaseRequest as Request, TimingClockPhaseSourceKind as Kind,
    TimingClockPhaseSourceSlot as Slot,
};
use crate::budget::{Budget, BudgetKind};
use crate::pi_bounds::CheckedPiBounds;
use crate::{Fault, FaultKind, InvalidCode, Rational};
use std::cmp::Ordering;

/// One exact affine inequality in the lifted coordinates.
#[derive(Clone)]
pub(super) struct Row {
    /// The coefficients of the lifted coordinates.
    pub(super) coefficient: Vec<Rational>,
    /// The symbolic upper bound of this inequality.
    pub(super) bound: PiAffine,
}
/// Return the fixed fault for an invalid claim.
pub(super) const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedCertificate, AT)
}
/// Locate a pi-authority failure at the pi premise.
const fn pi_fault(mut fault: Fault) -> Fault {
    if matches!(fault.kind, FaultKind::Invalid(InvalidCode::PendingPremise)) {
        fault.location = PI_AT;
    }
    fault
}
/// Create a checked exact integer coefficient.
fn integer(value: i64, b: &mut Budget) -> Result<Rational, Fault> {
    Rational::from_ratio(value, 1, b)
}
/// Add an integer turn to one exact phase value.
fn lifted(value: &Rational, turn: i64, b: &mut Budget) -> Result<PiAffine, Fault> {
    Ok(PiAffine {
        base: value.clone(),
        pi: integer(turn.checked_mul(2).ok_or_else(invalid)?, b)?,
    })
}
/// Reconstruct the complete source slot sequence.
fn expected_slots(request: &Request) -> Result<Vec<Slot>, Fault> {
    let mut result = Vec::new();
    let mut add = |kind, count| -> Result<(), Fault> {
        result
            .try_reserve(count)
            .map_err(|_| Fault::allocation(AT))?;
        for declaration in 0..count {
            result.push(Slot { kind, declaration });
        }
        Ok(())
    };
    add(Kind::Rate, request.rate_declarations.len())?;
    add(Kind::Initial, request.initial_declarations.len())?;
    add(Kind::Clock, request.clock_declarations.len())?;
    add(Kind::Offset, request.offset_declarations.len())?;
    for (i, observation) in request.observations.iter().enumerate() {
        add(Kind::Error(i), observation.error_declarations.len())?;
        add(Kind::Time(i), observation.time_declarations.len())?;
        add(Kind::Measured(i), observation.measured_declarations.len())?;
        add(Kind::Physical(i), observation.physical_declarations.len())?;
    }
    Ok(result)
}
/// Check source versions, order, and shared declarations.
fn verify_sources(request: &Request, b: &mut Budget) -> Result<(), Fault> {
    let expected = expected_slots(request)?;
    if expected.len() != request.source_order.len()
        || expected.len() != request.source_versions.len()
    {
        return Err(invalid());
    }
    for slot in &expected {
        let mut count = 0;
        for current in &request.source_order {
            b.work(1, AT)?;
            count += usize::from(slot == current);
        }
        if count != 1 {
            return Err(invalid());
        }
    }
    Ok(())
}
/// Intersect all declarations of one common support.
fn common_range(
    effective: &[Rational; 2],
    declarations: &[[Rational; 2]],
    b: &mut Budget,
) -> Result<(), Fault> {
    let Some(first) = declarations.first() else {
        return Err(invalid());
    };
    let mut low = first[0].clone();
    let mut high = first[1].clone();
    for range in declarations {
        b.work(1, AT)?;
        if range[0].compare(&range[1], b)? == Ordering::Greater {
            return Err(invalid());
        }
        if range[0].compare(&low, b)? == Ordering::Greater {
            low.clone_from(&range[0]);
        }
        if range[1].compare(&high, b)? == Ordering::Less {
            high.clone_from(&range[1]);
        }
    }
    if low.compare(&high, b)? == Ordering::Greater || low != effective[0] || high != effective[1] {
        return Err(invalid());
    }
    Ok(())
}
/// Intersect all symbolic phase-support declarations.
fn phase_range(declarations: &[[Rational; 2]], b: &mut Budget) -> Result<[Rational; 2], Fault> {
    let Some(first) = declarations.first() else {
        return Err(invalid());
    };
    let mut result = first.clone();
    for range in declarations {
        b.work(1, AT)?;
        if range[0].compare(&result[0], b)? == Ordering::Greater {
            result[0].clone_from(&range[0]);
        }
        if range[1].compare(&result[1], b)? == Ordering::Less {
            result[1].clone_from(&range[1]);
        }
    }
    if result[0].compare(&result[1], b)? == Ordering::Greater {
        return Err(invalid());
    }
    Ok(result)
}
/// Prove that the supplied turn domains cover each observation.
fn lift_complete(request: &Request, pi: &CheckedPiBounds, b: &mut Budget) -> Result<(), Fault> {
    for item in &request.observations {
        let [minimum, maximum] = item.lift_domain;
        if minimum > maximum || item.lift < minimum || item.lift > maximum {
            return Err(invalid());
        }
        let below = minimum.checked_sub(1).ok_or_else(invalid)?;
        let above = maximum.checked_add(1).ok_or_else(invalid)?;
        let physical = phase_range(&item.physical_declarations, b)?;
        let mut low: Option<Rational> = None;
        let mut high: Option<Rational> = None;
        for rate in &request.rate {
            for clock in &request.clock {
                for error in &item.error {
                    let time = item.reported.add(clock, b)?.add(error, b)?;
                    let product = rate.mul(&time, b)?;
                    if let Some(value) = &low {
                        if product.compare(value, b)? == Ordering::Less {
                            low = Some(product.clone());
                        }
                    } else {
                        low = Some(product.clone());
                    }
                    if let Some(value) = &high {
                        if product.compare(value, b)? == Ordering::Greater {
                            high = Some(product);
                        }
                    } else {
                        high = Some(product);
                    }
                }
            }
        }
        let image_low = PiAffine::rational(request.initial[0].add(&low.ok_or_else(invalid)?, b)?);
        let image_high = PiAffine::rational(request.initial[1].add(&high.ok_or_else(invalid)?, b)?);
        if image_low
            .compare(&lifted(&physical[1], below, b)?, pi, b)
            .map_err(pi_fault)?
            != Ordering::Greater
            || image_high
                .compare(&lifted(&physical[0], above, b)?, pi, b)
                .map_err(pi_fault)?
                != Ordering::Less
        {
            return Err(invalid());
        }
    }
    Ok(())
}
/// Append one checked affine inequality.
fn row(
    rows: &mut Vec<Row>,
    dim: usize,
    terms: &[(usize, Rational)],
    bound: PiAffine,
) -> Result<(), Fault> {
    let mut coefficient = Vec::new();
    coefficient
        .try_reserve_exact(dim)
        .map_err(|_| Fault::allocation(AT))?;
    coefficient.resize(dim, Rational::zero());
    for (index, value) in terms {
        coefficient[*index].clone_from(value);
    }
    rows.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
    rows.push(Row { coefficient, bound });
    Ok(())
}
/// Build all shared-support inequalities.
fn support_rows(
    request: &Request,
    rows: &mut Vec<Row>,
    dim: usize,
    b: &mut Budget,
) -> Result<(), Fault> {
    let one = Rational::one();
    let minus = integer(-1, b)?;
    let zero = Rational::zero();
    for range in &request.rate_declarations {
        row(
            rows,
            dim,
            &[(0, minus.clone())],
            PiAffine::rational(range[0].negated(b)?),
        )?;
        row(
            rows,
            dim,
            &[(0, one.clone())],
            PiAffine::rational(range[1].clone()),
        )?;
    }
    for range in &request.initial_declarations {
        row(
            rows,
            dim,
            &[(1, minus.clone()), (2, minus.clone()), (3, one.clone())],
            PiAffine::rational(range[0].negated(b)?),
        )?;
        row(
            rows,
            dim,
            &[(1, one.clone()), (2, one.clone()), (3, minus.clone())],
            PiAffine::rational(range[1].clone()),
        )?;
    }
    for range in &request.clock_declarations {
        row(
            rows,
            dim,
            &[(0, range[0].clone()), (3, minus.clone())],
            PiAffine::rational(zero.clone()),
        )?;
        row(
            rows,
            dim,
            &[(0, range[1].negated(b)?), (3, one.clone())],
            PiAffine::rational(zero.clone()),
        )?;
    }
    for range in &request.offset_declarations {
        row(
            rows,
            dim,
            &[(2, minus.clone())],
            PiAffine::rational(range[0].negated(b)?),
        )?;
        row(
            rows,
            dim,
            &[(2, one.clone())],
            PiAffine::rational(range[1].clone()),
        )?;
    }
    Ok(())
}
/// Build every inequality for one observation.
fn observation_rows(
    request: &Request,
    rows: &mut Vec<Row>,
    dim: usize,
    b: &mut Budget,
) -> Result<(), Fault> {
    let one = Rational::one();
    let minus = integer(-1, b)?;
    let zero = Rational::zero();
    for (index, item) in request.observations.iter().enumerate() {
        let w = 4 + index;
        let r = &item.reported;
        for range in &item.error_declarations {
            row(
                rows,
                dim,
                &[(0, range[0].clone()), (w, minus.clone())],
                PiAffine::rational(zero.clone()),
            )?;
            row(
                rows,
                dim,
                &[(0, range[1].negated(b)?), (w, one.clone())],
                PiAffine::rational(zero.clone()),
            )?;
        }
        for range in &item.time_declarations {
            row(
                rows,
                dim,
                &[
                    (0, range[0].sub(r, b)?),
                    (3, minus.clone()),
                    (w, minus.clone()),
                ],
                PiAffine::rational(zero.clone()),
            )?;
            row(
                rows,
                dim,
                &[
                    (0, r.sub(&range[1], b)?),
                    (3, one.clone()),
                    (w, one.clone()),
                ],
                PiAffine::rational(zero.clone()),
            )?;
        }
        for range in &item.measured_declarations {
            row(
                rows,
                dim,
                &[(0, r.negated(b)?), (1, minus.clone()), (w, minus.clone())],
                lifted(&range[0], item.lift, b)?.negated(b)?,
            )?;
            row(
                rows,
                dim,
                &[(0, r.clone()), (1, one.clone()), (w, one.clone())],
                lifted(&range[1], item.lift, b)?,
            )?;
        }
        for range in &item.physical_declarations {
            row(
                rows,
                dim,
                &[
                    (0, r.negated(b)?),
                    (1, minus.clone()),
                    (2, minus.clone()),
                    (w, minus.clone()),
                ],
                lifted(&range[0], item.lift, b)?.negated(b)?,
            )?;
            row(
                rows,
                dim,
                &[
                    (0, r.clone()),
                    (1, one.clone()),
                    (2, one.clone()),
                    (w, one.clone()),
                ],
                lifted(&range[1], item.lift, b)?,
            )?;
        }
    }
    Ok(())
}
/// Rebuild the full ordered affine row inventory.
pub(super) fn rows(
    request: &Request,
    b: &mut Budget,
) -> Result<(Vec<Row>, CheckedPiBounds), Fault> {
    if request.revision != 1 || request.epoch.is_empty() || request.observations.is_empty() {
        return Err(invalid());
    }
    verify_sources(request, b)?;
    for (effective, declared) in [
        (&request.rate, &request.rate_declarations),
        (&request.initial, &request.initial_declarations),
        (&request.clock, &request.clock_declarations),
        (&request.offset, &request.offset_declarations),
    ] {
        common_range(effective, declared, b)?;
    }
    if request.rate[0].compare(&Rational::zero(), b)? != Ordering::Greater {
        return Err(invalid());
    }
    for item in &request.observations {
        common_range(&item.error, &item.error_declarations, b)?;
        if item.measured_declarations.is_empty()
            || item.physical_declarations.is_empty()
            || item.time_declarations.is_empty()
        {
            return Err(invalid());
        }
    }
    let pi = request.pi.check(b)?;
    lift_complete(request, &pi, b)?;
    let dim = request
        .observations
        .len()
        .checked_add(4)
        .ok_or_else(invalid)?;
    let mut result = Vec::new();
    support_rows(request, &mut result, dim, b)?;
    observation_rows(request, &mut result, dim, b)?;
    b.check(BudgetKind::MatrixEntries, result.len() as u64, AT)?;
    Ok((result, pi))
}
