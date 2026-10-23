// SPDX-License-Identifier: AGPL-3.0-only
//! Diagnostic prefix replay attributes a refused proposal to its first primitive application.

use crate::limits::{Budget, Limits};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::model::EvaluationScope;
use crate::solving::radicals::{
    construction::Construction,
    route::{Audit, Route},
};
use crate::solving::towers::dense::Dense;
use crate::testing::common::TestResult;

/// Each diagnostic root uses exactly its dependency closure; unrelated earlier roots are not evidence.
pub(crate) fn closure(
    steps: &[crate::proof_model::ApplicationDraft],
    root: usize,
) -> TestResult<Vec<crate::proof_model::ApplicationDraft>> {
    let mut reached = std::collections::BTreeSet::new();
    let mut pending = vec![root];
    while let Some(index) = pending.pop() {
        if !reached.insert(index) {
            continue;
        }
        for premise in &steps
            .get(index)
            .ok_or("diagnostic missing application")?
            .premises
        {
            let previous = usize::try_from(*premise)?;
            if previous >= index {
                return Err("diagnostic forward premise".into());
            }
            pending.push(previous);
        }
    }
    let mut indices = std::collections::BTreeMap::new();
    let mut result = Vec::new();
    for index in reached {
        let mut step = steps[index].clone();
        for premise in &mut step.premises {
            *premise = *indices
                .get(&usize::try_from(*premise)?)
                .ok_or("diagnostic missing dependency")?;
        }
        indices.insert(index, u32::try_from(result.len())?);
        result.push(step);
    }
    Ok(result)
}

/// A scalar inverse search must skip valid earlier coefficient identities with polynomial endpoints.
fn identity_inputs(
    steps: &[crate::proof_model::ApplicationDraft],
) -> Vec<(
    usize,
    crate::Handle,
    crate::Handle,
    crate::Handle,
    crate::Handle,
)> {
    steps
        .iter()
        .enumerate()
        .filter_map(|(index, step)| {
            let crate::proof_model::Certificate::Solve(certificate) = &step.certificate else {
                return None;
            };
            let crate::solving::SolveCertificate::CoefficientIdentity(value) = certificate.as_ref()
            else {
                return None;
            };
            Some((
                index,
                value.left.clone(),
                value.right.clone(),
                value.map.clone(),
                value.region.clone(),
            ))
        })
        .collect()
}

/// Source-only construction and generic route precede every independently checked diagnostic prefix.
pub(crate) fn prefixes(coefficients: &[i64]) -> TestResult {
    let (mut fixture, request) = super::source::polynomial(coefficients)?;
    let crate::task::Task::Solve(specification) = &request.task else {
        return Err("diagnostic solve request".into());
    };
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let mut construction = Construction {
        header: fixture.header.clone(),
        context: fixture.context.clone(),
        region: fixture.region.clone(),
        steps: Vec::new(),
    };
    let engine = Engine {
        state: &fixture.cas.state,
        domain: &fixture.algebra,
    };
    let mut entries = Vec::new();
    for coefficient in coefficients {
        let scalar = crate::Rational::from_ratio(*coefficient, 1, &mut budget)?;
        entries.push(engine.rational(&scalar, &mut budget)?);
    }
    let polynomial = Dense::new(&engine, entries, &mut budget)?;
    let map = construction.mapping(&mut fixture.cas, &fixture.algebra, &[], &mut budget)?;
    let scope = EvaluationScope {
        context: &fixture.context,
        region: &fixture.region,
        premises: &[],
        localization: &[],
    };
    let mut audit = Audit::default();
    let values = Route {
        cache: None,
        units: Vec::new(),
        choices: crate::solving::radicals::route::Choices::default(),
        cas: &mut fixture.cas,
        construction: &mut construction,
        scope: &scope,
        audit: &mut audit,
        budget: &mut budget,
    }
    .derive(&polynomial, &map, 0)?;
    crate::solving::radicals::proof::coverage(
        &mut fixture.cas,
        &mut construction,
        &specification.problem,
        &polynomial,
        &values,
        &mut budget,
    )?;
    for (index, step) in construction.steps.iter().enumerate() {
        fixture.cas.check_proof(&step.context, &closure(&construction.steps, index)?, &mut Budget::new(Limits::blind_cubic())?).map_err(|error| format!("first rejected application {index}: fault {error:?}; rule {:?}; conclusion {:?}; prior_identity_inputs=(application,left,right,map,region) {:?}", step.rule, step.conclusion, identity_inputs(&construction.steps[..index])))?;
    }
    Ok(())
}

