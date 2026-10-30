// SPDX-License-Identifier: AGPL-3.0-only
//! The original LC count is admitted from current boxes and retires with every derived descriptor.

use super::{
    decision_fixture::ledger,
    decision_frozen::{decision_source, inputs},
};
use crate::binding_testing::TestResult;
use nrz_bindings::readings::algebra::{Cas, proof::VerifiedWitness};
use nrz_matrix::{Coverage, DecisionSnapshotError, Heap, HeapError, HeapLimits, LiveDecisionError};
use nrz_scalar::{ArithmeticContext, LiteralLimits};
use std::rc::Rc;

/// A bounded exact-endpoint decode attempt; both attempts are prepaid on the original ledger.
const fn literals() -> LiteralLimits {
    LiteralLimits::new(32, 32, 1024)
}

/// Preserve the old certified selection cost while measuring the receiving owner separately.
/// ´test:integration:decision-live-lc-costs´
#[test]
fn decision_live_lc_costs() -> TestResult {
    let (association, mut source) = decision_source()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let inputs = inputs(&mut heap, &association)?;
    let mut budget = ledger()?;
    let frozen = heap.freeze_decision_inputs(&association, &inputs, &mut budget)?;
    let capture = budget.usage().work;
    let handle = frozen.operands()[0].input().handle();
    let stamp = frozen.operands()[0].input().stamp();
    let center = frozen.operands()[0].coordinates()[0];
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut budget)?;
    let selection = budget.usage().work;
    assert_eq!(selection - capture, 166_952);
    let prepared = heap.prepare_ratio_decision(
        frozen,
        Rc::new(source.frame.cas),
        witness,
        &ArithmeticContext::new()?,
        literals(),
        &mut budget,
    )?;
    let admission = budget.usage().work;
    let published = heap.publish_decision(prepared, &mut budget)?;
    let publication = budget.usage().work;
    let count = published.count(&heap, 3, &mut budget)?;
    let materialization = budget.usage().work;
    assert_eq!(count.current(&heap, &mut budget)?.value(), 3);
    let current = budget.usage().work;
    let foreign = Heap::new(HeapLimits::default())?;
    assert!(matches!(
        published.current(&foreign, &mut ledger()?),
        Err(LiveDecisionError::Snapshot(DecisionSnapshotError::Heap(
            HeapError::ForeignHandle
        )))
    ));
    let archived = published.current(&heap, &mut ledger()?)?.clone();
    let cached_count = count.clone();
    let layout = heap.value(handle)?.region().layout().clone();
    let alias = heap.alias(handle, &layout)?;
    heap.replace_declared_bounds(alias, stamp.version(), &[center], &[1.0 / 1024.0])?;
    assert!(matches!(
        cached_count.current(&heap, &mut ledger()?),
        Err(LiveDecisionError::Snapshot(DecisionSnapshotError::Heap(
            HeapError::StaleEvidence
        )))
    ));
    assert!(matches!(
        count.current(&heap, &mut ledger()?),
        Err(LiveDecisionError::Retired)
    ));
    assert!(matches!(
        published.current(&heap, &mut ledger()?),
        Err(LiveDecisionError::Retired)
    ));
    assert_eq!(
        archived.value(),
        &nrz_bindings::readings::algebra::Rational::from_ratio(3, 1, &mut ledger()?)?
    );
    assert_eq!(capture, 2952);
    assert_eq!(admission - selection, 2779);
    assert_eq!(publication - admission, 193);
    assert_eq!(materialization - publication, 125);
    assert_eq!(current - materialization, 66);
    assert_eq!(materialization, 173_001);
    assert_eq!(current, 173_067);
    Ok(())
}

