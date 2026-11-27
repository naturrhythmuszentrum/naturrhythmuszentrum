// SPDX-License-Identifier: AGPL-3.0-only
//! Independent elimination and exhaustive verification of lifted incidence.

use super::claim::{self, Row};
use super::geometry;
use super::{AT, ClockPhaseEdge, ClockPhaseRequest, ClockPhaseTopology, ClockPhaseVertex};
use crate::budget::Budget;
use crate::pi_bounds::CheckedPiBounds;
use crate::{Fault, Rational};
use std::cmp::Ordering;

use super::super::PiAffine;

/// Solve a four-row system by elimination, independently of the proposal's Cramer expansion.
fn solve(rows: &[Row], ids: [usize; 4], b: &mut Budget) -> Result<Option<[PiAffine; 4]>, Fault> {
    b.work(20, AT)?;
    let mut coefficients = std::array::from_fn::<_, 4, _>(|i| rows[ids[i]].coefficient.clone());
    let mut rhs = std::array::from_fn::<_, 4, _>(|i| rows[ids[i]].bound.clone());
    for column in 0..4 {
        let Some(pivot) = (column..4).find(|&i| !coefficients[i][column].is_zero()) else {
            return Ok(None);
        };
        coefficients.swap(column, pivot);
        rhs.swap(column, pivot);
        let pivot_row = coefficients[column].clone();
        let pivot_rhs = rhs[column].clone();
        b.work(5, AT)?;
        for i in column + 1..4 {
            if coefficients[i][column].is_zero() {
                continue;
            }
            let factor = coefficients[i][column].div(&pivot_row[column], b)?;
            for j in column..4 {
                coefficients[i][j] = coefficients[i][j].sub(&factor.mul(&pivot_row[j], b)?, b)?;
            }
            rhs[i] = rhs[i].sub(&pivot_rhs.mul(&factor, b)?, b)?;
        }
    }
    let mut point: [PiAffine; 4] = std::array::from_fn(|_| PiAffine::rational(Rational::zero()));
    for i in (0..4).rev() {
        let mut remaining = rhs[i].clone();
        b.work(1, AT)?;
        for j in i + 1..4 {
            remaining = remaining.sub(&point[j].mul(&coefficients[i][j], b)?, b)?;
        }
        point[i] = remaining.div(&coefficients[i][i], b)?;
    }
    Ok(Some(point))
}

