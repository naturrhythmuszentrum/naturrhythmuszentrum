// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded degree-generic discovery with independent coverage and presentation replay.

mod read;
#[expect(
    clippy::redundant_pub_crate,
    reason = "generic radical cursors share the deterministic scheduler"
)]
pub(crate) mod schedule;

use crate::budget::Budget;
use crate::error::{Fault, FaultKind, Location};
use crate::graph::{Cas, Handle};
use crate::request_model::{ContextConsistency, Outcome, Request};
use crate::solving::coefficients::model::{CoefficientFailure, EvaluationScope};
use crate::solving::elementary::proof::problem;
use crate::solving::radicals::{construction::Construction, route::Audit};
use crate::solving::{SolveAnswer, SolveForm, SolveReason};

/// Stable attribution for parsing and scheduling work.
pub(crate) const AT: Location = Location::new("radical solve search");

/// Production uses existing independently checked schemas; it adds no trusted primitive.
pub(crate) const fn availability() -> super::Availability {
    super::Availability::Available
}

/// Offer the generic service only with the implemented producer.
pub(crate) const fn executor() -> Option<super::SolveExecutor> {
    match availability() {
        super::Availability::Available => Some(execute),
        super::Availability::Unavailable => None,
    }
}

/// Map arithmetic capability boundaries to honest unresolved outcomes without retaining failed proposals.
fn pending(record: &Handle, reason: SolveReason, budget: &Budget) -> Outcome {
    Outcome::Unresolved {
        request: record.clone(),
        solve_reason: Some(reason),
        consistency: ContextConsistency::Unknown,
        blocker: None,
        usage: budget.usage(),
    }
}

/// The original region, domain and complete request premises govern every coefficient inversion.
fn produce(
    cas: &mut Cas,
    request: &Request,
    audit: &mut Audit,
    budget: &mut Budget,
    checker: &mut Budget,
) -> Result<SolveAnswer, CoefficientFailure> {
    let crate::task::Task::Solve(specification) = &request.task else {
        return Err(crate::solving::coefficients::storage::invalid().into());
    };
    let original = problem(cas, &specification.problem)?;
    if original.ambient != crate::model::Field::Complex {
        return Err(CoefficientFailure::Unsupported(
            SolveReason::RealIsolationUnavailable,
        ));
    }
    if matches!(
        specification.form,
        SolveForm::LocalBranch | SolveForm::ExactFamily
    ) {
        return Err(CoefficientFailure::Unsupported(
            SolveReason::CoveragePending,
        ));
    }
    let polynomial = read::polynomial(cas, original, budget)?;
    let domain = original.coefficient_domain.clone();
    let region = original.region.clone();
    let mut construction = Construction {
        header: original.header.clone(),
        context: request.context.clone(),
        region: region.clone(),
        steps: Vec::new(),
    };
    if polynomial.coefficients.is_empty() {
        let original = problem(cas, &specification.problem)?;
        if !original.denominators.is_empty()
            || !matches!(
                cas.state.region(original.domain.logical_id())?.expression,
                crate::domain_model::RegionExpr::Ambient
            )
        {
            return Err(CoefficientFailure::Unsupported(
                SolveReason::CoveragePending,
            ));
        }
        return Ok(crate::solving::radicals::family::finish(
            cas,
            &mut construction,
            &specification.problem,
            specification.form,
            budget,
            checker,
        )?);
    }
    let images = coefficient_images(cas, &construction, &domain, budget)?;
    let map = construction.mapping(cas, &domain, &images, budget)?;
    let scope = EvaluationScope {
        context: &request.context,
        region: &region,
        premises: &request.premises,
        localization: &[],
    };
    let values = crate::solving::radicals::route::Route {
        cas,
        construction: &mut construction,
        scope: &scope,
        audit,
        budget,
    }
    .derive(&polynomial, &map, 0)?;
    let proposal = crate::solving::radicals::proof::Proposal {
        problem: &specification.problem,
        polynomial: &polynomial,
        values: &values,
        form: specification.form,
    };
    Ok(crate::solving::radicals::proof::finish(
        cas,
        &mut construction,
        &proposal,
        budget,
        checker,
    )?)
}

/// Independent checking is the only route to a solved answer; unsupported work rolls back its syntax.
fn execute(
    cas: &mut Cas,
    request: &Request,
    record: &Handle,
    _policy: &crate::search::SearchPolicy,
    generation: &mut Budget,
    checker: &mut Budget,
) -> Option<Outcome> {
    if !matches!(request.task, crate::task::Task::Solve(_)) {
        return None;
    }
    let mut reason = None;
    let mut audit = Audit::default();
    let result = crate::solving::radicals::staged(cas, generation, |candidate, budget| {
        produce(candidate, request, &mut audit, budget, checker).map_err(|failure| match failure {
            CoefficientFailure::Fault(error) => error,
            CoefficientFailure::Unsupported(missing) => {
                reason = Some(missing);
                crate::solving::coefficients::storage::invalid()
            }
        })
    });
    Some(match result {
        Ok(answer) if answer.unresolved_regions.is_empty() => Outcome::Solved {
            request: record.clone(),
            answer,
            usage: generation.usage(),
        },
        Ok(answer) => {
            let reason = if answer.facts.is_empty() {
                SolveReason::RadicalFormUnavailable
            } else {
                SolveReason::ConditionalCoverage
            };
            Outcome::SolvePartial {
                request: record.clone(),
                answer,
                reason,
                usage: generation.usage(),
            }
        }
        Err(_) if reason.is_some() => pending(
            record,
            reason.unwrap_or(SolveReason::CoveragePending),
            generation,
        ),
        Err(error) => failure(error, generation, checker),
    })
}

/// Preserve the first failing ledger and its exact location, with no unverified best answer.
const fn failure(fault: Fault, generation: &Budget, checker: &Budget) -> Outcome {
    match fault.kind {
        FaultKind::Exhausted { .. } => Outcome::Exhausted {
            solve_best: None,
            best: None,
            replay_usage: checker.usage(),
            fault,
            usage: generation.usage(),
        },
        FaultKind::Invalid(_) => Outcome::Invalid {
            fault,
            usage: generation.usage(),
        },
    }
}

/// Coefficient interpretation follows declared keys in their ordered frame, including renamed inputs.
fn coefficient_images(
    cas: &mut Cas,
    construction: &Construction,
    domain: &Handle,
    budget: &mut Budget,
) -> Result<Vec<Handle>, Fault> {
    let mut keys = crate::solving::coefficients::storage::vector(0, budget)?;
    if let crate::solving::CoefficientDomainData::Fraction { coordinates, .. } =
        crate::solving::coefficients::storage::domain(&cas.state, domain)?
    {
        let frame = crate::solving::elementary::proof::coordinates(cas, coordinates)?;
        crate::solving::coefficients::storage::reserve(&mut keys, frame.coordinates.len(), budget)?;
        for coordinate in &frame.coordinates {
            budget.work(crate::graph::size(coordinate.key.len(), AT)?, AT)?;
            keys.push(coordinate.key.clone());
        }
    }
    let mut images = crate::solving::coefficients::storage::vector(keys.len(), budget)?;
    for key in keys {
        images.push(construction.expression(
            cas,
            crate::model::Operator::Free(key),
            &[],
            false,
            budget,
        )?);
    }
    Ok(images)
}
