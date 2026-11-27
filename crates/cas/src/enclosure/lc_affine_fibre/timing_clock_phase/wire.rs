// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical originals and portable variable-dimensional proposal.

use super::super::PiAffine;
use super::claim::invalid;
use super::{
    AT, TimingClockPhaseEdge as Edge, TimingClockPhaseRequest as Request,
    TimingClockPhaseSourceKind as Kind, TimingClockPhaseTopology as Topology,
    TimingClockPhaseVertex as Vertex,
};
use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::wire_read::Reader;
use crate::{Fault, Rational};

/// Encode one exact interval pair.
fn pair(value: &[Rational; 2], out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.number(&value[0])?;
    out.number(&value[1])
}
/// Encode one symbolic affine value.
fn affine(value: &PiAffine, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.number(&value.base)?;
    out.number(&value.pi)
}
/// Encode the complete authenticated source scope.
pub(super) fn scope(request: &Request, b: &mut Budget) -> Result<Vec<u8>, Fault> {
    if request.source_order.len() != request.source_versions.len() {
        return Err(invalid());
    }
    let mut out = Encoder::witness(b, AT);
    out.raw(b"NRZLTP01")?;
    out.unsigned(u64::from(request.revision))?;
    for range in [
        &request.rate,
        &request.initial,
        &request.clock,
        &request.offset,
    ] {
        pair(range, &mut out)?;
    }
    for ranges in [
        &request.rate_declarations,
        &request.initial_declarations,
        &request.clock_declarations,
        &request.offset_declarations,
    ] {
        out.count(ranges.len())?;
        for range in ranges {
            pair(range, &mut out)?;
        }
    }
    out.count(request.observations.len())?;
    for item in &request.observations {
        out.number(&item.reported)?;
        pair(&item.error, &mut out)?;
        for ranges in [
            &item.error_declarations,
            &item.time_declarations,
            &item.measured_declarations,
            &item.physical_declarations,
        ] {
            out.count(ranges.len())?;
            for range in ranges {
                pair(range, &mut out)?;
            }
        }
        out.raw(&item.lift.to_le_bytes())?;
        for value in item.lift_domain {
            out.raw(&value.to_le_bytes())?;
        }
    }
    out.name(&request.epoch)?;
    out.count(request.source_order.len())?;
    for (slot, version) in request.source_order.iter().zip(&request.source_versions) {
        let (tag, index) = match slot.kind {
            Kind::Rate => (0, 0),
            Kind::Initial => (1, 0),
            Kind::Clock => (2, 0),
            Kind::Offset => (3, 0),
            Kind::Error(i) => (4, i),
            Kind::Time(i) => (5, i),
            Kind::Measured(i) => (6, i),
            Kind::Physical(i) => (7, i),
        };
        out.byte(tag)?;
        out.count(index)?;
        out.count(slot.declaration)?;
        out.unsigned(*version)?;
    }
    out.unsigned(u64::from(request.pi.version))?;
    for term in request.pi.terms {
        out.unsigned(u64::from(term))?;
    }
    for value in &request.pi.sums {
        out.number(value)?;
    }
    out.number(&request.pi.lower)?;
    out.number(&request.pi.upper)?;
    Ok(out.finish())
}
/// Encode the proposed topology with its scope.
pub(super) fn encode(scope: &[u8], topology: &Topology, b: &mut Budget) -> Result<Vec<u8>, Fault> {
    let mut out = Encoder::witness(b, AT);
    out.raw(b"NRZLTT01")?;
    out.count(scope.len())?;
    out.raw(scope)?;
    out.byte(topology.dimension.unwrap_or(255))?;
    out.count(topology.vertices.len())?;
    for vertex in &topology.vertices {
        out.count(vertex.point.len())?;
        for value in &vertex.point {
            affine(value, &mut out)?;
        }
        out.count(vertex.active.len())?;
        for id in &vertex.active {
            out.count(*id as usize)?;
        }
    }
    out.count(topology.edges.len())?;
    for edge in &topology.edges {
        for id in edge.ends {
            out.count(id as usize)?;
        }
        out.count(edge.supports.len())?;
        for id in &edge.supports {
            out.count(*id as usize)?;
        }
    }
    Ok(out.finish())
}
/// Decode a retained topology under the current scope.
pub(super) fn decode(
    bytes: &[u8],
    scope: &[u8],
    dim: usize,
    b: &mut Budget,
) -> Result<Topology, Fault> {
    let mut reader = Reader::new(bytes, b)?;
    if reader.take(8)? != b"NRZLTT01" {
        return Err(invalid());
    }
    let length = reader.index()? as usize;
    if reader.take(length)? != scope {
        return Err(invalid());
    }
    let dimension = match reader.byte()? {
        255 => None,
        value if usize::from(value) <= dim => Some(value),
        _ => return Err(invalid()),
    };
    let count = reader.count(BudgetKind::MatrixEntries, 1, size_of::<Vertex>() as u64)?;
    let mut vertices = Vec::new();
    vertices
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        if reader.index()? as usize != dim {
            return Err(invalid());
        }
        let mut point = Vec::new();
        point
            .try_reserve_exact(dim)
            .map_err(|_| Fault::allocation(AT))?;
        for _ in 0..dim {
            point.push(PiAffine {
                base: reader.rational()?,
                pi: reader.rational()?,
            });
        }
        let n = reader.count(BudgetKind::MatrixEntries, 1, 4)?;
        let mut active = Vec::new();
        active
            .try_reserve_exact(n)
            .map_err(|_| Fault::allocation(AT))?;
        for _ in 0..n {
            active.push(reader.index()?);
        }
        vertices.push(Vertex { point, active });
    }
    let count = reader.count(BudgetKind::MatrixEntries, 1, size_of::<Edge>() as u64)?;
    let mut edges = Vec::new();
    edges
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        let ends = [reader.index()?, reader.index()?];
        let n = reader.count(BudgetKind::MatrixEntries, 1, 4)?;
        let mut supports = Vec::new();
        supports
            .try_reserve_exact(n)
            .map_err(|_| Fault::allocation(AT))?;
        for _ in 0..n {
            supports.push(reader.index()?);
        }
        edges.push(Edge { ends, supports });
    }
    reader.finished()?;
    Ok(Topology {
        dimension,
        vertices,
        edges,
    })
}
