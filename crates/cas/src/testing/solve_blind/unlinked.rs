// SPDX-License-Identifier: AGPL-3.0-only
//! After search, independently rotating one character preserves its power but loses the full product.

use crate::graph::{Cas, ExpressionAttachment, Handle};
use crate::limits::{Budget, Limits};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::maps::canonical;
use crate::solving::coefficients::model::Element;
use crate::solving::radicals::{construction::Construction, roots::Values};
use crate::solving::towers::{arithmetic, dense::Dense};
use crate::testing::common::TestResult;

/// Read the actual sealed factors and their one exact coefficient interpretation.
fn factors(
    cas: &Cas,
    steps: &[crate::proof_model::ApplicationDraft],
    budget: &mut Budget,
) -> TestResult<Values> {
    let Some(crate::proof_model::Certificate::Solve(certificate)) =
        steps.last().map(|step| &step.certificate)
    else {
        return Err("missing product certificate".into());
    };
    let crate::solving::SolveCertificate::ProductCompleteness(certificate) = certificate.as_ref()
    else {
        return Err("missing product completeness".into());
    };
    let mut roots = Vec::new();
    let mut mapping = None;
    for factor in &certificate.factors {
        let Some(ExpressionAttachment::ExactCoefficient { value, map }) =
            &cas.state.expression(factor.root.logical_id())?.attachment
        else {
            return Err("missing mapped root".into());
        };
        let map = cas.handle(*map);
        if mapping.as_ref().is_some_and(|previous| previous != &map) {
            return Err("different root maps".into());
        }
        let stored = crate::solving::coefficients::storage::value(&cas.state, &cas.handle(*value))?;
        let engine = Engine {
            state: &cas.state,
            domain: &stored.domain,
        };
        for _ in 0..factor.multiplicity {
            roots.push(engine.read(&cas.handle(*value), budget)?);
        }
        mapping = Some(map);
    }
    let domain = roots
        .first()
        .ok_or("empty factor inventory")?
        .domain
        .clone();
    Ok(Values {
        domain,
        map: mapping.ok_or("missing map")?,
        roots,
    })
}

/// The first quotient in this returned radical tower supplies its computed primitive character.
fn unity(cas: &Cas, domain: &Handle, budget: &mut Budget) -> TestResult<Element> {
    let mut current = domain.clone();
    loop {
        let engine = Engine {
            state: &cas.state,
            domain: &current,
        };
        let (_, extension) = arithmetic::level(&engine)?;
        if matches!(
            crate::solving::coefficients::storage::domain(&cas.state, &extension.parent)?,
            crate::solving::CoefficientDomainData::Quotient { .. }
        ) {
            let parent = extension.parent.clone();
            current = parent;
        } else {
            let parent = Engine {
                state: &cas.state,
                domain: &extension.parent,
            };
            let variable = Dense::new(
                &parent,
                vec![
                    parent.constant(false, budget)?,
                    parent.constant(true, budget)?,
                ],
                budget,
            )?;
            let generator = arithmetic::reduced(&engine, variable, budget)?;
            return Ok(canonical(&cas.state, &generator, domain, budget)?);
        }
    }
}

/// Recover one character from the returned roots by the generic forward transform.
fn character(
    engine: &Engine<'_>,
    roots: &[Element],
    primitive: &Element,
    budget: &mut Budget,
) -> TestResult<Element> {
    let degree = u32::try_from(roots.len())?;
    let mut result = engine.constant(false, budget)?;
    for (index, root) in roots.iter().enumerate() {
        let exponent = u32::try_from(index)?
            .checked_mul(degree - 1)
            .ok_or("character exponent")?
            % degree;
        let weight = engine.pow(primitive, exponent, budget)?;
        result = engine.add(&result, &engine.mul(&weight, root, budget)?, budget)?;
    }
    Ok(result)
}

/// Rotate only that character; all other Fourier components retain their original choices.
fn rotate(
    cas: &Cas,
    values: &mut Values,
    primitive: &Element,
    difference: &Element,
    budget: &mut Budget,
) -> TestResult {
    let degree = u32::try_from(values.roots.len())?;
    let engine = Engine {
        state: &cas.state,
        domain: &values.domain,
    };
    let scalar = crate::Rational::from_ratio(1, u64::from(degree), budget)?;
    let scale = engine.rational(&scalar, budget)?;
    for (index, root) in values.roots.iter_mut().enumerate() {
        let exponent = (degree
            - u32::try_from(index)?
                .checked_mul(degree - 1)
                .ok_or("inverse exponent")?
                % degree)
            % degree;
        let weight = engine.mul(&scale, &engine.pow(primitive, exponent, budget)?, budget)?;
        *root = engine.add(root, &engine.mul(&weight, difference, budget)?, budget)?;
    }
    Ok(())
}