/// Compare two checked vertices in canonical coordinate order.
fn point_order(
    left: &[PiAffine; 4],
    right: &[PiAffine; 4],
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<Ordering, Fault> {
    for (a, c) in left.iter().zip(right) {
        let order = a.compare(c, pi, b)?;
        if order != Ordering::Equal {
            return Ok(order);
        }
    }
    Ok(Ordering::Equal)
}

/// Verify every proposed vertex against every original row and its active inventory.
fn vertex(
    proposed: &ClockPhaseVertex,
    rows: &[Row],
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<(), Fault> {
    let mut active = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        match geometry::residual(row, &proposed.point, b)?.sign(pi, b)? {
            Ordering::Greater => return Err(claim::invalid()),
            Ordering::Equal => {
                active.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
                active.push(u32::try_from(index).map_err(|_| claim::invalid())?);
            }
            Ordering::Less => (),
        }
    }
    if proposed.active != active || independent_rank(&active, rows, b)? != 4 {
        return Err(claim::invalid());
    }
    Ok(())
}

/// Evaluate a square rational minor by a checker-local Laplace expansion.
fn minor(matrix: &[Vec<Rational>], b: &mut Budget) -> Result<Rational, Fault> {
    if matrix.len() == 1 {
        return Ok(matrix[0][0].clone());
    }
    let mut sum = Rational::zero();
    for column in 0..matrix.len() {
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
        let term = matrix[0][column].mul(&minor(&child, b)?, b)?;
        sum = if column % 2 == 0 {
            sum.add(&term, b)?
        } else {
            sum.sub(&term, b)?
        };
    }
    Ok(sum)
}

/// Test one set of row IDs for an independent minor of the requested size.
fn independent_minor(
    ids: &[u32],
    rows: &[Row],
    columns: &[usize],
    b: &mut Budget,
) -> Result<bool, Fault> {
    let mut matrix = Vec::new();
    matrix
        .try_reserve_exact(ids.len())
        .map_err(|_| Fault::allocation(AT))?;
    for id in ids {
        let mut values = Vec::new();
        values
            .try_reserve_exact(columns.len())
            .map_err(|_| Fault::allocation(AT))?;
        for column in columns {
            b.work(1, AT)?;
            values.push(rows[*id as usize].coefficient[*column].clone());
        }
        matrix.push(values);
    }
    Ok(!minor(&matrix, b)?.is_zero())
}

/// Find exact normal rank from nonzero minors, independently of proposal elimination.
fn independent_rank(ids: &[u32], rows: &[Row], b: &mut Budget) -> Result<usize, Fault> {
    for size in (1..=4.min(ids.len())).rev() {
        let row_choices: Vec<Vec<u32>> = subsets(ids, size, b)?;
        let columns: Vec<usize> = (0..4).collect();
        for chosen_rows in &row_choices {
            for chosen_columns in subsets(&columns, size, b)? {
                if independent_minor(chosen_rows, rows, &chosen_columns, b)? {
                    return Ok(size);
                }
            }
        }
    }
    Ok(0)
}

/// Enumerate stable size-k subsets with fallible reservations.
fn subsets<T: Clone>(items: &[T], size: usize, b: &mut Budget) -> Result<Vec<Vec<T>>, Fault> {
    let mut choices = Vec::new();
    if size > items.len() {
        return Ok(choices);
    }
    let mut indices: Vec<usize> = (0..size).collect();
    loop {
        b.work(1, AT)?;
        let mut choice = Vec::new();
        choice
            .try_reserve_exact(size)
            .map_err(|_| Fault::allocation(AT))?;
        for &index in &indices {
            choice.push(items[index].clone());
        }
        choices.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
        choices.push(choice);
        let Some(position) = (0..size)
            .rev()
            .find(|&i| indices[i] != i + items.len() - size)
        else {
            break;
        };
        indices[position] += 1;
        for i in position + 1..size {
            indices[i] = indices[i - 1] + 1;
        }
    }
    Ok(choices)
}

/// Rebuild dimension and edge supports through checker-local normal minors.
fn independent_incidence(
    vertices: &[ClockPhaseVertex],
    rows: &[Row],
    b: &mut Budget,
) -> Result<(Option<u8>, Vec<ClockPhaseEdge>), Fault> {
    let Some(first) = vertices.first() else {
        return Ok((None, Vec::new()));
    };
    let mut common = Vec::new();
    for id in &first.active {
        b.work(vertices.len() as u64, AT)?;
        if vertices.iter().all(|vertex| vertex.active.contains(id)) {
            common.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
            common.push(*id);
        }
    }
    let dimension =
        Some(u8::try_from(4 - independent_rank(&common, rows, b)?).map_err(|_| claim::invalid())?);
    let mut edges = Vec::new();
    for (left_id, left) in vertices.iter().enumerate() {
        for (right_id, right) in vertices.iter().enumerate().skip(left_id + 1) {
            let mut supports = Vec::new();
            for id in &left.active {
                b.work(right.active.len() as u64, AT)?;
                if right.active.contains(id) {
                    supports.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
                    supports.push(*id);
                }
            }
            if independent_rank(&supports, rows, b)? >= 3 {
                edges.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
                edges.push(ClockPhaseEdge {
                    ends: [
                        u32::try_from(left_id).map_err(|_| claim::invalid())?,
                        u32::try_from(right_id).map_err(|_| claim::invalid())?,
                    ],
                    supports,
                });
            }
        }
    }
    Ok((dimension, edges))
}

/// Enumerate every independent feasible intersection, detecting omitted vertices.
fn complete(
    proposed: &ClockPhaseTopology,
    rows: &[Row],
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<(), Fault> {
    for ids in geometry::combinations(rows) {
        let Some(point) = solve(rows, ids, b)? else {
            continue;
        };
        let mut feasible = true;
        for row in rows {
            if geometry::residual(row, &point, b)?.sign(pi, b)? == Ordering::Greater {
                feasible = false;
                break;
            }
        }
        if !feasible {
            continue;
        }
        let mut found = false;
        for vertex in &proposed.vertices {
            if point_order(&point, &vertex.point, pi, b)? == Ordering::Equal {
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

/// Rebuild all original rows and independently verify complete topology.
pub(super) fn verify(
    request: &ClockPhaseRequest,
    proposed: &ClockPhaseTopology,
    b: &mut Budget,
) -> Result<(), Fault> {
    let (rows, pi) = claim::rows(request, b)?;
    for (index, current) in proposed.vertices.iter().enumerate() {
        vertex(current, &rows, &pi, b)?;
        if let Some(previous) = index.checked_sub(1).and_then(|i| proposed.vertices.get(i))
            && point_order(&previous.point, &current.point, &pi, b)? != Ordering::Less
        {
            return Err(claim::invalid());
        }
    }
    complete(proposed, &rows, &pi, b)?;
    let (dimension, edges) = independent_incidence(&proposed.vertices, &rows, b)?;
    if proposed.dimension != dimension || proposed.edges != edges {
        return Err(claim::invalid());
    }
    Ok(())
}
