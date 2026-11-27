// SPDX-License-Identifier: AGPL-3.0-only
//! Exact variable-dimensional proposal and ordered predicate inventory.

use super::super::{PiAffine, SignPredicate};
use super::claim::{self, Row};
use super::{
    AT, Signs, TimingClockPhaseEdge as Edge, TimingClockPhaseRequest as Request,
    TimingClockPhaseTopology as Topology, TimingClockPhaseVertex as Vertex,
};
use crate::budget::{Budget, BudgetKind};
use crate::pi_bounds::CheckedPiBounds;
use crate::{Fault, Rational};
use std::cmp::Ordering;

/// Stable combinations of row IDs, including every independent-row candidate.
pub(super) fn combinations(
    count: usize,
    size: usize,
    b: &mut Budget,
) -> Result<Vec<Vec<usize>>, Fault> {
    let mut result = Vec::new();
    if size > count {
        return Ok(result);
    }
    let mut ids: Vec<usize> = (0..size).collect();
    loop {
        b.work(1, AT)?;
        result.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
        result.push(ids.clone());
        let Some(position) = (0..size).rev().find(|&i| ids[i] != i + count - size) else {
            break;
        };
        ids[position] += 1;
        for i in position + 1..size {
            ids[i] = ids[i - 1] + 1;
        }
    }
    Ok(result)
}
/// Laplace determinant for proposal Cramer expansion.
fn determinant(matrix: &[Vec<Rational>], b: &mut Budget) -> Result<Rational, Fault> {
    if matrix.len() == 1 {
        return Ok(matrix[0][0].clone());
    }
    let mut sum = Rational::zero();
    for column in 0..matrix.len() {
        if matrix[0][column].is_zero() {
            continue;
        }
        let mut child = Vec::new();
        child
            .try_reserve_exact(matrix.len() - 1)
            .map_err(|_| Fault::allocation(AT))?;
        for row in matrix.iter().skip(1) {
            let mut values = Vec::new();
            values
                .try_reserve_exact(matrix.len() - 1)
                .map_err(|_| Fault::allocation(AT))?;
            for (index, value) in row.iter().enumerate() {
                if index != column {
                    b.work(1, AT)?;
                    values.push(value.clone());
                }
            }
            child.push(values);
        }
        let term = matrix[0][column].mul(&determinant(&child, b)?, b)?;
        sum = if column % 2 == 0 {
            sum.add(&term, b)?
        } else {
            sum.sub(&term, b)?
        };
    }
    Ok(sum)
}
/// Extract exact row normals for a selected intersection.
fn matrix(rows: &[Row], ids: &[usize], b: &mut Budget) -> Result<Vec<Vec<Rational>>, Fault> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(ids.len())
        .map_err(|_| Fault::allocation(AT))?;
    for id in ids {
        b.work(ids.len() as u64, AT)?;
        result.push(rows[*id].coefficient.clone());
    }
    Ok(result)
}
/// Solve one selected affine-row intersection.
fn intersect(
    rows: &[Row],
    ids: &[usize],
    det: &Rational,
    b: &mut Budget,
) -> Result<Vec<PiAffine>, Fault> {
    let source = matrix(rows, ids, b)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(ids.len())
        .map_err(|_| Fault::allocation(AT))?;
    for coordinate in 0..ids.len() {
        let mut base = source.clone();
        let mut pi = source.clone();
        b.work((2 * ids.len() * ids.len()) as u64, AT)?;
        for (row, id) in ids.iter().enumerate() {
            base[row][coordinate].clone_from(&rows[*id].bound.base);
            pi[row][coordinate].clone_from(&rows[*id].bound.pi);
        }
        result.push(PiAffine {
            base: determinant(&base, b)?.div(det, b)?,
            pi: determinant(&pi, b)?.div(det, b)?,
        });
    }
    Ok(result)
}
/// Evaluate symbolic row slack at a candidate point.
pub(super) fn residual(row: &Row, point: &[PiAffine], b: &mut Budget) -> Result<PiAffine, Fault> {
    if row.coefficient.len() != point.len() {
        return Err(claim::invalid());
    }
    let mut sum = PiAffine::rational(Rational::zero());
    for (coefficient, value) in row.coefficient.iter().zip(point) {
        sum = sum.add(&value.mul(coefficient, b)?, b)?;
    }
    sum.sub(&row.bound, b)
}
/// Order exact symbolic points on the pi interval.
pub(super) fn point_order(
    left: &[PiAffine],
    right: &[PiAffine],
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<Ordering, Fault> {
    if left.len() != right.len() {
        return Err(claim::invalid());
    }
    for (a, z) in left.iter().zip(right) {
        let order = a.compare(z, pi, b)?;
        if order != Ordering::Equal {
            return Ok(order);
        }
    }
    Ok(Ordering::Equal)
}
/// Validate and retain an intersection candidate.
fn candidate(
    point: Vec<PiAffine>,
    rows: &[Row],
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<Option<Vertex>, Fault> {
    let mut active = Vec::new();
    for (id, row) in rows.iter().enumerate() {
        match residual(row, &point, b)?.sign(pi, b)? {
            Ordering::Greater => return Ok(None),
            Ordering::Equal => {
                active.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
                active.push(u32::try_from(id).map_err(|_| claim::invalid())?);
            }
            Ordering::Less => (),
        }
    }
    Ok(Some(Vertex { point, active }))
}
/// Insert a unique vertex in exact order.
fn insert(
    vertices: &mut Vec<Vertex>,
    vertex: Vertex,
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<(), Fault> {
    let mut position = vertices.len();
    for (index, existing) in vertices.iter().enumerate() {
        match point_order(&vertex.point, &existing.point, pi, b)? {
            Ordering::Less => {
                position = index;
                break;
            }
            Ordering::Equal => return Ok(()),
            Ordering::Greater => (),
        }
    }
    vertices.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
    vertices.insert(position, vertex);
    Ok(())
}
/// Proposal-only elimination rank for normal vectors.
fn rank(ids: &[u32], rows: &[Row], b: &mut Budget) -> Result<usize, Fault> {
    let dim = rows[0].coefficient.len();
    let mut matrix = Vec::new();
    matrix
        .try_reserve_exact(ids.len())
        .map_err(|_| Fault::allocation(AT))?;
    for id in ids {
        b.work(dim as u64, AT)?;
        matrix.push(rows[*id as usize].coefficient.clone());
    }
    let mut rank = 0;
    for column in 0..dim {
        let Some(pivot) = (rank..matrix.len()).find(|&i| !matrix[i][column].is_zero()) else {
            continue;
        };
        matrix.swap(rank, pivot);
        let pivot_row = matrix[rank].clone();
        for row in matrix.iter_mut().skip(rank + 1) {
            if row[column].is_zero() {
                continue;
            }
            let factor = row[column].div(&pivot_row[column], b)?;
            for j in column..dim {
                row[j] = row[j].sub(&factor.mul(&pivot_row[j], b)?, b)?;
            }
        }
        rank += 1;
    }
    Ok(rank)
}
/// Build all edge incidence from the exact vertex inventory.
fn incidence(
    vertices: &[Vertex],
    rows: &[Row],
    b: &mut Budget,
) -> Result<(Option<u8>, Vec<Edge>), Fault> {
    let Some(first) = vertices.first() else {
        return Ok((None, Vec::new()));
    };
    let dim = rows[0].coefficient.len();
    let common: Vec<u32> = first
        .active
        .iter()
        .copied()
        .filter(|id| vertices.iter().all(|vertex| vertex.active.contains(id)))
        .collect();
    b.work((first.active.len() * vertices.len()) as u64, AT)?;
    let dimension = u8::try_from(dim - rank(&common, rows, b)?).map_err(|_| claim::invalid())?;
    let mut edges = Vec::new();
    for (i, left) in vertices.iter().enumerate() {
        for (j, right) in vertices.iter().enumerate().skip(i + 1) {
            let shared: Vec<u32> = left
                .active
                .iter()
                .copied()
                .filter(|id| right.active.contains(id))
                .collect();
            b.work((left.active.len() * right.active.len()) as u64, AT)?;
            if rank(&shared, rows, b)? >= dim - 1 {
                edges.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
                edges.push(Edge {
                    ends: [
                        u32::try_from(i).map_err(|_| claim::invalid())?,
                        u32::try_from(j).map_err(|_| claim::invalid())?,
                    ],
                    supports: shared,
                });
            }
        }
    }
    Ok((Some(dimension), edges))
}
/// Enumerate the complete lifted affine topology.
pub(super) fn propose(request: &Request, b: &mut Budget) -> Result<Topology, Fault> {
    let (rows, pi) = claim::rows(request, b)?;
    let dim = rows[0].coefficient.len();
    let mut vertices = Vec::new();
    for ids in combinations(rows.len(), dim, b)? {
        let det = determinant(&matrix(&rows, &ids, b)?, b)?;
        if det.is_zero() {
            continue;
        }
        if let Some(vertex) = candidate(intersect(&rows, &ids, &det, b)?, &rows, &pi, b)? {
            insert(&mut vertices, vertex, &pi, b)?;
        }
    }
    let (dimension, edges) = incidence(&vertices, &rows, b)?;
    Ok(Topology {
        dimension,
        vertices,
        edges,
    })
}
/// Evaluate the exact interval image of a symbolic value.
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
/// Add one ordered sign predicate and its interval image.
fn push_sign(
    signs: &mut Signs,
    value: &PiAffine,
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<(), Fault> {
    let required = value.sign(pi, b)?;
    signs.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
    signs.push(SignPredicate {
        required,
        image: image(value, pi, b)?,
    });
    Ok(())
}
/// Build the complete ordered symbolic sign stratum.
pub(super) fn predicates(request: &Request, b: &mut Budget) -> Result<Signs, Fault> {
    let (rows, pi) = claim::rows(request, b)?;
    let dim = rows[0].coefficient.len();
    let combinations = combinations(rows.len(), dim, b)?;
    let max = combinations
        .len()
        .checked_mul(rows.len() + 1)
        .and_then(|v| v.checked_add(rows.len() + 1))
        .ok_or_else(claim::invalid)?;
    b.check(
        BudgetKind::StagedPayloadBytes,
        (max as u64).saturating_mul(size_of::<SignPredicate>() as u64),
        AT,
    )?;
    let mut signs = Vec::new();
    push_sign(
        &mut signs,
        &PiAffine::rational(request.rate[0].clone()),
        &pi,
        b,
    )?;
    for range in request
        .rate_declarations
        .iter()
        .chain(&request.initial_declarations)
        .chain(&request.clock_declarations)
        .chain(&request.offset_declarations)
        .chain(request.observations.iter().flat_map(|item| {
            item.error_declarations
                .iter()
                .chain(&item.time_declarations)
                .chain(&item.measured_declarations)
                .chain(&item.physical_declarations)
        }))
    {
        push_sign(
            &mut signs,
            &PiAffine::rational(range[1].sub(&range[0], b)?),
            &pi,
            b,
        )?;
    }
    for ids in combinations {
        let det = determinant(&matrix(&rows, &ids, b)?, b)?;
        push_sign(&mut signs, &PiAffine::rational(det.clone()), &pi, b)?;
        if det.is_zero() {
            continue;
        }
        let point = intersect(&rows, &ids, &det, b)?;
        for row in &rows {
            push_sign(&mut signs, &residual(row, &point, b)?, &pi, b)?;
        }
    }
    Ok(signs)
}