/// The mathematical certificate remains portable while a wider live box refuses the selection.
/// ´test:integration:decision-live-radius-crossing-refuses-readmission´
#[test]
fn decision_live_radius_crossing_refuses_readmission() -> TestResult {
    let (association, mut source) = decision_source()?;
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut ledger()?)?;
    let graph = Rc::new(source.frame.cas);
    let before = graph.counts();
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut bindings = Vec::new();
    for operand in association.operands() {
        let (center, total) = if operand.name() == "duration" {
            (1311.0_f64 / 131_072.0, 1.0 / 1024.0)
        } else {
            (103.0_f64 / 32_768.0, 1.0 / 262_144.0)
        };
        bindings.push(heap.import_symbols(
            operand.symbols(),
            &[center],
            Some(&[total]),
            Some(&[0.0]),
            Coverage::DeclaredBounds,
        )?);
    }
    let retained = heap.provenance_usage();
    let frozen = heap.freeze_decision_inputs(&association, &bindings, &mut ledger()?)?;
    assert!(matches!(
        heap.prepare_ratio_decision(
            frozen,
            Rc::clone(&graph),
            witness,
            &ArithmeticContext::new()?,
            literals(),
            &mut ledger()?
        ),
        Err(LiveDecisionError::Enclosure)
    ));
    assert_eq!(heap.provenance_usage(), retained);
    assert_eq!(graph.counts(), before);
    Ok(())
}

/// Construct a complete private attachment for independent publication refusal twins.
fn prepared(
    heap: &Heap,
    association: &nrz_bindings::DecisionInputs,
    inputs: &[nrz_matrix::ValueBinding<f64>],
    graph: &Rc<Cas>,
    witness: &VerifiedWitness,
) -> TestResult<nrz_matrix::PreparedDecision<f64>> {
    let frozen = heap.freeze_decision_inputs(association, inputs, &mut ledger()?)?;
    Ok(heap.prepare_ratio_decision(
        frozen,
        Rc::clone(graph),
        witness.clone(),
        &ArithmeticContext::new()?,
        literals(),
        &mut ledger()?,
    )?)
}

/// Foreign publication and a late input mutation expose no partial selected attachment.
/// ´test:integration:decision-live-publication-refusals´
#[test]
fn decision_live_publication_refusals() -> TestResult {
    let (association, mut source) = decision_source()?;
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut ledger()?)?;
    let graph = Rc::new(source.frame.cas);
    let mut heap = Heap::new(HeapLimits::default())?;
    let inputs = inputs(&mut heap, &association)?;
    let foreign = Heap::new(HeapLimits::default())?;
    let candidate = prepared(&heap, &association, &inputs, &graph, &witness)?;
    assert!(matches!(
        foreign.publish_decision(candidate, &mut ledger()?),
        Err(LiveDecisionError::Snapshot(DecisionSnapshotError::Heap(
            HeapError::ForeignHandle
        )))
    ));
    let frozen = heap.freeze_decision_inputs(&association, &inputs, &mut ledger()?)?;
    let handle = frozen.operands()[0].input().handle();
    let stamp = frozen.operands()[0].input().stamp();
    let center = frozen.operands()[0].coordinates()[0];
    let candidate = prepared(&heap, &association, &inputs, &graph, &witness)?;
    heap.replace_declared_bounds(handle, stamp.version(), &[center], &[1.0 / 1024.0])?;
    let before = heap.provenance_usage();
    assert!(matches!(
        heap.publish_decision(candidate, &mut ledger()?),
        Err(LiveDecisionError::Snapshot(DecisionSnapshotError::Heap(
            HeapError::StaleEvidence
        )))
    ));
    assert_eq!(heap.provenance_usage(), before);
    assert_eq!(
        heap.value(handle)?.region().get(0, 0)?.to_bits(),
        center.to_bits()
    );
    Ok(())
}

