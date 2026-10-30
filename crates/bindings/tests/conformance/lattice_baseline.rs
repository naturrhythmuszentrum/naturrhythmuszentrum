// SPDX-License-Identifier: AGPL-3.0-only
//! Measured scalar owners to reuse when binding a physical lattice question.

use super::{
    decision_admission::specification,
    decision_fixture::{fixture, ledger},
    lc_count::physical_source,
};
use crate::testing::declarations::TestResult;
use nrz_bindings::{
    AdmissionError, Budget, CountDomain, DecisionError, DecisionExpression, DecisionOperand,
    IntegerDecisionCase, Limits, SelectedDecision,
};
use nrz_cas::{Cas, ExactConstant, FaultKind, InvalidCode, Location, limits::BudgetKind};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// Reuse the complete physical LC declaration and check each independently owned increment.
fn selected(budget: &mut Budget) -> TestResult<SelectedDecision> {
    let owner = physical_source("1")?;
    let start = budget.usage().work;
    let model = DecisionExpression::declared(Rc::clone(&owner), "model", budget)?;
    assert_eq!(model.exact(), Some(&ExactConstant::one()));
    assert_eq!(budget.usage().work - start, 34_713);
    let physical = DecisionExpression::declared(owner, "quotient", budget)?;
    assert_eq!(budget.usage().work - start, 116_076);
    let mut source = fixture(false, "3", "10/Pi")?;
    let case = IntegerDecisionCase::declare(
        &source.frame.cas,
        physical,
        specification(&source, CountDomain::NONNEGATIVE)?,
        budget,
    )?;
    assert_eq!(budget.usage().work - start, 148_371);
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, budget)?;
    assert_eq!(budget.usage().work - start, 315_323);
    let graph = Arc::new(Mutex::new(source.frame.cas));
    let before = graph.lock().map_err(|_| "graph owner poisoned")?.counts();
    let result = case.admit(Arc::clone(&graph), witness, case.attachment(), budget)?;
    assert_eq!(budget.usage().work - start, 315_819);
    assert_eq!(
        graph.lock().map_err(|_| "graph owner poisoned")?.counts(),
        before
    );
    Ok(result)
}

/// The lattice lane remeasures the scalar lifecycle, record closure and original-input association.
/// ´test:integration:bindings-lattice-scalar-baseline´
#[test]
fn bindings_lattice_scalar_baseline() -> TestResult {
    let mut budget = ledger()?;
    let selected = selected(&mut budget)?;
    let count = selected.count(3, &mut budget)?;
    assert_eq!(count.value(), 3);
    assert_eq!(budget.usage().work, 315_878);
    assert_eq!(selected.retained_payload_bytes(), 13_842);
    assert_eq!(count.retained_payload_bytes(), 2_068);
    let case = selected.case();
    let associated = case.associate_inputs(&mut budget)?;
    assert_eq!(budget.usage().work, 364_566);
    assert_eq!(associated.retained_payload_bytes(), 17_858);
    let originals = associated
        .operands()
        .iter()
        .map(DecisionOperand::name)
        .collect::<Vec<_>>();
    assert_eq!(originals, ["cycle", "duration", "radian", "tau"]);
    let symbols = associated
        .operands()
        .iter()
        .map(DecisionOperand::symbols)
        .collect::<Vec<_>>();
    associated.authenticate(case, &symbols, &mut budget)?;
    assert_eq!(budget.usage().work, 364_580);
    let graph = selected.cas()?;
    let before = graph.counts();
    let bytes = graph.export_witness(selected.witness(), &mut ledger()?)?;
    drop(graph);
    assert_eq!(bytes.len(), 1_562);
    let mut replay = ledger()?;
    let mut receiver = Cas::new(Limits::default())?;
    let witness = receiver.inspect(&bytes, &mut replay)?;
    assert_eq!(replay.usage().work, 247_365);
    assert_eq!(receiver.counts().vertices, 36);
    assert_eq!(witness.hypotheses().len(), 1);
    let error = selected
        .count(2, &mut ledger()?)
        .err()
        .ok_or("capacity ignored")?;
    let DecisionError::Algebra(fault) = error else {
        return Err("wrong capacity refusal".into());
    };
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::DecisionMaterializationCapacity)
    );
    assert_eq!(
        selected
            .cas()?
            .export_witness(selected.witness(), &mut ledger()?)?,
        bytes
    );
    assert_eq!(selected.cas()?.counts(), before);
    Ok(())
}

/// Distinct association and authentication owners retain literal cumulative Work boundaries.
/// ´test:integration:bindings-lattice-scalar-association-work´
#[test]
fn bindings_lattice_scalar_association_work() -> TestResult {
    let selected = selected(&mut ledger()?)?;
    let case = selected.case();
    let before = case.source().owner().cas()?.counts();
    for (work, accepted) in [(48_705, true), (48_704, false)] {
        let mut budget = prior(work)?;
        let result = case.associate_inputs(&mut budget);
        if accepted {
            assert_eq!(result?.operands().len(), 4);
            assert_eq!(budget.usage().work, work);
        } else {
            let error = result.err().ok_or("short association accepted")?;
            let DecisionError::Symbols(AdmissionError::Algebra(fault)) = error else {
                return Err("not a symbol-construction refusal".into());
            };
            work_fault(&fault);
        }
        assert!(budget.usage().work >= 17);
        assert_eq!(case.source().owner().cas()?.counts(), before);
    }
    let associated = case.associate_inputs(&mut ledger()?)?;
    let symbols = associated
        .operands()
        .iter()
        .map(DecisionOperand::symbols)
        .collect::<Vec<_>>();
    for (work, accepted) in [(31, true), (30, false)] {
        let mut budget = prior(work)?;
        let result = associated.authenticate(case, &symbols, &mut budget);
        if accepted {
            result?;
            assert_eq!(budget.usage().work, work);
        } else {
            exhausted(&result.err().ok_or("short authentication accepted")?)?;
        }
        assert!(budget.usage().work >= 17);
        assert_eq!(case.source().owner().cas()?.counts(), before);
    }
    Ok(())
}

