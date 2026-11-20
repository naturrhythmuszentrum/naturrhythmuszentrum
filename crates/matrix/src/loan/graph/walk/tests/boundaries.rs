// SPDX-License-Identifier: AGPL-3.0-only
//! Independent literal histories use the immutable ledger at exact and one-below limits.

use super::{TestResult, account, graph};
use crate::ExecutionAccount;
use crate::loan::graph::walk::{Error, INVALIDATING, READY};
use nrz_bindings::{Budget, Limits};

/// C=4097, V=3, E=2, Q=65+2+1=68, h=3; the original owner recipe is unchanged.
pub(super) fn invalidation() -> TestResult {
    // O: H=2, N=4103, E=2, I=12493, P=9; total 16609.
    const RESERVE: u64 = 16_609;
    // W: H=2, N=8214, E=8, X=6, M=71, P=4146; total 12447.
    // N = contracts3 + slots4097 + incidence4 + DFS roots4097 + endpoints2
    //     + source cursors2 + pops3 + readiness inspection3 + publication3 = 8214.
    // E = proposed2 + initialization2 + DFS2 + cleanup inventory2 = 8.
    // P = teardown4097+2 + nodes3*(3+2*3) + edges2*(4+2*3), L=0 = 4146.
    const ADMIT: u64 = 12_447;
    // Nonzero prior5, O, W, handle H+N=2, invalidation H2+A1+N1+M4+X1=9.
    const TOTAL: u64 = 5 + RESERVE + ADMIT + 2 + 9;
    for short in [0, 1] {
        let mut budget = Budget::new(Limits {
            work: TOTAL - short,
            ..Limits::default()
        })?;
        budget.work(5, crate::loan::AT)?;
        let mut account = ExecutionAccount::new(budget)?;
        let mut core = graph(
            4097,
            &[8, 8, 8],
            &[super::edge(0, 1, 0), super::edge(1, 2, 1)],
            &mut account,
        )?;
        assert_eq!(account.usage().work, 5 + RESERVE + ADMIT);
        // Original header P=1 plus C=4097 inspections and V*9; D=2 plus E*10.
        assert_eq!(core.reservation.backing.cleanup.records, 4125);
        assert_eq!(core.reservation.backing.cleanup.edges, 22);
        let root = core.handle(0, &mut account)?;
        let result = core.invalidate(root, &mut account);
        if short == 0 {
            result?;
            assert_eq!(account.usage().work, TOTAL);
            assert_ne!(core.reservation.backing.control.flags & INVALIDATING, 0);
        } else {
            assert!(matches!(
                result,
                Err(Error::Native(crate::LoanError::Budget(_)))
            ));
            // H/A/N are spent; the six-unit atomic commit prepayment refuses unchanged.
            assert_eq!(account.usage().work, TOTAL - 6);
            assert_eq!(core.reservation.backing.control.flags & INVALIDATING, 0);
            assert_eq!(core.reservation.backing.marks[0], 0);
            assert_ne!(core.reservation.backing.nodes[0].flags & READY, 0);
        }
    }
    Ok(())
}
/// Every currently used counter refuses before changing epoch, gate, link or mark state.
pub(super) fn counters() -> TestResult {
    for counter in [0, 1, 3, 4] {
        let mut account = account()?;
        let mut core = graph(1, &[4], &[], &mut account)?;
        let root = core.handle(0, &mut account)?;
        if counter == 0 {
            super::drive(&mut core, &mut account)?;
        }
        let backing =
            std::rc::Rc::get_mut(&mut core.reservation.backing).ok_or("exclusive core")?;
        backing.control.epochs[counter] = u64::MAX;
        let old = backing.control;
        let mut receipt = None;
        let result = match counter {
            0 => core.complete(root, &mut account),
            1 => core.invalidate(root, &mut account),
            3 => core.begin_capacity_epoch(&mut account),
            _ => core
                .drive_one(
                    &mut account,
                    |_, _| {
                        Ok(super::Capacity::Wait {
                            required: 2,
                            available: 1,
                        })
                    },
                    |_, _| Ok(super::Demand::Satisfied),
                    |_| {},
                    |value| receipt = Some(value),
                )
                .map(|_| ()),
        };
        assert!(matches!(
            result,
            Err(Error::Native(crate::LoanError::CountOverflow))
        ));
        if counter == 4 {
            assert_eq!(
                receipt,
                Some(super::super::waiting::ScanReceipt {
                    handle: root,
                    capacity: Some(super::Capacity::Wait {
                        required: 2,
                        available: 1
                    }),
                })
            );
            assert_eq!(core.reservation.backing.nodes[0].attempts, 3);
        } else {
            assert_eq!(receipt, None);
        }
        let control = core.reservation.backing.control;
        assert_eq!(control.epochs, old.epochs);
        assert_eq!(control.waiting, old.waiting);
        assert_eq!(control.pending, old.pending);
        assert_eq!(control.flags, old.flags);
        assert_eq!(core.reservation.backing.marks[0], 0);
    }
    let mut account = account()?;
    let mut core = graph(1, &[4], &[], &mut account)?;
    let root = core.handle(0, &mut account)?;
    std::rc::Rc::get_mut(&mut core.reservation.backing)
        .ok_or("exclusive core")?
        .control
        .epochs[1] = u64::MAX - 1;
    core.invalidate(root, &mut account)?;
    assert_eq!(core.reservation.backing.control.epochs[1], u64::MAX);
    assert!(core.resume_invalidation(1, &mut account)?);
    assert!(matches!(
        core.invalidate(root, &mut account),
        Err(Error::Native(crate::LoanError::CountOverflow))
    ));
    Ok(())
}