/// Publication and count failures spend Work without publishing or retiring an existing result.
/// ´test:integration:decision-live-publication-work-boundaries´
#[test]
fn decision_live_publication_work_boundaries() -> TestResult {
    let (association, mut source) = decision_source()?;
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut ledger()?)?;
    let graph = Rc::new(source.frame.cas);
    let mut heap = Heap::new(HeapLimits::default())?;
    let inputs = inputs(&mut heap, &association)?;
    let before = heap.provenance_usage();
    for (work, output, accepted) in [
        (210, 16_850, true),
        (209, 16_850, false),
        (210, 16_849, false),
    ] {
        let candidate = prepared(&heap, &association, &inputs, &graph, &witness)?;
        let mut budget = nrz_bindings::Budget::new(nrz_bindings::Limits {
            work,
            output_bytes: output,
            ..nrz_bindings::Limits::default()
        })?;
        budget.work(
            17,
            nrz_bindings::readings::algebra::Location::new("prior publication caller"),
        )?;
        let result = heap.publish_decision(candidate, &mut budget);
        if accepted {
            result?.current(&heap, &mut ledger()?)?;
            assert_eq!(budget.usage().work, 210);
        } else {
            let resources = matches!(
                result,
                Err(LiveDecisionError::Algebra(_)
                    | LiveDecisionError::Snapshot(DecisionSnapshotError::Resources(_)))
            );
            assert!(resources);
        }
        assert_eq!(heap.provenance_usage(), before);
        let monotone = budget.usage().work >= 17 && budget.usage().work <= work;
        assert!(monotone);
    }
    let candidate = prepared(&heap, &association, &inputs, &graph, &witness)?;
    let published = heap.publish_decision(candidate, &mut ledger()?)?;
    for (work, accepted) in [(142, true), (141, false)] {
        let mut budget = nrz_bindings::Budget::new(nrz_bindings::Limits {
            work,
            ..nrz_bindings::Limits::default()
        })?;
        budget.work(
            17,
            nrz_bindings::readings::algebra::Location::new("prior count caller"),
        )?;
        let result = published.count(&heap, 3, &mut budget);
        assert_eq!(result.is_ok(), accepted);
        if accepted {
            assert_eq!(result?.current(&heap, &mut ledger()?)?.value(), 3);
            assert_eq!(budget.usage().work, 142);
        }
        let monotone = budget.usage().work >= 17 && budget.usage().work <= work;
        assert!(monotone);
        published.current(&heap, &mut ledger()?)?;
    }
    assert!(matches!(
        published.count(&heap, 2, &mut ledger()?),
        Err(LiveDecisionError::Admission(_))
    ));
    published.current(&heap, &mut ledger()?)?;
    assert_eq!(heap.provenance_usage(), before);
    Ok(())
}

/// Equal coherent centers with a changed exact period map cannot reuse the old attachment.
/// ´test:integration:decision-live-exact-map-refusal´
#[test]
fn decision_live_exact_map_refusal() -> TestResult {
    let (association, mut source) = decision_source()?;
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut ledger()?)?;
    let graph = Rc::new(source.frame.cas);
    let mut heap = Heap::new(HeapLimits::default())?;
    let bindings = inputs(&mut heap, &association)?;
    let candidate = prepared(&heap, &association, &bindings, &graph, &witness)?;
    let published = heap.publish_decision(candidate, &mut ledger()?)?;
    let (changed, mut changed_source) = super::decision_frozen::decision_source_map(true)?;
    let changed_bindings = inputs(&mut heap, &changed)?;
    for (original, replacement) in bindings.iter().zip(&changed_bindings) {
        assert_eq!(
            heap.value_by_symbol(original.symbol(), original)?
                .region()
                .get(0, 0)?
                .to_bits(),
            heap.value_by_symbol(replacement.symbol(), replacement)?
                .region()
                .get(0, 0)?
                .to_bits()
        );
    }
    assert_eq!(
        changed.case().source().exact(),
        association.case().source().exact()
    );
    assert_ne!(
        changed.operands()[1].original()?.descriptor(),
        association.operands()[1].original()?.descriptor()
    );
    assert!(matches!(
        published.current_for(&heap, &changed, &mut ledger()?),
        Err(LiveDecisionError::Admission(_))
    ));
    let mut budget = ledger()?;
    published.current_for(&heap, &association, &mut budget)?;
    assert_eq!(budget.usage().work, 74);
    let frozen = heap.freeze_decision_inputs(&changed, &changed_bindings, &mut ledger()?)?;
    let witness = changed_source.frame.cas.check_proof(
        &changed_source.context,
        &changed_source.steps,
        &mut ledger()?,
    )?;
    let candidate = heap.prepare_ratio_decision(
        frozen,
        Rc::new(changed_source.frame.cas),
        witness,
        &ArithmeticContext::new()?,
        literals(),
        &mut ledger()?,
    )?;
    let replacement = heap.publish_decision(candidate, &mut ledger()?)?;
    let count = replacement.count(&heap, 3, &mut ledger()?)?;
    assert_eq!(count.current(&heap, &mut ledger()?)?.value(), 3);
    Ok(())
}

