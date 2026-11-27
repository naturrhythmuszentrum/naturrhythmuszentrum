// SPDX-License-Identifier: AGPL-3.0-only
//! Exhaustive exact four-coordinate incidence and ordered sign predicates.

use super::claim::{self, Row};
use super::{AT, ClockPhaseEdge, ClockPhaseRequest, ClockPhaseTopology, ClockPhaseVertex, Signs};
use crate::budget::{Budget, BudgetKind};
use crate::pi_bounds::CheckedPiBounds;
use crate::{Fault, Rational};
use std::cmp::Ordering;

use super::super::{PiAffine, SignPredicate};

/// Exact determinant of a four-row coefficient matrix.
fn determinant(matrix: &[[Rational; 4]; 4], b: &mut Budget) -> Result<Rational, Fault> {
    let mut sum = Rational::zero();
    for a in 0..4 {
        for c in 0..4 {
            if c == a {
                continue;
            }
            for d in 0..4 {
                if d == a || d == c {
                    continue;
                }
                let e = 6 - a - c - d;
                let permutation = [a, c, d, e];
                let inversions = (0..4)
                    .flat_map(|i| (i + 1..4).map(move |j| (i, j)))
                    .filter(|&(i, j)| permutation[i] > permutation[j])
                    .count();
                let term = matrix[0][a]
                    .mul(&matrix[1][c], b)?
                    .mul(&matrix[2][d], b)?
                    .mul(&matrix[3][e], b)?;
                sum = if inversions % 2 == 0 {
                    sum.add(&term, b)?
                } else {
                    sum.sub(&term, b)?
                };
            }
        }
    }
    Ok(sum)
}

/// Copy four coefficient rows under the caller ledger.
fn matrix(rows: &[Row], ids: [usize; 4], b: &mut Budget) -> Result<[[Rational; 4]; 4], Fault> {
    b.work(16, AT)?;
    Ok(std::array::from_fn(|index| {
        rows[ids[index]].coefficient.clone()
    }))
}

/// Solve one independent four-row intersection with exact affine-pi right sides.
pub(super) fn intersect(
    rows: &[Row],
    ids: [usize; 4],
    det: &Rational,
    b: &mut Budget,
) -> Result<[PiAffine; 4], Fault> {
    let original = matrix(rows, ids, b)?;
    let mut point = Vec::new();
    point
        .try_reserve_exact(4)
        .map_err(|_| Fault::allocation(AT))?;
    for coordinate in 0..4 {
        let mut base = original.clone();
        let mut pi = original.clone();
        b.work(32, AT)?;
        for row in 0..4 {
            base[row][coordinate].clone_from(&rows[ids[row]].bound.base);
            pi[row][coordinate].clone_from(&rows[ids[row]].bound.pi);
        }
        point.push(PiAffine {
            base: determinant(&base, b)?.div(det, b)?,
            pi: determinant(&pi, b)?.div(det, b)?,
        });
    }
    point.try_into().map_err(|_| claim::invalid())
}

/// Exact residual of one original inequality at one lifted point.
pub(super) fn residual(
    row: &Row,
    point: &[PiAffine; 4],
    b: &mut Budget,
) -> Result<PiAffine, Fault> {
    let mut sum = PiAffine::rational(Rational::zero());
    for (coefficient, value) in row.coefficient.iter().zip(point) {
        sum = sum.add(&value.mul(coefficient, b)?, b)?;
    }
    sum.sub(&row.bound, b)
}

