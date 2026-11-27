// SPDX-License-Identifier: AGPL-3.0-only
//! Portable exact timing claim and untrusted topology fields.

use super::claim::invalid;
use super::{AT, Dimension, Edge, PiAffine, TimingRequest, TimingSourceKind, Topology, Vertex};
use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::wire_read::Reader;
use crate::{Fault, Rational};

/// Encode one exact affine pi expression.
fn affine(value: &PiAffine, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.number(&value.base)?;
    out.number(&value.pi)
}

/// Decode one bounded exact affine pi expression.
fn read_affine(reader: &mut Reader<'_, '_>) -> Result<PiAffine, Fault> {
    Ok(PiAffine {
        base: reader.rational()?,
        pi: reader.rational()?,
    })
}

/// Encode a signed turn without losing its bit pattern.
fn signed(value: i64, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.raw(&value.to_le_bytes())
}

/// Encode both endpoints of one original rational support.
fn pair(range: &[Rational; 2], out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.number(&range[0])?;
    out.number(&range[1])
}

/// Canonical complete original inventory, including all versions and pi theorem data.
pub(super) fn scope(request: &TimingRequest, b: &mut Budget) -> Result<Vec<u8>, Fault> {
    let mut out = Encoder::witness(b, AT);
    out.raw(b"NRZLAF01")?;
    out.unsigned(u64::from(request.revision))?;
    pair(&request.rate, &mut out)?;
    pair(&request.initial, &mut out)?;
    out.count(request.rate_declarations.len())?;
    for range in &request.rate_declarations {
        pair(range, &mut out)?;
    }
    out.count(request.initial_declarations.len())?;
    for range in &request.initial_declarations {
        pair(range, &mut out)?;
    }
    out.count(request.observations.len())?;
    for item in &request.observations {
        pair(&item.time, &mut out)?;
        pair(&item.phase, &mut out)?;
        out.count(item.time_declarations.len())?;
        for range in &item.time_declarations {
            pair(range, &mut out)?;
        }
        out.count(item.phase_declarations.len())?;
        for range in &item.phase_declarations {
            pair(range, &mut out)?;
        }
        signed(item.lift, &mut out)?;
        signed(item.lift_domain[0], &mut out)?;
        signed(item.lift_domain[1], &mut out)?;
    }
    out.name(&request.epoch)?;
    out.count(request.nominal.len())?;
    for nominal in &request.nominal {
        out.number(nominal)?;
    }
    out.count(request.source_versions.len())?;
    for (slot, version) in request.source_order.iter().zip(&request.source_versions) {
        let (tag, observation) = match slot.kind {
            TimingSourceKind::Rate => (0, 0),
            TimingSourceKind::Initial => (1, 0),
            TimingSourceKind::Time(index) => (2, index),
            TimingSourceKind::Phase(index) => (3, index),
        };
        out.byte(tag)?;
        out.count(observation)?;
        out.count(slot.declaration)?;
        out.unsigned(*version)?;
    }
    out.unsigned(u64::from(request.pi.version))?;
    for term in request.pi.terms {
        out.unsigned(u64::from(term))?;
    }
    for sum in &request.pi.sums {
        out.number(sum)?;
    }
    out.number(&request.pi.lower)?;
    out.number(&request.pi.upper)?;
    Ok(out.finish())
}

/// Encode the proposal separately from original scope; neither is a receipt identity.
pub(super) fn encode(scope: &[u8], topology: &Topology, b: &mut Budget) -> Result<Vec<u8>, Fault> {
    let mut out = Encoder::witness(b, AT);
    out.raw(b"NRZLAT01")?;
    out.count(scope.len())?;
    out.raw(scope)?;
    out.byte(match topology.dimension {
        Dimension::Empty => 0,
        Dimension::Point => 1,
        Dimension::Segment => 2,
        Dimension::Polygon => 3,
    })?;
    out.count(topology.vertices.len())?;
    for vertex in &topology.vertices {
        affine(&vertex.point[0], &mut out)?;
        affine(&vertex.point[1], &mut out)?;
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

/// Decode bounded proposal fields against a freshly reconstructed expected scope.
pub(super) fn decode(bytes: &[u8], scope: &[u8], b: &mut Budget) -> Result<Topology, Fault> {
    let mut reader = Reader::new(bytes, b)?;
    if reader.take(8)? != b"NRZLAT01" {
        return Err(invalid());
    }
    let length = reader.index()? as usize;
    if reader.take(length)? != scope {
        return Err(invalid());
    }
    let dimension = match reader.byte()? {
        0 => Dimension::Empty,
        1 => Dimension::Point,
        2 => Dimension::Segment,
        3 => Dimension::Polygon,
        _ => return Err(invalid()),
    };
    let count = reader.count(BudgetKind::MatrixEntries, 1, size_of::<Vertex>() as u64)?;
    let mut vertices = Vec::new();
    vertices
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        let point = [read_affine(&mut reader)?, read_affine(&mut reader)?];
        let active_count = reader.count(BudgetKind::MatrixEntries, 1, 4)?;
        let mut active = Vec::new();
        active
            .try_reserve_exact(active_count)
            .map_err(|_| Fault::allocation(AT))?;
        for _ in 0..active_count {
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
        let length = reader.count(BudgetKind::MatrixEntries, 1, 4)?;
        let mut supports = Vec::new();
        supports
            .try_reserve_exact(length)
            .map_err(|_| Fault::allocation(AT))?;
        for _ in 0..length {
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