/// Preparation, current access and the full physical lifecycle use literal cumulative ceilings.
/// ´test:integration:decision-live-admission-work-boundaries´
#[test]
fn decision_live_admission_work_boundaries() -> TestResult {
    let (association, mut source) = decision_source()?;
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut ledger()?)?;
    let graph = Rc::new(source.frame.cas);
    let mut heap = Heap::new(HeapLimits::default())?;
    let bindings = inputs(&mut heap, &association)?;
    let before = heap.provenance_usage();
    for (work, accepted) in [(2796, true), (2795, false)] {
        let frozen = heap.freeze_decision_inputs(&association, &bindings, &mut ledger()?)?;
        let mut budget = nrz_bindings::Budget::new(nrz_bindings::Limits {
            work,
            ..nrz_bindings::Limits::default()
        })?;
        budget.work(
            17,
            nrz_bindings::readings::algebra::Location::new("prior live preparation"),
        )?;
        let result = heap.prepare_ratio_decision(
            frozen,
            Rc::clone(&graph),
            witness.clone(),
            &ArithmeticContext::new()?,
            literals(),
            &mut budget,
        );
        assert_eq!(result.is_ok(), accepted);
        if accepted {
            assert_eq!(budget.usage().work, 2796);
        }
        let monotone = budget.usage().work >= 17 && budget.usage().work <= work;
        assert!(monotone);
        assert_eq!(heap.provenance_usage(), before);
    }
    let candidate = prepared(&heap, &association, &bindings, &graph, &witness)?;
    let published = heap.publish_decision(candidate, &mut ledger()?)?;
    let count = published.count(&heap, 3, &mut ledger()?)?;
    for (work, accepted) in [(83, true), (82, false)] {
        let mut budget = nrz_bindings::Budget::new(nrz_bindings::Limits {
            work,
            ..nrz_bindings::Limits::default()
        })?;
        budget.work(
            17,
            nrz_bindings::readings::algebra::Location::new("prior live count access"),
        )?;
        let result = count.current(&heap, &mut budget);
        assert_eq!(result.is_ok(), accepted);
        if accepted {
            assert_eq!(result?.value(), 3);
            assert_eq!(budget.usage().work, 83);
        }
        published.current(&heap, &mut ledger()?)?;
    }
    Ok(())
}

/// Physical source, case, association and every live stage share the original caller allowance.
/// ´test:integration:decision-live-complete-lc-work-boundaries´
#[test]
fn decision_live_complete_lc_work_boundaries() -> TestResult {
    for (work, accepted) in [(282_308, true), (282_307, false)] {
        let mut budget = nrz_bindings::Budget::new(nrz_bindings::Limits {
            work,
            ..nrz_bindings::Limits::default()
        })?;
        budget.work(
            17,
            nrz_bindings::readings::algebra::Location::new("prior complete LC caller"),
        )?;
        let (association, mut source) =
            super::decision_frozen::decision_source_budget(false, &mut budget)?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let bindings = inputs(&mut heap, &association)?;
        let before = heap.provenance_usage();
        let frozen = heap.freeze_decision_inputs(&association, &bindings, &mut budget)?;
        let witness = source
            .frame
            .cas
            .check_proof(&source.context, &source.steps, &mut budget)?;
        let candidate = heap.prepare_ratio_decision(
            frozen,
            Rc::new(source.frame.cas),
            witness,
            &ArithmeticContext::new()?,
            literals(),
            &mut budget,
        )?;
        let published = heap.publish_decision(candidate, &mut budget)?;
        let result = published.count(&heap, 3, &mut budget);
        assert_eq!(result.is_ok(), accepted);
        if accepted {
            assert_eq!(result?.current(&heap, &mut ledger()?)?.value(), 3);
            assert_eq!(budget.usage().work, 282_308);
        }
        let monotone = budget.usage().work >= 17 && budget.usage().work <= work;
        assert!(monotone);
        assert_eq!(heap.provenance_usage(), before);
    }
    Ok(())
}

