// SPDX-License-Identifier: AGPL-3.0-only
//! Typed distribution request routing, named refusals, and replayed results.

use crate::budget::Budget;
use crate::case_producer::Plan;
use crate::distribution_model::DistributionStatement;
use crate::distribution_producer::Runner;
use crate::distribution_syntax::{AT, invalid};
use crate::domain_model::Layer;
use crate::error::{Fault, FaultKind, InvalidCode};
use crate::graph::{Cas, Handle, LogicalId};
use crate::model::{Operator, Sort};
use crate::proof_model::Certificate;
use crate::request_model::{RefusalReason, Request};
use crate::rule::Primitive;
use crate::search::SearchPolicy;
use crate::service::Execution;
use crate::task::Task;
use std::collections::BTreeMap;

/// Distribution roots and scalar actions never enter the scalar projection search.
pub(crate) fn handles(cas: &Cas, task: &Task, inputs: &[LogicalId]) -> Result<bool, Fault> {
    if matches!(
        task,
        Task::EmbedRegular { .. }
            | Task::ApplyAt { .. }
            | Task::Product { .. }
            | Task::Pair { .. }
            | Task::SmoothMultiply { .. }
            | Task::Differentiate {
                layer: Layer::Distribution,
                ..
            }
    ) {
        return Ok(true);
    }
    if matches!(task, Task::Simplify | Task::Equal { .. }) {
        for input in inputs {
            if cas.state.expression(*input)?.space.is_some() {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Named refusals are chosen before any ill-sorted operation constructor is attempted.
pub(crate) fn execute(
    cas: &mut Cas,
    request: &Request,
    inputs: &[LogicalId],
    policy: &SearchPolicy,
    generation: &mut Budget,
    checker: &mut Budget,
) -> Result<Execution, Fault> {
    crate::distribution_preflight::check(cas, request, inputs, generation)?;
    if request_space(cas, &request.task, inputs)?.is_none() {
        return Ok(Execution {
            result: None,
            fault: None,
        });
    }
    crate::distribution_schedule::execute(cas, request, inputs, policy, generation, checker)
}

/// Each scheduled route has its own logical path accounting and shares only bounded ledgers.
#[expect(
    clippy::too_many_arguments,
    reason = "typed route intent, frozen baseline and independent ledgers stay explicit"
)]
pub(crate) fn route(
    cas: &mut Cas,
    request: &Request,
    inputs: &[LogicalId],
    policy: &SearchPolicy,
    named: bool,
    known: &[crate::distribution_known::Known],
    generation: &mut Budget,
    checker: &mut Budget,
) -> Result<crate::search::SearchResult, Fault> {
    let space = request_space(cas, &request.task, inputs)?.ok_or_else(invalid)?;
    let mut candidate = cas.fork(generation)?;
    let header = candidate.context_header(&request.context)?;
    let roots: Vec<_> = inputs.iter().map(|id| candidate.handle(*id)).collect();
    let mut runner = Runner {
        cas: &mut candidate,
        context: request.context.clone(),
        header,
        space,
        plan: Plan::new(policy),
        generation,
        checker,
        regular: BTreeMap::new(),
        derivatives: BTreeMap::new(),
        premises: request
            .premises
            .iter()
            .map(|value| value.proof().clone())
            .collect(),
    };
    let (output, proof) =
        run(&mut runner, &request.task, &roots, named, known).map_err(|mut fault| {
            if crate::case_obligations::unknown(&fault) {
                fault.kind = FaultKind::Invalid(InvalidCode::PendingPremise);
            }
            fault
        })?;
    let result = finish(runner, &roots, &output, &proof)?;
    cas.state = candidate.state;
    Ok(result)
}

/// Seal and independently replay one complete typed route before publishing its result.
fn finish(
    mut runner: Runner<'_>,
    roots: &[Handle],
    output: &Handle,
    proof: &Handle,
) -> Result<crate::search::SearchResult, Fault> {
    let before = roots.first().map_or(Ok(1), |root| {
        runner
            .cas
            .state
            .expression(root.logical_id())
            .map(|v| v.unfolded)
    })?;
    let after = runner.cas.state.expression(output.logical_id())?.unfolded;
    runner.plan.expansion(before, after, runner.generation)?;
    let trace = roots.first().map_or_else(
        || vec![output.clone()],
        |input| vec![input.clone(), output.clone()],
    );
    let expanded = crate::distribution_known::expanded_steps(
        runner.cas,
        proof.logical_id(),
        runner.generation,
    )?;
    let mut result = runner
        .plan
        .finish(runner.cas, proof, trace, runner.checker)?;
    result.primitive_steps = expanded;
    result.payload = Some(Box::new(
        if matches!(&result.witness.judgment().statement,
        crate::proof_model::Statement::Distribution(value) if matches!(value.as_ref(), DistributionStatement::Pairing { .. }))
        {
            crate::derivative_model::ResultPayload::PairingScalar(vec![result.witness.clone()])
        } else {
            crate::derivative_model::ResultPayload::Distribution(vec![result.witness.clone()])
        },
    ));
    Ok(result)
}

/// Resolve the exact space from intent or from an already typed distribution input.
fn request_space(cas: &Cas, task: &Task, inputs: &[LogicalId]) -> Result<Option<Handle>, Fault> {
    match task {
        Task::EmbedRegular { space }
        | Task::Pair { space, .. }
        | Task::SmoothMultiply { space } => Ok(Some(space.clone())),
        _ => {
            for input in inputs {
                if let Some(space) = cas.state.expression(*input)?.space {
                    return Ok(Some(cas.handle(space)));
                }
            }
            Ok(None)
        }
    }
}

/// Execute one typed recipe; unsupported analytic evidence stays pending.
fn run(
    runner: &mut Runner<'_>,
    task: &Task,
    inputs: &[Handle],
    named: bool,
    known: &[crate::distribution_known::Known],
) -> Result<(Handle, Handle), Fault> {
    let first = inputs.first().ok_or_else(invalid)?;
    match task {
        Task::EmbedRegular { .. } => runner.embed(first),
        Task::Differentiate { order, .. } => {
            if named {
                runner.named_route(first, *order, known)
            } else {
                runner.differentiate(first, *order)
            }
        }
        Task::Pair { action_mode, .. } => {
            let test = inputs.get(1).ok_or_else(invalid)?;
            if *action_mode == crate::task::ActionMode::Test
                && matches!(
                    runner.cas.state.expression(first.logical_id())?.operator,
                    Operator::DistributionDerivative { order: 1 }
                )
            {
                runner.integration_by_parts(first, test)
            } else {
                runner.pair(first, test, action_mode)
            }
        }
        Task::SmoothMultiply { .. } | Task::Product { .. } => {
            let second = inputs.get(1).ok_or_else(invalid)?;
            if runner.cas.state.expression(first.logical_id())?.sort == Sort::Distribution {
                runner.multiply(second, first)
            } else {
                runner.multiply(first, second)
            }
        }
        Task::Simplify => simplify(runner, first, named, known),
        Task::Equal { .. } => equal(runner, first, inputs.get(1).ok_or_else(invalid)?),
        _ => Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT)),
    }
}

