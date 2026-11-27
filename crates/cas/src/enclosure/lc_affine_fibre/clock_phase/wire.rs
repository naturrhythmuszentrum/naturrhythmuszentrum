// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical original inventory and portable clock-phase proposal.

use super::super::PiAffine;
use super::claim::invalid;
use super::{
    AT, ClockPhaseEdge, ClockPhaseRequest, ClockPhaseSourceKind, ClockPhaseTopology,
    ClockPhaseVertex,
};
use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::wire_read::Reader;
use crate::{Fault, Rational};

/// Encode both original rational endpoints.
fn pair(value: &[Rational; 2], out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.number(&value[0])?;
    out.number(&value[1])
}

/// Encode one exact affine-pi expression.
fn affine(value: &PiAffine, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.number(&value.base)?;
    out.number(&value.pi)
}

/// Encode the complete canonical original request.
pub(super) fn scope(request: &ClockPhaseRequest, b: &mut Budget) -> Result<Vec<u8>, Fault> {
    if request.source_order.len() != request.source_versions.len() {
        return Err(invalid());
    }
    let mut out = Encoder::witness(b, AT);
    out.raw(b"NRZLCP01")?;
    out.unsigned(u64::from(request.revision))?;
    for range in [
        &request.rate,
        &request.initial,
        &request.clock,
        &request.offset,
    ] {
        pair(range, &mut out)?;
    }
    for declarations in [
        &request.rate_declarations,
        &request.initial_declarations,
        &request.clock_declarations,
        &request.offset_declarations,
    ] {
        out.count(declarations.len())?;
        for range in declarations {
            pair(range, &mut out)?;
        }
    }
    out.count(request.observations.len())?;
    for item in &request.observations {
        out.number(&item.reported)?;
        for declarations in [
            &item.time_declarations,
            &item.measured_declarations,
            &item.physical_declarations,
        ] {
            out.count(declarations.len())?;
            for range in declarations {
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
            ClockPhaseSourceKind::Rate => (0, 0),
            ClockPhaseSourceKind::Initial => (1, 0),
            ClockPhaseSourceKind::Clock => (2, 0),
            ClockPhaseSourceKind::Offset => (3, 0),
            ClockPhaseSourceKind::Time(index) => (4, index),
            ClockPhaseSourceKind::Measured(index) => (5, index),
            ClockPhaseSourceKind::Physical(index) => (6, index),
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
    for sum in &request.pi.sums {
        out.number(sum)?;
    }
    out.number(&request.pi.lower)?;
    out.number(&request.pi.upper)?;
    Ok(out.finish())
}

/// Encode a portable topology beside its original scope.
pub(super) fn encode(
    scope: &[u8],
    topology: &ClockPhaseTopology,
    b: &mut Budget,
) -> Result<Vec<u8>, Fault> {
    let mut out = Encoder::witness(b, AT);
    out.raw(b"NRZLCT01")?;
    out.count(scope.len())?;
    out.raw(scope)?;
    out.byte(topology.dimension.unwrap_or(255))?;
    out.count(topology.vertices.len())?;
    for vertex in &topology.vertices {
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

/// Decode bounded untrusted topology fields against expected scope.
pub(super) fn decode(
    bytes: &[u8],
    scope: &[u8],
    b: &mut Budget,
) -> Result<ClockPhaseTopology, Fault> {
    let mut reader = Reader::new(bytes, b)?;
    if reader.take(8)? != b"NRZLCT01" {
        return Err(invalid());
    }
    let length = reader.index()? as usize;
    if reader.take(length)? != scope {
        return Err(invalid());
    }
    let dimension = match reader.byte()? {
        255 => None,
        value @ 0..=4 => Some(value),
        _ => return Err(invalid()),
    };
    let count = reader.count(
        BudgetKind::MatrixEntries,
        1,
        size_of::<ClockPhaseVertex>() as u64,
    )?;
    let mut vertices = Vec::new();
    vertices
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        let point = std::array::from_fn::<_, 4, _>(|_| PiAffine::rational(Rational::zero()));
        let mut point = point;
        for value in &mut point {
            *value = PiAffine {
                base: reader.rational()?,
                pi: reader.rational()?,
            };
        }
        let n = reader.count(BudgetKind::MatrixEntries, 1, 4)?;
        let mut active = Vec::new();
        active
            .try_reserve_exact(n)
            .map_err(|_| Fault::allocation(AT))?;
        for _ in 0..n {
            active.push(reader.index()?);
        }
        vertices.push(ClockPhaseVertex { point, active });
    }
    let count = reader.count(
        BudgetKind::MatrixEntries,
        1,
        size_of::<ClockPhaseEdge>() as u64,
    )?;
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
        edges.push(ClockPhaseEdge { ends, supports });
    }
    reader.finished()?;
    Ok(ClockPhaseTopology {
        dimension,
        vertices,
        edges,
    })
}
