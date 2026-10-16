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
fn closure(
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
        fixture.cas.check_proof(&step.context, &closure(&construction.steps, index)?, &mut Budget::new(Limits::blind_cubic())?).map_err(|error| format!("first rejected application {index}: rule {:?}; conclusion {:?}; fault {error:?}", step.rule, step.conclusion))?;
    }
    Ok(())
}
