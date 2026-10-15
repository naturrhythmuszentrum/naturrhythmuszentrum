// SPDX-License-Identifier: AGPL-3.0-only
//! Standalone punctured restrictions reuse the existing checked jet and trace recipes.

use crate::boundary_model::BoundaryCertificate;
use crate::boundary_producer::{Runner, pending};
use crate::budget::Budget;
use crate::case_producer::Plan;
use crate::case_syntax::invalid;
use crate::error::Fault;
use crate::germ_check::disagreement;
use crate::germ_model::{GermCertificate, GermMode, GermValue};
use crate::graph::{Cas, Handle, LogicalId};
use crate::number::Rational;
use crate::proof_model::{Certificate, Ownership, Statement};
use crate::request_model::Request;
use crate::rule::Primitive;
use crate::search::{SearchPolicy, SearchResult};
use crate::task::Task;

/// Check and replay the complete dependency closure once before publishing any new claim.
pub(crate) fn execute(
    cas: &mut Cas,
    request: &Request,
    inputs: &[LogicalId],
    policy: &SearchPolicy,
    generation: &mut Budget,
    checker: &mut Budget,
) -> Result<SearchResult, Fault> {
    let [input] = inputs else {
        return Err(invalid());
    };
    if !request.premises.is_empty() {
        return Err(invalid());
    }
    let mut candidate = cas.fork(generation)?;
    let function = candidate.handle(*input);
    let mut runner = Runner {
        cas: &mut candidate,
        request,
        plan: Plan::new(policy),
        generation,
        checker,
        facts: Vec::new(),
        deferred: Some(Vec::new()),
    };
    let claim = proposal(&function, &request.task)?;
    let proof = produce(&mut runner, claim)?;
    let result = runner
        .plan
        .finish(runner.cas, &proof, vec![function], runner.checker)?;
    cas.state = candidate.state;
    Ok(result)
}

/// Preserve the exact requested mode and approach; the result is still untrusted data.
fn proposal(function: &Handle, task: &Task) -> Result<GermValue, Fault> {
    let (at, mode) = match task {
        Task::Limit(at) => (at.as_ref().clone(), GermMode::Limit),
        Task::Series(series) => (
            series.at.clone(),
            GermMode::Series {
                order: series.order,
                analytic: series.analytic,
            },
        ),
        _ => return Err(invalid()),
    };
    Ok(GermValue {
        function: function.clone(),
        at,
        mode,
        coefficients: Vec::new(),
    })
}

/// Generate every required punctured side, then check the separate restriction schema.
fn produce(runner: &mut Runner<'_>, mut claim: GermValue) -> Result<Handle, Fault> {
    crate::boundary_roles::point(
        runner.cas,
        &claim.function,
        &claim.at.point,
        &claim.at.coordinate,
    )?;
    let order = match claim.mode {
        GermMode::Limit => 0,
        GermMode::Series { order, .. } => order,
    };
    runner.generation.check(
        crate::budget::BudgetKind::JetOrder,
        u64::from(order),
        crate::germ_check::AT,
    )?;
    let domain = runner.cas.handle(
        runner
            .cas
            .state
            .context(runner.cas.checked(&runner.request.context)?)?
            .admissible,
    );
    let mut premises = Vec::new();
    let mut remainders = Vec::new();
    for side in crate::germ_check::sides(&claim) {
        let region = runner.incident(
            &claim.function,
            &domain,
            &claim.at.point,
            side,
            &claim.at.coordinate,
        )?;
        let jet = runner.jet(
            &claim.function,
            &region,
            &claim.at.point,
            side,
            order,
            &claim.at.coordinate,
        )?;
        let values = coefficients(runner, &jet)?;
        if !claim.coefficients.is_empty() && claim.coefficients != values {
            return Err(disagreement());
        }
        claim.coefficients = values;
        if claim.mode == GermMode::Limit {
            premises.push(runner.projection(&jet, 0)?.0);
        } else {
            remainders.push(tail(runner, &jet, order)?);
            premises.push(jet);
        }
    }
    restrict(runner, claim, premises, remainders)
}

/// Glue only the evidence required by the selected relation and retain its complete expansion.
fn restrict(
    runner: &mut Runner<'_>,
    claim: GermValue,
    mut premises: Vec<Handle>,
    remainders: Vec<Vec<Rational>>,
) -> Result<Handle, Fault> {
    if matches!(claim.mode, GermMode::Series { analytic: true, .. })
        && remainders.first() != remainders.get(1)
    {
        return Err(disagreement());
    }
    if matches!(claim.mode, GermMode::Series { .. }) && premises.len() == 2 {
        premises.push(owned(runner, &claim)?);
    }
    let (rule, certificate) = if claim.mode == GermMode::Limit {
        (Primitive::LimitCaseRestrict, Certificate::None)
    } else {
        (
            Primitive::SeriesCaseRestrict,
            Certificate::Germ(Box::new(GermCertificate { remainders })),
        )
    };
    runner.plan.push(
        runner.cas,
        &runner.request.context,
        rule,
        Statement::Germ(Box::new(claim)),
        &premises,
        certificate,
        runner.generation,
        runner.checker,
    )
}

/// Divide the independently checked derivative traces by their factorials.
fn coefficients(runner: &mut Runner<'_>, proof: &Handle) -> Result<Vec<Rational>, Fault> {
    let Statement::Jet { values, .. } = runner.statement(proof)? else {
        return Err(invalid());
    };
    let mut factorial = Rational::one();
    let mut result = Vec::new();
    result
        .try_reserve_exact(values.len())
        .map_err(|_| Fault::allocation(crate::germ_check::AT))?;
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            factorial = factorial.mul(
                &Rational::from_ratio(
                    i64::try_from(index).map_err(|_| invalid())?,
                    1,
                    runner.generation,
                )?,
                runner.generation,
            )?;
        }
        result.push(
            crate::boundary_check::rational(runner.cas, value)?
                .div(&factorial, runner.generation)?,
        );
    }
    Ok(result)
}

/// Extract a finite exact remainder by shifting the local polynomial into the increment variable.
fn tail(runner: &mut Runner<'_>, proof: &Handle, order: u32) -> Result<Vec<Rational>, Fault> {
    let previous =
        crate::proof_view::application(runner.cas, runner.cas.checked(proof)?, runner.generation)?;
    let (Statement::Jet { boundary, .. }, Certificate::Boundary(mode)) =
        (&previous.judgment.statement, &previous.certificate)
    else {
        return Err(invalid());
    };
    let BoundaryCertificate::Polynomial { coefficients, .. } = mode.as_ref() else {
        return Err(invalid());
    };
    let centre = crate::region::boundary_point(
        &runner.cas.state,
        runner.cas.checked(boundary)?,
        runner.generation,
    )?;
    let shifted = crate::germ_polynomial::shift(coefficients, &centre, runner.generation)?;
    let first = usize::try_from(order)
        .map_err(|_| invalid())?
        .checked_add(1)
        .ok_or_else(invalid)?;
    Ok(shifted.coefficients.get(first..).unwrap_or(&[]).to_vec())
}

/// Actual centre ownership is required for ordinary Taylor and analytic gluing.
fn owned(runner: &mut Runner<'_>, claim: &GermValue) -> Result<Handle, Fault> {
    let proof = runner.ownership(&claim.function, &claim.at.point)?;
    let Statement::Ownership {
        value: Ownership::Present(value),
        ..
    } = runner.statement(&proof)?
    else {
        return Err(pending());
    };
    if claim.coefficients.first() != Some(crate::boundary_check::rational(runner.cas, &value)?) {
        return Err(disagreement());
    }
    Ok(proof)
}
