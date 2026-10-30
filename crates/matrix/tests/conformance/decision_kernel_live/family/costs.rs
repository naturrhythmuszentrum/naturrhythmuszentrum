// SPDX-License-Identifier: AGPL-3.0-only
//! Fresh owner boundaries for the conditional physical-family quotient.

use super::{inputs, ledger, pi, select, source};
use crate::binding_testing::TestResult;
use crate::conformance::decision_fixture::SelectionFixture;
use nrz_bindings::readings::algebra::{Cas, Fault, FaultKind, Location, limits::BudgetKind};
use nrz_bindings::{Budget, DecisionInputs, Limits};
use nrz_matrix::{FrozenRatioKernel, Heap, HeapLimits, LiveDecisionError, ValueBinding};
use nrz_scalar::ArithmeticContext;
use std::sync::{Arc, Mutex};

/// Independent setup outside the owner being bounded.
struct Fixture {
    /// Complete physical original association.
    association: DecisionInputs,
    /// Original unselected mathematical question.
    source: SelectionFixture,
    /// Actual native numerical owner.
    heap: Heap,
    /// Complete value and companion bindings.
    bindings: Vec<ValueBinding<f64>>,
}
impl Fixture {
    /// Prepare immutable physical declarations and actual LC inputs.
    fn new() -> TestResult<Self> {
        let (association, source) = source(&mut ledger()?)?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let bindings = inputs(&mut heap, &association)?;
        Ok(Self {
            association,
            source,
            heap,
            bindings,
        })
    }
    /// Capture outside the trace owner's budget and execute only the measured trace owner.
    fn trace(&mut self, budget: &mut Budget) -> Result<FrozenRatioKernel<f64>, LiveDecisionError> {
        let frozen = self.heap.freeze_decision_inputs(
            &self.association,
            &self.bindings,
            &mut ledger().map_err(|_| LiveDecisionError::Recipe)?,
        )?;
        self.heap.trace_ratio_decision(
            frozen,
            &mut self.source.frame.cas,
            &pi().map_err(|_| LiveDecisionError::Recipe)?,
            &ArithmeticContext::new()?,
            budget,
        )
    }
}

/// Preserve seventeen units of prior caller Work under an exact inclusive ceiling.
fn allowance(work: u64) -> TestResult<Budget> {
    let mut budget = Budget::new(Limits {
        work,
        ..Limits::default()
    })?;
    budget.work(17, Location::new("prior live kernel owner"))?;
    Ok(budget)
}

/// Only an exhausted Work ceiling, with the exact final request, is the expected refusal.
fn exhausted(fault: &Fault, limit: u64, expected: u64) {
    let FaultKind::Exhausted {
        budget,
        limit: actual,
        needed,
        ..
    } = fault.kind
    else {
        panic!("expected owner Work exhaustion");
    };
    assert_eq!(budget, BudgetKind::Work);
    assert_eq!(actual, limit);
    assert_eq!(needed, expected);
}

/// Trace construction has a literal N/N−1 boundary, full records and atomic graph/heap refusal.
/// ´test:integration:kernel-physical-family-trace-work-boundary´
/// ´claim:matrix:kernel-physical-family-trace-work-boundary´
#[test]
fn kernel_physical_family_trace_work_boundary() -> TestResult {
    for work in [312_761, 312_760] {
        let mut source = Fixture::new()?;
        let graph = source.source.frame.cas.counts();
        let heap = source.heap.provenance_usage();
        let mut budget = allowance(work)?;
        let limits = budget.limits().clone();
        let result = source.trace(&mut budget);
        if work == 312_761 {
            let trace = result?;
            assert_eq!(trace.evidence().scalar_usage().work, 732);
            assert_eq!(trace.retained_payload_bytes(), 8672);
            assert_eq!(budget.usage().work, work);
        } else {
            let Err(LiveDecisionError::Algebra(fault)) = result else {
                panic!("trace owner must exhaust before receipt copy");
            };
            exhausted(&fault, work, 312_761);
            assert_eq!(fault.location, Location::new("exact rational enclosure"));
            assert!(budget.usage().work >= 17);
        }
        assert_eq!(budget.limits(), &limits);
        if work != 312_761 {
            assert_eq!(source.source.frame.cas.counts(), graph);
        }
        assert_eq!(source.heap.provenance_usage(), heap);
    }
    Ok(())
}

