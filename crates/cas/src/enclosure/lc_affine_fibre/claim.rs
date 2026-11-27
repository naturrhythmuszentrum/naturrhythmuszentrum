// SPDX-License-Identifier: AGPL-3.0-only
//! Exact timing rows and an untrusted finite topology proposal.

use super::{
    AT, Dimension, Edge, PiAffine, SignPredicate, TimingRequest, TimingSourceKind, Topology, Vertex,
};
use crate::budget::{Budget, BudgetKind};
use crate::pi_bounds::CheckedPiBounds;
use crate::{Fault, InvalidCode, Rational};
use std::cmp::Ordering;

/// Return the fixed malformed timing claim refusal.
pub(super) const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedCertificate, AT)
}

/// Return the fixed undecided symbolic sign refusal.
const fn undecided() -> Fault {
    Fault::invalid(InvalidCode::PendingPremise, AT)
}

impl PiAffine {
    /// Add two exact affine pi expressions.
    pub(super) fn add(&self, other: &Self, b: &mut Budget) -> Result<Self, Fault> {
        Ok(Self {
            base: self.base.add(&other.base, b)?,
            pi: self.pi.add(&other.pi, b)?,
        })
    }

    /// Subtract two exact affine pi expressions.
    pub(super) fn sub(&self, other: &Self, b: &mut Budget) -> Result<Self, Fault> {
        Ok(Self {
            base: self.base.sub(&other.base, b)?,
            pi: self.pi.sub(&other.pi, b)?,
        })
    }

    /// Scale an affine pi expression by an exact rational.
    pub(super) fn mul(&self, factor: &Rational, b: &mut Budget) -> Result<Self, Fault> {
        Ok(Self {
            base: self.base.mul(factor, b)?,
            pi: self.pi.mul(factor, b)?,
        })
    }

    /// Divide an affine pi expression by a nonzero exact rational.
    pub(super) fn div(&self, factor: &Rational, b: &mut Budget) -> Result<Self, Fault> {
        Ok(Self {
            base: self.base.div(factor, b)?,
            pi: self.pi.div(factor, b)?,
        })
    }

    /// Negate an affine pi expression.
    pub(super) fn negated(&self, b: &mut Budget) -> Result<Self, Fault> {
        Ok(Self {
            base: self.base.negated(b)?,
            pi: self.pi.negated(b)?,
        })
    }

    /// Decide the exact sign using certified pi bounds or refuse uncertainty.
    pub(super) fn sign(&self, pi: &CheckedPiBounds, b: &mut Budget) -> Result<Ordering, Fault> {
        b.work(1, AT)?;
        if self.pi.is_zero() {
            return self.base.compare(&Rational::zero(), b);
        }
        let endpoints = &pi.contract();
        let (low, high) = if self.pi.is_negative() {
            (&endpoints.upper, &endpoints.lower)
        } else {
            (&endpoints.lower, &endpoints.upper)
        };
        let lower = self.base.add(&self.pi.mul(low, b)?, b)?;
        let upper = self.base.add(&self.pi.mul(high, b)?, b)?;
        if lower.compare(&Rational::zero(), b)? == Ordering::Greater {
            return Ok(Ordering::Greater);
        }
        if upper.compare(&Rational::zero(), b)? == Ordering::Less {
            return Ok(Ordering::Less);
        }
        Err(undecided())
    }

    /// Compare two exact affine pi expressions.
    pub(super) fn compare(
        &self,
        other: &Self,
        pi: &CheckedPiBounds,
        b: &mut Budget,
    ) -> Result<Ordering, Fault> {
        self.sub(other, b)?.sign(pi, b)
    }
}

/// An original affine inequality in rate and initial phase.
#[derive(Clone)]
pub(super) struct Row {
    /// Coefficient of projected rate.
    pub(super) a: Rational,
    /// Coefficient of projected initial phase.
    pub(super) phase: Rational,
    /// Exact upper bound for the affine inequality.
    pub(super) bound: PiAffine,
}

/// Embed a signed integer as an exact rational.
fn ratio(value: i64, b: &mut Budget) -> Result<Rational, Fault> {
    Rational::from_ratio(value, 1, b)
}