/// Retired generations cannot authenticate a later occupation; maximal generations never reenter.
pub(super) fn reuse() -> TestResult {
    for generation in [1, u64::MAX] {
        let mut account = account()?;
        let mut core = graph(1, &[4], &[], &mut account)?;
        std::rc::Rc::get_mut(&mut core.reservation.backing)
            .ok_or("exclusive core")?
            .nodes[0]
            .generation = generation;
        let original = core.handle(0, &mut account)?;
        super::drive(&mut core, &mut account)?;
        core.complete(original, &mut account)?;
        core.acknowledge(original, &mut account)?;
        assert_eq!(
            super::drive(&mut core, &mut account)?,
            super::Visit::Control(super::Control::Retired(0))
        );
        let before = account.usage().work;
        let records = core.reservation.backing.cleanup.records;
        let replacement = core.reuse_for_control(&mut account)?;
        // Vacant-head check H=1; reusable occupation adds N+H+M=3 and P=5.
        assert_eq!(
            account.usage().work - before,
            if generation == u64::MAX { 1 } else { 9 }
        );
        assert_eq!(
            core.reservation.backing.cleanup.records - records,
            if generation == u64::MAX { 0 } else { 5 }
        );
        if generation == u64::MAX {
            assert_eq!(replacement, None);
        } else {
            let replacement = replacement.ok_or("missing reusable occupation")?;
            assert_eq!(replacement.generation, 2);
            assert!(matches!(
                core.complete(original, &mut account),
                Err(Error::Native(crate::LoanError::StaleOccupation))
            ));
            assert_eq!(
                super::drive(&mut core, &mut account)?,
                super::Visit::Started(replacement)
            );
        }
    }
    Ok(())
}

/// Outside occupations retain their far-path bit, state and original attempt allowance.
pub(super) fn unaffected() -> TestResult {
    let mut account = account()?;
    let mut core = graph(
        4097,
        &vec![8; 4097],
        &[super::edge(0, 1, 0), super::edge(1, 2, 1)],
        &mut account,
    )?;
    let root = core.handle(0, &mut account)?;
    let outside = core.handle(4096, &mut account)?;
    let node = &core.reservation.backing.nodes[4096];
    let flags = node.flags;
    let attempts = node.attempts;
    let bit = core.reservation.backing.ready[64] & 1;
    assert_eq!(bit, 1);
    core.invalidate(root, &mut account)?;
    assert!(!core.resume_invalidation(1, &mut account)?);
    assert_eq!(
        super::drive(&mut core, &mut account)?,
        super::Visit::ControlPending
    );
    assert_eq!(core.reservation.backing.nodes[4096].flags, flags);
    assert_eq!(core.reservation.backing.nodes[4096].attempts, attempts);
    assert_eq!(core.reservation.backing.ready[64] & 1, bit);
    assert!(core.resume_invalidation(4, &mut account)?);
    assert_eq!(core.reservation.backing.nodes[4096].flags, flags);
    assert_eq!(core.reservation.backing.nodes[4096].attempts, attempts);
    assert_eq!(core.reservation.backing.ready[64] & 1, bit);
    // Stable order dispatches the unchanged outside set, including the far summary child.
    for slot in 3..4096 {
        let super::Visit::Started(handle) = super::drive(&mut core, &mut account)? else {
            return Err("outside node did not dispatch".into());
        };
        assert_eq!(handle.slot, slot);
    }
    assert_eq!(
        super::drive(&mut core, &mut account)?,
        super::Visit::Started(outside)
    );
    assert_eq!(core.reservation.backing.nodes[4096].attempts, attempts - 1);
    Ok(())
}

/// A later wait-append Work refusal preserves the entered probe's bounded receipt.
pub(super) fn drive_receipt() -> TestResult {
    let mut account = account()?;
    let mut core = graph(1, &[4], &[], &mut account)?;
    let root = core.handle(0, &mut account)?;
    // H entry + M search + N phase + N application + H cursor + M clear = 6.
    let remaining = Limits::default().work - account.usage().work - 6;
    account.ledger.work(remaining, crate::loan::AT)?;
    let mut receipt = None;
    let capacity = super::Capacity::Wait {
        required: 2,
        available: 1,
    };
    let result = core.drive_one(
        &mut account,
        |_, _| Ok(capacity),
        |_, _| Ok(super::Demand::Satisfied),
        |_| {},
        |value| receipt = Some(value),
    );
    assert!(matches!(
        result,
        Err(Error::Native(crate::LoanError::Budget(_)))
    ));
    assert_eq!(
        receipt,
        Some(super::super::waiting::ScanReceipt {
            handle: root,
            capacity: Some(capacity)
        })
    );
    assert_eq!(core.reservation.backing.nodes[0].attempts, 3);
    assert_ne!(
        core.reservation.backing.control.flags & super::super::STOPPED,
        0
    );
    assert_eq!(core.reservation.backing.control.waiting, [super::NONE; 3]);
    Ok(())
}