/// Equal individual character powers do not establish their required joint product.
pub(crate) fn refuses() -> TestResult {
    let coefficients = [-4, -15, 0, 1];
    let (mut fixture, request) = super::source::polynomial(&coefficients)?;
    let crate::request::Outcome::Solved { answer, .. } = fixture.cas.run(&request) else {
        return Err("unlinked mutation baseline".into());
    };
    let coverage = answer.coverage.ok_or("unlinked coverage")?;
    let steps = super::audit::applications(&fixture.cas, &coverage)?;
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let mut values = factors(&fixture.cas, &steps, &mut budget)?;
    let primitive = unity(&fixture.cas, &values.domain, &mut budget)?;
    let engine = Engine {
        state: &fixture.cas.state,
        domain: &values.domain,
    };
    let original = character(&engine, &values.roots, &primitive, &mut budget)?;
    let altered = engine.mul(&original, &primitive, &mut budget)?;
    let difference = engine.sub(&altered, &original, &mut budget)?;
    assert!(!engine.formal_zero(&difference, &mut budget)?);
    let mut construction = Construction {
        header: fixture.header.clone(),
        context: fixture.context.clone(),
        region: fixture.region.clone(),
        steps,
    };
    power_identity(
        &mut fixture.cas,
        &mut construction,
        &values,
        &original,
        &altered,
        &mut budget,
    )?;
    assert!(
        fixture
            .cas
            .check_proof(
                &fixture.context,
                &construction.steps,
                &mut Budget::new(Limits::blind_cubic())?
            )?
            .is_discharged()
    );
    rotate(
        &fixture.cas,
        &mut values,
        &primitive,
        &difference,
        &mut budget,
    )?;
    false_product(
        &mut fixture,
        &request,
        &coefficients,
        &values,
        &mut construction,
        &mut budget,
    )?;
    assert_eq!(
        fixture
            .cas
            .export_witness(&coverage, &mut Budget::new(Limits::blind_cubic())?)?,
        coverage.bytes
    );
    Ok(())
}

/// Replay the unchanged individual power before submitting the incompatible joint factorization.
fn power_identity(
    cas: &mut Cas,
    construction: &mut Construction,
    values: &Values,
    original: &Element,
    altered: &Element,
    budget: &mut Budget,
) -> TestResult {
    let degree = i64::try_from(values.roots.len())?;
    let first = construction.literal(cas, original, &values.map, budget)?;
    let second = construction.literal(cas, altered, &values.map, budget)?;
    let left = construction.expression(
        cas,
        crate::model::Operator::IntegerPower(degree),
        &[first],
        false,
        budget,
    )?;
    let right = construction.expression(
        cas,
        crate::model::Operator::IntegerPower(degree),
        &[second],
        false,
        budget,
    )?;
    crate::solving::radicals::proof::identity(construction, [&left, &right], &values.map, budget)?;
    Ok(())
}

/// The ordinary product producer proposes the changed factors; independent replay must reject them.
fn false_product(
    fixture: &mut super::source::Fixture,
    request: &crate::request_model::Request,
    coefficients: &[i64],
    values: &Values,
    construction: &mut Construction,
    budget: &mut Budget,
) -> TestResult {
    let crate::task::Task::Solve(specification) = &request.task else {
        return Err("missing solve request".into());
    };
    let engine = Engine {
        state: &fixture.cas.state,
        domain: &fixture.algebra,
    };
    let mut entries = Vec::new();
    for value in coefficients {
        entries.push(engine.rational(&crate::Rational::from_ratio(*value, 1, budget)?, budget)?);
    }
    let polynomial = Dense::new(&engine, entries, budget)?;
    crate::solving::radicals::proof::coverage(
        &mut fixture.cas,
        construction,
        &specification.problem,
        &polynomial,
        values,
        budget,
    )?;
    let error = fixture
        .cas
        .check_proof(
            &fixture.context,
            &construction.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )
        .err()
        .ok_or("unlinked choices accepted")?;
    assert!(
        matches!(error.kind, crate::error::FaultKind::Invalid(_)),
        "{error:?}"
    );
    Ok(())
}
