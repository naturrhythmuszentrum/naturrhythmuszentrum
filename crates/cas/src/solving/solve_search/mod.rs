// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded degree-generic discovery with independent coverage and presentation replay.
//!
//! # Original solve requests and replayed answers
//!
//! `Task::Solve(Box<SolveSpec>)` names the original problem, candidate or complete goal, exact presentation and `PreserveOriginal` exclusions. The covered predicate must agree exactly with that problem. The radical search producer parses an original univariate polynomial, derives its affine translation, and proposes binomial or strictly smaller symmetric-resolvent routes. Computed rational binomials of odd degree, and positive rational square-root cases, use named real existence followed by independently checked complex selection. Full factor coverage and requested presentation are independently replayed before a solved answer is returned. The zero polynomial returns its whole-coordinate identity family. The symbolic Complex cubic preserves its original fraction algebra, Complex declaration frame and every degree and coefficient stratum through complete Coverage and Radicals Presentation. Exact Gaussian input retains both components. Unfinished higher-degree work still reports its precise capability or resource boundary. The symbolic Real cubic explicitly requests `AnyExact` and returns complete Coverage with checked `RealExact` Presentation on every coefficient stratum. The producer keeps its original Real declarations and uses real radicals on the one-root and degenerate strata, and three count-one Real cells in the casus irreducibilis stratum. If presentation work exhausts after coverage has independently replayed, the response retains that coverage at `AnyExact` strength and reports the exact later fault. `SolveAnswer` separates sealed facts, optional coverage and unresolved regions; construction cannot manufacture a successful answer. `Statement::Solve` and `Certificate::Solve` retain closed typed payloads for subsequent replay checkers. The sole current `NRZCAS04` witness format includes the typed objects, scalar attachments and closed solving payloads; unavailable theorem schemas still refuse replay.
//!
//! Symbolic presentation extends the same request’s already checked Coverage graph. Every reused draft and ordered premise edge must match the sealed dependency closure; every new presentation application undergoes ordinary scoped checking. A failed suffix rolls back its new proof nodes while preserving coverage and spent work. Exported witnesses contain the complete proof, and a fresh receiving store independently checks every application.

mod guard;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the Real producer shares deferred presentation scheduling"
)]
pub(crate) mod presentation;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the Real entrypoint preserves the same original polynomial parser"
)]
pub(crate) mod read;
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
    /// Parameter requests extend their own sealed coverage through checked graph premises.
    reuse_coverage: bool,
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
    if original.ambient == crate::model::Field::Real {
        let form = if specification.form == SolveForm::AnyExact {
            SolveForm::RealExact
        } else {
            specification.form
        };
        let (construction, set, presentations) =
            super::real::producer::propose(cas, request, audit, budget)?;
        return Ok(Proposed {
            construction,
            set,
            form,
            presentations,
            reuse_coverage: true,
        });
    }
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
    let region = if cas
        .state
        .region(original.region.logical_id())?
        .lies_over
        .is_some()
    {
        let current = cas.handle(cas.state.context(request.context.logical_id())?.admissible);
        if !crate::solving::family::region::subset(
            cas,
            current.logical_id(),
            original.region.logical_id(),
            budget,
        )? {
            return Err(crate::solving::family::invalid().into());
        }
        current
    } else {
        original.region.clone()
    };
    let mut construction = Construction {
        header: original.header.clone(),
        context: request.context.clone(),
        region,
        steps: Vec::new(),
    };
    let reuse_coverage = matches!(
        crate::solving::coefficients::storage::domain(&cas.state, &polynomial.domain)?,
        super::CoefficientDomainData::Fraction { .. }
    );
    let (proposal, presentations) = {
        let cache = reuse_coverage.then(crate::solving::radicals::cache::Cache::default);
        let mut search = regional::Regional {
            cas,
            construction: &mut construction,
            request,
            problem: &specification.problem,
            original: &polynomial,
            audit,
            budget,
            guards: Vec::new(),
            cache,
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
        reuse_coverage,
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
        reuse_coverage,
    } = propose(cas, request, audit, budget)?;
    audit.mark("coverage-replay", &construction.region, 0, budget)?;
    let coverage = cas.check_proof(&construction.context, &construction.steps, checker)?;
    let (context, region) = (construction.context.clone(), construction.region.clone());
    let prefix = reuse_coverage.then_some(construction.steps.len());
    let prepared = presentations
        .emit(&mut construction, form, budget)
        .map(|()| prefix);
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
                if let crate::task::Task::Solve(specification) = &request.task
                    && specification.form == SolveForm::Radicals
                    && problem(cas, &specification.problem)
                        .is_ok_and(|problem| problem.ambient == crate::model::Field::Real)
                {
                    SolveReason::RequestedRealRadicalsUnavailable
                } else {
                    SolveReason::RadicalFormUnavailable
                }
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
pub(crate) fn coefficient_images(
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
