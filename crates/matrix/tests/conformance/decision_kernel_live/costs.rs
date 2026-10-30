// SPDX-License-Identifier: AGPL-3.0-only
//! Literal owner boundaries and independent attribution of the closed live quotient receipt.

use super::{citations, decision_source, inputs, ledger, pi, select};
use crate::binding_testing::TestResult;
use crate::conformance::decision_fixture::SelectionFixture;
use nrz_bindings::readings::algebra::{
    Cas, ExactConstant, Fault, FaultKind, Location, Rational,
    decision::{EnclosureOrigin, ScalarKernelInputValue, ScalarKernelLaw, ScalarKernelRecord},
    limits::BudgetKind,
    syntax::Constant,
};
use nrz_bindings::{Budget, DecisionInputs, Limits};
use nrz_matrix::{FrozenRatioKernel, Heap, HeapLimits, LiveDecisionError, ValueBinding};
use nrz_scalar::ArithmeticContext;
use std::cmp::Ordering;
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
        let (association, source) = decision_source()?;
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
/// ´test:integration:kernel-live-trace-work-boundary´
/// ´claim:matrix:kernel-live-trace-work-boundary´
#[test]
fn kernel_live_trace_work_boundary() -> TestResult {
    for work in [393_684, 393_683] {
        let mut source = Fixture::new()?;
        let graph = source.source.frame.cas.counts();
        let heap = source.heap.provenance_usage();
        let mut budget = allowance(work)?;
        let limits = budget.limits().clone();
        let result = source.trace(&mut budget);
        if work == 393_684 {
            let trace = result?;
            super::records(&trace)?;
            assert_eq!(trace.retained_payload_bytes(), 8320);
            assert_eq!(budget.usage().work, work);
        } else {
            let Err(LiveDecisionError::Algebra(fault)) = result else {
                panic!("trace owner must exhaust before receipt copy");
            };
            exhausted(&fault, work, 393_684);
            assert_eq!(fault.location, Location::new("exact rational enclosure"));
            assert_eq!(budget.usage().work, 389_012);
        }
        assert_eq!(budget.limits(), &limits);
        assert_eq!(source.source.frame.cas.counts(), graph);
        assert_eq!(source.heap.provenance_usage(), heap);
    }
    Ok(())
}

/// Selection proof retains the full scalar contract at literal N/N−1 Work with no failed graph prefix.
/// ´test:integration:kernel-live-selection-work-boundary´
/// ´claim:matrix:kernel-live-selection-work-boundary´
#[test]
fn kernel_live_selection_work_boundary() -> TestResult {
    for work in [253_645, 253_644] {
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
        if work == 253_645 {
            let witness = result?;
            citations(&source.source.frame.cas, &witness)?;
            assert_eq!(budget.usage().work, work);
        } else {
            let Err(fault) = result else {
                panic!("scalar selection must refuse below its full Work boundary");
            };
            exhausted(&fault, work, 253_645);
            assert_eq!(source.source.frame.cas.counts(), before);
            assert!(budget.usage().work >= 17);
        }
        assert_eq!(budget.limits(), &limits);
    }
    Ok(())
}

