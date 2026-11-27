// SPDX-License-Identifier: AGPL-3.0-only
//! Exact original rows for the common clock and phase branch.

use super::{AT, ClockPhaseRequest, ClockPhaseSourceKind, ClockPhaseSourceSlot};
use crate::budget::{Budget, BudgetKind};
use crate::pi_bounds::CheckedPiBounds;
use crate::{Fault, InvalidCode, Rational};
use std::cmp::Ordering;

use super::super::PiAffine;

/// One original lifted inequality `coefficient dot (a,gamma,s,z) <= bound`.
#[derive(Clone)]
pub(super) struct Row {
    /// Rational coefficients in `(a,gamma,s,z)` order.
    pub(super) coefficient: [Rational; 4],
    /// Exact affine-pi upper bound.
    pub(super) bound: PiAffine,
}

/// Return the unsupported-claim refusal for this tag.
pub(super) const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedCertificate, AT)
}

/// Embed a signed integer as a rational.
fn integer(value: i64, b: &mut Budget) -> Result<Rational, Fault> {
    Rational::from_ratio(value, 1, b)
}

/// Add the selected integer turn to a phase endpoint.
fn lifted(value: &Rational, turn: i64, b: &mut Budget) -> Result<PiAffine, Fault> {
    Ok(PiAffine {
        base: value.clone(),
        pi: integer(turn.checked_mul(2).ok_or_else(invalid)?, b)?,
    })
}

/// Enumerate every original declaration position.
fn expected_slots(request: &ClockPhaseRequest) -> Result<Vec<ClockPhaseSourceSlot>, Fault> {
    let mut slots = Vec::new();
    let mut add = |kind: ClockPhaseSourceKind, count: usize| -> Result<(), Fault> {
        slots
            .try_reserve(count)
            .map_err(|_| Fault::allocation(AT))?;
        for declaration in 0..count {
            slots.push(ClockPhaseSourceSlot { kind, declaration });
        }
        Ok(())
    };
    add(ClockPhaseSourceKind::Rate, request.rate_declarations.len())?;
    add(
        ClockPhaseSourceKind::Initial,
        request.initial_declarations.len(),
    )?;
    add(
        ClockPhaseSourceKind::Clock,
        request.clock_declarations.len(),
    )?;
    add(
        ClockPhaseSourceKind::Offset,
        request.offset_declarations.len(),
    )?;
    for (index, observation) in request.observations.iter().enumerate() {
        add(
            ClockPhaseSourceKind::Time(index),
            observation.time_declarations.len(),
        )?;
        add(
            ClockPhaseSourceKind::Measured(index),
            observation.measured_declarations.len(),
        )?;
        add(
            ClockPhaseSourceKind::Physical(index),
            observation.physical_declarations.len(),
        )?;
    }
    Ok(slots)
}

