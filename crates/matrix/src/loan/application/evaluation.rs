// SPDX-License-Identifier: AGPL-3.0-only
//! Complete traversal prepayment leaves no resource suspension point after the first store.

use super::{
    ApplicationCore, ApplicationError, ApplicationEvaluation, CellOutcome, OwnedApplication,
};
use crate::loan::{ExecutionAccount, HostChannel, LoanError, charge, payload::set};
use crate::{Elementwise, KernelError, kernels::arithmetic::Entry};
use nrz_scalar::{ArithmeticContext, Scalar};
mod prepared;

/// Single-use private traversal authority; only whole-phase prepayment constructs it.
pub(in crate::loan) struct Traversal {
    /// Admitted arithmetic context is retained throughout the exact traversal.
    context: ArithmeticContext,
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
                validate(&owner.core)?;
                let context = ArithmeticContext::new()
                    .map_err(KernelError::from)
                    .map_err(crate::OperationError::from)?;
                if context.revision() != 1 {
                    return Err(ApplicationError::Operation(KernelError::Contract.into()));
                }
                let components = owner.core.host.declaration.extent.columns();
                let loads = if owner.core.evidence.source.symbols.bound().is_some() {
                    2
                } else {
                    1
                };
                let per_cell = (if owner.core.evidence.prepared.is_some() {
                    6_usize
                } else {
                    15_usize + loads
                })
                .checked_mul(components)
                .and_then(|steps| steps.checked_add(2))
                .ok_or(LoanError::CountOverflow)?;
                let work = owner
                    .core
                    .population
                    .checked_mul(per_cell)
                    .and_then(|steps| {
                        owner
                            .core
                            .filled
                            .len()
                            .checked_mul(2)
                            .and_then(|words| steps.checked_add(words))
                    })
                    .ok_or(LoanError::CountOverflow)?;
                charge(budget, work)?;
                Ok::<_, ApplicationError>(Traversal { context })
            });
    let permit = match prepared {
        Ok(permit) => permit,
        Err(error) => return ApplicationEvaluation::ApplicationNotStarted(error, owner),
    };
    if owner.core.evidence.prepared.is_some() {
        account
            .ledger
            .with_work_phase(super::FAMILY, "prepared native traversal", |_| {
                prepared::run(&mut owner.core, &permit);
                Ok::<(), std::convert::Infallible>(())
            })
            .unwrap_or_else(|never| match never {});
    } else {
        run(&mut owner.core, &permit);
    }
    owner.core.evidence.revision = permit.context.revision();
    owner.core.candidate = true;
    ApplicationEvaluation::ApplicationCandidate(owner)
}
/// Constant-size sealed law and owner checks require no selected numerical access.
fn validate<P: Scalar + 'static>(core: &ApplicationCore<P>) -> Result<(), LoanError> {
    core.evidence
        .validate_revision(&core.host.declaration, core.host.identity, 0)?;
    super::endpoints::validate_core(core)?;
    let prepared = core.evidence.prepared.is_some();
    let extent = crate::Extent::new(1, core.host.declaration.extent.columns())?;
    let law = &core.contract;
    if law.inputs.as_ref() != [extent]
        || law.output != extent
        || law.work
            != extent
                .entries()
                .checked_mul(if prepared { 0 } else { 12 })
                .ok_or(LoanError::CountOverflow)?
        || !(if prepared {
            matches!(law.recipe, crate::kernels::contract::Recipe::PreparedExpm1)
        } else {
            matches!(
                law.recipe,
                crate::kernels::contract::Recipe::Elementwise(Elementwise::Square)
            )
        })
    {
        return Err(LoanError::WrongLoan);
    }
    let input = core.input.as_ref().ok_or(LoanError::WrongLoan)?;
    if input.ticket()?.identity != core.evidence.source_identity
        || core.filled.len() != crate::loan::words(core.host.declaration.extent.rows())
    {
        return Err(LoanError::WrongLoan);
    }
    input.checked_reader()?;
    Ok(())
}
/// Enumerate words, cells and components in stable original order, including refused suffixes.
fn run<P: Scalar + 'static>(core: &mut ApplicationCore<P>, permit: &Traversal) {
    let cells = core.host.declaration.extent.rows();
    let components = core.host.declaration.extent.columns();
    let reader = core
        .input
        .as_ref()
        .and_then(|pin| pin.checked_reader().ok());
    for word_index in 0..core.filled.len() {
        let mut remaining = core.filled[word_index];
        while remaining != 0 {
            let offset = remaining.trailing_zeros() as usize;
            remaining &= remaining - 1;
            let cell = word_index * 64 + offset;
            let mut first = None;
            for component in 0..components {
                let entry = cell + cells * component;
                let result = reader
                    .as_ref()
                    .ok_or(KernelError::Contract)
                    .and_then(|reader| {
                        scalar(
                            reader,
                            core.evidence.source.symbols.bound().is_some(),
                            cell,
                            component,
                            entry,
                            permit,
                        )
                    });
                let values = match result {
                    Ok(result) => [result.center, result.total, result.error],
                    Err(reason) => {
                        if first.is_none() {
                            first = Some(CellOutcome::CellRefused { component, reason });
                        }
                        [P::ZERO; 3]
                    }
                };
                for (channel, value) in core.host.channels.iter_mut().zip(values) {
                    if let Some(channel) = channel {
                        if channel.matrix.write(entry, value).is_err() && first.is_none() {
                            first = Some(CellOutcome::CellRefused {
                                component,
                                reason: KernelError::Contract,
                            });
                        }
                        set(&mut channel.components, entry, true);
                    }
                }
            }
            if let Some(refused) = first {
                core.evidence.outcomes[cell] = refused;
                set(&mut core.filled, cell, false);
            } else {
                core.evidence.outcomes[cell] = CellOutcome::Completed;
            }
        }
    }
}
/// Exactly one center and optional total load precede each shared scalar-entry invocation.
fn scalar<P: Scalar + 'static>(
    reader: &crate::loan::TileReader<'_, P>,
    bounded: bool,
    cell: usize,
    component: usize,
    entry: usize,
    permit: &Traversal,
) -> Result<Entry<P>, KernelError> {
    let center = reader
        .read_prepaid(HostChannel::Center, cell, component, permit)
        .map_err(|_| KernelError::Contract)?;
    let total = if bounded {
        reader
            .read_prepaid(HostChannel::Total, cell, component, permit)
            .map_err(|_| KernelError::Contract)?
    } else {
        P::ZERO
    };
    let radius = nrz_accuracy::Nonnegative::new(total)?;
    crate::kernels::nonlinear::evaluate_selected(
        center,
        radius,
        Elementwise::Square,
        entry,
        &permit.context,
    )
}
