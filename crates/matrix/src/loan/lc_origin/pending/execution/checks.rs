// SPDX-License-Identifier: AGPL-3.0-only
//! Independent assertions inspect final private records without observing intermediate kernels.
use super::LcApplicationEvaluated;
use crate::loan::payload::bit;
use nrz_scalar::Scalar;
type ExpectedCell<P> = (usize, [u16; 3], Option<[P; 3]>);
type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Expected public-path values are compared bit for bit with the final private output only.
pub(in crate::loan) fn cells<P: Scalar + 'static>(
    candidate: &LcApplicationEvaluated<'_, P, 5>,
    expected: &[ExpectedCell<P>],
) -> TestResult {
    let storage = &candidate
        .owner
        .native
        .as_ref()
        .expect("private evaluated storage")
        .storage;
    for (cell, record) in storage.outcomes.iter().enumerate() {
        let wanted = expected.iter().find(|entry| entry.0 == cell);
        let code = wanted.map_or([0; 3], |entry| entry.1);
        assert_eq!(
            [
                u16::from(record.status),
                u16::from(record.target),
                record.reason
            ],
            code
        );
        let filled = code[0] == 2;
        assert_eq!(bit(&storage.completed.words, cell), filled);
        for channel in &storage.channels {
            assert_eq!(bit(&channel.components, cell), filled);
            assert_eq!(bit(&channel.complete, cell), filled);
        }
        if let Some((_, _, Some(values))) = wanted {
            for (channel, value) in storage.channels.iter().zip(values) {
                assert_eq!(
                    channel.numerical.read::<P>(cell)?.to_f64().to_bits(),
                    value.to_f64().to_bits()
                );
            }
        }
    }
    Ok(())
}
/// Each supported cause and each defensive rejection is enumerated independently.
pub(in crate::loan) fn mapping() -> TestResult {
    mapping::check()
}
mod mapping;

/// New wrappers reuse the same native owner; binding and per-cell workspace are stack values.
pub(in crate::loan) fn layout<P: Scalar>() {
    assert_eq!(size_of::<super::LcApplicationEvaluated<'_, P, 5>>(), 312);
    assert_eq!(size_of::<super::LcApplicationFailed<'_, P, 5>>(), 312);
    assert_eq!(size_of::<super::readiness::Source<'_>>(), 96);
    assert_eq!(size_of::<crate::LcInputReaders<'_, P, 5>>(), 376);
    assert_eq!(size_of::<crate::LcInputCells<'_, P, 5>>(), 536);
    assert_eq!(size_of::<crate::LcInputProduct<'_, P, 5>>(), 560);
    assert_eq!(size_of::<super::readiness::Ready<'_, P, 5>>(), 576);
    assert_eq!(size_of::<super::Traversal>(), 0);
    // Ready has tau (3 scalars), rate (4 scalar-widths including its tag),
    // and three optional values (12 widths). The outer enum uses a vacant
    // Option tag: no extra discriminant/padding beyond those 19 widths.
    assert_eq!(size_of::<Option<[P; 3]>>(), 4 * size_of::<P>());
    assert_eq!(
        size_of::<crate::LcRequestedValues<P>>(),
        12 * size_of::<P>()
    );
    assert_eq!(
        std::mem::align_of::<crate::LcRequestedState<P>>(),
        std::mem::align_of::<P>()
    );
    let expected = if size_of::<P>() == 4 {
        [384, 416, 76]
    } else {
        [464, 504, 152]
    };
    assert_eq!(
        [
            size_of::<crate::LcInputCell<'_, P, 5>>(),
            size_of::<crate::LcProductCell<'_, P, 5>>(),
            size_of::<crate::LcRequestedState<P>>()
        ],
        expected
    );
}

/// A defensive late record corruption cannot turn an evaluated prefix into retry authority.
pub(in crate::loan) fn failed<P: Scalar + 'static>(
    mut pending: super::super::LcApplicationPending<'_, P, 5>,
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
) -> TestResult {
    pending
        .native
        .as_mut()
        .expect("pending storage")
        .storage
        .outcomes[11]
        .status = 6;
    let ticket = pending.ticket();
    let before = account.usage().work;
    crate::loan::fault::reset_reads();
    let Err(super::LcApplicationEvaluationFailure::Failed {
        mut owner,
        error: crate::LcProductError::Kernel(crate::KernelError::Contract),
    }) = pending.evaluate(account)
    else {
        return Err("started traversal must become cancel-only".into());
    };
    assert_eq!(account.usage().work - before, 436);
    assert_eq!(format!("{owner:?}"), "LcApplicationFailed { .. }");
    assert!(matches!(
        heap.loan_inventory(ticket, account),
        Err(crate::LoanError::WrongLoan)
    ));
    assert!(heap.acquire_pin::<P>(ticket, account).is_err());
    assert_eq!(crate::loan::fault::reads(), 30);
    let before = account.usage().work;
    assert_eq!(owner.cancel(heap, account)?.ticket(), ticket);
    assert_eq!(account.usage().work, before);
    Ok(())
}
