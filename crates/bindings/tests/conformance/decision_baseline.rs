// SPDX-License-Identifier: AGPL-3.0-only
//! Measured public CAS prerequisites; these do not constitute bindings decision admission.

use super::decision_fixture::{fixture, ledger};
use crate::testing::declarations::TestResult;
use nrz_bindings::{BindingSymbols, Descriptor};
use nrz_cas::{
    Cas, FaultKind, InvalidCode, Location,
    limits::{Budget, BudgetKind, Limits},
};

/// The bindings client reaches the original LC selection and retains its source assumption.
/// ´test:integration:bindings-lc-selection-baseline´
#[test]
fn bindings_lc_selection_baseline() -> TestResult {
    for (bounded, selection, replay, length) in [
        (false, 166_952, 247_365, 1_562),
        (true, 199_780, 299_788, 1_812),
    ] {
        let mut source = fixture(bounded, "3", "10/Pi")?;
        let mut budget = ledger()?;
        let witness = source
            .frame
            .cas
            .check_proof(&source.context, &source.steps, &mut budget)?;
        assert_eq!(budget.usage().work, selection);
        assert!(witness.is_discharged());
        assert!(!witness.is_unconditional());
        assert_eq!(witness.hypotheses().len(), 1);
        assert_eq!(
            source
                .frame
                .cas
                .materialize_integer_selection(&witness, 3, &mut budget)?,
            3
        );
        assert_eq!(budget.usage().work, selection + 37);
        let bytes = source.frame.cas.export_witness(&witness, &mut ledger()?)?;
        assert_eq!(bytes.len(), length);
        let mut receiver = Cas::new(Limits::default())?;
        let mut replay_budget = ledger()?;
        let imported = receiver.inspect(&bytes, &mut replay_budget)?;
        assert_eq!(replay_budget.usage().work, replay);
        assert_eq!(imported.hypotheses().len(), 1);
        assert_eq!(
            receiver.materialize_integer_selection(&imported, 3, &mut ledger()?)?,
            3
        );
    }
    Ok(())
}

/// Literal selection N/N−1 boundaries include prior caller Work and refuse atomically.
/// ´test:integration:bindings-lc-selection-work-baseline´
#[test]
fn bindings_lc_selection_work_baseline() -> TestResult {
    for (bounded, required) in [(false, 166_952), (true, 199_780)] {
        for (allowance, accepted) in [(required + 17, true), (required + 16, false)] {
            let mut source = fixture(bounded, "3", "10/Pi")?;
            let before = source.frame.cas.counts();
            let mut budget = Budget::new(Limits {
                work: allowance,
                ..Limits::default()
            })?;
            budget.work(17, Location::new("prior bindings decision work"))?;
            let result = source
                .frame
                .cas
                .check_proof(&source.context, &source.steps, &mut budget);
            assert_eq!(result.is_ok(), accepted);
            if accepted {
                assert_eq!(budget.usage().work, required + 17);
            } else {
                let fault = result.err().ok_or("short proof accepted")?;
                assert!(matches!(
                    fault.kind,
                    FaultKind::Exhausted {
                        budget: BudgetKind::Work,
                        ..
                    }
                ));
                assert_eq!(source.frame.cas.counts(), before);
                assert!(budget.usage().work >= 17);
            }
        }
    }
    Ok(())
}

/// Capacity and a short Work ledger preserve the already checked exact Integer theorem.
/// ´test:integration:bindings-lc-materialization-baseline´
#[test]
fn bindings_lc_materialization_baseline() -> TestResult {
    let mut source = fixture(false, "3", "10/Pi")?;
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut ledger()?)?;
    let before = source.frame.cas.counts();
    for (allowance, accepted) in [(37, true), (36, false)] {
        let mut budget = Budget::new(Limits {
            work: allowance,
            ..Limits::default()
        })?;
        let result = source
            .frame
            .cas
            .materialize_integer_selection(&witness, 3, &mut budget);
        if accepted {
            assert_eq!(result?, 3);
            assert_eq!(budget.usage().work, 37);
        } else {
            let fault = result.err().ok_or("short materialization accepted")?;
            assert!(matches!(
                fault.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::Work,
                    ..
                }
            ));
        }
        assert_eq!(source.frame.cas.counts(), before);
    }
    let fault = source
        .frame
        .cas
        .materialize_integer_selection(&witness, 2, &mut ledger()?)
        .err()
        .ok_or("capacity ignored")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::DecisionMaterializationCapacity)
    );
    assert_eq!(source.frame.cas.counts(), before);
    Ok(())
}

/// Freezing the source environment reduces cumulative declaration and copy Work from 24642 to 23783.
/// ´test:integration:bindings-symbol-copy-decision-baseline´
/// ´claim:bindings:bindings-symbol-copy-decision-baseline´ A source declaration and its unchanged copy graph succeed at 23783 Work and refuse at 23782.
#[test]
fn bindings_symbol_copy_decision_baseline() -> TestResult {
    for (allowance, accepted) in [(23_783, true), (23_782, false)] {
        let mut budget = Budget::new(Limits {
            work: allowance,
            ..Limits::default()
        })?;
        let symbols =
            BindingSymbols::declare(&Descriptor::ratio(), [1, 1], [true, true], &mut budget)?;
        let result = symbols.copy_result([1, 1], &mut budget);
        if accepted {
            result?;
            assert_eq!(budget.usage().work, 23_783);
        } else {
            let Err(nrz_bindings::AdmissionError::Algebra(fault)) = result else {
                panic!("one-below symbol copy must refuse");
            };
            assert!(matches!(
                fault.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::Work,
                    ..
                }
            ));
        }
    }
    Ok(())
}