/// Native preparation retains the identical source lease and checked trace at a literal Work boundary.
/// ´test:integration:kernel-live-preparation-work-boundary´
/// ´claim:matrix:kernel-live-preparation-work-boundary´
#[test]
fn kernel_live_preparation_work_boundary() -> TestResult {
    for work in [5254, 5253] {
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
        if work == 5254 {
            result?;
            assert_eq!(budget.usage().work, work);
        } else {
            let Err(LiveDecisionError::Admission(fault)) = result else {
                panic!("native preparation must exhaust before selection ownership");
            };
            let nrz_bindings::DecisionError::Algebra(fault) = *fault else {
                panic!("native preparation must name Work exhaustion");
            };
            exhausted(&fault, work, 5254);
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
/// ´test:integration:kernel-live-wire-work-boundaries´
/// ´claim:matrix:kernel-live-wire-work-boundaries´
#[test]
fn kernel_live_wire_work_boundaries() -> TestResult {
    let mut source = Fixture::new()?;
    let trace = source.trace(&mut ledger()?)?;
    let witness = select(&mut source.source, &trace, &mut ledger()?)?;
    let graph = &source.source.frame.cas;
    let before = graph.counts();
    let mut exact = allowance(1917)?;
    let bytes = graph.export_witness(&witness, &mut exact)?;
    assert_eq!(exact.usage().work, 1917);
    assert_eq!(bytes.len(), 1900);
    let mut short = allowance(1916)?;
    let Err(fault) = graph.export_witness(&witness, &mut short) else {
        panic!("export must refuse below its byte-copy Work boundary");
    };
    exhausted(&fault, 1916, 1917);
    assert_eq!(graph.counts(), before);
    for work in [477_757, 477_756] {
        let mut receiver = Cas::new(Limits::default())?;
        let before = receiver.counts();
        let mut budget = allowance(work)?;
        let limits = budget.limits().clone();
        let result = receiver.inspect(&bytes, &mut budget);
        if work == 477_757 {
            let witness = result?;
            citations(&receiver, &witness)?;
            assert_eq!(budget.usage().work, work);
            assert_eq!(receiver.export_witness(&witness, &mut ledger()?)?, bytes);
        } else {
            let Err(fault) = result else {
                panic!("fresh scalar replay must exhaust below its full Work boundary");
            };
            exhausted(&fault, work, 477_757);
            assert_eq!(receiver.counts(), before);
        }
        assert_eq!(budget.limits(), &limits);
    }
    Ok(())
}

/// Count one independent inclusive containment relation over already exported rationals.
fn contains(record: &ScalarKernelRecord, value: &Rational, budget: &mut Budget) -> TestResult {
    assert_ne!(record.lower.compare(value, budget)?, Ordering::Greater);
    assert_ne!(value.compare(&record.upper, budget)?, Ordering::Greater);
    Ok(())
}

/// Independently price full native-set and arithmetic relations, excluding Machin series replay.
fn relations(records: &[ScalarKernelRecord], budget: &mut Budget) -> TestResult {
    for record in records {
        budget.work(1, Location::new("relation component"))?;
        record.lower.compare(&record.upper, budget)?;
        match &record.law {
            ScalarKernelLaw::Input(input) => {
                budget.work(3, Location::new("input coverage component"))?;
                let lower = input.center.sub(&input.radius, budget)?;
                let upper = input.center.add(&input.radius, budget)?;
                record.lower.compare(&lower, budget)?;
                upper.compare(&record.upper, budget)?;
                match &input.value {
                    ScalarKernelInputValue::Rational(value) => contains(record, value, budget)?,
                    ScalarKernelInputValue::PiMultiple {
                        factor,
                        certificate,
                    } => {
                        contains(record, &factor.mul(&certificate.lower, budget)?, budget)?;
                        contains(record, &factor.mul(&certificate.upper, budget)?, budget)?;
                    }
                }
            }
            ScalarKernelLaw::Reciprocal { input } => {
                let value = &records[usize::try_from(*input)?];
                contains(record, &Rational::one().div(&value.upper, budget)?, budget)?;
                contains(record, &Rational::one().div(&value.lower, budget)?, budget)?;
            }
            ScalarKernelLaw::Product { left, right } => {
                let first = &records[usize::try_from(*left)?];
                let second = &records[usize::try_from(*right)?];
                for a in [&first.lower, &first.upper] {
                    for b in [&second.lower, &second.upper] {
                        contains(record, &a.mul(b, budget)?, budget)?;
                    }
                }
            }
            ScalarKernelLaw::Rational(_)
            | ScalarKernelLaw::Sum { .. }
            | ScalarKernelLaw::SquareRoot { .. } => return Err("unexpected LC law".into()),
        }
    }
    Ok(())
}

/// Reconstruct the exact semantic values through the public independent constant service.
fn meaning(records: &[ScalarKernelRecord], budget: &mut Budget) -> TestResult {
    let mut values: Vec<ExactConstant> = Vec::new();
    for record in records {
        let value = match &record.law {
            ScalarKernelLaw::Input(input) => match &input.value {
                ScalarKernelInputValue::Rational(value) => value.clone().into(),
                ScalarKernelInputValue::PiMultiple { factor, .. } => {
                    ExactConstant::named(Constant::Pi, budget)?
                        .mul(&factor.clone().into(), budget)?
                }
            },
            ScalarKernelLaw::Reciprocal { input } => {
                ExactConstant::one().div(&values[usize::try_from(*input)?], budget)?
            }
            ScalarKernelLaw::Product { left, right } => {
                values[usize::try_from(*left)?].mul(&values[usize::try_from(*right)?], budget)?
            }
            ScalarKernelLaw::Rational(_)
            | ScalarKernelLaw::Sum { .. }
            | ScalarKernelLaw::SquareRoot { .. } => return Err("unexpected LC law".into()),
        };
        values.push(value);
    }
    Ok(())
}

/// Independent receipt components distinguish Machin, full-set relations, exact meaning and ownership.
/// ´test:integration:kernel-live-trace-cost-components´
/// ´claim:matrix:kernel-live-trace-cost-components´
#[test]
fn kernel_live_trace_cost_components() -> TestResult {
    let mut source = Fixture::new()?;
    let trace = source.trace(&mut ledger()?)?;
    let contract = trace.evidence().contract();
    let EnclosureOrigin::ScalarKernel(certificate) = &contract.origin else {
        return Err("scalar origin".into());
    };
    let mut complete = ledger()?;
    let receipt = source
        .source
        .frame
        .cas
        .check_rational_enclosure(contract, &mut complete)?;
    assert_eq!(
        receipt.constant_schema().map(|schema| schema.name),
        Some("constant.pi-machin-bounds")
    );
    let mut machin = ledger()?;
    let checked_pi = pi()?.check(&mut machin)?;
    let machin = machin.usage().work - checked_pi.retained_bytes();
    let mut endpoints = ledger()?;
    relations(&certificate.records, &mut endpoints)?;
    let mut original = ledger()?;
    ExactConstant::from_expression(&source.source.frame.cas, &contract.target, &mut original)?;
    let mut semantic = ledger()?;
    meaning(&certificate.records, &mut semantic)?;
    assert_eq!(complete.usage().work, 106_098);
    assert_eq!(receipt.retained_bytes(), 4672);
    assert_eq!(machin, 18_019);
    assert_eq!(endpoints.usage().work, 57_674);
    assert_eq!(original.usage().work, 27_877);
    assert_eq!(semantic.usage().work, 112_475);
    let citation = 73;
    let mathematical = citation + endpoints.usage().work + original.usage().work;
    assert_eq!(complete.usage().work - mathematical, 20_474);
    Ok(())
}
