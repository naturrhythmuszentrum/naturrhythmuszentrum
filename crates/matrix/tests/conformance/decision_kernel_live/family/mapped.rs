// SPDX-License-Identifier: AGPL-3.0-only
//! Actual affine scalar operations retain the defined reading, source premises and complete live lease.

mod costs;
mod versions;

use super::{TestResult, associate, inputs, ledger, physical, pi, select};
use crate::conformance::decision_fixture::{SelectionFixture, mapped_fixture};
use nrz_bindings::readings::algebra::{
    Cas, Handle, Location,
    decision::{EnclosureOrigin, ScalarKernelLaw},
};
use nrz_bindings::{Budget, DecisionExpression, DecisionInputs};
use nrz_matrix::{
    DecisionSnapshotError, FrozenRatioKernel, Heap, HeapError, HeapLimits, LiveDecisionError,
};
use nrz_scalar::{ArithmeticContext, KernelLaw};
use std::sync::{Arc, Mutex};

/// Bind the original physical quotient to its actual one-entry affine reading definition.
fn mapped_source(budget: &mut Budget) -> TestResult<(DecisionInputs, SelectionFixture)> {
    let (source, family, reading) = mapped_fixture()?;
    let physical = DecisionExpression::mapped_family(
        &source.frame.cas,
        physical()?,
        "quotient",
        &source.context,
        vec![("duration".into(), family)],
        vec![("duration".into(), reading)],
        budget,
    )?;
    let association = associate(&source, physical, budget)?;
    Ok((association, source))
}

/// All actual imports, products and sums retain exact intermediate correspondence and map identity.
fn records(trace: &FrozenRatioKernel<f64>, definition: &Handle) -> TestResult {
    let evidence = trace.evidence();
    assert_eq!(evidence.scalar_usage().work, 1796);
    assert_eq!(evidence.scalar_usage().exported, 8);
    let records = evidence.records();
    assert_eq!(records.len(), 8);
    assert!(matches!(
        records[2].law,
        KernelLaw::Rational {
            numerator: 2,
            denominator: 1
        }
    ));
    assert!(matches!(
        records[3].law,
        KernelLaw::Rational {
            numerator: -1,
            denominator: 100
        }
    ));
    let KernelLaw::Product { left, right } = records[4].law else {
        return Err("mapped product".into());
    };
    assert_eq!(left, records[0].output);
    assert_eq!(right, records[2].output);
    let KernelLaw::Sum { left, right } = records[5].law else {
        return Err("mapped sum".into());
    };
    assert_eq!(left, records[4].output);
    assert_eq!(right, records[3].output);
    let EnclosureOrigin::ScalarKernel(certificate) = &evidence.contract().origin else {
        return Err("mapped origin".into());
    };
    assert_eq!(certificate.version, 4);
    assert_eq!(certificate.records.len(), 5);
    let ScalarKernelLaw::Reading(reading) = &certificate.records[2].law else {
        return Err("map law".into());
    };
    assert_eq!(&reading.definition, definition);
    assert_eq!(reading.input, 0);
    Ok(())
}

