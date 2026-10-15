// SPDX-License-Identifier: AGPL-3.0-only
//! Named and expanded native routes enter both queues before completed-result dominance.

use crate::budget::Budget;
use crate::distribution_syntax::{AT, invalid};
use crate::error::{Fault, FaultKind};
use crate::frontier::{Frontier, Metrics};
use crate::graph::{Cas, LogicalId, size};
use crate::model::Operator;
use crate::request_model::Request;
use crate::search::{SearchPolicy, SearchResult};
use crate::service::Execution;
use crate::task::Task;
use std::collections::BTreeSet;

/// Immutable producer intent and its snapshot taken before either route runs.
struct Routes<'a> {
    /// Exact caller task and source support.
    request: &'a Request,
    /// Complete typed input roots.
    inputs: &'a [LogicalId],
    /// Pinned positive cost table.
    policy: &'a SearchPolicy,
    /// Strictly older native equality transitions.
    known: Vec<crate::distribution_known::Known>,
    /// Whether route zero preserves transparent names.
    named: bool,
}

/// Snapshot older proofs first, then schedule all eligible producer intents in stable order.
pub(crate) fn execute(
    cas: &mut Cas,
    request: &Request,
    inputs: &[LogicalId],
    policy: &SearchPolicy,
    generation: &mut Budget,
    checker: &mut Budget,
) -> Result<Execution, Fault> {
    let routes = Routes {
        request,
        inputs,
        policy,
        known: crate::known::typed_snapshot(
            cas,
            request.context.logical_id(),
            &policy.costs,
            generation,
        )?,
        named: eligible(cas, &request.task, inputs)?,
    };
    let mut queue = Frontier::default();
    let key = intent_key(cas, request, inputs, generation)?;
    for id in 0..if routes.named { 2 } else { 1 } {
        queue.producer(
            id,
            key.clone(),
            pending_metrics(cas, request, inputs, &key)?,
            generation,
        )?;
    }
    cas.transaction(generation, |candidate, generation| {
        let result = explore(candidate, &routes, &mut queue, generation, checker);
        Ok(result)
    })
}

/// A failure after a complete replay never erases the already accepted best result.
fn explore(
    cas: &mut Cas,
    routes: &Routes<'_>,
    queue: &mut Frontier,
    generation: &mut Budget,
    checker: &mut Budget,
) -> Execution {
    let mut best: Option<SearchResult> = None;
    let mut failure = None;
    let mut completed = Frontier::default();
    loop {
        let id = match queue.pop(generation) {
            Ok(Some(id)) => id,
            Ok(None) => break,
            Err(fault) => {
                failure = Some(fault);
                break;
            }
        };
        match crate::distribution_search::route(
            cas,
            routes.request,
            routes.inputs,
            routes.policy,
            routes.named && id == 0,
            &routes.known,
            generation,
            checker,
        ) {
            Ok(result) => {
                let comparison =
                    compare(cas, routes.request, id, &result, &mut completed, generation);
                if !matches!(comparison, Ok(false))
                    && best.as_ref().is_none_or(|previous| {
                        (result.cost, result.peak, &result.witness.bytes)
                            < (previous.cost, previous.peak, &previous.witness.bytes)
                    })
                {
                    best = Some(result);
                }
                if let Err(fault) = comparison {
                    failure = Some(fault);
                    break;
                }
            }
            Err(fault) => {
                let exhausted = matches!(fault.kind, FaultKind::Exhausted { .. });
                failure = Some(fault);
                if exhausted {
                    break;
                }
            }
        }
    }
    if let Some(result) = &mut best {
        result.statistics.visits = result
            .statistics
            .visits
            .saturating_add(u64::try_from(queue.visits.len()).unwrap_or(u64::MAX));
        result.statistics.dominated = completed.dominated;
        if failure.is_some_and(|fault| !matches!(fault.kind, FaultKind::Exhausted { .. })) {
            failure = None;
        }
    }
    Execution {
        result: best,
        fault: failure,
    }
}

/// Completed dominance is diagnostic; an already replayed best survives accounting exhaustion.
fn compare(
    cas: &Cas,
    request: &Request,
    id: usize,
    result: &SearchResult,
    completed: &mut Frontier,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let key = crate::wire_build::judgment_key(cas, result.witness.judgment(), budget)?;
    let metrics = crate::search::typed_metrics(cas, result, request, budget)?;
    completed.insert(id, key, [0, result.peak, result.cost, 0], metrics, budget)
}

/// Both routes are meaningful for transparent named regular distributions.
fn eligible(cas: &Cas, task: &Task, inputs: &[LogicalId]) -> Result<bool, Fault> {
    let root = cas.handle(*inputs.first().ok_or_else(invalid)?);
    let input = match task {
        Task::Differentiate { .. } => root,
        Task::Simplify => {
            let value = cas.state.expression(root.logical_id())?;
            if !matches!(value.operator, Operator::DistributionDerivative { .. }) {
                return Ok(false);
            }
            cas.handle(*value.operands.first().ok_or_else(invalid)?)
        }
        _ => return Ok(false),
    };
    crate::distribution_recipes::named(cas, &input)
}

/// Full context, request payload, spaces, roots and source support remain in the queue identity.
fn intent_key(
    cas: &Cas,
    request: &Request,
    inputs: &[LogicalId],
    budget: &mut Budget,
) -> Result<Vec<u8>, Fault> {
    let context = crate::wire_build::syntax_key(cas, request.context.logical_id(), budget)?;
    let mut roots = Vec::new();
    for input in inputs {
        roots.push(crate::wire_build::syntax_key(cas, *input, budget)?);
    }
    let mut references = std::collections::BTreeMap::new();
    for reference in request.task.references() {
        references.insert(
            reference.logical_id(),
            crate::wire_build::syntax_key(cas, reference.logical_id(), budget)?,
        );
    }
    let mut out = crate::encoding::Encoder::new(budget, AT);
    out.count(context.len())?;
    out.raw(&context)?;
    out.count(roots.len())?;
    for root in roots {
        out.count(root.len())?;
        out.raw(&root)?;
    }
    crate::task::encode_with(&request.task, &mut out, |handle, out| {
        let bytes = references.get(&handle.logical_id()).ok_or_else(invalid)?;
        out.count(bytes.len())?;
        out.raw(bytes)
    })?;
    Ok(out.finish())
}

/// Pending producer support is explicit even before any mathematical proof exists.
fn pending_metrics(
    cas: &Cas,
    request: &Request,
    inputs: &[LogicalId],
    key: &[u8],
) -> Result<Metrics, Fault> {
    let first = cas.state.expression(*inputs.first().ok_or_else(invalid)?)?;
    let mut support: BTreeSet<_> = request
        .inputs
        .iter()
        .map(|input| input.root.logical_id())
        .collect();
    support.extend(
        request
            .premises
            .iter()
            .map(|value| value.proof().logical_id()),
    );
    Ok(Metrics {
        cost: 1,
        peak: first.unfolded,
        credits: 0,
        depth: first.depth,
        bytes: size(key.len(), AT)?,
        support,
        encoding: key.to_vec(),
    })
}
