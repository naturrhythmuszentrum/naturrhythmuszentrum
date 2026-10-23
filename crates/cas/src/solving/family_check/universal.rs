// SPDX-License-Identifier: AGPL-3.0-only
//! Polynomial implications over complete parameter products, never finite samples.

use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::proof_model::{ApplicationDraft, CheckedApplication};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::forms::Variable;
use crate::solving::coefficients::storage::vector;
use crate::solving::elementary::constraints::Constraints;
use crate::solving::elementary::evaluate::Evaluation;
use crate::solving::elementary::proof::{coordinates, family, invalid, problem};
use crate::solving::elementary::transport::{apply, unknowns};
use crate::solving::elementary::value::Value;
use crate::solving::{
    CertificateParameterImageCoverage, CoordinateRole, FamilyMapData, SolveProblemData,
};

/// Coordinate roles and the exact original target establish the parameterization's meaning.
fn shape(
    cas: &Cas,
    map: &FamilyMapData<Handle>,
    original: &SolveProblemData<Handle>,
) -> Result<(), Fault> {
    if (&map.header, &map.coefficients, &map.target, &map.scope)
        != (
            &original.header,
            &original.coefficients,
            &original.unknowns,
            &original.scope,
        )
    {
        return Err(invalid());
    }
    if coordinates(cas, &map.family)?
        .coordinates
        .iter()
        .any(|entry| entry.role != CoordinateRole::Family)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Fresh formal slots are disjoint from all coefficient and unknown declarations.
fn parameters(
    evaluation: &Evaluation<'_>,
    map: &FamilyMapData<Handle>,
    budget: &mut Budget,
) -> Result<Vec<Value>, Fault> {
    let first = evaluation
        .cas
        .state
        .header(map.header.logical_id())?
        .environment
        .declarations
        .len();
    let source = coordinates(evaluation.cas, &map.family)?;
    let mut values = vector(source.coordinates.len(), budget)?;
    for index in 0..source.coordinates.len() {
        let slot =
            u32::try_from(first.checked_add(index).ok_or_else(invalid)?).map_err(|_| invalid())?;
        values.push(Value::variable(
            Variable::Slot(slot),
            &evaluation.engine,
            budget,
        )?);
    }
    Ok(values)
}

/// Every source parameter in the full declared region gives an original-domain solution.
fn membership(
    evaluation: &Evaluation<'_>,
    map: &FamilyMapData<Handle>,
    original: &SolveProblemData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let input = parameters(evaluation, map, budget)?;
    let lifted = Evaluation {
        bound: &input,
        ..evaluation.unbound()
    };
    let mut source = Constraints::new();
    source.domain(&lifted, map.region.logical_id(), budget)?;
    let output = apply(evaluation, map, &input, budget)?;
    let target = coordinates(evaluation.cas, &original.unknowns)?;
    let mut replacements = vector(output.len(), budget)?;
    for (coordinate, value) in target.coordinates.iter().zip(&output) {
        replacements.push((
            coordinate.key.as_str(),
            value.copy(&evaluation.engine, budget)?,
        ));
    }
    let substituted = Evaluation {
        free: &replacements,
        ..evaluation.unbound()
    };
    let mut requirements = Constraints::problem(&substituted, original, budget)?;
    for value in &output {
        requirements.defined(value, evaluation, budget)?;
    }
    source.implies(evaluation, &requirements, budget)
}

/// Recovery must be defined on every original solution and land inside the entire source region.
fn recover(
    evaluation: &Evaluation<'_>,
    map: &FamilyMapData<Handle>,
    recovery: &FamilyMapData<Handle>,
    original: &SolveProblemData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if (
        &recovery.header,
        &recovery.coefficients,
        &recovery.scope,
        &recovery.family,
        &recovery.target,
    ) != (
        &map.header,
        &map.coefficients,
        &map.scope,
        &map.target,
        &map.family,
    ) {
        return Err(invalid());
    }
    let input = unknowns(evaluation, original, budget)?;
    let source = Constraints::problem(evaluation, original, budget)?;
    let mut requirements = Constraints::new();
    let lifted = Evaluation {
        bound: &input,
        ..evaluation.unbound()
    };
    requirements.domain(&lifted, recovery.region.logical_id(), budget)?;
    let parameters = apply(evaluation, recovery, &input, budget)?;
    let recovered = Evaluation {
        bound: &parameters,
        ..evaluation.unbound()
    };
    requirements.domain(&recovered, map.region.logical_id(), budget)?;
    for value in &parameters {
        requirements.defined(value, evaluation, budget)?;
    }
    let output = apply(evaluation, map, &parameters, budget)?;
    if output.len() != input.len() {
        return Err(invalid());
    }
    for (value, expected) in output.iter().zip(&input) {
        requirements.defined(value, evaluation, budget)?;
        source.zero(
            evaluation,
            &value.sub(expected, &evaluation.engine, budget)?,
            budget,
        )?;
    }
    source.implies(evaluation, &requirements, budget)
}

/// Only symbolic implications over arbitrary parameters and arbitrary solutions justify coverage.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateParameterImageCoverage,
    premises: &[CheckedApplication],
    coverage: bool,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let original = problem(cas, &certificate.problem)?;
    crate::solving::elementary::proof::region(cas, &draft.context, original, &certificate.region)?;
    crate::solving::elementary::member::contained(
        cas,
        &certificate.region,
        &original.region,
        budget,
    )?;
    let map = family(cas, &certificate.family)?;
    shape(cas, map, original)?;
    let evaluation = Evaluation {
        exclusions: &[],
        cas,
        engine: Engine {
            state: &cas.state,
            domain: &original.coefficient_domain,
        },
        region: &certificate.region,
        premises,
        free: &[],
        bound: &[],
    };
    membership(&evaluation, map, original, budget)?;
    if coverage {
        recover(
            &evaluation,
            map,
            family(cas, certificate.recovery.as_ref().ok_or_else(invalid)?)?,
            original,
            budget,
        )?;
    }
    Ok(())
}
