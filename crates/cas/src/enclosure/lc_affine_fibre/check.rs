// SPDX-License-Identifier: AGPL-3.0-only
//! Independent exhaustive checker for an untrusted timing topology.

use super::claim::{self, Row};
use super::{AT, Dimension, Edge, TimingRequest, Topology, Vertex};
use crate::budget::Budget;
use crate::pi_bounds::CheckedPiBounds;
use crate::{Fault, Rational};
use std::cmp::Ordering;

/// Check a proposed vertex against every original inequality and active row.
fn verify_vertex(
    vertex: &Vertex,
    rows: &[Row],
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<(), Fault> {
    let mut active = Vec::new();
    for (id, row) in rows.iter().enumerate() {
        b.work(1, AT)?;
        match claim::residual(row, &vertex.point, b)?.sign(pi, b)? {
            Ordering::Greater => return Err(claim::invalid()),
            Ordering::Equal => {
                active.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
                active.push(u32::try_from(id).map_err(|_| claim::invalid())?);
            }
            Ordering::Less => (),
        }
    }
    if active != vertex.active {
        return Err(claim::invalid());
    }
    let mut rank_two = false;
    for (index, first) in active.iter().enumerate() {
        for second in &active[index + 1..] {
            let det = claim::determinant(&rows[*first as usize], &rows[*second as usize], b)?;
            rank_two |= !det.is_zero();
        }
    }
    if !rank_two {
        return Err(claim::invalid());
    }
    Ok(())
}

/// Compare two exact projected points using checked pi bounds.
fn same_point(
    left: &Vertex,
    right: &Vertex,
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<bool, Fault> {
    Ok(
        left.point[0].compare(&right.point[0], pi, b)? == Ordering::Equal
            && left.point[1].compare(&right.point[1], pi, b)? == Ordering::Equal,
    )
}

/// Enumerate all independent feasible row intersections to detect omissions.
fn verify_complete(
    vertices: &[Vertex],
    rows: &[Row],
    pi: &CheckedPiBounds,
    b: &mut Budget,
) -> Result<(), Fault> {
    for (i, first) in rows.iter().enumerate() {
        for second in &rows[i + 1..] {
            b.work(1, AT)?;
            let det = claim::determinant(first, second, b)?;
            if det == Rational::zero() {
                continue;
            }
            let point = claim::intersect(first, second, &det, b)?;
            let mut feasible = true;
            for row in rows {
                if claim::residual(row, &point, b)?.sign(pi, b)? == Ordering::Greater {
                    feasible = false;
                    break;
                }
            }
            if feasible {
                let probe = Vertex {
                    point,
                    active: Vec::new(),
                };
                let mut found = false;
                for vertex in vertices {
                    if same_point(vertex, &probe, pi, b)? {
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Err(claim::invalid());
                }
            }
        }
    }
    Ok(())
}

/// Derive boundary edges independently from checked vertex incidence.
fn expected_edges(
    vertices: &[Vertex],
    rows: &[Row],
    dimension: Dimension,
) -> Result<Vec<Edge>, Fault> {
    let mut edges: Vec<Edge> = Vec::new();
    if dimension == Dimension::Segment {
        let mut supports = Vec::new();
        for id in 0..rows.len() {
            let row_id = u32::try_from(id).map_err(|_| claim::invalid())?;
            if vertices.iter().all(|v| v.active.contains(&row_id)) {
                supports.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
                supports.push(row_id);
            }
        }
        edges.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
        edges.push(Edge {
            ends: [
                0,
                u32::try_from(vertices.len() - 1).map_err(|_| claim::invalid())?,
            ],
            supports,
        });
    }
    if dimension == Dimension::Polygon {
        for id in 0..rows.len() {
            let row_id = u32::try_from(id).map_err(|_| claim::invalid())?;
            let mut ends: Option<[u32; 2]> = None;
            for (index, vertex) in vertices.iter().enumerate() {
                if vertex.active.contains(&row_id) {
                    let next = u32::try_from(index).map_err(|_| claim::invalid())?;
                    ends = Some(ends.map_or([next, next], |[first, _]| [first, next]));
                }
            }
            if let Some([first, last]) = ends {
                if first == last {
                    continue;
                }
                if let Some(edge) = edges.iter_mut().find(|edge| edge.ends == [first, last]) {
                    edge.supports
                        .try_reserve(1)
                        .map_err(|_| Fault::allocation(AT))?;
                    edge.supports.push(row_id);
                } else {
                    let mut supports = Vec::new();
                    supports
                        .try_reserve_exact(1)
                        .map_err(|_| Fault::allocation(AT))?;
                    supports.push(row_id);
                    edges.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
                    edges.push(Edge {
                        ends: [first, last],
                        supports,
                    });
                }
            }
        }
        edges.sort_by_key(|edge| edge.ends);
    }
    Ok(edges)
}

/// Rebuild every original row, intersection, feasibility test and edge from expected inputs.
pub(super) fn verify(
    request: &TimingRequest,
    topology: &Topology,
    b: &mut Budget,
) -> Result<(), Fault> {
    let (rows, pi) = claim::rows(request, b)?;
    for (index, vertex) in topology.vertices.iter().enumerate() {
        verify_vertex(vertex, &rows, &pi, b)?;
        if let Some(previous) = index.checked_sub(1).and_then(|i| topology.vertices.get(i)) {
            let x = previous.point[0].compare(&vertex.point[0], &pi, b)?;
            let y = previous.point[1].compare(&vertex.point[1], &pi, b)?;
            if x == Ordering::Greater || (x == Ordering::Equal && y != Ordering::Less) {
                return Err(claim::invalid());
            }
        }
    }
    verify_complete(&topology.vertices, &rows, &pi, b)?;
    let expected_dimension = claim::dimension(&topology.vertices, &rows);
    if topology.dimension != expected_dimension
        || topology.edges != expected_edges(&topology.vertices, &rows, expected_dimension)?
    {
        return Err(claim::invalid());
    }
    Ok(())
}
