// SPDX-License-Identifier: AGPL-3.0-only
//! Complete traversal prepayment leaves no resource suspension point after the first store.

use super::member::Member;
use super::{
    ApplicationCore, ApplicationError, ApplicationEvaluation, CellOutcome, OwnedApplication,
};
use crate::KernelError;
use crate::loan::{ExecutionAccount, LoanError, charge, payload::set};
use nrz_scalar::{ArithmeticContext, Scalar};
mod copy;
mod selected;

/// Single-use private traversal authority; only whole-phase prepayment constructs it.
pub(in crate::loan) struct Traversal {
    /// Admitted arithmetic context is retained throughout the exact traversal.
    context: ArithmeticContext,
    /// Selected native observation boundary, with no additional exact Work.
    native_phase: Option<&'static str>,
}
/// Authenticate phase and all fixed metadata before spending the traversal allowance.
pub(super) fn evaluate<P: Scalar + 'static>(
    mut owner: OwnedApplication<P>,
    account: &mut ExecutionAccount,
) -> ApplicationEvaluation<P> {
    if let Err(error) = account.authenticate(owner.core.host.identity.account) {
        return ApplicationEvaluation::ApplicationRejected(error.into(), owner);
    }
    if owner.core.candidate {
        return ApplicationEvaluation::ApplicationRejected(
            LoanError::UnsupportedContract.into(),
            owner,
        );
    }
    let prepared =
        account
            .ledger
            .with_work_phase(super::FAMILY, "pointwise evaluation", |budget| {
                charge(budget, 1)?;
                if owner.core.evidence.prepared.is_some() {
                    charge(budget, 1)?;
                    owner
                        .core
                        .host
                        .retained_law
                        .as_ref()
                        .ok_or(LoanError::WrongLoan)?
                        .validate()?;
                    charge(budget, 1)?;
                }
                if owner.core.evidence.endpoint_tokens.binary().is_some() {
                    charge(budget, 1)?;
                }
                let (member, seals) = validate(&owner.core, budget)?;
                let context = ArithmeticContext::new()
                    .map_err(KernelError::from)
                    .map_err(crate::OperationError::from)?;
                if context.revision() != 1 {
                    return Err(ApplicationError::Operation(KernelError::Contract.into()));
                }
                let work = traversal_work(&owner.core, member)?;
                charge(
                    budget,
                    work.checked_add(seals).ok_or(LoanError::CountOverflow)?,
                )?;
                Ok::<_, ApplicationError>(Traversal {
                    context,
                    native_phase: member.native_phase(),
                })
            });
    let permit = match prepared {
        Ok(permit) => permit,
        Err(error) => return ApplicationEvaluation::ApplicationNotStarted(error, owner),
    };
    if let Some(phase) = permit.native_phase {
        let result = account
            .ledger
            .with_work_phase(super::FAMILY, phase, |_| run(&mut owner.core, &permit));
        if let Err(error) = result {
            return ApplicationEvaluation::ApplicationRejected(error.into(), owner);
        }
    } else if let Err(error) = run(&mut owner.core, &permit) {
        return ApplicationEvaluation::ApplicationRejected(error.into(), owner);
    }
    owner.core.evidence.revision = permit.context.revision();
    owner.core.candidate = true;
    ApplicationEvaluation::ApplicationCandidate(owner)
}
/// Complete selected traversal prepayment uses the unchanged member and per-role tariffs.
fn traversal_work<P: Scalar>(
    core: &ApplicationCore<P>,
    member: Member<'_>,
) -> Result<usize, LoanError> {
    let components = core.host.declaration.extent.columns();
    let inherited_error = member.is_transform() && core.evidence.source.symbols.error().is_some();
    let loads = 1
        + usize::from(inherited_error)
        + usize::from(core.evidence.source.symbols.bound().is_some())
        + core.evidence.endpoint_tokens.binary().map_or(0, |side| {
            1 + usize::from(side.source.symbols.bound().is_some())
        });
    // Two scalar-coordinate projections per output component are prepaid for tile broadcast.
    let projections = core.evidence.endpoint_tokens.binary().map_or(0, |side| {
        let left = core.evidence.source.extent != core.host.declaration.extent;
        let right = side.source.extent != core.host.declaration.extent;
        if member.cartesian().is_none() && (left || right) {
            2
        } else {
            0
        }
    });
    let invocation = member.invocation(components)?;
    let per_cell = (loads + member.roles() + projections)
        .checked_mul(components)
        .and_then(|steps| steps.checked_add(invocation))
        .and_then(|steps| steps.checked_add(2 + 2 * usize::from(member.cartesian().is_some())))
        .ok_or(LoanError::CountOverflow)?;
    core.population
        .checked_mul(per_cell)
        .and_then(|steps| {
            steps.checked_add(if member.is_projected() {
                3
            } else if member.cartesian().is_some() {
                6
            } else {
                0
            })
        })
        .and_then(|steps| {
            core.filled
                .len()
                .checked_mul(2)
                .and_then(|words| steps.checked_add(words))
        })
        .ok_or(LoanError::CountOverflow)
}
/// Constant-size sealed law and owner checks require no selected numerical access.
fn validate<'a, P: Scalar + 'static>(
    core: &'a ApplicationCore<P>,
    budget: &mut nrz_bindings::Budget,
) -> Result<(Member<'a>, usize), LoanError> {
    core.evidence
        .validate_revision(&core.host.declaration, core.host.identity, 0)?;
    super::endpoints::validate_core(core)?;
    let member = Member::check(
        &core.evidence.operation,
        core.evidence.law.kind(),
        core.evidence.prepared.as_ref(),
    )?;
    member.validate_cell(&core.contract, core.host.declaration.extent.columns())?;
    let input = core.input.as_ref().ok_or(LoanError::WrongLoan)?;
    if core.filled.len() != crate::loan::words(core.host.declaration.extent.rows()) {
        return Err(LoanError::WrongLoan);
    }
    super::chain::captured(input, &core.evidence, core.host.identity.account)?;
    let seals = super::chain::seal_work(input)?;
    charge(budget, seals)?;
    input.checked_reader()?;
    if let Some(side) = core.evidence.endpoint_tokens.binary() {
        side.validate_pin::<P>(core.host.identity.account)?;
    }
    Ok((member, seals))
}
/// Enumerate words, cells and components in stable original order, including refused suffixes.
fn run<P: Scalar + 'static>(
    core: &mut ApplicationCore<P>,
    permit: &Traversal,
) -> Result<(), LoanError> {
    let member = Member::check(
        &core.evidence.operation,
        core.evidence.law.kind(),
        core.evidence.prepared.as_ref(),
    )?;
    let cells = core.host.declaration.extent.rows();
    let components = core.host.declaration.extent.columns();
    let reader = core
        .input
        .as_ref()
        .and_then(|pin| pin.checked_reader().ok());
    let copy = copy::bind(
        reader.as_ref(),
        member.is_projected() || member.cartesian().is_some(),
    )?;
    let projection = if member.is_projected() {
        Some(member.map()?)
    } else {
        None
    };
    let side = core.evidence.endpoint_tokens.binary();
    let right = side
        .and_then(|side| side.input.as_ref())
        .and_then(|pin| pin.checked_reader::<P>().ok());
    let right_copy = copy::bind(right.as_ref(), member.cartesian().is_some())?;
    let product = member.cartesian();
    for word_index in 0..core.filled.len() {
        let mut remaining = core.filled[word_index];
        while remaining != 0 {
            let cell = word_index * 64 + remaining.trailing_zeros() as usize;
            remaining &= remaining - 1;
            let original = match projection {
                Some(map) => super::movement::source_cell(map, cell)?,
                None => cell,
            };
            let source_cells = match product {
                Some(product) => product.coordinates(cell)?,
                None => [original; 2],
            };
            let mut first = None;
            for component in 0..components {
                let entry = cell + cells * component;
                let result = if let (Some(left), Some(right)) = (&copy, &right_copy) {
                    copy::binary(member, left, right, source_cells, component, entry, permit)
                } else if let Some(copy) = &copy {
                    copy.evaluate(member, original, component, entry, permit)
                } else {
                    member.scalar(
                        reader.as_ref(),
                        right.as_ref(),
                        side.is_some_and(|side| side.source.symbols.bound().is_some()),
                        core.host.retained_law.as_ref(),
                        core.evidence.source.symbols.bound().is_some(),
                        source_cells,
                        component,
                        entry,
                        permit,
                    )
                };
                store_result(
                    result,
                    member,
                    component,
                    entry,
                    &mut first,
                    &mut core.host.channels,
                    &mut core.endpoints,
                );
            }
            if let Some(refused) = first {
                core.evidence.outcomes[cell] = refused;
                set(&mut core.filled, cell, false);
            } else {
                core.evidence.outcomes[cell] = CellOutcome::Completed;
            }
        }
    }
    Ok(())
}

/// Store all admitted roles even after a refusal, preserving the first failing component.
fn store_result<P: Scalar + 'static>(
    result: Result<selected::Values<P>, CellOutcome>,
    member: Member<'_>,
    component: usize,
    entry: usize,
    first: &mut Option<CellOutcome>,
    channels: &mut [Option<crate::loan::payload::LoanChannel>; 3],
    endpoints: &mut Option<[crate::loan::payload::LoanChannel; 2]>,
) {
    let values = match result {
        Ok(values) => values,
        Err(refused) => {
            if first.is_none() {
                *first = Some(refused);
            }
            member.zeros()
        }
    };
    if values.store(channels, endpoints, entry) && first.is_none() {
        *first = Some(member.storage_failure(component));
    }
}