/// Formal derivative syntax is expanded only through its checked typed derivative theorem.
fn simplify(
    runner: &mut Runner<'_>,
    input: &Handle,
    named: bool,
    known: &[crate::distribution_known::Known],
) -> Result<(Handle, Handle), Fault> {
    let value = runner.cas.state.expression(input.logical_id())?.clone();
    if let Operator::DistributionDerivative { order } = value.operator {
        let source = runner
            .cas
            .handle(*value.operands.first().ok_or_else(invalid)?);
        let (output, derivative) = if named {
            runner.named_route(&source, order, known)?
        } else {
            runner.differentiate(&source, order)?
        };
        let proof = runner.push(
            Primitive::DistDerivative,
            DistributionStatement::Equality {
                left: input.clone(),
                right: output.clone(),
                space: runner.space.clone(),
            },
            &[derivative],
            Certificate::None,
        )?;
        return Ok((output, proof));
    }
    let (output, premises) = reduce(runner, input)?;
    let proof = runner.push(
        Primitive::DistLinear,
        DistributionStatement::Equality {
            left: input.clone(),
            right: output.clone(),
            space: runner.space.clone(),
        },
        &premises,
        Certificate::None,
    )?;
    Ok((output, proof))
}

/// Reduce nested formal derivatives before collecting the enclosing rational coefficient map.
fn reduce(runner: &mut Runner<'_>, input: &Handle) -> Result<(Handle, Vec<Handle>), Fault> {
    runner.ensure(input)?;
    let mut premises = runner.premises.clone();
    let order = crate::case_syntax::order(runner.cas, &[input.logical_id()], runner.generation)?;
    for id in order {
        let value = runner.cas.state.expression(id)?.clone();
        match value.operator {
            Operator::DistributionDerivative { .. } => {
                let term = runner.cas.handle(id);
                premises.push(simplify(runner, &term, false, &[])?.1);
            }
            Operator::SmoothMultiply => {
                let function = runner
                    .cas
                    .handle(*value.operands.first().ok_or_else(invalid)?);
                let distribution = runner
                    .cas
                    .handle(*value.operands.get(1).ok_or_else(invalid)?);
                premises.push(runner.multiply(&function, &distribution)?.1);
            }
            _ => {}
        }
    }
    let earlier = runner.applications(&premises)?;
    let terms = crate::distribution_linear::terms(
        runner.cas,
        input.logical_id(),
        &earlier,
        runner.generation,
    )?;
    let output = crate::distribution_terms::linear(
        runner.cas,
        &runner.header,
        &runner.space,
        &terms,
        false,
        runner.generation,
    )?;
    Ok((output, premises))
}

