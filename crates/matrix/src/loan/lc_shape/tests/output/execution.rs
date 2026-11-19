// SPDX-License-Identifier: AGPL-3.0-only
//! Original-ledger execution controls preserve captured authority and independent image oracles.
mod boundaries;
mod cases;
pub(super) mod lifetime;
mod oracle;
use super::{TestResult, demand, request, setup};
use crate::loan::lc_origin::pending::execution::checks;
use crate::{LcApplicationEvaluationFailure, LcOmegaOutput, LcProductError, LoanError};
use nrz_bindings::Limits;
use nrz_scalar::Scalar;

pub(super) fn images<P: Scalar + 'static>() -> TestResult {
    cases::run::<P>()?;
    for positions in [[0, 1], [4, 1]] {
        for selected in [vec![], vec![0, 3, 11], (0..12).collect()] {
            let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
            let mut declaration = request(&s.registry, &s.radian);
            declaration.positions = positions;
            let output = LcOmegaOutput::declare(&s.inventory, declaration, &mut s.account)?;
            let members = super::super::inventory::readers::publish(
                &s.inventory,
                &s.axes,
                &mut s.heap,
                &mut s.account,
                2,
                1,
            )?;
            let prepared = output
                .prepare(
                    &mut s.heap,
                    &members.references(),
                    &s.registry,
                    &mut s.account,
                )
                .map_err(|f| f.into_parts().1)?;
            let expected = oracle::captured(&prepared, &selected, positions, &mut s.account)?;
            let mask = demand(&mut s.heap, &s.axes, &selected, &mut s.account)?;
            let pending = prepared
                .admit(&mut s.heap, mask, &mut s.account)
                .map_err(|f| f.into_parts().1)?;
            let ticket = pending.ticket();
            let layouts = members.detach();
            crate::loan::fault::reset_reads();
            crate::loan::fault::reset_scalars();
            let injection = crate::loan::fault::Injection::after(0);
            let before = s.account.usage().work;
            let mut candidate = pending.evaluate(&mut s.account).map_err(failure)?;
            assert_eq!(candidate.ticket(), ticket);
            let d = u64::try_from(selected.len())?;
            assert_eq!(s.account.usage().work - before, 214 + 74 * d);
            assert_eq!(
                [
                    crate::loan::fault::reads(),
                    crate::loan::fault::scalars(),
                    crate::loan::fault::Injection::attempts()
                ],
                [15 * selected.len(), 3 * selected.len(), 0]
            );
            drop(injection);
            checks::cells(&candidate, &expected)?;
            let before = s.account.usage().work;
            candidate.cancel(&mut s.heap, &mut s.account)?;
            assert_eq!(s.account.usage().work, before);
            drop(candidate);
            s.heap.retire_mask(mask, &mut s.account.ledger)?;
            super::fixture::retire_layout(&mut s.heap, &mut s.account, mask.layout())?;
            for layout in layouts {
                super::fixture::retire_layout(&mut s.heap, &mut s.account, layout)?;
            }
            super::fixture::empty(&mut s.heap, &mut s.account)?;
        }
    }
    Ok(())
}
fn failure<P: Scalar, const N: usize>(
    failure: LcApplicationEvaluationFailure<'_, P, N>,
) -> LcProductError {
    match failure {
        LcApplicationEvaluationFailure::Pending { error, .. }
        | LcApplicationEvaluationFailure::Failed { error, .. } => error,
    }
}

pub(super) fn work<P: Scalar + 'static>() -> TestResult {
    boundaries::work::<P>()?;
    for (remaining, spent, complete) in [
        (10, 0, false),
        (11, 11, false),
        (248, 12, false),
        (249, 249, false),
        (260, 249, false),
        (435, 261, false),
        (436, 436, true),
    ] {
        let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
        let output = LcOmegaOutput::declare(
            &s.inventory,
            request(&s.registry, &s.radian),
            &mut s.account,
        )?;
        let members = super::super::inventory::readers::publish(
            &s.inventory,
            &s.axes,
            &mut s.heap,
            &mut s.account,
            2,
            1,
        )?;
        let prepared = output
            .prepare(
                &mut s.heap,
                &members.references(),
                &s.registry,
                &mut s.account,
            )
            .map_err(|f| f.into_parts().1)?;
        let mask = demand(&mut s.heap, &s.axes, &[0, 3, 11], &mut s.account)?;
        let pending = prepared
            .admit(&mut s.heap, mask, &mut s.account)
            .map_err(|f| f.into_parts().1)?;
        let ticket = pending.ticket();
        let consume = s.account.limits().work - s.account.usage().work - remaining;
        s.account.ledger.work(consume, crate::loan::AT)?;
        let before = s.account.usage().work;
        crate::loan::fault::reset_reads();
        let injection = crate::loan::fault::Injection::after(0);
        let result = pending.evaluate(&mut s.account);
        assert_eq!(s.account.usage().work - before, spent);
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        drop(injection);
        if complete {
            let mut candidate = result.map_err(failure)?;
            assert_eq!(candidate.ticket(), ticket);
            assert_eq!(crate::loan::fault::reads(), 45);
            candidate.cancel(&mut s.heap, &mut s.account)?;
        } else {
            let Err(LcApplicationEvaluationFailure::Pending {
                mut owner,
                error: LcProductError::Loan(LoanError::Budget(_)),
            }) = result
            else {
                return Err("expected untouched pending budget refusal".into());
            };
            assert_eq!(owner.ticket(), ticket);
            assert_eq!(crate::loan::fault::reads(), 0);
            crate::loan::lc_origin::pending::assert_private_demand_0_3_11(&owner);
            owner.cancel(&mut s.heap, &mut s.account)?;
        }
        assert_eq!(s.account.usage().work - before, spent);
    }
    Ok(())
}

