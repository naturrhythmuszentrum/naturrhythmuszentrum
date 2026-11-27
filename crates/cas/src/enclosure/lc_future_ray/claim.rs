// SPDX-License-Identifier: AGPL-3.0-only
//! Original source, reference and finite projected-ray proposal.

use super::{
    AT, EXTREMUM_AT, FutureRayForm, FutureRayProposal, FutureRayRequest, FutureRaySource,
    FutureRayVertex, REFERENCE_AT, TOPOLOGY_AT, UNION_AT,
};
use crate::budget::Budget;
use crate::enclosure::lc_affine_fibre::PiAffine;
use crate::pi_bounds::CheckedPiBounds;
use crate::{Fault, InvalidCode, Rational};
use std::cmp::Ordering;

/// A checked finite source topology in original lifted coordinate order.
pub(super) type SourcePoints = Vec<Vec<PiAffine>>;

/// Refuse a malformed or excluded original.
pub(super) const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedCertificate, AT)
}
/// Refuse a reference that is false or undecidable under current pi authority.
const fn reference_fault() -> Fault {
    Fault::invalid(InvalidCode::PendingPremise, REFERENCE_AT)
}
/// Refuse an absent whole-ray extremizer.
const fn extremum_fault() -> Fault {
    Fault::invalid(InvalidCode::PendingPremise, EXTREMUM_AT)
}
/// Refuse undecidable projected ordering or hull orientation.
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
/// Refuse a missing or non-equivalent branch gauge map.
const fn union_fault() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedCertificate, UNION_AT)
}