/// Equality of regular representatives uses finite exceptions; other equalities stay structural.
fn equal(
    runner: &mut Runner<'_>,
    left: &Handle,
    right: &Handle,
) -> Result<(Handle, Handle), Fault> {
    runner.ensure(left)?;
    runner.ensure(right)?;
    let mut premises = runner.premises.clone();
    let both_regular = [left, right].iter().all(|value| {
        runner
            .cas
            .state
            .expression(value.logical_id())
            .is_ok_and(|expression| expression.operator == Operator::RegularEmbed)
    });
    let (rule, certificate) = if both_regular {
        let first = crate::distribution_ae::representative(runner.cas, left)?;
        let second = crate::distribution_ae::representative(runner.cas, right)?;
        let certificate = crate::distribution_ae::certificate(
            runner.cas,
            &first,
            &second,
            &runner.space,
            runner.generation,
        )?;
        (
            Primitive::DistAe,
            Certificate::Distribution(Box::new(certificate)),
        )
    } else {
        premises.push(simplify(runner, left, false, &[])?.1);
        premises.push(simplify(runner, right, false, &[])?.1);
        (Primitive::DistLinear, Certificate::None)
    };
    let proof = runner.push(
        rule,
        DistributionStatement::Equality {
            left: left.clone(),
            right: right.clone(),
            space: runner.space.clone(),
        },
        &premises,
        certificate,
    )?;
    Ok((right.clone(), proof))
}

/// Preserve the public named refusal rather than collapsing it into a sort error.
pub(crate) const fn refusal(fault: Fault) -> RefusalReason {
    match fault.kind {
        FaultKind::Invalid(InvalidCode::DistributionPointValue) => {
            RefusalReason::DistributionPointValue
        }
        FaultKind::Invalid(InvalidCode::DistributionProductUndefined) => {
            RefusalReason::DistributionProductUndefined
        }
        FaultKind::Invalid(InvalidCode::TestSpaceMismatch) => RefusalReason::TestSpaceMismatch,
        FaultKind::Invalid(InvalidCode::EmptyDomain) => RefusalReason::EmptyDomain,
        FaultKind::Invalid(InvalidCode::OverlapConflict) => RefusalReason::OverlapConflict,
        FaultKind::Invalid(InvalidCode::CoverageHole) => RefusalReason::CoverageHole,
        _ => RefusalReason::RefutedDomain,
    }
}
