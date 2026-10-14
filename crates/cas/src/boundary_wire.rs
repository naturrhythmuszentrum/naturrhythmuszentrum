// SPDX-License-Identifier: AGPL-3.0-only
//! Structural boundary payloads shared by graph identity and portable records.

use crate::boundary_model::BoundaryCertificate;
use crate::encoding::Encoder;
use crate::error::Fault;
use crate::graph::{Edge, Handle};
use crate::proof_model::Statement;

/// Encode references with the caller's local or portable representation.
pub(crate) fn statement(
    value: &Statement,
    out: &mut Encoder<'_>,
    mut write: impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    match value {
        Statement::BoundaryPartition(value) => {
            out.byte(17)?;
            write(&value.function, out)?;
            write(&value.domain, out)?;
            out.name(&value.coordinate)?;
            handles(&value.boundaries, out, &mut write)?;
            handles(&value.cells, out, &mut write)
        }
        Statement::CaseDerivative(value) => {
            out.byte(18)?;
            write(&value.function, out)?;
            write(&value.derivative, out)?;
            out.name(&value.coordinate)?;
            out.unsigned(u64::from(value.order))?;
            write(&value.domain, out)
        }
        _ => point_statement(value, out, write),
    }
}

/// Point facts retain the original function and explicit iterated order.
fn point_statement(
    value: &Statement,
    out: &mut Encoder<'_>,
    mut write: impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    match value {
        Statement::DerivativeAt(value) => {
            out.byte(19)?;
            write(&value.function, out)?;
            write(&value.boundary, out)?;
            out.name(&value.coordinate)?;
            out.unsigned(u64::from(value.order))?;
            write(&value.value, out)
        }
        Statement::DerivativeDoesNotExist(value) => {
            out.byte(20)?;
            write(&value.function, out)?;
            write(&value.boundary, out)?;
            out.name(&value.coordinate)?;
            out.unsigned(u64::from(value.order))?;
            out.byte(reason(value.reason))
        }
        Statement::TraceValue(value) => {
            out.byte(21)?;
            write(&value.function, out)?;
            write(&value.boundary, out)?;
            out.byte(match value.side {
                crate::domain_model::Side::Below => 0,
                crate::domain_model::Side::Above => 1,
            })?;
            out.unsigned(u64::from(value.order))?;
            write(&value.value, out)
        }
        _ => Err(crate::case_syntax::invalid()),
    }
}

/// Encode the complete ordered family without cloning its handles.
fn handles(
    values: &[Handle],
    out: &mut Encoder<'_>,
    write: &mut impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    out.count(values.len())?;
    for value in values {
        write(value, out)?;
    }
    Ok(())
}

/// Stable finite nonexistence reasons.
const fn reason(value: crate::boundary_model::NonexistenceReason) -> u8 {
    use crate::boundary_model::NonexistenceReason;
    use crate::domain_model::Side;
    match value {
        NonexistenceReason::Discontinuous(Side::Below) => 0,
        NonexistenceReason::Discontinuous(Side::Above) => 1,
        NonexistenceReason::UnequalSlopes => 2,
    }
}

/// Encode only the data belonging to the selected closed mode.
pub(crate) fn certificate(
    value: &BoundaryCertificate,
    out: &mut Encoder<'_>,
    mut write: impl FnMut(&Handle, &mut Encoder<'_>) -> Result<(), Fault>,
) -> Result<(), Fault> {
    match value {
        BoundaryCertificate::Partition { regions, partition } => {
            out.byte(0)?;
            handles(regions, out, &mut write)?;
            crate::statement::case_cells(partition, out)
        }
        BoundaryCertificate::Polynomial {
            polynomial,
            coefficients,
        } => {
            out.byte(1)?;
            write(polynomial, out)?;
            out.count(coefficients.len())?;
            for value in coefficients {
                out.number(value)?;
            }
            Ok(())
        }
        BoundaryCertificate::Projection => out.byte(2),
        BoundaryCertificate::Jump => out.byte(3),
        BoundaryCertificate::Interior => out.byte(4),
        BoundaryCertificate::Point => out.byte(5),
        BoundaryCertificate::Assemble => out.byte(6),
    }
}

/// Every typed object participates in scope validation and the dependency DAG.
pub(crate) fn objects(value: &Statement) -> Vec<&Handle> {
    match value {
        Statement::BoundaryPartition(value) => {
            let mut result = vec![&value.function, &value.domain];
            result.extend(&value.boundaries);
            result.extend(&value.cells);
            result
        }
        Statement::CaseDerivative(value) => {
            vec![&value.function, &value.derivative, &value.domain]
        }
        Statement::DerivativeAt(value) => {
            vec![&value.function, &value.boundary, &value.value]
        }
        Statement::DerivativeDoesNotExist(value) => {
            vec![&value.function, &value.boundary]
        }
        Statement::TraceValue(value) => {
            vec![&value.function, &value.boundary, &value.value]
        }
        _ => Vec::new(),
    }
}

/// Preserve function, point and domain roles in graph snapshots.
pub(crate) const fn edge(value: &Statement, position: u32) -> Edge {
    match (value, position) {
        (_, 0) => Edge::OfFunction,
        (Statement::BoundaryPartition(_), 1) | (Statement::CaseDerivative(_), 2) => {
            Edge::ArgumentRegion(0)
        }
        (
            Statement::DerivativeAt(_)
            | Statement::DerivativeDoesNotExist(_)
            | Statement::TraceValue(_),
            1,
        ) => Edge::AtBoundary,
        _ => Edge::ConclusionObject(position),
    }
}
