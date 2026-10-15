// SPDX-License-Identifier: AGPL-3.0-only
//! Reversible coordinate changes preserve both equations and strict domains.

use super::constraints::Constraints;
use super::evaluate::Evaluation;
use super::proof::{AT, coordinates, family, invalid, problem, tuple};
use super::value::Value;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::proof_model::{ApplicationDraft, CheckedApplication, Statement};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::forms::Variable;
use crate::solving::coefficients::storage::vector;
use crate::solving::{
    CertificateSolutionTransport, FamilyMapData, SolveProblemData, SolveStatement,
};

/// Map sources explicitly bind a fresh family product with the original ordered scalar fields.
fn shape(
    cas: &Cas,
    map: &FamilyMapData<Handle>,
    source: &SolveProblemData<Handle>,
    target: &SolveProblemData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if (&map.header, &map.coefficients, &map.target, &map.scope)
        != (
            &source.header,
            &source.coefficients,
            &target.unknowns,
            &source.scope,
        )
    {
        return Err(invalid());
    }
    let input = coordinates(cas, &map.family)?;
    let original = coordinates(cas, &source.unknowns)?;
    if input.coordinates.len() != original.coordinates.len() {
        return Err(invalid());
    }
    for (coordinate, expected) in input.coordinates.iter().zip(&original.coordinates) {
        budget.work(1, AT)?;
        if coordinate.field != expected.field {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Interpret the arbitrary source tuple as independent formal unknown coordinates.
pub(crate) fn unknowns(
    evaluation: &Evaluation<'_>,
    source: &SolveProblemData<Handle>,
    budget: &mut Budget,
) -> Result<Vec<Value>, Fault> {
    let coordinates = coordinates(evaluation.cas, &source.unknowns)?;
    let environment = &evaluation
        .cas
        .state
        .header(source.header.logical_id())?
        .environment;
    let mut result = vector(coordinates.coordinates.len(), budget)?;
    for coordinate in &coordinates.coordinates {
        let mut found = None;
        for (index, declaration) in environment.declarations.iter().enumerate() {
            budget.work(1, AT)?;
            if declaration.key == coordinate.key {
                found = Some(u32::try_from(index).map_err(|_| invalid())?);
                break;
            }
        }
        result.push(Value::variable(
            Variable::Slot(found.ok_or_else(invalid)?),
            &evaluation.engine,
            budget,
        )?);
    }
    Ok(result)
}

/// Substitute the complete source tuple simultaneously into one bound map body.
pub(crate) fn apply(
    evaluation: &Evaluation<'_>,
    map: &FamilyMapData<Handle>,
    input: &[Value],
    budget: &mut Budget,
) -> Result<Vec<Value>, Fault> {
    let body = tuple(evaluation.cas, &map.body)?;
    let mut result = vector(body.entries.len(), budget)?;
    let lifted = Evaluation {
        bound: input,
        ..evaluation.unbound()
    };
    for entry in &body.entries {
        result.push(lifted.expression(entry.logical_id(), false, budget)?);
    }
    Ok(result)
}

/// A direction checks solution preservation, source admission and the full inverse composition.
fn direction(
    evaluation: &Evaluation<'_>,
    source: &SolveProblemData<Handle>,
    target: &SolveProblemData<Handle>,
    forward: &FamilyMapData<Handle>,
    backward: &FamilyMapData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    shape(evaluation.cas, forward, source, target, budget)?;
    let original = Constraints::problem(evaluation, source, budget)?;
    let input = unknowns(evaluation, source, budget)?;
    let output = apply(evaluation, forward, &input, budget)?;
    let target_coordinates = coordinates(evaluation.cas, &target.unknowns)?;
    let mut replacements = vector(output.len(), budget)?;
    for (coordinate, value) in target_coordinates.coordinates.iter().zip(&output) {
        replacements.push((
            coordinate.key.as_str(),
            value.copy(&evaluation.engine, budget)?,
        ));
    }
    let substituted = Evaluation {
        free: &replacements,
        ..evaluation.unbound()
    };
    let mut requirements = Constraints::problem(&substituted, target, budget)?;
    let lifted = Evaluation {
        bound: &input,
        ..evaluation.unbound()
    };
    requirements.domain(&lifted, forward.region.logical_id(), budget)?;
    for value in &output {
        requirements.defined(value, evaluation, budget)?;
    }
    original.implies(evaluation, &requirements, budget)?;
    let recovered = apply(evaluation, backward, &output, budget)?;
    if recovered.len() != input.len() {
        return Err(invalid());
    }
    for (value, expected) in recovered.iter().zip(&input) {
        let mut domain = Constraints::new();
        domain.defined(value, evaluation, budget)?;
        original.implies(evaluation, &domain, budget)?;
        original.zero(
            evaluation,
            &value.sub(expected, &evaluation.engine, budget)?,
            budget,
        )?;
    }
    Ok(())
}

/// Independently recompute both directions; supplied identities never replace inverse checking.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateSolutionTransport,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let source = problem(cas, &certificate.source)?;
    let target = problem(cas, &certificate.target)?;
    if (
        &source.header,
        &source.coefficients,
        &source.coefficient_domain,
        &source.scope,
        &source.ambient,
    ) != (
        &target.header,
        &target.coefficients,
        &target.coefficient_domain,
        &target.scope,
        &target.ambient,
    ) {
        return Err(invalid());
    }
    super::proof::region(cas, &draft.context, source, &certificate.region)?;
    super::member::contained(cas, &certificate.region, &source.region, budget)?;
    super::member::contained(cas, &certificate.region, &target.region, budget)?;
    for slot in certificate
        .definedness
        .iter()
        .chain(&certificate.equations)
        .chain(&certificate.forward_inverse)
        .chain(&certificate.backward_inverse)
    {
        super::proof::premise(premises, *slot)?;
    }
    let forward = family(cas, &certificate.forward)?;
    let backward = family(cas, &certificate.backward)?;
    let evaluation = Evaluation {
        cas,
        engine: Engine {
            state: &cas.state,
            domain: &source.coefficient_domain,
        },
        region: &certificate.region,
        premises,
        free: &[],
        bound: &[],
    };
    direction(&evaluation, source, target, forward, backward, budget)?;
    direction(&evaluation, target, source, backward, forward, budget)?;
    conclusion(&evaluation, draft, certificate, budget)
}

/// Transported facts use the appropriate edge and retain exactly the certificate's region.
fn conclusion(
    evaluation: &Evaluation<'_>,
    draft: &ApplicationDraft,
    certificate: &CertificateSolutionTransport,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    match statement.as_ref() {
        SolveStatement::Equivalent(value) => {
            if (
                &value.source,
                &value.target,
                &value.forward,
                &value.backward,
                &value.region,
            ) == (
                &certificate.source,
                &certificate.target,
                &certificate.forward,
                &certificate.backward,
                &certificate.region,
            ) {
                Ok(())
            } else {
                Err(invalid())
            }
        }
        SolveStatement::Coverage(value) => covered(evaluation, certificate, value, budget),
        SolveStatement::Member(value) => member(evaluation, certificate, value, budget),
        _ => Err(invalid()),
    }
}

/// Select the actual reverse edge when transporting back from a transformed problem.
fn edge<'a>(
    certificate: &'a CertificateSolutionTransport,
    destination: &Handle,
) -> Result<(&'a Handle, &'a Handle), Fault> {
    if destination == &certificate.source {
        Ok((&certificate.target, &certificate.backward))
    } else if destination == &certificate.target {
        Ok((&certificate.source, &certificate.forward))
    } else {
        Err(invalid())
    }
}

/// A transported member still passes the original predicate/domain check after exact map evaluation.
fn member(
    evaluation: &Evaluation<'_>,
    certificate: &CertificateSolutionTransport,
    target: &crate::solving::StatementMember,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if target.region != certificate.region {
        return Err(invalid());
    }
    let (origin, map) = edge(certificate, &target.problem)?;
    let mut source = None;
    for premise in evaluation.premises {
        budget.work(1, AT)?;
        if let Statement::Solve(statement) = &premise.judgment.statement
            && let SolveStatement::Member(value) = statement.as_ref()
            && (&value.problem, &value.region) == (origin, &certificate.region)
        {
            source = Some(value);
            break;
        }
    }
    let supplied = tuple(evaluation.cas, &source.ok_or_else(invalid)?.tuple)?;
    let mut input = vector(supplied.entries.len(), budget)?;
    for entry in &supplied.entries {
        input.push(evaluation.expression(entry.logical_id(), true, budget)?);
    }
    let output = apply(evaluation, family(evaluation.cas, map)?, &input, budget)?;
    let claimed = tuple(evaluation.cas, &target.tuple)?;
    if output.len() != claimed.entries.len() {
        return Err(invalid());
    }
    for (value, handle) in output.iter().zip(&claimed.entries) {
        evaluation.zero(
            &value.sub(
                &evaluation.expression(handle.logical_id(), true, budget)?,
                &evaluation.engine,
                budget,
            )?,
            budget,
        )?;
    }
    let original = problem(evaluation.cas, &target.problem)?;
    super::product::membership(evaluation, original, &claimed.entries, budget)
}

/// A finite covered image is checked in both directions, including the empty set.
fn covered(
    evaluation: &Evaluation<'_>,
    certificate: &CertificateSolutionTransport,
    target: &crate::solving::StatementCoverage,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if target.region != certificate.region {
        return Err(invalid());
    }
    let (origin, map) = edge(certificate, &target.problem)?;
    let mut source = None;
    for premise in evaluation.premises {
        budget.work(1, AT)?;
        if let Statement::Solve(statement) = &premise.judgment.statement
            && let SolveStatement::Coverage(value) = statement.as_ref()
            && (&value.problem, &value.region) == (origin, &certificate.region)
        {
            source = Some(value);
            break;
        }
    }
    let inputs = super::product::entries(evaluation.cas, source.ok_or_else(invalid)?, budget)?;
    let targets = super::product::entries(evaluation.cas, target, budget)?;
    let mut outputs = vector(inputs.len(), budget)?;
    for entry in &inputs {
        let input = [evaluation.expression(entry.logical_id(), true, budget)?];
        let mut image = apply(evaluation, family(evaluation.cas, map)?, &input, budget)?;
        if image.len() != 1 {
            return Err(invalid());
        }
        outputs.push(image.pop().ok_or_else(invalid)?);
    }
    let mut actual = vector(targets.len(), budget)?;
    for entry in &targets {
        actual.push(evaluation.expression(entry.logical_id(), true, budget)?);
    }
    same_values(evaluation, &outputs, &actual, budget)?;
    same_values(evaluation, &actual, &outputs, budget)?;
    super::product::membership(
        evaluation,
        problem(evaluation.cas, &target.problem)?,
        &targets,
        budget,
    )
}

/// Every value has a proved equal image, without assuming syntactic set order.
pub(crate) fn same_values(
    evaluation: &Evaluation<'_>,
    left: &[Value],
    right: &[Value],
    budget: &mut Budget,
) -> Result<(), Fault> {
    for value in left {
        let mut found = false;
        for candidate in right {
            if super::product::decides(
                evaluation.zero(&value.sub(candidate, &evaluation.engine, budget)?, budget),
            )? {
                found = true;
                break;
            }
        }
        if !found {
            return Err(invalid());
        }
    }
    Ok(())
}
