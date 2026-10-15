// SPDX-License-Identifier: AGPL-3.0-only
//! Proposed affine inverses and finite rational roots become checked portable pullback theorems.

use crate::budget::{Budget, BudgetKind};
use crate::case_producer::Plan;
use crate::case_syntax::invalid;
use crate::distribution_model::DistributionStatement;
use crate::error::Fault;
use crate::graph::{Cas, Handle, LogicalId, size};
use crate::model::Operator;
use crate::number::Rational;
use crate::proof_model::{Certificate, Statement};
use crate::pullback_geometry::{AT, map_error};
use crate::pullback_model::{
    PullbackCertificate, PullbackKind, PullbackMode, PullbackValue, PullbackZero,
};
use crate::request_model::Request;
use crate::rule::Primitive;
use crate::search::{SearchPolicy, SearchResult};
use crate::task::Task;

/// Source distribution and scalar map each have their own complete occurrence receipt.
pub(crate) fn execute(
    cas: &mut Cas,
    request: &Request,
    inputs: &[LogicalId],
    policy: &SearchPolicy,
    generation: &mut Budget,
    checker: &mut Budget,
) -> Result<SearchResult, Fault> {
    let Task::Pullback {
        source_space,
        target_space,
        map_kind,
    } = &request.task
    else {
        return Err(invalid());
    };
    let [source, map] = inputs else {
        return Err(invalid());
    };
    if !request.premises.is_empty() {
        return Err(invalid());
    }
    let mut candidate = cas.fork(generation)?;
    let mut claim = PullbackValue {
        source: candidate.handle(*source),
        result: candidate.handle(*source),
        map: candidate.handle(*map),
        inverse: None,
        source_space: source_space.clone(),
        target_space: target_space.clone(),
        mode: match map_kind {
            PullbackKind::Affine => PullbackMode::Affine,
            PullbackKind::SimpleZeros(_) => PullbackMode::SimpleZeros,
        },
    };
    let certificate = produce(&mut candidate, &mut claim, map_kind, generation)?;
    let header = candidate.context_header(&request.context)?;
    claim.result =
        crate::pullback_output::build(&mut candidate, &header, &claim, &certificate, generation)?;
    let result = seal(
        &mut candidate,
        request,
        claim,
        certificate,
        policy,
        generation,
        checker,
    )?;
    cas.state = candidate.state;
    Ok(result)
}

/// Check the entire proposal and its portable closure before returning any pullback claim.
fn seal(
    cas: &mut Cas,
    request: &Request,
    claim: PullbackValue,
    certificate: PullbackCertificate,
    policy: &SearchPolicy,
    generation: &mut Budget,
    checker: &mut Budget,
) -> Result<SearchResult, Fault> {
    let mut plan = Plan::new(policy);
    let before = cas.state.expression(claim.map.logical_id())?.unfolded;
    let after = cas.state.expression(claim.result.logical_id())?.unfolded;
    plan.expansion(before, after, generation)?;
    let rule = match claim.mode {
        PullbackMode::Affine => Primitive::DistAffinePullback,
        PullbackMode::SimpleZeros => Primitive::DistSimpleZeroPullback,
    };
    let trace = vec![
        claim.source.clone(),
        claim.map.clone(),
        claim.result.clone(),
    ];
    let proof = plan.push(
        cas,
        &request.context,
        rule,
        Statement::Distribution(Box::new(DistributionStatement::Pullback(Box::new(claim)))),
        &[],
        Certificate::Pullback(Box::new(certificate)),
        generation,
        checker,
    )?;
    plan.finish(cas, &proof, trace, checker)
}

/// An affine inverse is constructed; supplied polynomial roots remain untrusted until checked.
fn produce(
    cas: &mut Cas,
    claim: &mut PullbackValue,
    kind: &PullbackKind,
    budget: &mut Budget,
) -> Result<PullbackCertificate, Fault> {
    let support = crate::pullback_check::source(cas, claim, budget)?;
    let coefficients =
        crate::pullback_polynomial::coefficients(cas, &claim.map, &claim.target_space, budget)?;
    let leading = coefficients
        .last()
        .ok_or_else(crate::pullback_check::incomplete)?
        .clone();
    let roots = match kind {
        PullbackKind::Affine => {
            budget.specialized(BudgetKind::PullbackZeros, 1, false, AT)?;
            let [offset, scale] = coefficients.as_slice() else {
                return Err(map_error());
            };
            if scale.is_zero() {
                return Err(map_error());
            }
            claim.inverse = Some(inverse(cas, &claim.source_space, offset, scale, budget)?);
            vec![support.sub(offset, budget)?.div(scale, budget)?]
        }
        PullbackKind::SimpleZeros(roots) => {
            budget.specialized(BudgetKind::PullbackZeros, size(roots.len(), AT)?, false, AT)?;
            budget.work(size(roots.len(), AT)?.saturating_mul(512), AT)?;
            roots.clone()
        }
    };
    let derivative =
        crate::univariate::Polynomial::new(&coefficients, budget)?.derivative(budget)?;
    let mut zeros = Vec::new();
    zeros
        .try_reserve_exact(roots.len())
        .map_err(|_| Fault::allocation(AT))?;
    for point in roots {
        let slope = crate::split_polynomial::evaluate(&derivative.coefficients, &point, budget)?;
        if slope.is_zero() {
            return Err(crate::pullback_check::multiple());
        }
        let absolute = if slope.is_negative() {
            slope.negated(budget)?
        } else {
            slope.clone()
        };
        zeros.push(PullbackZero {
            point,
            slope,
            inverse_jacobian: Rational::one().div(&absolute, budget)?,
        });
    }
    Ok(PullbackCertificate { leading, zeros })
}

/// Retain the inverse as actual scalar syntax in the source coordinate's own header.
fn inverse(
    cas: &mut Cas,
    space: &Handle,
    offset: &Rational,
    scale: &Rational,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let definition = cas.state.space(cas.checked(space)?)?.clone();
    let domain = cas.handle(definition.domain);
    let model = crate::boundary_regions::term(
        cas,
        &domain,
        Operator::Free(definition.coordinate),
        &[],
        budget,
    )?;
    let offset = crate::boundary_regions::term(
        cas,
        &domain,
        Operator::Rational(offset.clone()),
        &[],
        budget,
    )?;
    let numerator =
        crate::boundary_regions::term(cas, &domain, Operator::Sub, &[model, offset], budget)?;
    let scale = crate::boundary_regions::term(
        cas,
        &domain,
        Operator::Rational(scale.clone()),
        &[],
        budget,
    )?;
    crate::boundary_regions::term(cas, &domain, Operator::Div, &[numerator, scale], budget)
}
