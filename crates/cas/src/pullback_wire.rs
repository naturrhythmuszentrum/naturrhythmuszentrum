// SPDX-License-Identifier: AGPL-3.0-only
//! Portable coordinate maps, inverse maps and local change-of-variable certificates.

use crate::budget::BudgetKind;
use crate::encoding::Encoder;
use crate::error::Fault;
use crate::graph::Handle;
use crate::pullback_geometry::AT;
use crate::pullback_model::{
    PullbackCertificate, PullbackKind, PullbackMode, PullbackValue, PullbackZero,
};
use crate::wire_decode::{Decoded, object};
use crate::wire_read::{Reader, invalid};

/// Request roots are untrusted exact factorization data, never hidden producer state.
pub(crate) fn intent(value: &PullbackKind, out: &mut Encoder<'_>) -> Result<(), Fault> {
    match value {
        PullbackKind::Affine => out.byte(0),
        PullbackKind::SimpleZeros(roots) => {
            out.byte(1)?;
            crate::split_wire::numbers(roots, out)
        }
    }
}

/// Retain both spaces and the optional affine inverse as distinct semantic dependencies.
pub(crate) fn objects(value: &PullbackValue) -> Vec<&Handle> {
    let mut result = vec![
        &value.source,
        &value.result,
        &value.map,
        &value.source_space,
        &value.target_space,
    ];
    result.extend(value.inverse.iter());
    result
}

/// Encode the exact coordinate convention before the optional inverse dependency.
pub(crate) fn statement(
    value: &PullbackValue,
    out: &mut Encoder<'_>,
    mut write: impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    for object in [
        &value.source,
        &value.result,
        &value.map,
        &value.source_space,
        &value.target_space,
    ] {
        write(object, out)?;
    }
    out.byte(match value.mode {
        PullbackMode::Affine => 0,
        PullbackMode::SimpleZeros => 1,
    })?;
    out.byte(u8::from(value.inverse.is_some()))?;
    if let Some(inverse) = &value.inverse {
        write(inverse, out)?;
    }
    Ok(())
}

/// Every space and map is a backward typed reference; role validation runs independently.
pub(crate) fn decode_statement(
    reader: &mut Reader<'_, '_>,
    records: &[Decoded],
) -> Result<PullbackValue, Fault> {
    let source = object(reader, records)?;
    let result = object(reader, records)?;
    let map = object(reader, records)?;
    let source_space = object(reader, records)?;
    let target_space = object(reader, records)?;
    let mode = match reader.byte()? {
        0 => PullbackMode::Affine,
        1 => PullbackMode::SimpleZeros,
        _ => return Err(invalid()),
    };
    let inverse = if reader.boolean()? {
        Some(object(reader, records)?)
    } else {
        None
    };
    Ok(PullbackValue {
        source,
        result,
        map,
        inverse,
        source_space,
        target_space,
        mode,
    })
}

/// All finite roots, signed slopes and positive inverse absolute Jacobians are authoritative payload.
pub(crate) fn certificate(value: &PullbackCertificate, out: &mut Encoder<'_>) -> Result<(), Fault> {
    out.number(&value.leading)?;
    out.count(value.zeros.len())?;
    for zero in &value.zeros {
        out.number(&zero.point)?;
        out.number(&zero.slope)?;
        out.number(&zero.inverse_jacobian)?;
    }
    Ok(())
}

/// Apply the specialized zero ceiling before allocating any root evidence.
pub(crate) fn decode_certificate(
    reader: &mut Reader<'_, '_>,
) -> Result<PullbackCertificate, Fault> {
    let leading = reader.rational()?;
    let count = reader.count(BudgetKind::PullbackZeros, 27, 512)?;
    let mut zeros = Vec::new();
    zeros
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    for _ in 0..count {
        zeros.push(PullbackZero {
            point: reader.rational()?,
            slope: reader.rational()?,
            inverse_jacobian: reader.rational()?,
        });
    }
    Ok(PullbackCertificate { leading, zeros })
}

/// Meter finite root intent before any route copies or encodes the request payload.
pub(crate) fn validate_intent(
    value: &PullbackKind,
    budget: &mut crate::budget::Budget,
) -> Result<(), Fault> {
    let count = match value {
        PullbackKind::Affine => 1,
        PullbackKind::SimpleZeros(roots) => crate::graph::size(roots.len(), AT)?,
    };
    budget.specialized(BudgetKind::PullbackZeros, count, false, AT)?;
    if let PullbackKind::SimpleZeros(roots) = value {
        budget.check(
            BudgetKind::StagedPayloadBytes,
            count.saturating_mul(512),
            AT,
        )?;
        for root in roots {
            root.check(budget, AT)?;
        }
    }
    Ok(())
}