/// Verify source order is a permutation of all original positions.
fn verify_sources(request: &ClockPhaseRequest, b: &mut Budget) -> Result<(), Fault> {
    let expected = expected_slots(request)?;
    if expected.len() != request.source_order.len()
        || expected.len() != request.source_versions.len()
    {
        return Err(invalid());
    }
    for slot in &expected {
        let mut matches = 0;
        for current in &request.source_order {
            b.work(1, AT)?;
            matches += usize::from(current == slot);
        }
        if matches != 1 {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Check an effective support equals the intersection of its declarations.
fn common_range(
    effective: &[Rational; 2],
    declared: &[[Rational; 2]],
    b: &mut Budget,
) -> Result<(), Fault> {
    let Some(first) = declared.first() else {
        return Err(invalid());
    };
    let mut lower = first[0].clone();
    let mut upper = first[1].clone();
    for range in declared {
        b.work(1, AT)?;
        if range[0].compare(&range[1], b)? == Ordering::Greater {
            return Err(invalid());
        }
        if range[0].compare(&lower, b)? == Ordering::Greater {
            lower.clone_from(&range[0]);
        }
        if range[1].compare(&upper, b)? == Ordering::Less {
            upper.clone_from(&range[1]);
        }
    }
    if lower.compare(&upper, b)? == Ordering::Greater
        || lower != effective[0]
        || upper != effective[1]
    {
        return Err(invalid());
    }
    Ok(())
}

/// Intersect original physical phase declarations.
fn phase_range(declared: &[[Rational; 2]], b: &mut Budget) -> Result<[Rational; 2], Fault> {
    let Some(first) = declared.first() else {
        return Err(invalid());
    };
    let mut result = first.clone();
    for range in declared {
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

/// Prove immediately adjacent omitted turns lie outside an outer physical image.
fn lift_complete(
    request: &ClockPhaseRequest,
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<(), Fault> {
    for observation in &request.observations {
        let [minimum, maximum] = observation.lift_domain;
        if minimum > maximum || observation.lift < minimum || observation.lift > maximum {
            return Err(invalid());
        }
        let below = minimum.checked_sub(1).ok_or_else(invalid)?;
        let above = maximum.checked_add(1).ok_or_else(invalid)?;
        let physical = phase_range(&observation.physical_declarations, b)?;
        let mut products = Vec::new();
        products
            .try_reserve_exact(4)
            .map_err(|_| Fault::allocation(AT))?;
        for rate in &request.rate {
            for clock in &request.clock {
                let time = observation.reported.add(clock, b)?;
                products.push(rate.mul(&time, b)?);
            }
        }
        let mut low = products[0].clone();
        let mut high = products[0].clone();
        for product in &products[1..] {
            if product.compare(&low, b)? == Ordering::Less {
                low.clone_from(product);
            }
            if product.compare(&high, b)? == Ordering::Greater {
                high.clone_from(product);
            }
        }
        let image_low = PiAffine::rational(request.initial[0].add(&low, b)?);
        let image_high = PiAffine::rational(request.initial[1].add(&high, b)?);
        let below_high = lifted(&physical[1], below, b)?;
        let above_low = lifted(&physical[0], above, b)?;
        if image_low.compare(&below_high, pi, b)? != Ordering::Greater
            || image_high.compare(&above_low, pi, b)? != Ordering::Less
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Append one original lifted inequality.
fn add_row(rows: &mut Vec<Row>, coefficient: [Rational; 4], bound: PiAffine) -> Result<(), Fault> {
    rows.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
    rows.push(Row { coefficient, bound });
    Ok(())
}

/// Translate all shared support declarations into lifted rows.
fn support_rows(
    request: &ClockPhaseRequest,
    rows: &mut Vec<Row>,
    b: &mut Budget,
) -> Result<(), Fault> {
    let z = Rational::zero();
    let one = Rational::one();
    let minus = integer(-1, b)?;
    for range in &request.rate_declarations {
        add_row(
            rows,
            [minus.clone(), z.clone(), z.clone(), z.clone()],
            PiAffine::rational(range[0].negated(b)?),
        )?;
        add_row(
            rows,
            [one.clone(), z.clone(), z.clone(), z.clone()],
            PiAffine::rational(range[1].clone()),
        )?;
    }
    for range in &request.initial_declarations {
        add_row(
            rows,
            [z.clone(), minus.clone(), minus.clone(), one.clone()],
            PiAffine::rational(range[0].negated(b)?),
        )?;
        add_row(
            rows,
            [z.clone(), one.clone(), one.clone(), minus.clone()],
            PiAffine::rational(range[1].clone()),
        )?;
    }
    for range in &request.clock_declarations {
        add_row(
            rows,
            [range[0].clone(), z.clone(), z.clone(), minus.clone()],
            PiAffine::rational(z.clone()),
        )?;
        add_row(
            rows,
            [range[1].negated(b)?, z.clone(), z.clone(), one.clone()],
            PiAffine::rational(z.clone()),
        )?;
    }
    for range in &request.offset_declarations {
        add_row(
            rows,
            [z.clone(), z.clone(), minus.clone(), z.clone()],
            PiAffine::rational(range[0].negated(b)?),
        )?;
        add_row(
            rows,
            [z.clone(), z.clone(), one.clone(), z.clone()],
            PiAffine::rational(range[1].clone()),
        )?;
    }
    Ok(())
}

/// Translate all timestamp and phase declarations into lifted rows.
fn observation_rows(
    request: &ClockPhaseRequest,
    rows: &mut Vec<Row>,
    b: &mut Budget,
) -> Result<(), Fault> {
    let z = Rational::zero();
    let one = Rational::one();
    let minus = integer(-1, b)?;
    for observation in &request.observations {
        let r = &observation.reported;
        for range in &observation.time_declarations {
            add_row(
                rows,
                [range[0].sub(r, b)?, z.clone(), z.clone(), minus.clone()],
                PiAffine::rational(z.clone()),
            )?;
            add_row(
                rows,
                [r.sub(&range[1], b)?, z.clone(), z.clone(), one.clone()],
                PiAffine::rational(z.clone()),
            )?;
        }
        for range in &observation.measured_declarations {
            add_row(
                rows,
                [r.negated(b)?, minus.clone(), z.clone(), z.clone()],
                lifted(&range[0], observation.lift, b)?.negated(b)?,
            )?;
            add_row(
                rows,
                [r.clone(), one.clone(), z.clone(), z.clone()],
                lifted(&range[1], observation.lift, b)?,
            )?;
        }
        for range in &observation.physical_declarations {
            add_row(
                rows,
                [r.negated(b)?, minus.clone(), minus.clone(), z.clone()],
                lifted(&range[0], observation.lift, b)?.negated(b)?,
            )?;
            add_row(
                rows,
                [r.clone(), one.clone(), one.clone(), z.clone()],
                lifted(&range[1], observation.lift, b)?,
            )?;
        }
    }
    Ok(())
}

/// Rebuild every original lifted row in declaration order.
pub(super) fn rows(
    request: &ClockPhaseRequest,
    b: &mut Budget,
) -> Result<(Vec<Row>, CheckedPiBounds), Fault> {
    if request.revision != 1 || request.epoch.is_empty() || request.observations.is_empty() {
        return Err(invalid());
    }
    verify_sources(request, b)?;
    common_range(&request.rate, &request.rate_declarations, b)?;
    common_range(&request.initial, &request.initial_declarations, b)?;
    common_range(&request.clock, &request.clock_declarations, b)?;
    common_range(&request.offset, &request.offset_declarations, b)?;
    if request.rate[0].compare(&Rational::zero(), b)? != Ordering::Greater {
        return Err(invalid());
    }
    let pi = request.pi.check(b)?;
    lift_complete(request, &pi, b)?;
    let mut result = Vec::new();
    support_rows(request, &mut result, b)?;
    observation_rows(request, &mut result, b)?;
    b.check(BudgetKind::MatrixEntries, result.len() as u64, AT)?;
    Ok((result, pi))
}
