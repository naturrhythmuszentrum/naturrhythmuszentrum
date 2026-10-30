// SPDX-License-Identifier: AGPL-3.0-only
//! Executed live LC traces select the original 10/Pi target and retain complete source versions.

mod costs;

use super::decision_fixture::{SelectionFixture, cycle, ledger};
use super::decision_frozen::{decision_source, inputs};
use crate::binding_testing::TestResult;
use nrz_bindings::readings::algebra::{
    Cas, FaultKind, InvalidCode, Location, Rational,
    decision::{EnclosureOrigin, ScalarKernelLaw},
    pi_bounds::PiBoundCertificate,
    proof::{Certificate, VerifiedWitness},
};
use nrz_matrix::{
    DecisionSnapshotError, FrozenRatioKernel, Heap, HeapError, HeapLimits, LiveDecisionError,
};
use nrz_scalar::{ArithmeticContext, KernelLaw};
use std::sync::{Arc, Mutex};

/// The same two/one Machin sums with outward endpoints fitting the actual period input box.
fn pi() -> TestResult<PiBoundCertificate> {
    let mut value = cycle()?;
    value.lower = Rational::from_ratio(3215, 1024, &mut ledger()?)?;
    value.upper = Rational::from_ratio(1609, 512, &mut ledger()?)?;
    Ok(value)
}

/// Bind the complete actual trace contract into the existing original-target selection proposal.
fn select(
    source: &mut SelectionFixture,
    trace: &FrozenRatioKernel<f64>,
    budget: &mut nrz_bindings::Budget,
) -> TestResult<VerifiedWitness> {
    let Certificate::IntegerSelection(request) =
        &mut source.steps.last_mut().ok_or("selection")?.certificate
    else {
        return Err("missing selection".into());
    };
    **request = trace.evidence().contract().clone();
    Ok(source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, budget)?)
}

/// Fresh replay must retain the independently checked Machin theorem as an actual citation.
fn citations(cas: &Cas, witness: &VerifiedWitness) -> TestResult {
    let mut count = 0;
    for citation in cas.proof_citations(witness.proof(), &mut ledger()?)? {
        if let Some(schema) = citation.constant_schema(cas, &mut ledger()?)? {
            assert_eq!(
                (schema.name, schema.version),
                ("constant.pi-machin-bounds", 1)
            );
            count += 1;
        }
    }
    assert_eq!(count, 1);
    assert_eq!(witness.hypotheses().len(), 1);
    Ok(())
}

/// Actual input sums, reciprocal and product preserve their complete ordered interval references.
fn records(trace: &FrozenRatioKernel<f64>) -> TestResult {
    let evidence = trace.evidence();
    assert_eq!(evidence.scalar_usage().work, 732);
    assert_eq!(evidence.scalar_usage().constructed, 4);
    assert_eq!(evidence.scalar_usage().exported, 4);
    let records = evidence.records();
    assert!(matches!(records[0].law, KernelLaw::Sum { .. }));
    assert!(matches!(records[1].law, KernelLaw::Sum { .. }));
    let KernelLaw::Reciprocal { input } = records[2].law else {
        return Err("missing reciprocal".into());
    };
    assert_eq!(input, records[1].output);
    let KernelLaw::Product { left, right } = records[3].law else {
        return Err("missing product".into());
    };
    assert_eq!(left, records[0].output);
    assert_eq!(right, records[2].output);
    let EnclosureOrigin::ScalarKernel(certificate) = &evidence.contract().origin else {
        return Err("missing origin".into());
    };
    assert_eq!(certificate.version, 2);
    assert_eq!(certificate.records.len(), 4);
    for record in &certificate.records[..2] {
        let ScalarKernelLaw::Input(input) = &record.law else {
            return Err("missing original input".into());
        };
        assert_eq!(
            input.radius,
            Rational::from_ratio(1, 262_144, &mut ledger()?)?
        );
    }
    Ok(())
}