/// Add exact rational and pi coefficients.
pub(super) fn add(x: &PiAffine, y: &PiAffine, b: &mut Budget) -> Result<PiAffine, Fault> {
    Ok(PiAffine {
        base: x.base.add(&y.base, b)?,
        pi: x.pi.add(&y.pi, b)?,
    })
}
/// Subtract exact rational and pi coefficients.
pub(super) fn sub(x: &PiAffine, y: &PiAffine, b: &mut Budget) -> Result<PiAffine, Fault> {
    Ok(PiAffine {
        base: x.base.sub(&y.base, b)?,
        pi: x.pi.sub(&y.pi, b)?,
    })
}
/// Scale an exact affine-pi expression by a rational.
pub(super) fn scale(x: &PiAffine, y: &Rational, b: &mut Budget) -> Result<PiAffine, Fault> {
    Ok(PiAffine {
        base: x.base.mul(y, b)?,
        pi: x.pi.mul(y, b)?,
    })
}
/// Decide an affine-pi sign from certified bounds.
pub(super) fn sign(x: &PiAffine, pi: &CheckedPiBounds, b: &mut Budget) -> Result<Ordering, Fault> {
    b.work(1, EXTREMUM_AT)?;
    if x.pi.is_zero() {
        return x.base.compare(&Rational::zero(), b);
    }
    let bounds = pi.contract();
    let (low, high) = if x.pi.is_negative() {
        (&bounds.upper, &bounds.lower)
    } else {
        (&bounds.lower, &bounds.upper)
    };
    let low = x.base.add(&x.pi.mul(low, b)?, b)?;
    let high = x.base.add(&x.pi.mul(high, b)?, b)?;
    if low.compare(&Rational::zero(), b)? == Ordering::Greater {
        return Ok(Ordering::Greater);
    }
    if high.compare(&Rational::zero(), b)? == Ordering::Less {
        return Ok(Ordering::Less);
    }
    Err(extremum_fault())
}
/// Compare two exact symbolic expressions.
fn compare(
    x: &PiAffine,
    y: &PiAffine,
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<Ordering, Fault> {
    sign(&sub(x, y, b)?, pi, b)
}
/// Project one lifted point to rate and the declared query intercept.
pub(super) fn project(
    point: &[PiAffine],
    form: FutureRayForm,
    b: &mut Budget,
) -> Result<[PiAffine; 2], Fault> {
    let rate = point.first().ok_or_else(invalid)?.clone();
    let intercept = match point.len() {
        2 if form == FutureRayForm::Plain => point[1].clone(),
        n if n >= 4 => {
            let beta = add(&point[1], &point[2], b)?;
            if form == FutureRayForm::Narrow {
                beta
            } else {
                sub(&beta, &point[3], b)?
            }
        }
        _ => return Err(invalid()),
    };
    if !rate.pi.is_zero() {
        return Err(invalid());
    }
    Ok([rate, intercept])
}

/// Extract the source's original pi authority.
pub(super) const fn pi(source: &FutureRaySource) -> &crate::pi_bounds::PiBoundCertificate {
    match source {
        FutureRaySource::Timing(request, _) => &request.pi,
        FutureRaySource::ClockPhase(request, _) => &request.pi,
        FutureRaySource::TimingClockPhase(request, _) => &request.pi,
    }
}
/// Extract the source's original epoch.
pub(super) fn epoch(source: &FutureRaySource) -> &str {
    match source {
        FutureRaySource::Timing(request, _) => &request.epoch,
        FutureRaySource::ClockPhase(request, _) => &request.epoch,
        FutureRaySource::TimingClockPhase(request, _) => &request.epoch,
    }
}
/// Return each selected turn and its independently checked domain.
fn lifts(source: &FutureRaySource) -> Vec<(i64, [i64; 2])> {
    match source {
        FutureRaySource::Timing(request, _) => request
            .observations
            .iter()
            .map(|v| (v.lift, v.lift_domain))
            .collect(),
        FutureRaySource::ClockPhase(request, _) => request
            .observations
            .iter()
            .map(|v| (v.lift, v.lift_domain))
            .collect(),
        FutureRaySource::TimingClockPhase(request, _) => request
            .observations
            .iter()
            .map(|v| (v.lift, v.lift_domain))
            .collect(),
    }
}
/// Check all original fields except the branch's selected integer turns.
fn same_shape(a: &FutureRaySource, z: &FutureRaySource) -> bool {
    match (a, z) {
        (FutureRaySource::Timing(a, _), FutureRaySource::Timing(z, _)) => {
            let mut z = z.clone();
            for (new, old) in z.observations.iter_mut().zip(&a.observations) {
                new.lift = old.lift;
            }
            *a == z
        }
        (FutureRaySource::ClockPhase(a, _), FutureRaySource::ClockPhase(z, _)) => {
            let mut z = z.clone();
            for (new, old) in z.observations.iter_mut().zip(&a.observations) {
                new.lift = old.lift;
            }
            *a == z
        }
        (FutureRaySource::TimingClockPhase(a, _), FutureRaySource::TimingClockPhase(z, _)) => {
            let mut z = z.clone();
            for (new, old) in z.observations.iter_mut().zip(&a.observations) {
                new.lift = old.lift;
            }
            *a == z
        }
        _ => false,
    }
}
/// Compute a full-turn translated original lifted vertex.
fn gauge(point: &[PiAffine], shift: i64, b: &mut Budget) -> Result<Vec<PiAffine>, Fault> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(point.len())
        .map_err(|_| Fault::allocation(UNION_AT))?;
    copy.extend_from_slice(point);
    let turns = Rational::from_ratio(shift.checked_mul(2).ok_or_else(union_fault)?, 1, b)?;
    copy[1].pi = copy[1].pi.add(&turns, b)?;
    Ok(copy)
}
/// Check the declared complete lift tuple and bidirectional original-vertex gauge map.
pub(super) fn union(
    request: &FutureRayRequest,
    points: &[SourcePoints],
    b: &mut Budget,
) -> Result<(), Fault> {
    let first = request.branches.first().ok_or_else(union_fault)?;
    if first.shift != 0 || request.branches.len() != points.len() {
        return Err(union_fault());
    }
    let representative_lifts = lifts(&first.source);
    let mut count = 1usize;
    for (_, domain) in &representative_lifts {
        let length = domain[1]
            .checked_sub(domain[0])
            .and_then(|v| v.checked_add(1))
            .ok_or_else(union_fault)?;
        count = count
            .checked_mul(usize::try_from(length).map_err(|_| union_fault())?)
            .ok_or_else(union_fault)?;
    }
    if count != request.branches.len() {
        return Err(union_fault());
    }
    for (index, branch) in request.branches.iter().enumerate() {
        if index > 0 && request.branches[index - 1].id >= branch.id {
            return Err(union_fault());
        }
        if !same_shape(&first.source, &branch.source) || points[index].len() != points[0].len() {
            return Err(union_fault());
        }
        let current_lifts = lifts(&branch.source);
        if current_lifts.len() != representative_lifts.len() {
            return Err(union_fault());
        }
        for ((actual, domain), (base, base_domain)) in
            current_lifts.iter().zip(&representative_lifts)
        {
            if domain != base_domain
                || *actual < domain[0]
                || *actual > domain[1]
                || actual.checked_sub(*base) != Some(branch.shift)
            {
                return Err(union_fault());
            }
        }
        for earlier in request.branches.iter().take(index) {
            b.work(1, UNION_AT)?;
            if lifts(&earlier.source)
                .iter()
                .map(|(lift, _)| lift)
                .eq(current_lifts.iter().map(|(lift, _)| lift))
            {
                return Err(union_fault());
            }
        }
        for (from, to) in points[0].iter().zip(&points[index]) {
            if gauge(from, branch.shift, b)? != *to {
                return Err(union_fault());
            }
        }
    }
    Ok(())
}