/// Both complete recovered sets use the same original problem and coefficient interpretation.
pub(crate) fn transported_choices(reverse: bool, rotation: u32) -> TestResult {
    let coefficients = [-4, -15, 0, 1];
    let (mut fixture, request) = super::source::polynomial(&coefficients)?;
    let crate::task::Task::Solve(specification) = &request.task else {
        return Err("choice solve request".into());
    };
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let engine = Engine {
        state: &fixture.cas.state,
        domain: &fixture.algebra,
    };
    let mut entries = Vec::new();
    for coefficient in coefficients {
        let scalar = crate::Rational::from_ratio(coefficient, 1, &mut budget)?;
        entries.push(engine.rational(&scalar, &mut budget)?);
    }
    let polynomial = Dense::new(&engine, entries, &mut budget)?;
    let scope = EvaluationScope {
        context: &fixture.context,
        region: &fixture.region,
        premises: &[],
        localization: &[],
    };
    let mut sets = Vec::new();
    let choices = crate::solving::radicals::route::Choices {
        inverse_unity: reverse,
        rotation,
    };
    for choices in [crate::solving::radicals::route::Choices::default(), choices] {
        let mut construction = Construction {
            header: fixture.header.clone(),
            context: fixture.context.clone(),
            region: fixture.region.clone(),
            steps: Vec::new(),
        };
        let map = construction.mapping(&mut fixture.cas, &fixture.algebra, &[], &mut budget)?;
        let values = Route {
            cache: None,
            units: Vec::new(),
            choices,
            cas: &mut fixture.cas,
            construction: &mut construction,
            scope: &scope,
            audit: &mut Audit::default(),
            budget: &mut budget,
        }
        .derive(&polynomial, &map, 0)?;
        let set = crate::solving::radicals::proof::coverage(
            &mut fixture.cas,
            &mut construction,
            &specification.problem,
            &polynomial,
            &values,
            &mut budget,
        )?;
        let coverage = fixture.cas.check_proof(
            &construction.context,
            &construction.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        assert!(coverage.is_discharged());
        super::oracle::product(&fixture.cas, &coverage, &coefficients)?;
        let bytes = fixture
            .cas
            .export_witness(&coverage, &mut Budget::new(Limits::blind_cubic())?)?;
        let mut fresh = crate::Cas::new(Limits::blind_cubic())?;
        assert!(
            fresh
                .inspect(&bytes, &mut Budget::new(Limits::blind_cubic())?)?
                .is_discharged()
        );
        sets.push(set);
    }
    assert_eq!(sets.first(), sets.get(1));
    Ok(())
}

/// Regional prefix replay uses each application's own context and never guesses the rejected primitive.
pub(crate) fn symbolic_prefixes(names: &[&str]) -> TestResult {
    let (mut fixture, request) = super::source::symbolic(names)?;
    let proposed = crate::solving::solve_search::propose(
        &mut fixture.cas,
        &request,
        &mut Audit::default(),
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    for (index, step) in proposed.construction.steps.iter().enumerate() {
        let steps = closure(&proposed.construction.steps, index)?;
        fixture.cas.check_proof(&step.context, &steps, &mut Budget::new(Limits::blind_cubic())?)
            .map_err(|error| format!("first rejected regional application {index}: fault {error:?}; rule {:?}; conclusion {:?}; certificate {:?}", step.rule, step.conclusion, step.certificate))?;
    }
    Ok(())
}