/// The complete original LC lifecycle retains its executed trace, fresh proof and retirement costs.
/// ´test:integration:kernel-live-trace-lifecycle´
/// ´claim:matrix:kernel-live-trace-lifecycle´
#[test]
fn kernel_live_trace_lifecycle() -> TestResult {
    let mut physical = ledger()?;
    let (association, mut source) =
        super::decision_frozen::decision_source_budget(false, &mut physical)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let bindings = inputs(&mut heap, &association)?;
    let before = heap.provenance_usage();
    let mut budget = ledger()?;
    budget.work(17, Location::new("prior live kernel"))?;
    let frozen = heap.freeze_decision_inputs(&association, &bindings, &mut budget)?;
    let capture = budget.usage().work;
    let counts = source.frame.cas.counts();
    let trace = heap.trace_ratio_decision(
        frozen,
        &mut source.frame.cas,
        &pi()?,
        &ArithmeticContext::new()?,
        &mut budget,
    )?;
    let traced = budget.usage().work;
    let trace_bytes = trace.retained_payload_bytes();
    records(&trace)?;
    assert_eq!(source.frame.cas.counts().vertices, counts.vertices + 2);
    assert_eq!(source.frame.cas.counts().edges, counts.edges + 6);
    assert_eq!(
        source.frame.cas.counts().payload_bytes,
        counts.payload_bytes + 13_206
    );
    let snapshot = &trace.inputs().operands()[0];
    let handle = snapshot.input().handle();
    let stamp = snapshot.input().stamp();
    let center = snapshot.coordinates()[0];
    let witness = select(&mut source, &trace, &mut budget)?;
    let selected = budget.usage().work;
    citations(&source.frame.cas, &witness)?;
    let mut wire = ledger()?;
    let bytes = source.frame.cas.export_witness(&witness, &mut wire)?;
    let exported = wire.usage().work;
    let mut receiver = Cas::new(ledger()?.limits().clone())?;
    let fresh = receiver.inspect(&bytes, &mut wire)?;
    let inspected = wire.usage().work;
    citations(&receiver, &fresh)?;
    assert_eq!(receiver.export_witness(&fresh, &mut ledger()?)?, bytes);
    let prepared = heap.prepare_ratio_kernel_decision(
        trace,
        Arc::new(Mutex::new(source.frame.cas)),
        witness,
        &mut budget,
    )?;
    let prepared_work = budget.usage().work;
    let published = heap.publish_decision(prepared, &mut budget)?;
    let publication = budget.usage().work;
    assert_eq!(published.kernel_trace().ok_or("trace")?.records().len(), 4);
    assert_eq!(published.inputs().operands()[0].input().stamp(), stamp);
    let count = published.count(&heap, 3, &mut budget)?;
    let counted = budget.usage().work;
    assert_eq!(count.current(&heap, &mut budget)?.value(), 3);
    let current = budget.usage().work;
    let layout = heap.value(handle)?.region().layout().clone();
    let alias = heap.alias(handle, &layout)?;
    heap.replace_declared_bounds(alias, stamp.version(), &[center], &[1.0 / 1024.0])?;
    let Err(fault) = count.current(&heap, &mut budget) else {
        panic!("changed radius must retire live kernel count");
    };
    assert!(matches!(
        fault,
        LiveDecisionError::Snapshot(DecisionSnapshotError::Heap(HeapError::StaleEvidence))
    ));
    let Err(fault) = published.current(&heap, &mut budget) else {
        panic!("retired kernel attachment must refuse");
    };
    assert!(matches!(fault, LiveDecisionError::Retired));
    assert_eq!(heap.provenance_usage(), before);
    assert_eq!(capture, 2969);
    assert_eq!(traced - capture, 393_667);
    assert_eq!(selected - traced, 253_628);
    assert_eq!(prepared_work - selected, 5237);
    assert_eq!(publication - prepared_work, 193);
    assert_eq!(counted - publication, 125);
    assert_eq!(current - counted, 66);
    assert_eq!(current, 655_885);
    assert_eq!(exported, 1900);
    assert_eq!(inspected - exported, 477_740);
    assert_eq!(trace_bytes, 8320);
    assert_eq!(bytes.len(), 1900);
    assert_eq!(physical.usage().work, 109_290);
    assert_eq!(budget.usage().work - current, 35);
    assert_eq!(physical.usage().work + budget.usage().work - 17, 765_193);
    Ok(())
}

/// A nominal singleton cannot replace a nonzero full input set at the rational coverage checker.
/// ´test:integration:kernel-live-trace-input-forgery´
/// ´claim:matrix:kernel-live-trace-input-forgery´
#[test]
fn kernel_live_trace_input_forgery() -> TestResult {
    let (association, mut source) = decision_source()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let bindings = inputs(&mut heap, &association)?;
    let frozen = heap.freeze_decision_inputs(&association, &bindings, &mut ledger()?)?;
    let trace = heap.trace_ratio_decision(
        frozen,
        &mut source.frame.cas,
        &pi()?,
        &ArithmeticContext::new()?,
        &mut ledger()?,
    )?;
    let mut request = trace.evidence().contract().clone();
    let EnclosureOrigin::ScalarKernel(certificate) = &mut request.origin else {
        return Err("scalar origin".into());
    };
    certificate.records[0].lower = Rational::from_ratio(1, 100, &mut ledger()?)?;
    certificate.records[0].upper = certificate.records[0].lower.clone();
    let before = source.frame.cas.counts();
    let mut budget = ledger()?;
    budget.work(17, Location::new("prior forged input"))?;
    let Err(fault) = source
        .frame
        .cas
        .check_rational_enclosure(&request, &mut budget)
    else {
        panic!("nominal-only input must refuse");
    };
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::DecisionBox));
    assert_eq!(
        fault.location,
        Location::new("scalar kernel input coverage")
    );
    assert_eq!(source.frame.cas.counts(), before);
    assert!(budget.usage().work > 17);
    let prior = budget.usage().work;
    let mut request = trace.evidence().contract().clone();
    let EnclosureOrigin::ScalarKernel(certificate) = &mut request.origin else {
        return Err("scalar origin".into());
    };
    let ScalarKernelLaw::Input(input) = &mut certificate.records[1].law else {
        return Err("period input".into());
    };
    let nrz_bindings::readings::algebra::decision::ScalarKernelInputValue::PiMultiple {
        certificate,
        ..
    } = &mut input.value
    else {
        return Err("Machin input".into());
    };
    certificate.sums[0] = Rational::zero();
    let Err(fault) = source
        .frame
        .cas
        .check_rational_enclosure(&request, &mut budget)
    else {
        panic!("false retained Machin sum must refuse");
    };
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(fault.location, Location::new("Machin pi bounds"));
    assert_eq!(source.frame.cas.counts(), before);
    assert!(budget.usage().work > prior);
    Ok(())
}