/// The full family selects three cycles, survives fresh replay and retires on a radius-only write.
/// ´test:integration:kernel-mapped-family-lifecycle´
/// ´claim:matrix:kernel-mapped-family-lifecycle´
#[test]
fn kernel_mapped_family_lifecycle() -> TestResult {
    let mut setup = ledger()?;
    let (association, mut source) = mapped_source(&mut setup)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let bindings = inputs(&mut heap, &association)?;
    let heap_before = heap.provenance_usage();
    let mut budget = ledger()?;
    budget.work(17, Location::new("prior live mapped family"))?;
    let frozen = heap.freeze_decision_inputs(&association, &bindings, &mut budget)?;
    let captured = budget.usage().work;
    let trace = heap.trace_ratio_decision(
        frozen,
        &mut source.frame.cas,
        &pi()?,
        &ArithmeticContext::new()?,
        &mut budget,
    )?;
    let traced = budget.usage().work;
    let retained = trace.retained_payload_bytes();
    records(
        &trace,
        association.case().source().input_readings()[0].1.node(),
    )?;
    let input = &trace.inputs().operands()[0];
    let handle = input.input().handle();
    let stamp = input.input().stamp();
    let center = input.coordinates()[0];
    let witness = select(&mut source, &trace, &mut budget)?;
    let selected = budget.usage().work;
    assert_eq!(witness.hypotheses().len(), 3);
    let mut wire = ledger()?;
    let bytes = source.frame.cas.export_witness(&witness, &mut wire)?;
    let exported = wire.usage().work;
    let mut receiver = Cas::new(nrz_bindings::Limits::default())?;
    let fresh = receiver.inspect(&bytes, &mut wire)?;
    let replayed = wire.usage().work - exported;
    assert_eq!(fresh.hypotheses().len(), 3);
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
    let count = published.count(&heap, 3, &mut budget)?;
    assert_eq!(count.current(&heap, &mut budget)?.value(), 3);
    let current = budget.usage().work;
    let layout = heap.value(handle)?.region().layout().clone();
    let alias = heap.alias(handle, &layout)?;
    heap.replace_declared_bounds(alias, stamp.version(), &[center], &[1.0 / 1024.0])?;
    let Err(fault) = count.current(&heap, &mut budget) else {
        panic!("changed family radius must retire the attachment");
    };
    assert!(matches!(
        fault,
        LiveDecisionError::Snapshot(DecisionSnapshotError::Heap(HeapError::StaleEvidence))
    ));
    let Err(fault) = published.current(&heap, &mut budget) else {
        panic!("retired family attachment must refuse");
    };
    assert!(matches!(fault, LiveDecisionError::Retired));
    assert_eq!(heap.provenance_usage(), heap_before);
    assert_eq!(setup.usage().work, 97_513);
    assert_eq!(captured - 17, 2952);
    assert_eq!(traced - captured, 351_397);
    assert_eq!(selected - traced, 365_135);
    assert_eq!(prepared_work - selected, 7272);
    assert_eq!(publication - prepared_work, 193);
    assert_eq!(current - publication, 191);
    assert_eq!(budget.usage().work - current, 35);
    assert_eq!(budget.usage().work, 727_192);
    assert_eq!(setup.usage().work + budget.usage().work - 17, 824_688);
    assert_eq!(retained, 10_408);
    assert_eq!(exported, 2877);
    assert_eq!(bytes.len(), 2877);
    assert_eq!(replayed, 549_133);
    Ok(())
}

/// An equal map under another result declaration cannot replace the retained reading identity.
/// ´test:integration:kernel-mapped-family-reading-substitution´
/// ´claim:matrix:kernel-mapped-family-reading-substitution´
#[test]
fn kernel_mapped_family_reading_substitution() -> TestResult {
    let (mut source, family, original) = mapped_fixture()?;
    let nrz_bindings::readings::algebra::proof::Statement::IntegerSelection(selection) =
        &source.steps.last().ok_or("selection")?.conclusion
    else {
        return Err("selection statement".into());
    };
    let alternate = nrz_bindings::readings::PhysicalReading::define(
        &mut source.frame.cas,
        original.family(),
        nrz_bindings::readings::ReadingDefinition {
            symbol: selection.family.coordinate.clone(),
            selection: nrz_bindings::readings::algebra::readings::SelectionSpec::Entry(
                nrz_bindings::readings::algebra::readings::EntryIndex { row: 0, column: 0 },
            ),
            map: original.map().clone(),
            entries: original.entries().to_vec(),
        },
        &mut ledger()?,
    )?;
    assert_ne!(alternate.node(), original.node());
    assert_eq!(alternate.map(), original.map());
    assert_eq!(alternate.selected(), original.selected());
    let physical = DecisionExpression::mapped_family(
        &source.frame.cas,
        physical()?,
        "quotient",
        &source.context,
        vec![("duration".into(), family)],
        vec![("duration".into(), alternate)],
        &mut ledger()?,
    )?;
    let before = source.frame.cas.counts();
    let mut budget = ledger()?;
    budget.work(17, Location::new("prior reading substitution"))?;
    let Err(fault) = associate(&source, physical, &mut budget) else {
        panic!("a different reading declaration must refuse the original quotient");
    };
    assert!(matches!(
        fault.downcast_ref::<nrz_bindings::DecisionError>(),
        Some(nrz_bindings::DecisionError::Target)
    ));
    assert_eq!(source.frame.cas.counts(), before);
    assert!(budget.usage().work >= 17);
    Ok(())
}
