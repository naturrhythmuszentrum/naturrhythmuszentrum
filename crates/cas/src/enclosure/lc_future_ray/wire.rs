// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical complete-source scope and portable projected-ray proposal.

use super::{
    AT, FutureRayForm, FutureRayProposal, FutureRayRequest, FutureRaySource, FutureRayVertex, claim,
};
use crate::Fault;
use crate::budget::Budget;
use crate::enclosure::lc_affine_fibre::PiAffine;
use crate::encoding::Encoder;
use crate::wire_read::Reader;

/// Encode one exact rational-plus-pi coordinate.
fn affine(value: &PiAffine, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.number(&value.base)?;
    out.number(&value.pi)
}
/// Decode one exact rational-plus-pi coordinate.
fn read_affine(reader: &mut Reader<'_, '_>) -> Result<PiAffine, Fault> {
    Ok(PiAffine {
        base: reader.rational()?,
        pi: reader.rational()?,
    })
}
/// Bind the exact reference, coordinate, epoch, inventory and original fibre certificates.
pub(super) fn scope(request: &FutureRayRequest, b: &mut Budget) -> Result<Vec<u8>, Fault> {
    let mut certificates = Vec::new();
    certificates
        .try_reserve_exact(request.branches.len())
        .map_err(|_| Fault::allocation(AT))?;
    for branch in &request.branches {
        certificates.push(match &branch.source {
            FutureRaySource::Timing(_, cert) => (0, cert.to_bytes(b)?),
            FutureRaySource::ClockPhase(_, cert) => (1, cert.to_bytes(b)?),
            FutureRaySource::TimingClockPhase(_, cert) => (2, cert.to_bytes(b)?),
        });
    }
    let mut out = Encoder::witness(b, AT);
    out.raw(b"NRZLFRS1")?;
    out.byte(match request.form {
        FutureRayForm::Plain => 0,
        FutureRayForm::Narrow => 1,
    })?;
    out.number(&request.reference)?;
    affine(&request.safe_reference, &mut out)?;
    out.byte(u8::from(request.narrowed))?;
    out.name(&request.epoch)?;
    out.count(request.branches.len())?;
    for (branch, (tag, certificate)) in request.branches.iter().zip(certificates) {
        out.unsigned(branch.id)?;
        out.raw(&branch.shift.to_le_bytes())?;
        out.byte(tag)?;
        out.count(certificate.len())?;
        out.raw(&certificate)?;
    }
    Ok(out.finish())
}
/// Encode a finite exact proposal after its complete-source scope.
pub(super) fn encode(
    scope: &[u8],
    proposal: &FutureRayProposal,
    b: &mut Budget,
) -> Result<Vec<u8>, Fault> {
    let mut out = Encoder::witness(b, AT);
    out.raw(b"NRZLFRP1")?;
    out.count(scope.len())?;
    out.raw(scope)?;
    out.count(proposal.vertices.len())?;
    for vertex in &proposal.vertices {
        out.count(vertex.branch as usize)?;
        out.count(vertex.source_vertex as usize)?;
        for value in &vertex.point {
            affine(value, &mut out)?;
        }
    }
    out.count(proposal.hull.len())?;
    for id in &proposal.hull {
        out.count(*id as usize)?;
    }
    out.count(proposal.low as usize)?;
    out.count(proposal.high as usize)?;
    out.count(proposal.comparisons.len())?;
    for row in &proposal.comparisons {
        for value in row {
            affine(value, &mut out)?;
        }
    }
    Ok(out.finish())
}
/// Decode bounded untrusted bytes; the checker reconstructs every field.
pub(super) fn decode(
    bytes: &[u8],
    scope: &[u8],
    b: &mut Budget,
) -> Result<FutureRayProposal, Fault> {
    let mut reader = Reader::new(bytes, b)?;
    if reader.take(8)? != b"NRZLFRP1" {
        return Err(claim::invalid());
    }
    let length = reader.index()? as usize;
    if reader.take(length)? != scope {
        return Err(claim::invalid());
    }
    let vertex_count = reader.index()? as usize;
    reader
        .allocation(vertex_count.saturating_mul(std::mem::size_of::<FutureRayVertex>()) as u64)?;
    let mut vertices = Vec::new();
    vertices
        .try_reserve_exact(vertex_count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..vertex_count {
        vertices.push(FutureRayVertex {
            branch: reader.index()?,
            source_vertex: reader.index()?,
            point: [read_affine(&mut reader)?, read_affine(&mut reader)?],
        });
    }
    let hull_count = reader.index()? as usize;
    reader.allocation(hull_count.saturating_mul(std::mem::size_of::<u32>()) as u64)?;
    let mut hull = Vec::new();
    hull.try_reserve_exact(hull_count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..hull_count {
        hull.push(reader.index()?);
    }
    let low = reader.index()?;
    let high = reader.index()?;
    let comparison_count = reader.index()? as usize;
    reader
        .allocation(comparison_count.saturating_mul(std::mem::size_of::<[PiAffine; 4]>()) as u64)?;
    let mut comparisons = Vec::new();
    comparisons
        .try_reserve_exact(comparison_count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..comparison_count {
        comparisons.push([
            read_affine(&mut reader)?,
            read_affine(&mut reader)?,
            read_affine(&mut reader)?,
            read_affine(&mut reader)?,
        ]);
    }
    reader.finished()?;
    Ok(FutureRayProposal {
        vertices,
        hull,
        low,
        high,
        comparisons,
    })
}
