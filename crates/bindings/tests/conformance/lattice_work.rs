// SPDX-License-Identifier: AGPL-3.0-only
//! Measured optional owners keep exact Work boundaries and publish no partial questions.

use super::{
    decision_fixture::ledger,
    lattice_fixture::lattice,
    lattice_physical::{source, specification},
};
use crate::testing::declarations::TestResult;
use nrz_bindings::{
    AdmissionError, Budget, DecisionError, DecisionOperand, LatticeDecisionCase, Limits,
};
use nrz_cas::{Fault, FaultKind, Location, limits::BudgetKind};

/// Independent physical declaration and input-association costs for one fixture.
struct Costs {
    /// Phase, two-angle or grid fixture selector.
    kind: u8,
    /// Complete physical question checking, excluding already prepared prerequisites.
    question: u64,
    /// Original input union and designated symbol construction.
    association: u64,
    /// Complete question and ordered companion authentication.
    authentication: u64,
    /// Newly owned question metadata and unique prepared exact judgments.
    question_bytes: u64,
    /// New association metadata and symbol graphs.
    input_bytes: u64,
    /// Distinct original physical constants.
    inputs: usize,
}

/// Literal measurements from the deliberately failing constructor probe, with unchanged defaults.
const COSTS: [Costs; 3] = [
    Costs {
        kind: 0,
        question: 67_825,
        association: 33_990,
        authentication: 11,
        question_bytes: 7_025,
        input_bytes: 13_426,
        inputs: 3,
    },
    Costs {
        kind: 1,
        question: 53_780,
        association: 56_586,
        authentication: 17,
        question_bytes: 5_711,
        input_bytes: 22_293,
        inputs: 5,
    },
    Costs {
        kind: 2,
        question: 34_395,
        association: 56_586,
        authentication: 17,
        question_bytes: 3_297,
        input_bytes: 22_293,
        inputs: 5,
    },
];

/// Original physical questions and their associations retain exact figures and add zero CAS records.
/// ´claim:bindings:lattice-question-costs´ Frozen input environments reduce association Work to 33990 or 56586 while physical question costs and retained bytes stay fixed.
/// ´test:integration:bindings-lattice-question-costs´
#[test]
fn bindings_lattice_question_costs() -> TestResult {
    for cost in COSTS {
        let mut fixture = lattice(cost.kind)?;
        let owner = source(cost.kind, "1", "original physical source")?;
        let spec = specification(&mut fixture, cost.kind, &owner)?;
        let before = fixture.frame.cas.counts();
        let mut budget = ledger()?;
        let case = LatticeDecisionCase::declare(&fixture.frame.cas, spec, &mut budget)?;
        assert_eq!(budget.usage().work, cost.question);
        assert_eq!(case.retained_payload_bytes(), cost.question_bytes);
        let inputs = case.associate_inputs(&mut budget)?;
        assert_eq!(budget.usage().work, cost.question + cost.association);
        assert_eq!(inputs.retained_payload_bytes(), cost.input_bytes);
        assert_eq!(inputs.operands().len(), cost.inputs);
        let symbols = inputs
            .operands()
            .iter()
            .map(DecisionOperand::symbols)
            .collect::<Vec<_>>();
        inputs.authenticate(&case, &symbols, &mut budget)?;
        assert_eq!(
            budget.usage().work,
            cost.question + cost.association + cost.authentication
        );
        assert_eq!(fixture.frame.cas.counts(), before);
    }
    Ok(())
}

/// Each question and association owner has one literal N/N−1 pair including prior caller spending.
/// ´claim:bindings:lattice-question-work´ Literal question and association boundaries retain prior spending and preserve the source graph on one-below refusal.
/// ´test:integration:bindings-lattice-question-work´
#[test]
fn bindings_lattice_question_work() -> TestResult {
    for cost in COSTS {
        let mut fixture = lattice(cost.kind)?;
        let owner = source(cost.kind, "1", "original physical source")?;
        let spec = specification(&mut fixture, cost.kind, &owner)?;
        let before = fixture.frame.cas.counts();
        for (work, accepted) in [(cost.question + 17, true), (cost.question + 16, false)] {
            let mut budget = prior(work)?;
            let result =
                LatticeDecisionCase::declare(&fixture.frame.cas, spec.clone(), &mut budget);
            if accepted {
                assert_eq!(result?.retained_payload_bytes(), cost.question_bytes);
                assert_eq!(budget.usage().work, work);
            } else {
                let Err(fault) = result else {
                    panic!("short physical question accepted");
                };
                work_error(&fault, false)?;
            }
            assert!(budget.usage().work >= 17);
            assert_eq!(fixture.frame.cas.counts(), before);
        }
        let case = LatticeDecisionCase::declare(&fixture.frame.cas, spec, &mut ledger()?)?;
        for (work, accepted) in [
            (cost.association + 17, true),
            (cost.association + 16, false),
        ] {
            let mut budget = prior(work)?;
            let result = case.associate_inputs(&mut budget);
            if accepted {
                assert_eq!(result?.operands().len(), cost.inputs);
                assert_eq!(budget.usage().work, work);
            } else {
                let Err(fault) = result else {
                    panic!("short input association accepted");
                };
                work_error(&fault, true)?;
            }
            assert!(budget.usage().work >= 17);
            assert_eq!(fixture.frame.cas.counts(), before);
        }
        authenticate(&case, &cost)?;
    }
    Ok(())
}