/// The measured scalar prerequisite preserves its complete caller ledger and retirement costs.
/// ´test:integration:lattice-live-scalar-baseline´
/// ´claim:matrix:lattice-live-scalar-baseline´
#[test]
fn lattice_live_scalar_baseline() -> TestResult {
    let mut budget = ledger()?;
    let (association, mut source) =
        super::decision_frozen::decision_source_budget(false, &mut budget)?;
    let originals = budget.usage().work;
    let mut heap = Heap::new(HeapLimits::default())?;
    let bindings = inputs(&mut heap, &association)?;
    let records = heap.provenance_usage();
    let frozen = heap.freeze_decision_inputs(&association, &bindings, &mut budget)?;
    let captured = budget.usage().work;
    let handle = frozen.operands()[0].input().handle();
    let stamp = frozen.operands()[0].input().stamp();
    let center = frozen.operands()[0].coordinates()[0];
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut budget)?;
    let selected = budget.usage().work;
    let graph = Rc::new(source.frame.cas);
    let graph_records = graph.counts();
    let prepared = heap.prepare_ratio_decision(
        frozen,
        Rc::clone(&graph),
        witness,
        &ArithmeticContext::new()?,
        literals(),
        &mut budget,
    )?;
    let admitted = budget.usage().work;
    let published = heap.publish_decision(prepared, &mut budget)?;
    let attached = budget.usage().work;
    let count = published.count(&heap, 3, &mut budget)?;
    let materialized = budget.usage().work;
    assert_eq!(count.current(&heap, &mut budget)?.value(), 3);
    let current = budget.usage().work;
    heap.replace_declared_bounds(handle, stamp.version(), &[center], &[1.0 / 1024.0])?;
    let Err(fault) = count.current(&heap, &mut budget) else {
        panic!("changed radius must retire the count");
    };
    assert!(matches!(fault, LiveDecisionError::Snapshot(_)));
    let retired = budget.usage().work;
    let Err(fault) = published.current(&heap, &mut budget) else {
        panic!("retired parent must refuse");
    };
    assert!(matches!(fault, LiveDecisionError::Retired));
    assert_eq!(graph.counts(), graph_records);
    assert_eq!(heap.provenance_usage(), records);
    assert_eq!(originals, 109_290);
    assert_eq!(captured - originals, 2952);
    assert_eq!(selected - captured, 166_952);
    assert_eq!(admitted - selected, 2779);
    assert_eq!(attached - admitted, 193);
    assert_eq!(materialized - attached, 125);
    assert_eq!(current - materialized, 66);
    assert_eq!(retired - current, 34);
    assert_eq!(budget.usage().work - retired, 1);
    assert_eq!(budget.usage().work, 282_392);
    assert_eq!(graph_records.vertices, 35);
    assert_eq!(graph_records.edges, 94);
    assert_eq!(records.records(), 0);
    Ok(())
}

/// Numerical origins must be checked independently before any live lattice selection.
/// ´test:integration:lattice-live-origin-refusal´
/// ´claim:matrix:lattice-live-origin-refusal´
#[test]
fn lattice_live_origin_refusal() -> TestResult {
    use nrz_bindings::readings::algebra::{
        FaultKind, InvalidCode, Location, decision::EnclosureOrigin, proof::Certificate,
    };
    let source = super::decision_fixture::fixture(false, "3", "10/Pi")?;
    let proposal = source.steps.last().ok_or("missing selection")?;
    let Certificate::IntegerSelection(original) = &proposal.certificate else {
        return Err("missing original enclosure".into());
    };
    for origin in [EnclosureOrigin::ScalarKernel, EnclosureOrigin::MatrixKernel] {
        let mut contract = original.clone();
        contract.origin = origin;
        let before = source.frame.cas.counts();
        let mut budget = ledger()?;
        budget.work(
            17,
            nrz_bindings::readings::algebra::Location::new("prior origin probe"),
        )?;
        let Err(fault) = source
            .frame
            .cas
            .check_rational_enclosure(&contract, &mut budget)
        else {
            panic!("unsupported numerical origin must refuse");
        };
        assert_eq!(source.frame.cas.counts(), before);
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::UnsupportedCertificate)
        );
        assert_eq!(fault.location, Location::new("exact rational enclosure"));
        assert_eq!(budget.usage().work, 17);
    }
    Ok(())
}
