// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded degree-generic discovery with independent coverage and presentation replay.

mod guard;
mod presentation;
mod read;
mod regional;
#[expect(
    clippy::redundant_pub_crate,
    reason = "generic radical cursors share the deterministic scheduler"
)]
pub(crate) mod schedule;

use crate::budget::Budget;
use crate::error::{Fault, FaultKind, Location};
use crate::graph::{Cas, Handle};
use crate::request_model::{ContextConsistency, Outcome, Request};
use crate::solving::coefficients::model::CoefficientFailure;
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

/// An unsealed candidate retains its exact proposed DAG and deferred presentation.
pub(crate) struct Proposed {
    /// Diagnostic consumers may independently replay each reachable prefix.
    pub(crate) construction: Construction,
    /// Proposed complete set, with no authority until replay succeeds.
    set: Handle,
    /// Requested representation strength.
    form: SolveForm,
    /// Postorder representation work is deferred until coverage checks.
    presentations: presentation::Docket,
}

/// The original region, domain and complete request premises govern every coefficient inversion.
pub(crate) fn propose(
    cas: &mut Cas,
    request: &Request,
    audit: &mut Audit,
    budget: &mut Budget,
) -> Result<Proposed, CoefficientFailure> {
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
    audit.mark("source-polynomial", &original.region, 0, budget)?;
    let polynomial = read::polynomial(cas, original, budget)?;
    let mut construction = Construction {
        header: original.header.clone(),
        context: request.context.clone(),
        region: original.region.clone(),
        steps: Vec::new(),
    };
    let (proposal, presentations) = {
        let mut search = regional::Regional {
            cas,
            construction: &mut construction,
            request,
            problem: &specification.problem,
            original: &polynomial,
            audit,
            budget,
            guards: Vec::new(),
            presentations: presentation::Docket::default(),
        };
        let proposal = search.solve()?;
        (proposal, search.presentations)
    };
    Ok(Proposed {
        construction,
        set: proposal.set,
        form: specification.form,
        presentations,
    })
}

/// Independently seal coverage before preparing or checking the requested representation.
pub(crate) fn produce(
    cas: &mut Cas,
    request: &Request,
    audit: &mut Audit,
    budget: &mut Budget,
    checker: &mut Budget,
) -> Result<crate::solving::radicals::proof::Produced, CoefficientFailure> {
    let Proposed {
        mut construction,
        set,
        form,
        presentations,
    } = propose(cas, request, audit, budget)?;
    audit.mark("coverage-replay", &construction.region, 0, budget)?;
    let coverage = cas.check_proof(&construction.context, &construction.steps, checker)?;
    let (context, region) = (construction.context.clone(), construction.region.clone());
    let prepared = presentations.emit(&mut construction, form, budget);
    construction.context = context;
    construction.region = region;
    Ok(crate::solving::radicals::proof::finish(
        cas,
        &construction,
        set,
        form,
        coverage,
        prepared,
        checker,
    ))
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
        Ok(crate::solving::radicals::proof::Produced {
            answer,
            fault: Some(error),
        }) => failure(error, Some(answer), generation, checker),
        Ok(crate::solving::radicals::proof::Produced {
            answer,
            fault: None,
        }) if answer.unresolved_regions.is_empty() => Outcome::Solved {
            request: record.clone(),
            answer,
            usage: generation.usage(),
        },
        Ok(crate::solving::radicals::proof::Produced {
            answer,
            fault: None,
        }) => {
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
        Err(error) => failure(error, None, generation, checker),
    })
}

/// Preserve the first failing ledger and any coverage already independently checked before it.
fn failure(
    fault: Fault,
    best: Option<SolveAnswer>,
    generation: &Budget,
    checker: &Budget,
) -> Outcome {
    match fault.kind {
        FaultKind::Exhausted { .. } => Outcome::Exhausted {
            solve_best: best.map(Box::new),
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
