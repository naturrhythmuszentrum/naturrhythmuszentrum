// SPDX-License-Identifier: AGPL-3.0-only
//! Inspect the exact replayed relation before admitting any numerical instruction.

use nrz_cas::Handle;
use nrz_cas::domain::Layer;
use nrz_cas::proof::{DomainContract, Statement, VerifiedWitness};
use nrz_cas::request::Outcome;
use nrz_cas::syntax::Operator;

use super::{SensitivityError, graph::Graph};
use crate::CheckedExpression;

/// Conditional and best-so-far results remain refusals even if they carry a witness.
pub(crate) fn completed(outcome: Outcome) -> Result<VerifiedWitness, SensitivityError> {
    match outcome {
        Outcome::Proved { result, .. }
            if result.witness.is_discharged() && result.payload.is_none() =>
        {
            Ok(result.witness)
        }
        other => Err(SensitivityError::Evidence(Box::new(other))),
    }
}

/// Verify common context, layer and completeness without equating assumptions with truth.
fn scope(witness: &VerifiedWitness, context: &Handle) -> Result<(), SensitivityError> {
    let judgment = witness.judgment();
    if judgment.context != *context
        || judgment.layer != Layer::Pointwise
        || judgment.space.is_some()
        || !witness.is_discharged()
    {
        return Err(SensitivityError::WitnessMismatch);
    }
    Ok(())
}

/// Equality transport must bind both exact endpoints and the same complete domain.
pub(crate) fn equality(
    witness: &VerifiedWitness,
    left: &Handle,
    right: &Handle,
    context: &Handle,
    domain: &Handle,
) -> Result<(), SensitivityError> {
    scope(witness, context)?;
    match &witness.judgment().statement {
        Statement::Equality {
            left: source,
            right: target,
            domain: region,
            domain_contract: DomainContract::DefinedAndEqual,
        } if source == left && target == right && region == domain => Ok(()),
        _ => Err(SensitivityError::WitnessMismatch),
    }
}

/// Check the actual assembled source column, then explicitly transpose cell enumeration.
pub(crate) fn jacobian_cells(
    graph: &mut Graph<'_>,
    witness: &VerifiedWitness,
    originals: &[CheckedExpression],
    coordinates: &[String],
) -> Result<Vec<Handle>, SensitivityError> {
    scope(witness, &graph.context)?;
    let Statement::Jacobian {
        input,
        output,
        variables,
        domain,
    } = &witness.judgment().statement
    else {
        return Err(SensitivityError::WitnessMismatch);
    };
    if variables != coordinates || domain != &graph.domain {
        return Err(SensitivityError::WitnessMismatch);
    }
    let rows = u32::try_from(originals.len()).map_err(|_| SensitivityError::ResourceLimit)?;
    let columns = u32::try_from(coordinates.len()).map_err(|_| SensitivityError::ResourceLimit)?;
    if graph.cas.expression(input)?.0 != &(Operator::Matrix { rows, columns: 1 })
        || graph.cas.expression(output)?.0 != &(Operator::Matrix { rows, columns })
    {
        return Err(SensitivityError::WitnessMismatch);
    }
    let sources = graph.cas.operands(input, &mut graph.budget)?;
    if sources.len() != originals.len()
        || sources
            .iter()
            .zip(originals)
            .any(|(source, original)| source != &original.coordinates)
    {
        return Err(SensitivityError::WitnessMismatch);
    }
    let cells = graph.cas.operands(output, &mut graph.budget)?;
    column_order(&cells, originals.len(), coordinates.len())
}

/// Permute row-ordered CAS cells into the heap's logical `row + rows*column` order.
fn column_order(
    cells: &[Handle],
    rows: usize,
    columns: usize,
) -> Result<Vec<Handle>, SensitivityError> {
    if rows.checked_mul(columns) != Some(cells.len()) {
        return Err(SensitivityError::WitnessMismatch);
    }
    let mut ordered = Vec::new();
    for column in 0..columns {
        for row in 0..rows {
            let offset = row
                .checked_mul(columns)
                .and_then(|value| value.checked_add(column))
                .ok_or(SensitivityError::ResourceLimit)?;
            ordered.push(
                cells
                    .get(offset)
                    .ok_or(SensitivityError::WitnessMismatch)?
                    .clone(),
            );
        }
    }
    Ok(ordered)
}

#[cfg(test)]
mod tests {
    use nrz_cas::{limits::Limits, syntax::Environment};

    use super::{SensitivityError, equality};
    use crate::{Context, ExponentPolicy, Registry, sensitivity::graph::Graph};

    /// A real replayed equality cannot be attached to another recipe coefficient.
    /// ´test:unit:sensitivity-witness-endpoint-association´
    #[test]
    fn sensitivity_witness_endpoint_association() -> Result<(), Box<dyn std::error::Error>> {
        let builder = Context::new(
            Registry::new(ExponentPolicy::Rational),
            Limits::default(),
            Environment::default(),
        )?;
        let mut graph = Graph::new(&builder, &nrz_cas::limits::LimitsOverride::default())?;
        let one = graph.constant(nrz_cas::Rational::one())?;
        let zero = graph.constant(nrz_cas::Rational::zero())?;
        let witness = graph.equal(&one, &one)?;
        assert!(matches!(
            equality(&witness, &one, &zero, &graph.context, &graph.domain),
            Err(SensitivityError::WitnessMismatch)
        ));
        Ok(())
    }

    /// An ambient reciprocal proof's residual guards cannot enter a checked schedule.
    /// ´test:unit:sensitivity-conditional-evidence-refusal´
    #[test]
    fn sensitivity_conditional_evidence_refusal() -> Result<(), Box<dyn std::error::Error>> {
        use nrz_cas::syntax::{AtomicSort, Declaration, Field, Operator, Sort};

        let builder = Context::new(
            Registry::new(ExponentPolicy::Rational),
            Limits::default(),
            Environment {
                declarations: vec![Declaration {
                    key: "x".into(),
                    display_name: "x".into(),
                    sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
                }],
                ..Environment::default()
            },
        )?;
        let mut graph = Graph::new(&builder, &nrz_cas::limits::LimitsOverride::default())?;
        let input = graph.term(Operator::Free("x".into()), &[])?;
        let inverse = graph.term(Operator::IntegerPower(-1), &[input])?;
        let error = graph
            .equal(&inverse, &inverse)
            .err()
            .ok_or("conditional equality admitted")?;
        let SensitivityError::Evidence(outcome) = error else {
            return Err("conditional outcome was not retained".into());
        };
        let nrz_cas::request::Outcome::Conditional { result, .. } = *outcome else {
            return Err("expected retained conditional result".into());
        };
        assert!(!result.witness.is_discharged());
        assert_ne!(result.witness.judgment().residuals.len(), 0);
        Ok(())
    }
}