/// Add an exact integer turn of two pi to a measured phase.
fn lifted(value: &Rational, turn: i64, b: &mut Budget) -> Result<PiAffine, Fault> {
    let twice = turn.checked_mul(2).ok_or_else(invalid)?;
    Ok(PiAffine {
        base: value.clone(),
        pi: ratio(twice, b)?,
    })
}

/// Independently prove that no omitted integer lift can intersect the original boxes.
fn lift_complete(
    request: &TimingRequest,
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<(), Fault> {
    for item in &request.observations {
        b.work(1, AT)?;
        let [minimum, maximum] = item.lift_domain;
        if minimum > maximum || item.lift < minimum || item.lift > maximum {
            return Err(invalid());
        }
        let below = minimum.checked_sub(1).ok_or_else(invalid)?;
        let above = maximum.checked_add(1).ok_or_else(invalid)?;
        let mut products = Vec::new();
        for rate in &request.rate {
            for time in &item.time {
                products.push(rate.mul(time, b)?);
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
        let below_high = lifted(&item.phase[1], below, b)?;
        let above_low = lifted(&item.phase[0], above, b)?;
        if image_low.compare(&below_high, pi, b)? != Ordering::Greater
            || image_high.compare(&above_low, pi, b)? != Ordering::Less
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Preserve each declared source exactly once, including repeated positions.
fn verify_source_order(request: &TimingRequest, b: &mut Budget) -> Result<(), Fault> {
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(request.observations.len())
        .map_err(|_| Fault::allocation(AT))?;
    let mut count = request
        .rate_declarations
        .len()
        .checked_add(request.initial_declarations.len())
        .ok_or_else(invalid)?;
    for item in &request.observations {
        b.work(1, AT)?;
        let time = count;
        count = count
            .checked_add(item.time_declarations.len())
            .ok_or_else(invalid)?;
        let phase = count;
        count = count
            .checked_add(item.phase_declarations.len())
            .ok_or_else(invalid)?;
        offsets.push((time, phase));
    }
    if count != request.source_order.len() {
        return Err(invalid());
    }
    let mut seen = Vec::new();
    seen.try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    seen.resize(count, false);
    for slot in &request.source_order {
        b.work(1, AT)?;
        let (base, length) = match slot.kind {
            TimingSourceKind::Rate => (0, request.rate_declarations.len()),
            TimingSourceKind::Initial => (
                request.rate_declarations.len(),
                request.initial_declarations.len(),
            ),
            TimingSourceKind::Time(index) => (
                offsets.get(index).ok_or_else(invalid)?.0,
                request.observations[index].time_declarations.len(),
            ),
            TimingSourceKind::Phase(index) => (
                offsets.get(index).ok_or_else(invalid)?.1,
                request.observations[index].phase_declarations.len(),
            ),
        };
        if slot.declaration >= length {
            return Err(invalid());
        }
        let index = base.checked_add(slot.declaration).ok_or_else(invalid)?;
        if seen[index] {
            return Err(invalid());
        }
        seen[index] = true;
    }
    Ok(())
}

/// Prove that effective supports are exactly the intersection of all originals.
fn verify_intersection(
    effective: &[Rational; 2],
    declarations: &[[Rational; 2]],
    b: &mut Budget,
) -> Result<(), Fault> {
    let mut iter = declarations.iter();
    let first = iter.next().ok_or_else(invalid)?;
    let mut value = first.clone();
    for range in declarations {
        for endpoint in range {
            endpoint.check(b, AT)?;
        }
        if range[0].compare(&range[1], b)? == Ordering::Greater {
            return Err(invalid());
        }
    }
    for range in iter {
        if range[0].compare(&value[0], b)? == Ordering::Greater {
            value[0].clone_from(&range[0]);
        }
        if range[1].compare(&value[1], b)? == Ordering::Less {
            value[1].clone_from(&range[1]);
        }
    }
    if value[0].compare(&value[1], b)? == Ordering::Greater || &value != effective {
        return Err(invalid());
    }
    Ok(())
}

/// Reconstruct every original timing strip in declaration order.
pub(super) fn rows(
    request: &TimingRequest,
    b: &mut Budget,
) -> Result<(Vec<Row>, CheckedPiBounds), Fault> {
    if request.revision != 1 {
        return Err(Fault::invalid(InvalidCode::UnsupportedVersion, AT));
    }
    let base_sources = request
        .rate_declarations
        .len()
        .checked_add(request.initial_declarations.len())
        .ok_or_else(invalid)?;
    let sources = request
        .observations
        .iter()
        .try_fold(base_sources, |count, item| {
            count
                .checked_add(item.time_declarations.len())?
                .checked_add(item.phase_declarations.len())
        })
        .ok_or_else(invalid)?;
    if request.observations.is_empty()
        || request.epoch.is_empty()
        || request.nominal.len() != request.observations.len()
        || request.source_versions.len() != sources
        || request.source_order.len() != sources
    {
        return Err(invalid());
    }
    verify_source_order(request, b)?;
    for nominal in &request.nominal {
        nominal.check(b, AT)?;
    }
    let pi = request.pi.check(b)?;
    verify_intersection(&request.rate, &request.rate_declarations, b)?;
    verify_intersection(&request.initial, &request.initial_declarations, b)?;
    if request.rate[0].compare(&Rational::zero(), b)? != Ordering::Greater {
        return Err(invalid());
    }
    for item in &request.observations {
        verify_intersection(&item.time, &item.time_declarations, b)?;
        verify_intersection(&item.phase, &item.phase_declarations, b)?;
    }
    lift_complete(request, &pi, b)?;
    Ok((assemble_rows(request, b)?, pi))
}

/// Assemble the bounded original affine row inventory after premise checks.
fn assemble_rows(request: &TimingRequest, b: &mut Budget) -> Result<Vec<Row>, Fault> {
    let base_rows = request
        .rate_declarations
        .len()
        .checked_add(request.initial_declarations.len())
        .and_then(|n| n.checked_mul(2))
        .ok_or_else(invalid)?;
    let count = request
        .observations
        .iter()
        .try_fold(base_rows, |count, item| {
            count.checked_add(
                item.time_declarations
                    .len()
                    .checked_mul(item.phase_declarations.len())?
                    .checked_mul(2)?,
            )
        })
        .ok_or_else(invalid)?;
    b.check(BudgetKind::MatrixEntries, count as u64, AT)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    let zero = Rational::zero();
    let one = Rational::one();
    let negative = ratio(-1, b)?;
    for range in &request.rate_declarations {
        result.push(Row {
            a: negative.clone(),
            phase: zero.clone(),
            bound: PiAffine::rational(range[0].negated(b)?),
        });
        result.push(Row {
            a: one.clone(),
            phase: zero.clone(),
            bound: PiAffine::rational(range[1].clone()),
        });
    }
    for range in &request.initial_declarations {
        result.push(Row {
            a: zero.clone(),
            phase: negative.clone(),
            bound: PiAffine::rational(range[0].negated(b)?),
        });
        result.push(Row {
            a: zero.clone(),
            phase: one.clone(),
            bound: PiAffine::rational(range[1].clone()),
        });
    }
    for item in &request.observations {
        for time in &item.time_declarations {
            for phase in &item.phase_declarations {
                let lower = lifted(&phase[0], item.lift, b)?;
                let upper = lifted(&phase[1], item.lift, b)?;
                result.push(Row {
                    a: time[1].negated(b)?,
                    phase: negative.clone(),
                    bound: lower.negated(b)?,
                });
                result.push(Row {
                    a: time[0].clone(),
                    phase: one.clone(),
                    bound: upper,
                });
            }
        }
    }
    Ok(result)
}

/// Compute the exact determinant of two rational row normals.
pub(super) fn determinant(left: &Row, right: &Row, b: &mut Budget) -> Result<Rational, Fault> {
    left.a
        .mul(&right.phase, b)?
        .sub(&right.a.mul(&left.phase, b)?, b)
}

/// Solve an independent pair of affine boundary rows exactly.
pub(super) fn intersect(
    left: &Row,
    right: &Row,
    determinant: &Rational,
    b: &mut Budget,
) -> Result<[PiAffine; 2], Fault> {
    let rate = left
        .bound
        .mul(&right.phase, b)?
        .sub(&right.bound.mul(&left.phase, b)?, b)?
        .div(determinant, b)?;
    let initial = right
        .bound
        .mul(&left.a, b)?
        .sub(&left.bound.mul(&right.a, b)?, b)?
        .div(determinant, b)?;
    Ok([rate, initial])
}

/// Compute the exact inequality residual for a projected point.
pub(super) fn residual(
    row: &Row,
    point: &[PiAffine; 2],
    b: &mut Budget,
) -> Result<PiAffine, Fault> {
    point[0]
        .mul(&row.a, b)?
        .add(&point[1].mul(&row.phase, b)?, b)?
        .sub(&row.bound, b)
}

/// Bound an exact affine expression with checked pi endpoints.
fn image(value: &PiAffine, pi: &CheckedPiBounds, b: &mut Budget) -> Result<[Rational; 2], Fault> {
    let contract = pi.contract();
    let (low, high) = if value.pi.is_negative() {
        (&contract.upper, &contract.lower)
    } else {
        (&contract.lower, &contract.upper)
    };
    Ok([
        value.base.add(&value.pi.mul(low, b)?, b)?,
        value.base.add(&value.pi.mul(high, b)?, b)?,
    ])
}

/// Inventory support order, determinant and candidate residual signs in fixed row order.
pub(super) fn predicates(
    request: &TimingRequest,
    b: &mut Budget,
) -> Result<Vec<SignPredicate>, Fault> {
    let (rows, pi) = rows(request, b)?;
    let pairs = rows
        .len()
        .checked_mul(rows.len().saturating_sub(1))
        .and_then(|n| n.checked_div(2))
        .ok_or_else(invalid)?;
    let maximum = pairs
        .checked_mul(rows.len().checked_add(1).ok_or_else(invalid)?)
        .and_then(|n| n.checked_add(request.source_order.len()))
        .and_then(|n| n.checked_add(1))
        .ok_or_else(invalid)?;
    b.check(BudgetKind::MatrixEntries, maximum as u64, AT)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(maximum)
        .map_err(|_| Fault::allocation(AT))?;
    let mut push = |value: PiAffine, b: &mut Budget| -> Result<(), Fault> {
        let required = value.sign(&pi, b)?;
        result.push(SignPredicate {
            required,
            image: image(&value, &pi, b)?,
        });
        Ok(())
    };
    push(PiAffine::rational(request.rate[0].clone()), b)?;
    for range in request
        .rate_declarations
        .iter()
        .chain(&request.initial_declarations)
    {
        push(PiAffine::rational(range[1].sub(&range[0], b)?), b)?;
    }
    for item in &request.observations {
        for range in item
            .time_declarations
            .iter()
            .chain(&item.phase_declarations)
        {
            push(PiAffine::rational(range[1].sub(&range[0], b)?), b)?;
        }
    }
    for (index, first) in rows.iter().enumerate() {
        for second in &rows[index + 1..] {
            b.work(1, AT)?;
            let det = determinant(first, second, b)?;
            push(PiAffine::rational(det.clone()), b)?;
            if !det.is_zero() {
                let point = intersect(first, second, &det, b)?;
                for row in &rows {
                    push(residual(row, &point, b)?, b)?;
                }
            }
        }
    }
    Ok(result)
}

/// Check candidate feasibility and collect every active original row.
fn vertex(
    rows: &[Row],
    point: [PiAffine; 2],
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<Option<Vertex>, Fault> {
    let mut active = Vec::new();
    for (id, row) in rows.iter().enumerate() {
        let sign = residual(row, &point, b)?.sign(pi, b)?;
        if sign == Ordering::Greater {
            return Ok(None);
        }
        if sign == Ordering::Equal {
            active.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
            active.push(u32::try_from(id).map_err(|_| invalid())?);
        }
    }
    let mut rank = false;
    for (i, first) in active.iter().enumerate() {
        for second in &active[i + 1..] {
            if !determinant(&rows[*first as usize], &rows[*second as usize], b)?.is_zero() {
                rank = true;
            }
        }
    }
    Ok(rank.then_some(Vertex { point, active }))
}

/// Order exact projected points lexicographically.
fn point_order(
    left: &Vertex,
    right: &Vertex,
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<Ordering, Fault> {
    let first = left.point[0].compare(&right.point[0], pi, b)?;
    if first == Ordering::Equal {
        left.point[1].compare(&right.point[1], pi, b)
    } else {
        Ok(first)
    }
}

/// Build an untrusted proposal by enumerating every independent boundary pair.
pub(super) fn propose(request: &TimingRequest, b: &mut Budget) -> Result<Topology, Fault> {
    let (rows, pi) = rows(request, b)?;
    let mut vertices = Vec::new();
    for (i, first) in rows.iter().enumerate() {
        for second in &rows[i + 1..] {
            b.work(1, AT)?;
            let det = determinant(first, second, b)?;
            if det.is_zero() {
                continue;
            }
            if let Some(candidate) = vertex(&rows, intersect(first, second, &det, b)?, &pi, b)? {
                let mut position = vertices.len();
                for (index, previous) in vertices.iter().enumerate() {
                    match point_order(&candidate, previous, &pi, b)? {
                        Ordering::Less => {
                            position = index;
                            break;
                        }
                        Ordering::Equal => {
                            position = usize::MAX;
                            break;
                        }
                        Ordering::Greater => (),
                    }
                }
                if position != usize::MAX {
                    vertices.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
                    vertices.insert(position, candidate);
                }
            }
        }
    }
    let dimension = dimension(&vertices, &rows);
    let edges = edges(&vertices, &rows, dimension, b)?;
    Ok(Topology {
        dimension,
        vertices,
        edges,
    })
}

/// Classify the exhaustive finite fibre from its vertices and common rows.
pub(super) fn dimension(vertices: &[Vertex], rows: &[Row]) -> Dimension {
    match vertices.len() {
        0 => Dimension::Empty,
        1 => Dimension::Point,
        _ => {
            if (0..rows.len()).any(|id| {
                u32::try_from(id).is_ok_and(|id| vertices.iter().all(|v| v.active.contains(&id)))
            }) {
                Dimension::Segment
            } else {
                Dimension::Polygon
            }
        }
    }
}

/// Construct maximal boundary edges and their original supports.
pub(super) fn edges(
    vertices: &[Vertex],
    rows: &[Row],
    dimension: Dimension,
    b: &mut Budget,
) -> Result<Vec<Edge>, Fault> {
    let mut result: Vec<Edge> = Vec::new();
    if dimension == Dimension::Segment {
        let mut supports = Vec::new();
        supports
            .try_reserve_exact(rows.len())
            .map_err(|_| Fault::allocation(AT))?;
        for id in 0..rows.len() {
            if u32::try_from(id).is_ok_and(|id| vertices.iter().all(|v| v.active.contains(&id))) {
                supports.push(u32::try_from(id).map_err(|_| invalid())?);
            }
        }
        result.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
        result.push(Edge {
            ends: [0, u32::try_from(vertices.len() - 1).map_err(|_| invalid())?],
            supports,
        });
    } else if dimension == Dimension::Polygon {
        for id in 0..rows.len() {
            b.work(1, AT)?;
            let row_id = u32::try_from(id).map_err(|_| invalid())?;
            let mut active = Vec::new();
            active
                .try_reserve_exact(vertices.len())
                .map_err(|_| Fault::allocation(AT))?;
            for (index, vertex) in vertices.iter().enumerate() {
                if vertex.active.contains(&row_id) {
                    active.push(index);
                }
            }
            if active.len() < 2 {
                continue;
            }
            let ends = [
                u32::try_from(active[0]).map_err(|_| invalid())?,
                u32::try_from(*active.last().ok_or_else(invalid)?).map_err(|_| invalid())?,
            ];
            if let Some(edge) = result.iter_mut().find(|edge| edge.ends == ends) {
                edge.supports
                    .try_reserve(1)
                    .map_err(|_| Fault::allocation(AT))?;
                edge.supports.push(row_id);
            } else {
                result.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
                let mut supports = Vec::new();
                supports
                    .try_reserve_exact(1)
                    .map_err(|_| Fault::allocation(AT))?;
                supports.push(row_id);
                result.push(Edge { ends, supports });
            }
        }
        result.sort_by_key(|edge| edge.ends);
    }
    Ok(result)
}
