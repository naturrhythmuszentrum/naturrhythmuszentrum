// SPDX-License-Identifier: AGPL-3.0-only
//! Actual shared L/C descendants and separate voltage RHS have different source closures.

use super::{TestResult, ledger, produce, setup};
use nrz_scalar::Scalar;

/// Original produced impedance retires with either L/C source; RHS retirement stays graph-scoped.
/// ´test:integration:lc-original-driven-source-invalidation´
#[test]
fn lc_original_driven_source_invalidation() -> TestResult {
    sources::<f32>()?;
    sources::<f64>()?;
    rhs::<f32>()?;
    rhs::<f64>()
}

/// Every actual scalar source and its numerical-only radius remain in all downstream groups.
fn sources<P: Scalar + 'static>() -> TestResult {
    for channel in 0..4 {
        let mut case = setup::<P>(false)?;
        let outputs = produce(
            &mut case,
            &mut crate::phasor_solve_testing::resources::fresh_budget()?,
        )?;
        let current = case.rows[0].current(outputs[0].readings().bindings(), &mut ledger()?)?;
        let current = case.heap.publish_phasor_solve(
            &current,
            &case.context,
            &mut crate::phasor_solve_testing::resources::fresh_budget()?,
        )?;
        let source = &case.spectrum.inputs()[channel / 2];
        if channel % 2 == 0 {
            let values = case.heap.reading_entries(source)?;
            case.heap
                .replace_reading_entries(source, &values, &mut ledger()?)?;
        } else {
            let parameter = source
                .companions()
                .iter()
                .position(|entry| {
                    entry.companion().kind()
                        == nrz_bindings::readings::CompanionReadingKind::Radius(
                            nrz_bindings::CompanionKind::NumericalError,
                        )
                        && entry
                            .targets()
                            .iter()
                            .any(|target| target.reading() == source.reading())
                })
                .ok_or("original numerical source radius")?;
            let values = case.heap.reading_companion_entries(source, parameter)?;
            case.heap.replace_reading_companion_entries(
                source,
                parameter,
                &values,
                &mut ledger()?,
            )?;
        }
        assert!(
            case.heap
                .reading_entries(&current.readings().bindings()[0])
                .is_err()
        );
        for output in &outputs {
            assert!(
                case.heap
                    .reading_entries(&output.readings().bindings()[0])
                    .is_err()
            );
            assert!(
                output.readings().bindings()[0]
                    .lc_driven_impedance()
                    .is_some()
            );
        }
        assert!(
            case.batch
                .result(0, &case.heap, &case.context, &mut ledger()?)
                .is_err()
        );
    }
    Ok(())
}

/// A coarse expanded RHS retires its same-graph row, preserving other rows and the independent root.
fn rhs<P: Scalar + 'static>() -> TestResult {
    let mut case = setup::<P>(false)?;
    let outputs = produce(
        &mut case,
        &mut crate::phasor_solve_testing::resources::fresh_budget()?,
    )?;
    let current = case.rows[0].current(outputs[0].readings().bindings(), &mut ledger()?)?;
    let current = case.heap.publish_phasor_solve(
        &current,
        &case.context,
        &mut crate::phasor_solve_testing::resources::fresh_budget()?,
    )?;
    let voltage = case.rows[0].voltage();
    let original = case.heap.reading_entries(voltage)?;
    case.heap
        .replace_reading_entries(voltage, &original, &mut ledger()?)?;
    assert!(
        case.heap
            .reading_entries(&current.readings().bindings()[0])
            .is_err()
    );
    assert!(
        case.heap
            .reading_entries(&outputs[0].readings().bindings()[0])
            .is_err()
    );
    for output in &outputs[1..] {
        case.heap
            .reading_entries(&output.readings().bindings()[0])?;
    }
    case.batch
        .result(0, &case.heap, &case.context, &mut ledger()?)?;
    Ok(())
}

/// Fixed point authority requires the written operands, actual invariant owner and deterministic inputs.
/// ´test:integration:lc-original-driven-owner-and-point-refusals´
#[test]
fn lc_original_driven_owner_and_point_refusals() -> TestResult {
    refusals::<f32>()?;
    refusals::<f64>()
}

/// Equal independently prepared models and an uncertain source cannot replace the declared experiment.
fn refusals<P: Scalar + 'static>() -> TestResult {
    let mut case = setup::<P>(false)?;
    let original = case.outputs[0].definition();
    let changed = nrz_bindings::LcDrivenImpedance::new(
        original.law().clone(),
        [
            nrz_bindings::readings::algebra::Rational::parse("1/20", &mut ledger()?)?,
            nrz_bindings::readings::algebra::Rational::parse("1/100000", &mut ledger()?)?,
        ],
        original.drive().clone(),
        original.radian().clone(),
        original.impedance().clone(),
        &mut ledger()?,
    );
    assert!(changed.is_err());
    let mut other = setup::<P>(false)?;
    other
        .batch
        .execute(&other.heap, &other.context, &mut ledger()?)?;
    let root = other
        .batch
        .result(0, &other.heap, &other.context, &mut ledger()?)?;
    assert!(
        case.heap
            .publish_lc_impedance_reusing(&case.outputs[0], root, &case.context, &mut ledger()?)
            .is_err()
    );
    let outputs = produce(
        &mut case,
        &mut crate::phasor_solve_testing::resources::fresh_budget()?,
    )?;
    let root = case
        .batch
        .result(0, &case.heap, &case.context, &mut ledger()?)?;
    assert!(
        case.heap
            .publish_lc_impedance_reusing(&case.outputs[0], root, &case.context, &mut ledger()?)
            .is_err()
    );
    case.heap
        .reading_entries(&outputs[0].readings().bindings()[0])?;
    let mut uncertain = setup::<P>(true)?;
    uncertain
        .batch
        .execute(&uncertain.heap, &uncertain.context, &mut ledger()?)?;
    let root = uncertain
        .batch
        .result(0, &uncertain.heap, &uncertain.context, &mut ledger()?)?;
    let before = uncertain.heap.provenance_usage();
    assert!(
        uncertain
            .heap
            .publish_lc_impedance_reusing(
                &uncertain.outputs[0],
                root,
                &uncertain.context,
                &mut ledger()?
            )
            .is_err()
    );
    assert_eq!(uncertain.heap.provenance_usage(), before);
    Ok(())
}