/// A late radius write or a different valid exact box cannot stand for the executed trace.
/// ´test:integration:kernel-live-trace-preparation-refuses´
/// ´claim:matrix:kernel-live-trace-preparation-refuses´
#[test]
fn kernel_live_trace_preparation_refuses() -> TestResult {
    for kind in 0..3 {
        let (association, mut source) = decision_source()?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let bindings = inputs(&mut heap, &association)?;
        let frozen = heap.freeze_decision_inputs(&association, &bindings, &mut ledger()?)?;
        let trace = heap.trace_ratio_decision(
            frozen,
            &mut source.frame.cas,
            &pi()?,
            &ArithmeticContext::new()?,
            &mut ledger()?,
        )?;
        let witness = if kind == 2 {
            let witness = select(&mut source, &trace, &mut ledger()?)?;
            let snapshot = &trace.inputs().operands()[0];
            heap.replace_declared_bounds(
                snapshot.input().handle(),
                snapshot.input().stamp().version(),
                &[snapshot.coordinates()[0]],
                &[1.0 / 1024.0],
            )?;
            witness
        } else if kind == 1 {
            let mut nominal = trace.evidence().contract().clone();
            let EnclosureOrigin::ScalarKernel(certificate) = &mut nominal.origin else {
                return Err("scalar origin".into());
            };
            let first = &mut certificate.records[0];
            first.lower = Rational::from_ratio(1, 100, &mut ledger()?)?;
            first.upper = first.lower.clone();
            let ScalarKernelLaw::Input(input) = &mut first.law else {
                return Err("native input".into());
            };
            input.center = first.lower.clone();
            input.radius = Rational::zero();
            // This remains a valid closed nominal certificate with the same final box.
            source
                .frame
                .cas
                .check_rational_enclosure(&nominal, &mut ledger()?)?;
            let Certificate::IntegerSelection(request) =
                &mut source.steps.last_mut().ok_or("selection")?.certificate
            else {
                return Err("selection".into());
            };
            **request = nominal;
            source
                .frame
                .cas
                .check_proof(&source.context, &source.steps, &mut ledger()?)?
        } else {
            source
                .frame
                .cas
                .check_proof(&source.context, &source.steps, &mut ledger()?)?
        };
        let graph = Arc::new(Mutex::new(source.frame.cas));
        let counts = graph.lock().map_err(|_| "graph owner poisoned")?.counts();
        let before = heap.provenance_usage();
        let Err(fault) =
            heap.prepare_ratio_kernel_decision(trace, Arc::clone(&graph), witness, &mut ledger()?)
        else {
            panic!("substituted trace preparation must refuse");
        };
        if kind == 2 {
            assert!(matches!(
                fault,
                LiveDecisionError::Snapshot(DecisionSnapshotError::Heap(HeapError::StaleEvidence))
            ));
        } else {
            assert!(matches!(fault, LiveDecisionError::Recipe));
        }
        assert_eq!(
            graph.lock().map_err(|_| "graph owner poisoned")?.counts(),
            counts
        );
        assert_eq!(heap.provenance_usage(), before);
    }
    Ok(())
}

/// A nonzero numerical companion is already covered by the total radius and never counted twice.
/// ´test:integration:kernel-live-trace-total-error-once´
/// ´claim:matrix:kernel-live-trace-total-error-once´
#[test]
fn kernel_live_trace_total_error_once() -> TestResult {
    let (association, mut source) = decision_source()?;
    let mut expected = None;
    for numerical in [0.0, 1.0 / 524_288.0] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let mut bindings = Vec::new();
        for operand in association.operands() {
            let center = if operand.name() == "duration" {
                1311.0 / 131_072.0
            } else {
                103.0 / 32_768.0
            };
            bindings.push(heap.import_symbols(
                operand.symbols(),
                &[center],
                Some(&[1.0 / 262_144.0]),
                Some(&[numerical]),
                nrz_matrix::Coverage::DeclaredBounds,
            )?);
        }
        let frozen = heap.freeze_decision_inputs(&association, &bindings, &mut ledger()?)?;
        let trace = heap.trace_ratio_decision(
            frozen,
            &mut source.frame.cas,
            &pi()?,
            &ArithmeticContext::new()?,
            &mut ledger()?,
        )?;
        records(&trace)?;
        if let Some(expected) = &expected {
            assert_eq!(trace.evidence().contract(), expected);
        } else {
            expected = Some(trace.evidence().contract().clone());
        }
        for input in trace.inputs().operands() {
            assert_eq!(input.coordinates()[2].to_bits(), numerical.to_bits());
        }
    }
    Ok(())
}