/// Compute the latest possible original observation time in the query coordinate.
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
            for item in &request.observations {
                let value = &item.time[1];
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
            for item in &request.observations {
                let value = if form == FutureRayForm::Plain {
                    item.reported.add(&request.clock[1], b)?
                } else {
                    item.reported.clone()
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
            for item in &request.observations {
                let value = if form == FutureRayForm::Plain {
                    item.reported
                        .add(&request.clock[1], b)?
                        .add(&item.error[1], b)?
                } else {
                    item.reported.add(&item.error[1], b)?
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
/// Check coordinate, source epoch, exact safe reference and after-window premises.
pub(super) fn reference(
    request: &FutureRayRequest,
    b: &mut Budget,
) -> Result<CheckedPiBounds, Fault> {
    let first = request.branches.first().ok_or_else(invalid)?;
    let authority = pi(&first.source).check(b)?;
    for branch in &request.branches {
        if epoch(&branch.source) != request.epoch || pi(&branch.source) != pi(&first.source) {
            return Err(invalid());
        }
        if request.form == FutureRayForm::Narrow
            && matches!(branch.source, FutureRaySource::Timing(..))
        {
            return Err(invalid());
        }
        let after = window(&branch.source, request.form, b)?;
        if request.reference.compare(&after, b)? != Ordering::Greater {
            return Err(reference_fault());
        }
    }
    let difference = sub(
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

/// Signed two-dimensional turn of three projected points.
fn turn(
    a: &[PiAffine; 2],
    z: &[PiAffine; 2],
    c: &[PiAffine; 2],
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<Ordering, Fault> {
    let da = z[0].base.sub(&a[0].base, b)?;
    let dz = c[0].base.sub(&a[0].base, b)?;
    let first = scale(&sub(&c[1], &a[1], b)?, &da, b)?;
    let second = scale(&sub(&z[1], &a[1], b)?, &dz, b)?;
    sign(&sub(&first, &second, b)?, pi, b).map_err(topology_cause)
}
/// Canonically sort and deduplicate projected source vertices.
fn projected(
    points: &SourcePoints,
    form: FutureRayForm,
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<Vec<FutureRayVertex>, Fault> {
    let mut result: Vec<FutureRayVertex> = Vec::new();
    result
        .try_reserve_exact(points.len())
        .map_err(|_| Fault::allocation(AT))?;
    for (id, source) in points.iter().enumerate() {
        let value = FutureRayVertex {
            branch: 0,
            source_vertex: u32::try_from(id).map_err(|_| invalid())?,
            point: project(source, form, b)?,
        };
        let mut pos = result.len();
        for (index, old) in result.iter().enumerate() {
            let order = compare(&value.point[0], &old.point[0], pi, b).map_err(topology_cause)?;
            let order = if order == Ordering::Equal {
                compare(&value.point[1], &old.point[1], pi, b).map_err(topology_cause)?
            } else {
                order
            };
            if order == Ordering::Equal {
                pos = usize::MAX;
                break;
            }
            if order == Ordering::Less {
                pos = index;
                break;
            }
        }
        if pos != usize::MAX {
            result.insert(pos, value);
        }
    }
    if result.is_empty() {
        return Err(invalid());
    }
    Ok(result)
}
/// Build a canonical convex hull from sorted unique projected points.
fn hull(
    vertices: &[FutureRayVertex],
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<Vec<u32>, Fault> {
    if vertices.len() == 1 {
        return Ok(vec![0]);
    }
    let mut lower: Vec<usize> = Vec::new();
    let mut upper: Vec<usize> = Vec::new();
    lower
        .try_reserve_exact(vertices.len())
        .map_err(|_| Fault::allocation(AT))?;
    upper
        .try_reserve_exact(vertices.len())
        .map_err(|_| Fault::allocation(AT))?;
    for index in 0..vertices.len() {
        while lower.len() >= 2
            && turn(
                &vertices[lower[lower.len() - 2]].point,
                &vertices[lower[lower.len() - 1]].point,
                &vertices[index].point,
                pi,
                b,
            )? != Ordering::Greater
        {
            lower.pop();
        }
        lower.push(index);
    }
    for index in (0..vertices.len()).rev() {
        while upper.len() >= 2
            && turn(
                &vertices[upper[upper.len() - 2]].point,
                &vertices[upper[upper.len() - 1]].point,
                &vertices[index].point,
                pi,
                b,
            )? != Ordering::Greater
        {
            upper.pop();
        }
        upper.push(index);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    let mut out = Vec::new();
    out.try_reserve_exact(lower.len())
        .map_err(|_| Fault::allocation(AT))?;
    for index in lower {
        out.push(u32::try_from(index).map_err(|_| invalid())?);
    }
    Ok(out)
}
/// Compute the four finite whole-ray differences for one projected vertex.
pub(super) fn differences(
    v: &[PiAffine; 2],
    low: &[PiAffine; 2],
    high: &[PiAffine; 2],
    r: &Rational,
    b: &mut Budget,
) -> Result<[PiAffine; 4], Fault> {
    let low_slope = sub(&v[0], &low[0], b)?;
    let low_at = add(&sub(&v[1], &low[1], b)?, &scale(&low_slope, r, b)?, b)?;
    let high_slope = sub(&high[0], &v[0], b)?;
    let high_at = add(&sub(&high[1], &v[1], b)?, &scale(&high_slope, r, b)?, b)?;
    Ok([low_slope, low_at, high_slope, high_at])
}
/// Propose exact projected hull and one ray-extremal pair.
pub(super) fn propose(
    request: &FutureRayRequest,
    points: &[SourcePoints],
    b: &mut Budget,
) -> Result<FutureRayProposal, Fault> {
    let pi = reference(request, b)?;
    union(request, points, b)?;
    let vertices = projected(&points[0], request.form, &pi, b)?;
    let hull = hull(&vertices, &pi, b)?;
    let mut low = 0usize;
    let mut high = 0usize;
    for (index, vertex) in vertices.iter().enumerate().skip(1) {
        if compare(&vertex.point[0], &vertices[low].point[0], &pi, b)? == Ordering::Less {
            low = index;
        }
        if compare(&vertex.point[0], &vertices[high].point[0], &pi, b)? == Ordering::Greater {
            high = index;
        }
    }
    for (index, vertex) in vertices.iter().enumerate() {
        if compare(&vertex.point[0], &vertices[low].point[0], &pi, b)? == Ordering::Equal
            && compare(&vertex.point[1], &vertices[low].point[1], &pi, b)? == Ordering::Less
        {
            low = index;
        }
        if compare(&vertex.point[0], &vertices[high].point[0], &pi, b)? == Ordering::Equal
            && compare(&vertex.point[1], &vertices[high].point[1], &pi, b)? == Ordering::Greater
        {
            high = index;
        }
    }
    let mut comparisons = Vec::new();
    comparisons
        .try_reserve_exact(vertices.len())
        .map_err(|_| Fault::allocation(AT))?;
    for vertex in &vertices {
        let values = differences(
            &vertex.point,
            &vertices[low].point,
            &vertices[high].point,
            &request.reference,
            b,
        )?;
        for value in &values {
            if sign(value, &pi, b)? == Ordering::Less {
                return Err(extremum_fault());
            }
        }
        comparisons.push(values);
    }
    Ok(FutureRayProposal {
        vertices,
        hull,
        low: u32::try_from(low).map_err(|_| invalid())?,
        high: u32::try_from(high).map_err(|_| invalid())?,
        comparisons,
    })
}
