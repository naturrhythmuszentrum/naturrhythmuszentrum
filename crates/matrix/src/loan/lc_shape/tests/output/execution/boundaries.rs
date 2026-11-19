// SPDX-License-Identifier: AGPL-3.0-only
//! Literal size and coverage neighbours retain one original cumulative ledger.
use super::{Scalar, TestResult, checks, demand, failure, request, setup};
use crate::{LcApplicationEvaluationFailure, LcOmegaOutput, LcProductError, LoanError};
use nrz_bindings::Limits;

pub(super) fn work<P: Scalar + 'static>() -> TestResult {
    for (counts, full, ready, evaluation) in [
        ([1, 1, 2], false, 179, 15),
        ([1, 1, 2], true, 219, 123),
        ([2, 3, 2], false, 189, 25),
        ([2, 3, 2], true, 429, 673),
    ] {
        for short in [false, true] {
            let mut s = setup::<P>(Limits::default(), counts)?;
            let output = LcOmegaOutput::declare(
                &s.inventory,
                request(&s.registry, &s.radian),
                &mut s.account,
            )?;
            let members = super::super::super::inventory::readers::publish(
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
            let selected: Vec<_> = if full {
                (0..s.axes.combined.axis().capacity()).collect()
            } else {
                vec![]
            };
            let mask = demand(&mut s.heap, &s.axes, &selected, &mut s.account)?;
            let pending = prepared
                .admit(&mut s.heap, mask, &mut s.account)
                .map_err(|f| f.into_parts().1)?;
            let consume = s.account.limits().work - s.account.usage().work - ready - evaluation
                + u64::from(short);
            s.account.ledger.work(consume, crate::loan::AT)?;
            let before = s.account.usage().work;
            crate::loan::fault::reset_reads();
            let injection = crate::loan::fault::Injection::after(0);
            let result = pending.evaluate(&mut s.account);
            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
            drop(injection);
            if short {
                let Err(LcApplicationEvaluationFailure::Pending {
                    mut owner,
                    error: LcProductError::Loan(LoanError::Budget(_)),
                }) = result
                else {
                    return Err("one-below evaluation".into());
                };
                assert_eq!(s.account.usage().work - before, ready + 12);
                assert_eq!(crate::loan::fault::reads(), 0);
                owner.cancel(&mut s.heap, &mut s.account)?;
            } else {
                let mut candidate = result.map_err(failure)?;
                assert_eq!(s.account.usage().work - before, ready + evaluation);
                assert_eq!(crate::loan::fault::reads(), 15 * selected.len());
                candidate.cancel(&mut s.heap, &mut s.account)?;
            }
        }
    }
    Ok(())
}

pub(super) fn holes<P: Scalar + 'static>() -> TestResult {
    for missing in 0..5 {
        for cell in [0, 11] {
            let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
            let output = LcOmegaOutput::declare(
                &s.inventory,
                request(&s.registry, &s.radian),
                &mut s.account,
            )?;
            let mut masks = std::array::from_fn(|_| vec![0, 3, 11]);
            masks[missing].retain(|selected| *selected != cell);
            let values = [[[P::ONE, P::ZERO, P::ZERO]; 2]; 5];
            let members = super::super::super::inventory::readers::cells::publish_values(
                &s.inventory,
                &s.axes,
                &mut s.heap,
                &mut s.account,
                &masks,
                1,
                &values,
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
            for _ in 0..2 {
                crate::loan::fault::reset_reads();
                crate::loan::fault::reset_scalars();
                let before = s.account.usage().work;
                let Err(LcApplicationEvaluationFailure::Pending {
                    owner,
                    error: LcProductError::Loan(LoanError::CoverageMismatch),
                }) = pending.evaluate(&mut s.account)
                else {
                    return Err("captured hole".into());
                };
                assert_eq!(s.account.usage().work - before, 249);
                assert_eq!(
                    [crate::loan::fault::reads(), crate::loan::fault::scalars()],
                    [0; 2]
                );
                crate::loan::lc_origin::pending::assert_private_demand_0_3_11(&owner);
                pending = owner;
            }
            pending.cancel(&mut s.heap, &mut s.account)?;
        }
    }
    Ok(())
}

pub(super) fn failed<P: Scalar + 'static>() -> TestResult {
    let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
    let output = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    let members = super::super::super::inventory::readers::publish(
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
    checks::failed(pending, &mut s.heap, &mut s.account)
}