/// Compare lifted coordinates lexicographically with exact pi signs.
fn compare_points(
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

/// Return one feasible candidate with its full active-row inventory.
fn candidate(
    point: [PiAffine; 4],
    rows: &[Row],
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<Option<ClockPhaseVertex>, Fault> {
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
    Ok(Some(ClockPhaseVertex { point, active }))
}

/// Insert one unique vertex in canonical coordinate order.
fn insert(
    vertices: &mut Vec<ClockPhaseVertex>,
    vertex: ClockPhaseVertex,
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<(), Fault> {
    let mut position = vertices.len();
    for (index, existing) in vertices.iter().enumerate() {
        match compare_points(&vertex.point, &existing.point, pi, b)? {
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

/// Iterate every ordered independent-row candidate in a stable input order.
pub(super) fn combinations(rows: &[Row]) -> impl Iterator<Item = [usize; 4]> + '_ {
    (0..rows.len()).flat_map(move |a| {
        (a + 1..rows.len()).flat_map(move |c| {
            (c + 1..rows.len()).flat_map(move |d| (d + 1..rows.len()).map(move |e| [a, c, d, e]))
        })
    })
}

/// Compute the rank of original rational row normals by exact elimination.
pub(super) fn rank(ids: &[u32], rows: &[Row], b: &mut Budget) -> Result<usize, Fault> {
    let mut matrix = Vec::new();
    matrix
        .try_reserve_exact(ids.len())
        .map_err(|_| Fault::allocation(AT))?;
    for id in ids {
        b.work(4, AT)?;
        matrix.push(rows[*id as usize].coefficient.clone());
    }
    let mut rank = 0;
    for column in 0..4 {
        let Some(pivot) = (rank..matrix.len()).find(|&i| !matrix[i][column].is_zero()) else {
            continue;
        };
        matrix.swap(rank, pivot);
        b.work(4, AT)?;
        let pivot_row = matrix[rank].clone();
        for row in matrix.iter_mut().skip(rank + 1) {
            if row[column].is_zero() {
                continue;
            }
            let factor = row[column].div(&pivot_row[column], b)?;
            for j in column..4 {
                row[j] = row[j].sub(&factor.mul(&pivot_row[j], b)?, b)?;
            }
        }
        rank += 1;
    }
    Ok(rank)
}

/// Compute dimension and all maximal edge incidence from original row ranks.
pub(super) fn incidence(
    vertices: &[ClockPhaseVertex],
    rows: &[Row],
    b: &mut Budget,
) -> Result<(Option<u8>, Vec<ClockPhaseEdge>), Fault> {
    let Some(first) = vertices.first() else {
        return Ok((None, Vec::new()));
    };
    let common: Vec<u32> = first
        .active
        .iter()
        .copied()
        .filter(|id| vertices.iter().all(|vertex| vertex.active.contains(id)))
        .collect();
    b.work((first.active.len() * vertices.len()) as u64, AT)?;
    let dimension = 4 - rank(&common, rows, b)?;
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
            if rank(&shared, rows, b)? >= 3 {
                edges.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
                edges.push(ClockPhaseEdge {
                    ends: [
                        u32::try_from(i).map_err(|_| claim::invalid())?,
                        u32::try_from(j).map_err(|_| claim::invalid())?,
                    ],
                    supports: shared,
                });
            }
        }
    }
    Ok((
        Some(u8::try_from(dimension).map_err(|_| claim::invalid())?),
        edges,
    ))
}

/// Propose the complete exact lifted vertex and edge incidence.
pub(super) fn propose(
    request: &ClockPhaseRequest,
    b: &mut Budget,
) -> Result<ClockPhaseTopology, Fault> {
    let (rows, pi) = claim::rows(request, b)?;
    let mut vertices = Vec::new();
    for ids in combinations(&rows) {
        let det = determinant(&matrix(&rows, ids, b)?, b)?;
        if det.is_zero() {
            continue;
        }
        let point = intersect(&rows, ids, &det, b)?;
        if let Some(vertex) = candidate(point, &rows, &pi, b)? {
            insert(&mut vertices, vertex, &pi, b)?;
        }
    }
    let (dimension, edges) = incidence(&vertices, &rows, b)?;
    Ok(ClockPhaseTopology {
        dimension,
        vertices,
        edges,
    })
}

/// Bound one affine-pi value using the checked pi authority.
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

/// Retain one exact sign and its rational enclosure.
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

/// Inventory original support order, independent determinants and all candidate residual signs.
pub(super) fn predicates(request: &ClockPhaseRequest, b: &mut Budget) -> Result<Signs, Fault> {
    let (rows, pi) = claim::rows(request, b)?;
    let n = rows.len();
    let maximum = n
        .checked_mul(n.saturating_sub(1))
        .and_then(|v| v.checked_mul(n.saturating_sub(2)))
        .and_then(|v| v.checked_mul(n.saturating_sub(3)))
        .and_then(|v| v.checked_div(24))
        .and_then(|v| v.checked_mul(n + 1))
        .and_then(|v| {
            v.checked_add(
                1 + request.rate_declarations.len()
                    + request.initial_declarations.len()
                    + request.clock_declarations.len()
                    + request.offset_declarations.len()
                    + request
                        .observations
                        .iter()
                        .map(|item| {
                            item.time_declarations.len()
                                + item.measured_declarations.len()
                                + item.physical_declarations.len()
                        })
                        .sum::<usize>(),
            )
        })
        .ok_or_else(claim::invalid)?;
    b.check(
        BudgetKind::StagedPayloadBytes,
        (maximum as u64).saturating_mul(size_of::<SignPredicate>() as u64),
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
            item.time_declarations
                .iter()
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
    for ids in combinations(&rows) {
        let det = determinant(&matrix(&rows, ids, b)?, b)?;
        push_sign(&mut signs, &PiAffine::rational(det.clone()), &pi, b)?;
        if det.is_zero() {
            continue;
        }
        let point = intersect(&rows, ids, &det, b)?;
        for row in &rows {
            push_sign(&mut signs, &residual(row, &point, b)?, &pi, b)?;
        }
    }
    Ok(signs)
}
