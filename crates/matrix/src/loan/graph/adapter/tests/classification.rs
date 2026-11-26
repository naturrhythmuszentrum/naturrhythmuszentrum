// SPDX-License-Identifier: AGPL-3.0-only
//! Refusal dimensions use independent integer decompositions and original native carriers.

use super::super::{
    gate::Gate,
    native::Failure,
    refusal::{self, Bytes, Class},
};
use super::fixture::TestResult;
use crate::{ExecutionAccount, HeapError, LoanError};
use nrz_bindings::{Budget, Limits, UnitError};

/// B+A belongs to one exact preflight; a saturated scalar alone supplies no subtraction authority.
/// ´test:unit:graph-native-refusal-dimensions´
#[test]
fn graph_native_refusal_dimensions() -> TestResult {
    let scope = Bytes {
        staged: true,
        baseline: 12,
        addition: 7,
        limit: 10,
    };
    let cause = LoanError::Heap(HeapError::ResourceBytes {
        staged: true,
        used: 19,
        limit: 10,
    });
    assert_eq!(
        refusal::loan(&cause, Some(scope)),
        Class::BytesWait {
            required: 7,
            available: 0
        }
    );
    assert_eq!(refusal::loan(&cause, None), Class::Terminal);
    let intrinsic = Bytes {
        staged: true,
        baseline: 4,
        addition: 15,
        limit: 10,
    };
    assert_eq!(refusal::loan(&cause, Some(intrinsic)), Class::Terminal);
    let overflow = Bytes {
        staged: true,
        baseline: u64::MAX,
        addition: 1,
        limit: 10,
    };
    assert_eq!(refusal::loan(&cause, Some(overflow)), Class::Terminal);
    let saturated = LoanError::Heap(HeapError::ResourceBytes {
        staged: false,
        used: u64::MAX,
        limit: u64::MAX - 2,
    });
    assert_eq!(refusal::loan(&saturated, None), Class::Terminal);
    assert_eq!(
        refusal::loan(
            &saturated,
            Some(Bytes {
                staged: false,
                baseline: u64::MAX - 1,
                addition: 1,
                limit: u64::MAX - 2
            })
        ),
        Class::BytesWait {
            required: 1,
            available: 0
        }
    );
    elements()?;
    work()
}

/// Each precision must fit independently, including the precision after the first blocked one.
fn elements() -> TestResult {
    let account = ExecutionAccount::new(Budget::new(Limits::default())?)?;
    let gate = Gate::new(&account);
    let capacity = LoanError::Heap(HeapError::Capacity);
    gate.element_failure([5, 0], [5, 0], [0, 0], [1, 11], 10);
    assert_eq!(gate.loan(&capacity), Class::Terminal);
    gate.element_failure([5, 0], [5, 0], [0, 0], [1, 10], 10);
    assert_eq!(gate.loan(&capacity), Class::ElementsWait);
    gate.element_failure([usize::MAX, 0], [1, 0], [0, 0], [1, 1], 10);
    assert_eq!(gate.loan(&capacity), Class::Terminal);
    Ok(())
}

/// Two requested Work visits cannot fit an original allowance of one; native layout work differs.
fn work() -> TestResult {
    let mut account = ExecutionAccount::new(Budget::new(Limits {
        work: 1,
        ..Limits::default()
    })?)?;
    let gate = Gate::new(&account);
    let layout = Failure::Native(LoanError::Heap(HeapError::WorkLimit));
    assert_eq!(gate.classify(&layout), Class::Terminal);
    let result = Err::<(), _>(crate::loan::graph::walk::Error::Adapter(layout));
    gate.observe(&result);
    assert!(!gate.stopped());
    let Err(LoanError::Budget(fault)) = crate::loan::charge(&mut account.ledger, 2) else {
        return Err("original Work refusal".into());
    };
    let wrapped = Failure::Native(LoanError::Heap(HeapError::Units(UnitError::Arithmetic(
        fault,
    ))));
    assert!(wrapped.work_stopped());
    gate.observe(&Err::<(), _>(crate::loan::graph::walk::Error::Adapter(
        wrapped,
    )));
    assert!(gate.stopped());
    Ok(())
}