/// Complete lifecycle exhaustion and fresh replay publish no partial physical or CAS result.
/// ´test:integration:bindings-lattice-scalar-cumulative-work´
#[test]
fn bindings_lattice_scalar_cumulative_work() -> TestResult {
    for (work, accepted) in [(364_597, true), (364_596, false)] {
        let mut budget = prior(work)?;
        let selected = selected(&mut budget)?;
        let before = selected.cas()?.counts();
        let count = selected.count(3, &mut budget)?;
        let association = selected.case().associate_inputs(&mut budget)?;
        let symbols = association
            .operands()
            .iter()
            .map(DecisionOperand::symbols)
            .collect::<Vec<_>>();
        let result = association.authenticate(selected.case(), &symbols, &mut budget);
        if accepted {
            result?;
            assert_eq!(budget.usage().work, work);
        } else {
            exhausted(&result.err().ok_or("short lifecycle accepted")?)?;
        }
        assert_eq!(count.value(), 3);
        assert_eq!(selected.cas()?.counts(), before);
        assert!(budget.usage().work >= 17);
    }
    let selected = selected(&mut ledger()?)?;
    let bytes = selected
        .cas()?
        .export_witness(selected.witness(), &mut ledger()?)?;
    for (work, accepted) in [(247_382, true), (247_381, false)] {
        let mut receiver = Cas::new(Limits::default())?;
        let before = receiver.counts();
        let snapshot = receiver.canonical_snapshot(&mut ledger()?)?;
        let mut budget = prior(work)?;
        let result = receiver.inspect(&bytes, &mut budget);
        if accepted {
            assert_eq!(result?.hypotheses().len(), 1);
            assert_eq!(receiver.counts().vertices, 36);
            assert_eq!(budget.usage().work, work);
        } else {
            exhausted(&result.err().ok_or("short replay accepted")?.into())?;
            assert_eq!(receiver.counts(), before);
            assert_eq!(receiver.canonical_snapshot(&mut ledger()?)?, snapshot);
        }
        assert!(budget.usage().work >= 17);
    }
    Ok(())
}

/// Every owner uses the caller's existing monotone ledger, including prior spending.
fn prior(work: u64) -> TestResult<Budget> {
    let mut budget = Budget::new(Limits {
        work,
        ..Limits::default()
    })?;
    budget.work(17, Location::new("prior lattice bindings baseline"))?;
    Ok(budget)
}

/// Pin resource refusal to the actual Work checker rather than a generic error.
pub(super) fn exhausted(error: &DecisionError) -> TestResult {
    let DecisionError::Algebra(fault) = error else {
        return Err("not a Work refusal".into());
    };
    work_fault(fault);
    Ok(())
}

/// A wrapper cannot hide a different underlying resource or semantic refusal.
fn work_fault(fault: &nrz_cas::Fault) {
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::Work,
            ..
        }
    ));
}

/// Selected aliases share the same guarded graph and see post-sharing checked constants.
/// ´test:integration:bindings-selected-scalar-graph-guards´
/// ´claim:bindings:selected-scalar-graph-guards´
#[test]
fn bindings_selected_scalar_graph_guards() -> TestResult {
    let selected = selected(&mut ledger()?)?;
    let alias = selected.clone();
    let first = selected.cas()?;
    let before = first.retained_judgment_count();
    let bytes = first.export_witness(selected.witness(), &mut ledger()?)?;
    drop(first);
    let spec = selected.case().specification();
    let mut authority = selected.cas()?;
    authority.prepare_constant(&spec.context, &spec.target, &mut ledger()?)?;
    drop(authority);
    let authority = alias.cas()?;
    assert_eq!(authority.retained_judgment_count(), before + 1);
    assert_eq!(
        authority.export_witness(alias.witness(), &mut ledger()?)?,
        bytes
    );
    drop(authority);
    Ok(())
}

/// A poisoned selected owner refuses all guarded authority without a replacement store.
/// ´test:integration:bindings-selected-scalar-graph-poison´
/// ´claim:bindings:selected-scalar-graph-poison´
#[test]
fn bindings_selected_scalar_graph_poison() -> TestResult {
    let selected = selected(&mut ledger()?)?;
    let alias = selected.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = selected.cas().expect("unpoisoned selected owner");
        panic!("deliberate owner poison");
    }));
    assert!(result.is_err());
    let Err(fault) = alias.cas() else {
        panic!("poisoned owner must refuse read access");
    };
    assert!(matches!(fault, DecisionError::GraphPoisoned));
    Ok(())
}
