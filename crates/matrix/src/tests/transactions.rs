// SPDX-License-Identifier: AGPL-3.0-only
//! Private-candidate corruption at the real consuming publication boundary.

use crate::binding_testing::{TestResult, operation, shape};
use crate::{
    Binding, ExecutionLimits, Extent, Heap, HeapError, HeapLimits, OperationDraft, OperationError,
};
use nrz_units::Descriptor;

/// Each later candidate may fail after previous copies passed, without publishing a prefix.
/// ´test:crate:connected-candidate-refusals-publish-no-prefix´
#[test]
fn connected_candidate_refusals_publish_no_prefix() -> TestResult {
    for rejected in 0..3 {
        let mut heap = Heap::new(HeapLimits::new(8, 8, 2))?;
        let input = heap.import_declared_bounds(
            Extent::new(1, 1)?,
            &Descriptor::ratio(),
            &[2.0_f64],
            &[0.25],
        )?;
        let stamp = heap.value(input.value())?.stamp();
        let recipe = OperationDraft::copy(vec![shape(1, 1, &Descriptor::ratio())?; 3])
            .check(ExecutionLimits::default())?;
        let bound = Binding::new(recipe, vec![input.value(); 3], ExecutionLimits::default())
            .validate(&mut heap)?;
        let mut candidate = bound.evaluate(bound.normalized_inputs()?)?;
        *candidate
            .outputs
            .get_mut(rejected)
            .ok_or("missing candidate")?
            .centers
            .first_mut()
            .ok_or("missing center")? = f64::NAN;
        let failure = bound
            .publish(candidate)
            .err()
            .ok_or("corrupt candidate admitted")?;
        assert!(matches!(
            failure.cause(),
            OperationError::Heap(HeapError::NonFinite { index: 0 })
        ));
        assert_eq!(failure.record().historical_evidence().len(), rejected);
        for index in 0..rejected {
            assert_eq!(failure.record().evidence(&heap, index)?.entries(), 1);
        }
        assert_eq!(heap.descriptors.iter().count(), 2);
        assert_eq!(heap.provenance_usage().records(), 0);
        assert_eq!(heap.value(input.value())?.stamp(), stamp);
        let retry = Binding::new(
            operation(1, 1)?,
            vec![input.value()],
            ExecutionLimits::default(),
        )
        .validate(&mut heap)?;
        let candidate = retry.evaluate(retry.normalized_inputs()?)?;
        assert_eq!(retry.publish(candidate)?.output().len(), 1);
    }
    Ok(())
}

/// Missing companions and wrong reserved target versions refuse the complete bundle.
/// ´test:crate:candidate-companion-contract-refusals´
#[test]
fn candidate_companion_contract_refusals() -> TestResult {
    for fault in 0..3 {
        let mut heap = Heap::new(HeapLimits::default())?;
        let input = heap.import_declared_bounds(
            Extent::new(1, 1)?,
            &Descriptor::ratio(),
            &[2.0_f64],
            &[0.25],
        )?;
        let bound = Binding::new(
            operation(1, 1)?,
            vec![input.value()],
            ExecutionLimits::default(),
        )
        .validate(&mut heap)?;
        let mut candidate = bound.evaluate(bound.normalized_inputs()?)?;
        let output = candidate.outputs.first_mut().ok_or("missing candidate")?;
        match fault {
            0 => output.bounds = None,
            1 => output.target.version = crate::ValueVersion(2),
            _ => *output.centers.first_mut().ok_or("missing center")? = 3.0,
        }
        let failure = bound
            .publish(candidate)
            .err()
            .ok_or("invalid contract admitted")?;
        assert!(matches!(failure.cause(), OperationError::CandidateContract));
        assert_eq!(heap.descriptors.iter().count(), 2);
        assert_eq!(
            heap.value(input.value())?.region().get(0, 0)?.to_bits(),
            2.0_f64.to_bits()
        );
    }
    Ok(())
}

