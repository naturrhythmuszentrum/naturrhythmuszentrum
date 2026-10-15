// SPDX-License-Identifier: AGPL-3.0-only
//! Finite split requests produce endpoint evidence and undergo fresh-store replay.

use crate::budget::Budget;
use crate::case_producer::Plan;
use crate::case_syntax::invalid;
use crate::error::Fault;
use crate::graph::{Cas, LogicalId};
use crate::number::Rational;
use crate::proof_model::{Certificate, Statement};
use crate::request_model::Request;
use crate::rule::Primitive;
use crate::search::{SearchPolicy, SearchResult};
use crate::split_model::{SplitCertificate, SplitKind, SplitValue};
use crate::task::Task;

/// Publish a complete finite calculation only after independent checking and canonical replay.
pub(crate) fn execute(
    cas: &mut Cas,
    request: &Request,
    inputs: &[LogicalId],
    policy: &SearchPolicy,
    generation: &mut Budget,
    checker: &mut Budget,
) -> Result<SearchResult, Fault> {
    let (kind, range) = match &request.task {
        Task::Integrate(range) => (SplitKind::Integral, range),
        Task::Sum(range) => (SplitKind::Sum, range),
        _ => return Err(invalid()),
    };
    let [input] = inputs else {
        return Err(invalid());
    };
    if !request.premises.is_empty() {
        return Err(invalid());
    }
    let mut candidate = cas.fork(generation)?;
    let basis = candidate.handle(
        candidate
            .state
            .context(candidate.checked(&request.context)?)?
            .admissible,
    );
    let domain = crate::split_geometry::domain(&mut candidate, &basis, range, generation)?;
    let mut value = SplitValue {
        kind,
        function: candidate.handle(*input),
        domain,
        range: range.as_ref().clone(),
        value: Rational::zero(),
    };
    let mut evidence = crate::split_geometry::reconstruct(&mut candidate, &value, generation)?;
    value.value = endpoints(&mut evidence, kind, generation)?;
    if range.lower.compare(&range.upper, generation)?.is_gt() {
        value.value = value.value.negated(generation)?;
    }
    let mut plan = Plan::new(policy);
    let size = candidate.state.expression(*input)?.unfolded;
    plan.expansion(size, size, generation)?;
    let rule = match kind {
        SplitKind::Integral => Primitive::IntegralCaseSplit,
        SplitKind::Sum => Primitive::SumCaseSplit,
    };
    let trace = vec![value.function.clone()];
    let proof = plan.push(
        &mut candidate,
        &request.context,
        rule,
        Statement::Split(Box::new(value)),
        &[],
        Certificate::Split(Box::new(evidence)),
        generation,
        checker,
    )?;
    let result = plan.finish(&candidate, &proof, trace, checker)?;
    cas.state = candidate.state;
    Ok(result)
}

/// Produce antiderivative or antidifference coefficients and Horner endpoint evaluations.
fn endpoints(
    evidence: &mut SplitCertificate,
    kind: SplitKind,
    budget: &mut Budget,
) -> Result<Rational, Fault> {
    let mut total = Rational::zero();
    for piece in &mut evidence.pieces {
        piece.primitive = crate::split_polynomial::primitive(&piece.polynomial, kind, budget)?;
        piece.start = crate::split_polynomial::evaluate(&piece.primitive, &piece.lower, budget)?;
        let upper = if kind == SplitKind::Sum {
            piece.upper.add(&Rational::one(), budget)?
        } else {
            piece.upper.clone()
        };
        piece.end = crate::split_polynomial::evaluate(&piece.primitive, &upper, budget)?;
        total = total.add(&piece.end.sub(&piece.start, budget)?, budget)?;
    }
    Ok(total)
}
