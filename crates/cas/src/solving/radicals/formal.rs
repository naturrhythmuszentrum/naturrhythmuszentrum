// SPDX-License-Identifier: AGPL-3.0-only
//! Visible symmetric expansion, exact Vieta substitution, and formal-root scope closure.

use super::audit::Audit;
use super::construction::Construction;
use super::formal_render::Rendering;
use super::formal_scope::Formal;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::proof_model::{ApplicationDraft, Certificate, Statement};
use crate::rule::Primitive;
use crate::solving::coefficients::model::{CoefficientFailure, EvaluationScope};
use crate::solving::coefficients::storage::{invalid, reserve, vector};
use crate::solving::symmetric::{algebra::Polynomial, reduction::Reduction};
use crate::solving::towers::dense::Dense;
use crate::solving::{SolveStatement, StatementCoefficientEquality};

/// The actual polynomial interpretation and an observer share the caller's ledger.
pub(crate) struct Input<'a, 's> {
    /// Original translated polynomial, without a specialized parameter assignment.
    pub(crate) source: &'a Dense,
    /// Explicit original and base maps used by the proof.
    pub(crate) maps: [&'a Handle; 2],
    /// Exact regional evaluation context and its available checked facts.
    pub(crate) scope: &'a EvaluationScope<'s>,
    /// Non-authoritative observations survive a failed generation transaction.
    pub(crate) audit: &'a mut Audit,
}

/// Independently expand the unreduced nested expression before applying any root equations.
fn expansion(
    cas: &mut Cas,
    construction: &mut Construction,
    formal: &Formal,
    reduction: &Reduction,
    zeros: &[Handle],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let left = formal
        .rendering
        .polynomial(cas, &reduction.source, &formal.roots, budget)?;
    let mut basis = vector(reduction.basis.len(), budget)?;
    for element in &reduction.basis {
        basis.push(
            formal
                .rendering
                .polynomial(cas, element, &formal.roots, budget)?,
        );
    }
    let right = formal
        .rendering
        .polynomial(cas, &reduction.expression, &basis, budget)?;
    formal.identity(cas, construction, [&left, &right], zeros, false, budget)
}

/// A genuine ideal combination records the simultaneous Vieta replacement, without divisions.
fn substitution(
    cas: &mut Cas,
    construction: &mut Construction,
    formal: &Formal,
    reduction: &Reduction,
    images: &[Polynomial],
    audit: &mut Audit,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    audit.observe("formal-compose-start", &[&reduction.expression], budget)?;
    let composed =
        super::formal_compose::compose(&reduction.expression, images, &cas.state, audit, budget)?;
    audit.observe("formal-compose-end", &[&composed], budget)?;
    let result = composed.stage(cas, budget)?;
    let left = reduction.source.stage(cas, budget)?;
    let mut multipliers = vector(images.len(), budget)?;
    for index in 0..images.len() {
        audit.observe(
            "formal-multiplier-start",
            &[
                &reduction.expression,
                reduction.basis.get(index).ok_or_else(invalid)?,
                images.get(index).ok_or_else(invalid)?,
            ],
            budget,
        )?;
        let multiplier = super::formal_combination::multiplier(
            &reduction.expression,
            &reduction.basis,
            images,
            index,
            &cas.state,
            budget,
        )?;
        audit.observe("formal-multiplier-end", &[&multiplier], budget)?;
        multipliers.push(multiplier.stage(cas, budget)?);
    }
    formal.identity(
        cas,
        construction,
        [&left, &result],
        &multipliers,
        false,
        budget,
    )?;
    Ok(result)
}

/// The final root-independent equality retains all preceding scoped steps as premises.
fn derivation(
    cas: &mut Cas,
    construction: &mut Construction,
    formal: &Formal,
    reduction: &Reduction,
    images: &[Polynomial],
    audit: &mut Audit,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let zero = reduction
        .source
        .identity(false, &cas.state, budget)?
        .stage(cas, budget)?;
    let mut zeros = vector(images.len(), budget)?;
    zeros.resize(images.len(), zero);
    audit.observe(
        "formal-expansion-start",
        &[&reduction.source, &reduction.expression],
        budget,
    )?;
    expansion(cas, construction, formal, reduction, &zeros, budget)?;
    audit.observe(
        "formal-expansion-end",
        &[&reduction.source, &reduction.expression],
        budget,
    )?;
    let right = substitution(cas, construction, formal, reduction, images, audit, budget)?;
    let mut constants = vector(images.len(), budget)?;
    for image in images {
        constants.push(
            formal
                .rendering
                .polynomial(cas, image, &formal.roots, budget)?,
        );
    }
    let left = formal
        .rendering
        .polynomial(cas, &reduction.expression, &constants, budget)?;
    audit.observe("formal-discharge-start", &[&reduction.expression], budget)?;
    formal.identity(cas, construction, [&left, &right], &zeros, true, budget)?;
    audit.observe("formal-discharge-end", &[&reduction.expression], budget)?;
    Ok(right)
}

/// The independently constructed target differs only by the erased, unused root frame.
fn close(
    cas: &mut Cas,
    construction: &mut Construction,
    reduction: &Reduction,
    images: &[Polynomial],
    right: &Handle,
    map: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let frame = construction
        .steps
        .last()
        .ok_or_else(invalid)?
        .context
        .clone();
    let premise = u32::try_from(
        construction
            .steps
            .len()
            .checked_sub(1)
            .ok_or_else(invalid)?,
    )
    .map_err(|_| invalid())?;
    let rendering = Rendering {
        header: construction.header.clone(),
        map: map.clone(),
        scope: Vec::new(),
    };
    let mut constants = vector(images.len(), budget)?;
    for image in images {
        let mut value = image.copy(&cas.state, budget)?;
        value.variables.clear();
        constants.push(rendering.polynomial(cas, &value, &[], budget)?);
    }
    let left = rendering.polynomial(cas, &reduction.expression, &constants, budget)?;
    reserve(&mut construction.steps, 1, budget)?;
    construction.steps.push(ApplicationDraft {
        rule: Primitive::ContextClose,
        context: construction.context.clone(),
        conclusion: Statement::Solve(Box::new(SolveStatement::CoefficientEquality(
            StatementCoefficientEquality {
                left,
                right: right.clone(),
                map: rendering.map,
                region: construction.region.clone(),
            },
        ))),
        premises: vec![premise],
        substitutions: Vec::new(),
        certificate: Certificate::Close { frame },
    });
    Ok(())
}

/// Close the exact original source derivation into its actual unscoped parent context.
pub(crate) fn certify(
    cas: &mut Cas,
    construction: &mut Construction,
    reduction: &Reduction,
    input: &mut Input<'_, '_>,
    budget: &mut Budget,
) -> Result<(), CoefficientFailure> {
    let source = input.source;
    input.audit.observe(
        "formal-base-start",
        &[&reduction.source, &reduction.expression],
        budget,
    )?;
    let reduction = super::formal_base::reduction(
        cas,
        construction,
        reduction,
        &source.domain,
        input.maps[0],
        budget,
    )?;
    input.audit.observe(
        "formal-base-end",
        &[&reduction.source, &reduction.expression],
        budget,
    )?;
    let images = super::formal_base::images(source, &reduction.source, input.scope, cas, budget)?;
    for image in &images {
        input.audit.observe("formal-images-end", &[image], budget)?;
    }
    let mut formal = Formal::new(
        cas,
        construction,
        &reduction,
        &images,
        input.maps[1],
        budget,
    )?;
    input
        .audit
        .observe("formal-frame-end", &[&reduction.source], budget)?;
    let engine = crate::solving::coefficients::arithmetic::Engine {
        state: &cas.state,
        domain: &source.domain,
    };
    let reciprocal = super::roots::inverse(
        &engine,
        source.coefficients.last().ok_or_else(invalid)?,
        input.scope,
        budget,
    )?;
    let (parent, region) = (construction.context.clone(), construction.region.clone());
    let result = formal
        .introduce(cas, construction, source, &reciprocal, budget)
        .and_then(|()| {
            input
                .audit
                .observe("formal-introduction-end", &[&reduction.source], budget)?;
            Ok(derivation(
                cas,
                construction,
                &formal,
                &reduction,
                &images,
                input.audit,
                budget,
            )?)
        });
    construction.context = parent;
    construction.region = region;
    let right = result?;
    input
        .audit
        .observe("formal-close-start", &[&reduction.expression], budget)?;
    close(
        cas,
        construction,
        &reduction,
        &images,
        &right,
        input.maps[1],
        budget,
    )?;
    input
        .audit
        .observe("formal-close-end", &[&reduction.expression], budget)?;
    Ok(())
}
