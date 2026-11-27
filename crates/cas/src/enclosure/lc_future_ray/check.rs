// SPDX-License-Identifier: AGPL-3.0-only
//! Independent exact reconstruction of projected vertices and ray comparisons.

use super::{
    AT, EXTREMUM_AT, FutureRayForm, FutureRayProposal, FutureRayRequest, FutureRaySource,
    FutureRayVertex, REFERENCE_AT, TOPOLOGY_AT, UNION_AT, claim,
};
use crate::budget::Budget;
use crate::enclosure::lc_affine_fibre::PiAffine;
use crate::pi_bounds::CheckedPiBounds;
use crate::{Fault, InvalidCode, Rational};
use std::cmp::Ordering;

/// Refuse a malformed projected proposal.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedCertificate, AT)
}
/// Refuse an undecidable or false whole-ray extremum.
const fn extremum() -> Fault {
    Fault::invalid(InvalidCode::PendingPremise, EXTREMUM_AT)
}
/// Refuse undecidable projected ordering or orientation.
const fn topology_fault() -> Fault {
    Fault::invalid(InvalidCode::PendingPremise, TOPOLOGY_AT)
}
/// Preserve resource faults while locating undecidable topology signs.
fn topology_cause(fault: Fault) -> Fault {
    if matches!(
        fault.kind,
        crate::FaultKind::Invalid(InvalidCode::PendingPremise)
    ) {
        topology_fault()
    } else {
        fault
    }
}
/// Refuse an undecidable or false reference premise.
const fn reference_fault() -> Fault {
    Fault::invalid(InvalidCode::PendingPremise, REFERENCE_AT)
}
/// Refuse an incomplete or non-equivalent gauge union.
const fn union_fault() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedCertificate, UNION_AT)
}
/// Subtract exact affine-pi expressions using checker-owned arithmetic.
fn minus(x: &PiAffine, y: &PiAffine, b: &mut Budget) -> Result<PiAffine, Fault> {
    Ok(PiAffine {
        base: x.base.sub(&y.base, b)?,
        pi: x.pi.sub(&y.pi, b)?,
    })
}
/// Add exact affine-pi expressions using checker-owned arithmetic.
fn plus(x: &PiAffine, y: &PiAffine, b: &mut Budget) -> Result<PiAffine, Fault> {
    Ok(PiAffine {
        base: x.base.add(&y.base, b)?,
        pi: x.pi.add(&y.pi, b)?,
    })
}
/// Multiply an exact affine-pi expression by a rational.
fn times(x: &PiAffine, scalar: &Rational, b: &mut Budget) -> Result<PiAffine, Fault> {
    Ok(PiAffine {
        base: x.base.mul(scalar, b)?,
        pi: x.pi.mul(scalar, b)?,
    })
}
/// Decide an exact affine-pi sign from checked outward pi bounds.
fn sign(value: &PiAffine, bounds: &CheckedPiBounds, b: &mut Budget) -> Result<Ordering, Fault> {
    if value.pi.is_zero() {
        return value.base.compare(&Rational::zero(), b);
    }
    let endpoints = bounds.contract();
    let (lower, upper) = if value.pi.is_negative() {
        (&endpoints.upper, &endpoints.lower)
    } else {
        (&endpoints.lower, &endpoints.upper)
    };
    let min = value.base.add(&value.pi.mul(lower, b)?, b)?;
    let max = value.base.add(&value.pi.mul(upper, b)?, b)?;
    if min.compare(&Rational::zero(), b)? == Ordering::Greater {
        Ok(Ordering::Greater)
    } else if max.compare(&Rational::zero(), b)? == Ordering::Less {
        Ok(Ordering::Less)
    } else {
        Err(extremum())
    }
}
/// Compare two exact affine-pi expressions.
fn ordering(
    x: &PiAffine,
    y: &PiAffine,
    bounds: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<Ordering, Fault> {
    sign(&minus(x, y, b)?, bounds, b).map_err(topology_cause)
}
/// Read the original branch pi authority.
const fn source_pi(source: &FutureRaySource) -> &crate::pi_bounds::PiBoundCertificate {
    match source {
        FutureRaySource::Timing(request, _) => &request.pi,
        FutureRaySource::ClockPhase(request, _) => &request.pi,
        FutureRaySource::TimingClockPhase(request, _) => &request.pi,
    }
}
/// Read the original branch epoch.
fn source_epoch(source: &FutureRaySource) -> &str {
    match source {
        FutureRaySource::Timing(request, _) => &request.epoch,
        FutureRaySource::ClockPhase(request, _) => &request.epoch,
        FutureRaySource::TimingClockPhase(request, _) => &request.epoch,
    }
}
/// Read the original selected turns and their complete domains.
fn source_lifts(source: &FutureRaySource) -> Vec<(i64, [i64; 2])> {
    match source {
        FutureRaySource::Timing(request, _) => request
            .observations
            .iter()
            .map(|o| (o.lift, o.lift_domain))
            .collect(),
        FutureRaySource::ClockPhase(request, _) => request
            .observations
            .iter()
            .map(|o| (o.lift, o.lift_domain))
            .collect(),
        FutureRaySource::TimingClockPhase(request, _) => request
            .observations
            .iter()
            .map(|o| (o.lift, o.lift_domain))
            .collect(),
    }
}
/// Compare every original field except selected turn values.
fn same_source_shape(first: &FutureRaySource, later: &FutureRaySource) -> bool {
    match (first, later) {
        (FutureRaySource::Timing(a, _), FutureRaySource::Timing(z, _)) => {
            let mut z = z.clone();
            for (left, right) in a.observations.iter().zip(&mut z.observations) {
                right.lift = left.lift;
            }
            *a == z
        }
        (FutureRaySource::ClockPhase(a, _), FutureRaySource::ClockPhase(z, _)) => {
            let mut z = z.clone();
            for (left, right) in a.observations.iter().zip(&mut z.observations) {
                right.lift = left.lift;
            }
            *a == z
        }
        (FutureRaySource::TimingClockPhase(a, _), FutureRaySource::TimingClockPhase(z, _)) => {
            let mut z = z.clone();
            for (left, right) in a.observations.iter().zip(&mut z.observations) {
                right.lift = left.lift;
            }
            *a == z
        }
        _ => false,
    }
}
/// Reconstruct complete turn coverage and original full-turn maps.
fn verify_union(
    request: &FutureRayRequest,
    points: &[claim::SourcePoints],
    b: &mut Budget,
) -> Result<(), Fault> {
    let first = request.branches.first().ok_or_else(union_fault)?;
    if first.shift != 0 || request.branches.len() != points.len() {
        return Err(union_fault());
    }
    let base = source_lifts(&first.source);
    let mut required = 1usize;
    for (_, domain) in &base {
        let length = domain[1]
            .checked_sub(domain[0])
            .and_then(|v| v.checked_add(1))
            .ok_or_else(union_fault)?;
        required = required
            .checked_mul(usize::try_from(length).map_err(|_| union_fault())?)
            .ok_or_else(union_fault)?;
    }
    if required != request.branches.len() {
        return Err(union_fault());
    }
    for (index, branch) in request.branches.iter().enumerate() {
        if index > 0 && request.branches[index - 1].id >= branch.id {
            return Err(union_fault());
        }
        if !same_source_shape(&first.source, &branch.source)
            || points[index].len() != points[0].len()
        {
            return Err(union_fault());
        }
        let lifts = source_lifts(&branch.source);
        if lifts.len() != base.len() {
            return Err(union_fault());
        }
        for ((actual, domain), (start, start_domain)) in lifts.iter().zip(&base) {
            if domain != start_domain
                || *actual < domain[0]
                || *actual > domain[1]
                || actual.checked_sub(*start) != Some(branch.shift)
            {
                return Err(union_fault());
            }
        }
        for earlier in request.branches.iter().take(index) {
            b.work(1, UNION_AT)?;
            if source_lifts(&earlier.source)
                .iter()
                .map(|(lift, _)| lift)
                .eq(lifts.iter().map(|(lift, _)| lift))
            {
                return Err(union_fault());
            }
        }
        let turn =
            Rational::from_ratio(branch.shift.checked_mul(2).ok_or_else(union_fault)?, 1, b)?;
        for (original, mapped) in points[0].iter().zip(&points[index]) {
            if original.len() != mapped.len() || original.len() < 2 {
                return Err(union_fault());
            }
            for (coordinate, (x, z)) in original.iter().zip(mapped).enumerate() {
                let expected = if coordinate == 1 {
                    PiAffine {
                        base: x.base.clone(),
                        pi: x.pi.add(&turn, b)?,
                    }
                } else {
                    x.clone()
                };
                if expected != *z {
                    return Err(union_fault());
                }
            }
        }
    }
    Ok(())
}
/// Recompute the latest possible original observation in the query coordinate.
fn window(
    source: &FutureRaySource,
    form: FutureRayForm,
    b: &mut Budget,
) -> Result<Rational, Fault> {
    let mut maximum: Option<Rational> = None;
    match source {
        FutureRaySource::Timing(request, _) => {
            if form != FutureRayForm::Plain {
                return Err(invalid());
            }
            for observation in &request.observations {
                let value = &observation.time[1];
                if maximum
                    .as_ref()
                    .map(|old| value.compare(old, b))
                    .transpose()?
                    .is_none_or(|order| order == Ordering::Greater)
                {
                    maximum = Some(value.clone());
                }
            }
        }
        FutureRaySource::ClockPhase(request, _) => {
            for observation in &request.observations {
                let value = if form == FutureRayForm::Plain {
                    observation.reported.add(&request.clock[1], b)?
                } else {
                    observation.reported.clone()
                };
                if maximum
                    .as_ref()
                    .map(|old| value.compare(old, b))
                    .transpose()?
                    .is_none_or(|order| order == Ordering::Greater)
                {
                    maximum = Some(value);
                }
            }
        }
        FutureRaySource::TimingClockPhase(request, _) => {
            for observation in &request.observations {
                let value = if form == FutureRayForm::Plain {
                    observation
                        .reported
                        .add(&request.clock[1], b)?
                        .add(&observation.error[1], b)?
                } else {
                    observation.reported.add(&observation.error[1], b)?
                };
                if maximum
                    .as_ref()
                    .map(|old| value.compare(old, b))
                    .transpose()?
                    .is_none_or(|order| order == Ordering::Greater)
                {
                    maximum = Some(value);
                }
            }
        }
    }
    maximum.ok_or_else(invalid)
}
/// Recompute epoch, coordinate, window and safe-reference premises.
fn verify_reference(request: &FutureRayRequest, b: &mut Budget) -> Result<CheckedPiBounds, Fault> {
    let first = request.branches.first().ok_or_else(invalid)?;
    let authority = source_pi(&first.source).check(b)?;
    for branch in &request.branches {
        if source_epoch(&branch.source) != request.epoch
            || source_pi(&branch.source) != source_pi(&first.source)
        {
            return Err(invalid());
        }
        if request.form == FutureRayForm::Narrow
            && matches!(branch.source, FutureRaySource::Timing(..))
        {
            return Err(invalid());
        }
        if request
            .reference
            .compare(&window(&branch.source, request.form, b)?, b)?
            != Ordering::Greater
        {
            return Err(reference_fault());
        }
    }
    let difference = minus(
        &PiAffine::rational(request.reference.clone()),
        &request.safe_reference,
        b,
    )?;
    let relation = sign(&difference, &authority, b).map_err(|fault| {
        if matches!(
            fault.kind,
            crate::FaultKind::Invalid(InvalidCode::PendingPremise)
        ) {
            reference_fault()
        } else {
            fault
        }
    })?;
    if relation == Ordering::Less || (relation == Ordering::Greater && !request.narrowed) {
        return Err(reference_fault());
    }
    Ok(authority)
}
/// Recompute one original vertex in the declared future coordinates.
fn project(
    point: &[PiAffine],
    form: FutureRayForm,
    b: &mut Budget,
) -> Result<[PiAffine; 2], Fault> {
    let a = point.first().ok_or_else(invalid)?.clone();
    if !a.pi.is_zero() {
        return Err(invalid());
    }
    let c = match (point.len(), form) {
        (2, FutureRayForm::Plain) => point[1].clone(),
        (n, FutureRayForm::Narrow) if n >= 4 => plus(&point[1], &point[2], b)?,
        (n, FutureRayForm::Plain) if n >= 4 => {
            minus(&plus(&point[1], &point[2], b)?, &point[3], b)?
        }
        _ => return Err(invalid()),
    };
    Ok([a, c])
}
/// Compute one exact signed projected orientation.
fn turn(
    a: &[PiAffine; 2],
    z: &[PiAffine; 2],
    c: &[PiAffine; 2],
    bounds: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<Ordering, Fault> {
    let dx = z[0].base.sub(&a[0].base, b)?;
    let dz = c[0].base.sub(&a[0].base, b)?;
    let y1 = times(&minus(&c[1], &a[1], b)?, &dx, b)?;
    let y2 = times(&minus(&z[1], &a[1], b)?, &dz, b)?;
    sign(&minus(&y1, &y2, b)?, bounds, b).map_err(topology_cause)
}
/// Reconstruct the canonical hull from all sorted projected vertices.
fn check_hull(
    vertices: &[FutureRayVertex],
    declared: &[u32],
    bounds: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<(), Fault> {
    if vertices.len() == 1 {
        return if declared == [0] {
            Ok(())
        } else {
            Err(invalid())
        };
    }
    let mut hull = Vec::<usize>::new();
    for i in 0..vertices.len() {
        while hull.len() >= 2
            && turn(
                &vertices[hull[hull.len() - 2]].point,
                &vertices[hull[hull.len() - 1]].point,
                &vertices[i].point,
                bounds,
                b,
            )? != Ordering::Greater
        {
            hull.pop();
        }
        hull.push(i);
    }
    let lower_len = hull.len();
    for i in (0..vertices.len() - 1).rev() {
        while hull.len() > lower_len
            && turn(
                &vertices[hull[hull.len() - 2]].point,
                &vertices[hull[hull.len() - 1]].point,
                &vertices[i].point,
                bounds,
                b,
            )? != Ordering::Greater
        {
            hull.pop();
        }
        hull.push(i);
    }
    hull.pop();
    if hull.len() != declared.len() || hull.iter().zip(declared).any(|(a, z)| *a != *z as usize) {
        Err(invalid())
    } else {
        Ok(())
    }
}
/// Independently verify the submitted proposal against the full checked source topologies.
pub(super) fn verify(
    request: &FutureRayRequest,
    points: &[claim::SourcePoints],
    proposal: &FutureRayProposal,
    b: &mut Budget,
) -> Result<(), Fault> {
    let bounds = verify_reference(request, b)?;
    verify_union(request, points, b)?;
    let first = points.first().ok_or_else(invalid)?;
    let mut expected = Vec::<FutureRayVertex>::new();
    expected
        .try_reserve_exact(first.len())
        .map_err(|_| Fault::allocation(AT))?;
    for (source_id, original) in first.iter().enumerate() {
        let vertex = FutureRayVertex {
            branch: 0,
            source_vertex: u32::try_from(source_id).map_err(|_| invalid())?,
            point: project(original, request.form, b)?,
        };
        let mut position = expected.len();
        for (i, prior) in expected.iter().enumerate() {
            let a = ordering(&vertex.point[0], &prior.point[0], &bounds, b)?;
            let comparison = if a == Ordering::Equal {
                ordering(&vertex.point[1], &prior.point[1], &bounds, b)?
            } else {
                a
            };
            if comparison == Ordering::Equal {
                position = usize::MAX;
                break;
            }
            if comparison == Ordering::Less {
                position = i;
                break;
            }
        }
        if position != usize::MAX {
            expected.insert(position, vertex);
        }
    }
    if expected.is_empty()
        || proposal.vertices != expected
        || proposal.comparisons.len() != expected.len()
    {
        return Err(invalid());
    }
    check_hull(&expected, &proposal.hull, &bounds, b)?;
    let low = expected.get(proposal.low as usize).ok_or_else(invalid)?;
    let high = expected.get(proposal.high as usize).ok_or_else(invalid)?;
    for (v, declared) in expected.iter().zip(&proposal.comparisons) {
        let low_da = minus(&v.point[0], &low.point[0], b)?;
        let high_da = minus(&high.point[0], &v.point[0], b)?;
        let actual = [
            low_da.clone(),
            plus(
                &minus(&v.point[1], &low.point[1], b)?,
                &times(&low_da, &request.reference, b)?,
                b,
            )?,
            high_da.clone(),
            plus(
                &minus(&high.point[1], &v.point[1], b)?,
                &times(&high_da, &request.reference, b)?,
                b,
            )?,
        ];
        if &actual != declared {
            return Err(invalid());
        }
        for value in &actual {
            if sign(value, &bounds, b)? == Ordering::Less {
                return Err(extremum());
            }
        }
    }
    Ok(())
}