/// The independent authentication owner borrows all symbols and returns no partial validation.
fn authenticate(case: &LatticeDecisionCase, cost: &Costs) -> TestResult {
    let inputs = case.associate_inputs(&mut ledger()?)?;
    let symbols = inputs
        .operands()
        .iter()
        .map(DecisionOperand::symbols)
        .collect::<Vec<_>>();
    for (work, accepted) in [
        (cost.authentication + 17, true),
        (cost.authentication + 16, false),
    ] {
        let mut budget = prior(work)?;
        let result = inputs.authenticate(case, &symbols, &mut budget);
        if accepted {
            result?;
            assert_eq!(budget.usage().work, work);
        } else {
            work_error(&result.err().ok_or("short authentication accepted")?, false)?;
        }
        assert!(budget.usage().work >= 17);
    }
    Ok(())
}

/// One caller ledger accumulates question, association and authentication without private resets.
/// ´claim:bindings:lattice-question-cumulative-work´ One caller ledger accumulates the measured question, association and authentication costs without refunding refusal Work.
/// ´test:integration:bindings-lattice-question-cumulative-work´
#[test]
fn bindings_lattice_question_cumulative_work() -> TestResult {
    for cost in COSTS {
        let mut fixture = lattice(cost.kind)?;
        let owner = source(cost.kind, "1", "original physical source")?;
        let spec = specification(&mut fixture, cost.kind, &owner)?;
        let before = fixture.frame.cas.counts();
        let required = cost.question + cost.association + cost.authentication;
        for (work, accepted) in [(required + 17, true), (required + 16, false)] {
            let mut budget = prior(work)?;
            let case = LatticeDecisionCase::declare(&fixture.frame.cas, spec.clone(), &mut budget)?;
            let inputs = case.associate_inputs(&mut budget)?;
            let symbols = inputs
                .operands()
                .iter()
                .map(DecisionOperand::symbols)
                .collect::<Vec<_>>();
            let result = inputs.authenticate(&case, &symbols, &mut budget);
            if accepted {
                result?;
                assert_eq!(budget.usage().work, work);
            } else {
                let Err(fault) = result else {
                    panic!("short complete question accepted");
                };
                work_error(&fault, false)?;
            }
            assert_eq!(case.question(), fixture.index.question());
            assert_eq!(fixture.frame.cas.counts(), before);
            assert!(budget.usage().work >= 17);
        }
    }
    Ok(())
}

/// Exhausted returned or temporary storage refuses the entire question before publication.
/// ´claim:bindings:lattice-question-storage´
/// ´test:integration:bindings-lattice-question-storage´
#[test]
fn bindings_lattice_question_storage() -> TestResult {
    let mut fixture = lattice(2)?;
    let owner = source(2, "1", "original physical source")?;
    let spec = specification(&mut fixture, 2, &owner)?;
    let before = fixture.frame.cas.counts();
    for expected in [BudgetKind::OutputBytes, BudgetKind::StagedPayloadBytes] {
        let mut limits = Limits::default();
        if expected == BudgetKind::OutputBytes {
            limits.output_bytes = 1;
        } else {
            limits.staged_payload_bytes = 1;
        }
        let mut budget = Budget::new(limits)?;
        budget.work(17, Location::new("prior question storage"))?;
        let error = LatticeDecisionCase::declare(&fixture.frame.cas, spec.clone(), &mut budget)
            .err()
            .ok_or("short storage accepted")?;
        let DecisionError::Algebra(fault) = error else {
            return Err("wrong storage refusal".into());
        };
        assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == expected));
        assert!(budget.usage().work >= 17);
        assert_eq!(fixture.frame.cas.counts(), before);
    }
    Ok(())
}

/// All measured boundaries include existing monotone caller Work.
fn prior(work: u64) -> TestResult<Budget> {
    let mut budget = Budget::new(Limits {
        work,
        ..Limits::default()
    })?;
    budget.work(17, Location::new("prior physical lattice work"))?;
    Ok(budget)
}

/// A symbol constructor and the physical wrapper retain their distinct error owners.
fn work_error(error: &DecisionError, symbols: bool) -> TestResult {
    let ((true, DecisionError::Symbols(AdmissionError::Algebra(fault)))
    | (false, DecisionError::Algebra(fault))) = (symbols, error)
    else {
        return Err("wrong Work refusal owner".into());
    };
    assert_work(fault);
    Ok(())
}

/// Verify the actual resource checker instead of accepting an arbitrary construction error.
fn assert_work(fault: &Fault) {
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::Work,
            ..
        }
    ));
}