/// Companion targets are revalidated after all numerical candidates have succeeded.
/// ´test:crate:reserved-companion-target-refuses-before-commit´
#[test]
fn reserved_companion_target_refuses_before_commit() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let input = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[2.0_f64],
        &[0.25],
    )?;
    let mut bound = Binding::new(
        operation(1, 1)?,
        vec![input.value()],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let companion = bound
        .reservations
        .records
        .values
        .get_mut(1)
        .ok_or("missing companion")?;
    let crate::heap::records::Meaning::Bound { target, .. } = &mut companion.meaning else {
        return Err("wrong category".into());
    };
    *target = input.value().key;
    let failure = bound
        .publish(candidate)
        .err()
        .ok_or("wrong target admitted")?;
    assert!(matches!(failure.cause(), OperationError::CandidateContract));
    assert_eq!(failure.record().evidence(&heap, 0)?.entries(), 1);
    assert_eq!(heap.descriptors.iter().count(), 2);
    Ok(())
}

/// A consumed token stays refused even if private state is forged to pass the binding gate.
/// ´test:crate:consumed-candidate-token-cannot-be-replayed´
#[test]
fn consumed_candidate_token_cannot_be_replayed() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
    let first = Binding::new(
        operation(1, 1)?,
        vec![source.value()],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    let candidate = first.evaluate(first.normalized_inputs()?)?;
    let consumed = candidate.token;
    first.publish(candidate)?;
    let before = heap.provenance_usage();
    let second = Binding::new(
        operation(1, 1)?,
        vec![source.value()],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    let mut replay = second.evaluate(second.normalized_inputs()?)?;
    replay.token = consumed;
    second.issued.set(Some(consumed));
    let refusal = second
        .publish(replay)
        .err()
        .ok_or("replayed token admitted")?;
    assert!(matches!(refusal.cause(), OperationError::ForeignCandidate));
    assert_eq!(heap.provenance_usage(), before);
    assert_eq!(heap.descriptors.iter().count(), 2);
    Ok(())
}

/// Companion occupations are independent dependencies even when the base version is unchanged.
/// ´test:crate:companion-only-occupation-change-invalidates-derived-evidence´
#[test]
fn companion_only_occupation_change_invalidates_derived_evidence() -> TestResult {
    use crate::heap::records::{Meaning, Record};
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[2.0_f64],
        &[0.25],
    )?;
    let captured = heap.value(source.value())?.stamp();
    let old_bound = source.bound().ok_or("missing source bound")?;
    let binding = Binding::new(
        operation(1, 1)?,
        vec![source.value()],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?;
    let candidate = binding.evaluate(binding.normalized_inputs()?)?;
    let outcome = binding.publish(candidate)?;
    let dependent = outcome
        .output()
        .first()
        .ok_or("missing output")?
        .bound()
        .ok_or("missing dependent bound")?;
    let old = heap.descriptors.get(old_bound.key)?;
    let replacement = Record {
        schema: old.schema,
        location: old.location,
        layout: old.layout.clone(),
        meaning: Meaning::Bound {
            target: source.value().key,
            stamp: captured,
        },
    };
    let plan = heap.descriptors.change(
        1,
        None,
        std::slice::from_ref(&old_bound.key),
        heap.limits.descriptors,
    )?;
    let current = *plan.keys().first().ok_or("missing new companion key")?;
    heap.descriptors.publish(plan.bind(vec![replacement])?);
    let Meaning::Value { bound, .. } = &mut heap.descriptors.get_mut(source.value().key)?.meaning
    else {
        return Err("missing base value".into());
    };
    *bound = Some(current);
    assert_eq!(heap.value(source.value())?.stamp(), captured);
    assert!(matches!(
        heap.companion(old_bound),
        Err(HeapError::StaleHandle)
    ));
    assert!(matches!(
        heap.companion(dependent),
        Err(HeapError::StaleEvidence)
    ));
    assert!(matches!(
        heap.operation_record(outcome.record()),
        Err(HeapError::StaleEvidence)
    ));
    Ok(())
}
