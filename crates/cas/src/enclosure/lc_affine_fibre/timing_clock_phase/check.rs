// SPDX-License-Identifier: AGPL-3.0-only
//! Independent Gaussian replay and complete incidence check.

use super::super::PiAffine;
use super::claim::{self, Row};
use super::{
    AT, TimingClockPhaseEdge as Edge, TimingClockPhaseRequest as Request,
    TimingClockPhaseTopology as Topology, TimingClockPhaseVertex as Vertex,
};
use crate::budget::Budget;
use crate::pi_bounds::CheckedPiBounds;
use crate::{Fault, Rational};
use std::cmp::Ordering;

/// Solve one independently chosen row intersection.
fn solve(rows: &[Row], ids: &[usize], b: &mut Budget) -> Result<Option<Vec<PiAffine>>, Fault> {
    let dim = ids.len();
    let mut matrix = Vec::new();
    let mut rhs = Vec::new();
    matrix
        .try_reserve_exact(dim)
        .map_err(|_| Fault::allocation(AT))?;
    rhs.try_reserve_exact(dim)
        .map_err(|_| Fault::allocation(AT))?;
    for id in ids {
        b.work(dim as u64, AT)?;
        matrix.push(rows[*id].coefficient.clone());
        rhs.push(rows[*id].bound.clone());
    }
    for column in 0..dim {
        let Some(pivot) = (column..dim).find(|&i| !matrix[i][column].is_zero()) else {
            return Ok(None);
        };
        matrix.swap(column, pivot);
        rhs.swap(column, pivot);
        let pivot_row = matrix[column].clone();
        let pivot_rhs = rhs[column].clone();
        for i in column + 1..dim {
            if matrix[i][column].is_zero() {
                continue;
            }
            let factor = matrix[i][column].div(&pivot_row[column], b)?;
            for j in column..dim {
                matrix[i][j] = matrix[i][j].sub(&factor.mul(&pivot_row[j], b)?, b)?;
            }
            rhs[i] = rhs[i].sub(&pivot_rhs.mul(&factor, b)?, b)?;
        }
    }
    let mut point = Vec::new();
    point
        .try_reserve_exact(dim)
        .map_err(|_| Fault::allocation(AT))?;
    point.resize(dim, PiAffine::rational(Rational::zero()));
    for i in (0..dim).rev() {
        let mut value = rhs[i].clone();
        for j in i + 1..dim {
            value = value.sub(&point[j].mul(&matrix[i][j], b)?, b)?;
        }
        point[i] = value.div(&matrix[i][i], b)?;
    }
    Ok(Some(point))
}
/// Evaluate the exact symbolic slack of a row at a point.
fn residual(row: &Row, point: &[PiAffine], b: &mut Budget) -> Result<PiAffine, Fault> {
    if row.coefficient.len() != point.len() {
        return Err(claim::invalid());
    }
    let mut total = PiAffine::rational(Rational::zero());
    for (coefficient, value) in row.coefficient.iter().zip(point) {
        total = total.add(&value.mul(coefficient, b)?, b)?;
    }
    total.sub(&row.bound, b)
}
/// Compare two symbolic values on the authenticated pi interval.
fn compare(
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
/// Compute the exact rank of selected row normals.
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
    let mut pivots = 0;
    for column in 0..dim {
        let Some(pivot) = (pivots..matrix.len()).find(|&i| !matrix[i][column].is_zero()) else {
            continue;
        };
        matrix.swap(pivots, pivot);
        let divisor = matrix[pivots][column].clone();
        for i in pivots + 1..matrix.len() {
            if matrix[i][column].is_zero() {
                continue;
            }
            let multiplier = matrix[i][column].div(&divisor, b)?;
            for j in column..dim {
                let term = matrix[pivots][j].mul(&multiplier, b)?;
                matrix[i][j] = matrix[i][j].sub(&term, b)?;
            }
        }
        pivots += 1;
    }
    Ok(pivots)
}
/// Enumerate every row subset of the requested size.
fn subsets(count: usize, size: usize, b: &mut Budget) -> Result<Vec<Vec<usize>>, Fault> {
    let mut result = Vec::new();
    if size > count {
        return Ok(result);
    }
    let mut indices: Vec<usize> = (0..size).collect();
    loop {
        b.work(1, AT)?;
        result.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
        result.push(indices.clone());
        let Some(position) = (0..size).rev().find(|&i| indices[i] != i + count - size) else {
            break;
        };
        indices[position] += 1;
        for i in position + 1..size {
            indices[i] = indices[i - 1] + 1;
        }
    }
    Ok(result)
}
/// Verify one proposed vertex and its full active-row inventory.
fn vertex(
    proposed: &Vertex,
    rows: &[Row],
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<(), Fault> {
    let dim = rows[0].coefficient.len();
    if proposed.point.len() != dim {
        return Err(claim::invalid());
    }
    let mut active = Vec::new();
    for (id, row) in rows.iter().enumerate() {
        match residual(row, &proposed.point, b)?.sign(pi, b)? {
            Ordering::Greater => return Err(claim::invalid()),
            Ordering::Equal => {
                active.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
                active.push(u32::try_from(id).map_err(|_| claim::invalid())?);
            }
            Ordering::Less => (),
        }
    }
    if proposed.active != active || rank(&active, rows, b)? != dim {
        return Err(claim::invalid());
    }
    Ok(())
}
/// Reconstruct all independent row intersections and compare vertices.
fn complete(
    proposed: &Topology,
    rows: &[Row],
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<(), Fault> {
    let dim = rows[0].coefficient.len();
    for ids in subsets(rows.len(), dim, b)? {
        let Some(point) = solve(rows, &ids, b)? else {
            continue;
        };
        let mut feasible = true;
        for row in rows {
            if residual(row, &point, b)?.sign(pi, b)? == Ordering::Greater {
                feasible = false;
                break;
            }
        }
        if !feasible {
            continue;
        }
        let mut found = false;
        for vertex in &proposed.vertices {
            if compare(&point, &vertex.point, pi, b)? == Ordering::Equal {
                found = true;
                break;
            }
        }
        if !found {
            return Err(claim::invalid());
        }
    }
    Ok(())
}
/// Reconstruct every edge and its exact supporting rows.
fn incidence(
    vertices: &[Vertex],
    rows: &[Row],
    b: &mut Budget,
) -> Result<(Option<u8>, Vec<Edge>), Fault> {
    let Some(first) = vertices.first() else {
        return Ok((None, Vec::new()));
    };
    let dim = rows[0].coefficient.len();
    let mut common = Vec::new();
    for id in &first.active {
        b.work(vertices.len() as u64, AT)?;
        if vertices.iter().all(|vertex| vertex.active.contains(id)) {
            common.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
            common.push(*id);
        }
    }
    let dimension =
        Some(u8::try_from(dim - rank(&common, rows, b)?).map_err(|_| claim::invalid())?);
    let mut edges = Vec::new();
    for (i, left) in vertices.iter().enumerate() {
        for (j, right) in vertices.iter().enumerate().skip(i + 1) {
            let mut supports = Vec::new();
            for id in &left.active {
                b.work(right.active.len() as u64, AT)?;
                if right.active.contains(id) {
                    supports.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
                    supports.push(*id);
                }
            }
            if rank(&supports, rows, b)? >= dim - 1 {
                edges.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
                edges.push(Edge {
                    ends: [
                        u32::try_from(i).map_err(|_| claim::invalid())?,
                        u32::try_from(j).map_err(|_| claim::invalid())?,
                    ],
                    supports,
                });
            }
        }
    }
    Ok((dimension, edges))
}
/// Independently verify the proposed complete topology.
pub(super) fn verify(request: &Request, proposed: &Topology, b: &mut Budget) -> Result<(), Fault> {
    let (rows, pi) = claim::rows(request, b)?;
    for (index, current) in proposed.vertices.iter().enumerate() {
        vertex(current, &rows, &pi, b)?;
        if let Some(previous) = index.checked_sub(1).and_then(|i| proposed.vertices.get(i))
            && compare(&previous.point, &current.point, &pi, b)? != Ordering::Less
        {
            return Err(claim::invalid());
        }
    }
    complete(proposed, &rows, &pi, b)?;
    let (dimension, edges) = incidence(&proposed.vertices, &rows, b)?;
    if proposed.dimension != dimension || proposed.edges != edges {
        return Err(claim::invalid());
    }
    Ok(())
}