/// Selection proof retains the full scalar contract at literal N/N−1 Work with no failed graph prefix.
/// ´test:integration:kernel-physical-family-selection-work-boundary´
/// ´claim:matrix:kernel-physical-family-selection-work-boundary´
#[test]
fn kernel_physical_family_selection_work_boundary() -> TestResult {
    for work in [328_336, 328_335] {
        let mut source = Fixture::new()?;
        let trace = source.trace(&mut ledger()?)?;
        let before = source.source.frame.cas.counts();
        let mut budget = allowance(work)?;
        let limits = budget.limits().clone();
        let proposal = source.source.steps.last_mut().ok_or("selection")?;
        proposal.certificate =
            nrz_bindings::readings::algebra::proof::Certificate::IntegerSelection(Box::new(
                trace.evidence().contract().clone(),
            ));
        let result = source.source.frame.cas.check_proof(
            &source.source.context,
            &source.source.steps,
            &mut budget,
        );
        if work == 328_336 {
            let witness = result?;
            assert_eq!(witness.hypotheses().len(), 3);
            assert_eq!(budget.usage().work, work);
        } else {
            let Err(fault) = result else {
                panic!("scalar selection must refuse below its full Work boundary");
            };
            exhausted(&fault, work, 328_336);
            assert_eq!(source.source.frame.cas.counts(), before);
            assert!(budget.usage().work >= 17);
        }
        assert_eq!(budget.limits(), &limits);
    }
    Ok(())
}

/// Native preparation retains the identical source lease and checked trace at a literal Work boundary.
/// ´test:integration:kernel-physical-family-preparation-work-boundary´
/// ´claim:matrix:kernel-physical-family-preparation-work-boundary´
#[test]
fn kernel_physical_family_preparation_work_boundary() -> TestResult {
    for work in [5676, 5675] {
        let mut source = Fixture::new()?;
        let trace = source.trace(&mut ledger()?)?;
        let witness = select(&mut source.source, &trace, &mut ledger()?)?;
        let graph = Arc::new(Mutex::new(source.source.frame.cas));
        let counts = graph.lock().map_err(|_| "graph owner poisoned")?.counts();
        let before = source.heap.provenance_usage();
        let mut budget = allowance(work)?;
        let limits = budget.limits().clone();
        let result = source.heap.prepare_ratio_kernel_decision(
            trace,
            Arc::clone(&graph),
            witness,
            &mut budget,
        );
        if work == 5676 {
            result?;
            assert_eq!(budget.usage().work, work);
        } else {
            let Err(LiveDecisionError::Admission(fault)) = result else {
                panic!("native preparation must exhaust before selection ownership");
            };
            let nrz_bindings::DecisionError::Algebra(fault) = *fault else {
                panic!("native preparation must name Work exhaustion");
            };
            exhausted(&fault, work, 5676);
            assert!(budget.usage().work >= 17);
        }
        assert_eq!(budget.limits(), &limits);
        assert_eq!(
            graph.lock().map_err(|_| "graph owner poisoned")?.counts(),
            counts
        );
        assert_eq!(source.heap.provenance_usage(), before);
    }
    Ok(())
}

/// Witness export and fresh inspection have literal Work boundaries and retain the same Machin citation.
/// ´test:integration:kernel-physical-family-wire-work-boundaries´
/// ´claim:matrix:kernel-physical-family-wire-work-boundaries´
#[test]
fn kernel_physical_family_wire_work_boundaries() -> TestResult {
    let mut bytes = Vec::new();
    for work in [2504, 2503] {
        let mut source = Fixture::new()?;
        let trace = source.trace(&mut ledger()?)?;
        let witness = select(&mut source.source, &trace, &mut ledger()?)?;
        let graph = &source.source.frame.cas;
        let before = graph.counts();
        let mut budget = allowance(work)?;
        let limits = budget.limits().clone();
        let result = graph.export_witness(&witness, &mut budget);
        if work == 2504 {
            bytes = result?;
            assert_eq!(bytes.len(), 2487);
            assert_eq!(budget.usage().work, work);
        } else {
            let Err(fault) = result else {
                panic!("export must refuse below its byte-copy Work boundary");
            };
            exhausted(&fault, work, 2504);
        }
        assert_eq!(graph.counts(), before);
        assert_eq!(budget.limits(), &limits);
    }
    for work in [485_081, 485_080] {
        let mut receiver = Cas::new(Limits::default())?;
        let before = receiver.counts();
        let mut budget = allowance(work)?;
        let limits = budget.limits().clone();
        let result = receiver.inspect(&bytes, &mut budget);
        if work == 485_081 {
            let witness = result?;
            assert_eq!(witness.hypotheses().len(), 3);
            assert_eq!(budget.usage().work, work);
            assert_eq!(receiver.export_witness(&witness, &mut ledger()?)?, bytes);
        } else {
            let Err(fault) = result else {
                panic!("fresh scalar replay must exhaust below its full Work boundary");
            };
            exhausted(&fault, work, 485_081);
            assert_eq!(receiver.counts(), before);
        }
        assert_eq!(budget.limits(), &limits);
    }
    Ok(())
}