pub(super) fn refusals<P: Scalar + 'static>() -> TestResult {
    boundaries::holes::<P>()?;
    boundaries::failed::<P>()?;
    for mode in [0, 1] {
        let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
        let output = LcOmegaOutput::declare(
            &s.inventory,
            request(&s.registry, &s.radian),
            &mut s.account,
        )?;
        let members = super::super::inventory::readers::publish(
            &s.inventory,
            &s.axes,
            &mut s.heap,
            &mut s.account,
            mode,
            1,
        )?;
        let prepared = output
            .prepare(
                &mut s.heap,
                &members.references(),
                &s.registry,
                &mut s.account,
            )
            .map_err(|f| f.into_parts().1)?;
        let mask = demand(&mut s.heap, &s.axes, &[0, 3, 11], &mut s.account)?;
        let mut pending = prepared
            .admit(&mut s.heap, mask, &mut s.account)
            .map_err(|f| f.into_parts().1)?;
        let ticket = pending.ticket();
        let mut foreign =
            crate::ExecutionAccount::new(nrz_bindings::Budget::new(Limits::default())?)?;
        let before = foreign.usage().work;
        let Err(LcApplicationEvaluationFailure::Pending {
            owner,
            error: LcProductError::Loan(LoanError::ForeignAccount),
        }) = pending.evaluate(&mut foreign)
        else {
            return Err("foreign evaluation".into());
        };
        assert_eq!(foreign.usage().work, before);
        pending = owner;
        for _ in 0..2 {
            let before = s.account.usage().work;
            crate::loan::fault::reset_reads();
            let Err(LcApplicationEvaluationFailure::Pending {
                owner,
                error: LcProductError::Loan(LoanError::CoverageMismatch),
            }) = pending.evaluate(&mut s.account)
            else {
                return Err("missing captured coverage".into());
            };
            assert_eq!(s.account.usage().work - before, 249);
            assert_eq!(crate::loan::fault::reads(), 0);
            assert_eq!(owner.ticket(), ticket);
            crate::loan::lc_origin::pending::assert_private_demand_0_3_11(&owner);
            pending = owner;
        }
        pending.cancel(&mut s.heap, &mut s.account)?;
    }
    Ok(())
}

pub(super) fn receipts<P: Scalar + 'static>() -> TestResult {
    for enabled in [false, true] {
        let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
        let output = LcOmegaOutput::declare(
            &s.inventory,
            request(&s.registry, &s.radian),
            &mut s.account,
        )?;
        let members = super::super::inventory::readers::publish(
            &s.inventory,
            &s.axes,
            &mut s.heap,
            &mut s.account,
            2,
            1,
        )?;
        let prepared = output
            .prepare(
                &mut s.heap,
                &members.references(),
                &s.registry,
                &mut s.account,
            )
            .map_err(|f| f.into_parts().1)?;
        let expected = oracle::captured(&prepared, &[0, 3, 11], [0, 1], &mut s.account)?;
        let mask = demand(&mut s.heap, &s.axes, &[0, 3, 11], &mut s.account)?;
        let pending = prepared
            .admit(&mut s.heap, mask, &mut s.account)
            .map_err(|f| f.into_parts().1)?;
        let application = pending.ticket().identity.request;
        let before = s.account.usage().work;
        let injection = crate::loan::fault::Injection::after(0);
        crate::loan::fault::reset_reads();
        crate::loan::fault::reset_scalars();
        let result = if enabled {
            super::pending::receipts::traversal([application, before, 249, 187, 3], || {
                pending.evaluate(&mut s.account).map_err(failure)
            })
        } else {
            tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
                pending.evaluate(&mut s.account).map_err(failure)
            })
        };
        let mut candidate = result?;
        assert_eq!(s.account.usage().work - before, 436);
        assert_eq!(
            [
                crate::loan::fault::reads(),
                crate::loan::fault::scalars(),
                crate::loan::fault::Injection::attempts()
            ],
            [45, 9, 0]
        );
        drop(injection);
        checks::cells(&candidate, &expected)?;
        candidate.cancel(&mut s.heap, &mut s.account)?;
    }
    Ok(())
}
