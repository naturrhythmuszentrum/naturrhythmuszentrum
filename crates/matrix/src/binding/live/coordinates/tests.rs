// SPDX-License-Identifier: AGPL-3.0-only
//! Mixed normalization and restoration are checked against independent rational component boxes.

mod costs;
mod fixture;
mod oracle;

use super::*;
use crate::binding::adapters::fixtures::{Fixture, TestResult, ledger, matrix};
use crate::conversion_oracle::covers;
use nrz_accuracy::NormKind;
use nrz_bindings::{Budget, Limits};
use oracle::{above, square_above};

/// Exact mixed references produce independently enclosed normalized and restored families in both formats.
/// ´test:unit:mixed-coordinate-roundtrip-retains-both-errors´
#[test]
fn mixed_coordinate_roundtrip_retains_both_errors() -> TestResult {
    roundtrip::<f32>()?;
    roundtrip::<f64>()
}

/// Every component uses its own reference and retains its original nominal-target radius.
fn roundtrip<P: Scalar + 'static>() -> TestResult {
    let (heap, binding, contract, _) = fixture::imported::<P>(true)?;
    let context = ArithmeticContext::new()?;
    let normalized = heap.normalize_reading(&binding, &contract, &context, &mut ledger()?)?;
    check(
        &normalized,
        &[(1, 2), (7, 10), (15, 8), (25, 8)],
        &[(23, 40), (5, 8), (75, 32), (85, 32)],
    )?;
    assert_eq!(normalized.direction(), CoordinateDirection::Normalize);
    let restored = heap.restore_reading(&normalized, &context, &mut ledger()?)?;
    check(
        &restored,
        &[(5, 2), (7, 2), (3, 16), (5, 16)],
        &[(23, 8), (25, 8), (15, 64), (17, 64)],
    )?;
    assert_eq!(restored.direction(), CoordinateDirection::Restore);
    assert_eq!(restored.source().reading(), binding.reading());
    assert_eq!(
        restored.previous().ok_or("normalized parent")?.centers(),
        normalized.centers()
    );
    assert!(restored.totals()[0].to_f64() < 0.501);
    assert!(restored.totals()[1].to_f64() < 0.063);
    assert!(matches!(
        heap.restore_reading(&restored, &context, &mut ledger()?),
        Err(CoordinateError::Identity)
    ));
    Ok(())
}

/// Decode returned IEEE values independently and compare exact rational interval endpoints.
fn check<P: Scalar>(
    value: &CoordinateReading<P>,
    total: &[(i128, i128); 4],
    error: &[(i128, i128); 4],
) -> TestResult {
    for index in 0..2 {
        for endpoint in 0..2 {
            let (top, bottom) = total[2 * index + endpoint];
            assert!(covers(
                value.centers()[index].to_f64(),
                value.totals()[index].to_f64(),
                top,
                bottom
            )?);
            let (top, bottom) = error[2 * index + endpoint];
            assert!(covers(
                value.centers()[index].to_f64(),
                value.errors()[index].to_f64(),
                top,
                bottom
            )?);
        }
        assert!(value.errors()[index] <= value.totals()[index]);
    }
    assert_eq!(value.coverage(), Coverage::DeclaredBounds);
    Ok(())
}

/// Explicit metric references change the named norm while total and numerical assurance remain separate.
/// ´test:unit:mixed-coordinate-norm-retains-declared-metric´
#[test]
fn mixed_coordinate_norm_retains_declared_metric() -> TestResult {
    norms::<f32>()?;
    norms::<f64>()
}

/// Compare each supported norm with independently evaluated component box radii.
fn norms<P: Scalar + 'static>() -> TestResult {
    let (heap, binding, contract, metric) = fixture::imported::<P>(true)?;
    let context = ArithmeticContext::new()?;
    assert!(matches!(
        heap.normalize_reading(&binding, &metric, &context, &mut ledger()?),
        Err(CoordinateError::Reading(ReadingError::Reference))
    ));
    let normalized = heap.normalize_reading(&binding, &contract, &context, &mut ledger()?)?;
    let restored = heap.restore_reading(&normalized, &context, &mut ledger()?)?;
    // These approximate values set only a generous tightness threshold; integer oracles prove containment.
    for (kind, lower) in [
        (NormKind::One, 0.725),
        (NormKind::Euclidean, 0.1_f64.hypot(0.625)),
        (NormKind::Infinity, 0.625),
    ] {
        let norm = heap.coordinate_norm(&restored, &contract, kind, &context, &mut ledger()?)?;
        assert_eq!(norm.total().kind(), kind);
        norm_contains(norm.total().upper().get().to_f64(), kind, 1)?;
        norm_contains(norm.error().upper().get().to_f64(), kind, 4)?;
        assert!(norm.total().upper().get().to_f64() < lower * 1.001);
        assert!(norm.error().upper().get() < norm.total().upper().get());
        assert_eq!(norm.coverage(), Coverage::DeclaredBounds);
        let changed = heap.coordinate_norm(&restored, &metric, kind, &context, &mut ledger()?)?;
        norm_contains(changed.total().upper().get().to_f64(), kind, 2)?;
        norm_contains(changed.error().upper().get().to_f64(), kind, 8)?;
        assert!(changed.total().upper().get().to_f64() < lower * 0.501);
    }
    assert!(matches!(
        heap.coordinate_norm(
            &restored,
            &metric,
            NormKind::Frobenius,
            &context,
            &mut ledger()?
        ),
        Err(CoordinateError::UnsupportedNorm)
    ));
    Ok(())
}

/// Replacing a selected physical coordinate invalidates restoration of its retained numerical snapshot.
/// ´test:unit:mixed-coordinate-restoration-refuses-stale-source´
#[test]
fn mixed_coordinate_restoration_refuses_stale_source() -> TestResult {
    let (mut heap, binding, contract, _) = fixture::imported::<f64>(false)?;
    let context = ArithmeticContext::new()?;
    let normalized = heap.normalize_reading(&binding, &contract, &context, &mut ledger()?)?;
    assert_eq!(normalized.coverage(), Coverage::RepresentedInputs);
    heap.replace_reading_entries(&binding, &[4.0, 0.25], &mut ledger()?)?;
    assert!(
        heap.restore_reading(&normalized, &context, &mut ledger()?)
            .is_err()
    );
    Ok(())
}

/// Every new numerical entry point shares cumulative Work and joint retained payload boundaries.
/// ´test:unit:mixed-coordinate-resource-boundaries´
#[test]
fn mixed_coordinate_resource_boundaries() -> TestResult {
    for (index, [work, staged, _]) in costs::profiles().into_iter().enumerate() {
        let operation = u32::try_from(index)?;
        let mut case = costs::Case::new(operation)?;
        assert_eq!(case.attempt(operation, Limits::default(), 0)?, work);
        case.attempt(
            operation,
            Limits {
                work,
                ..Limits::default()
            },
            0,
        )?;
        assert!(
            case.attempt(
                operation,
                Limits {
                    work: work - 1,
                    ..Limits::default()
                },
                0
            )
            .is_err()
        );
        assert!(
            case.attempt(
                operation,
                Limits {
                    work,
                    ..Limits::default()
                },
                1
            )
            .is_err()
        );
        case.attempt(
            operation,
            Limits {
                staged_payload_bytes: staged,
                ..Limits::default()
            },
            0,
        )?;
        assert!(
            case.attempt(
                operation,
                Limits {
                    staged_payload_bytes: staged - 1,
                    ..Limits::default()
                },
                0
            )
            .is_err()
        );
    }
    Ok(())
}

/// An outward restoration that crosses the format range leaves the source heap and snapshot intact.
/// ´test:unit:mixed-coordinate-restoration-overflow-is-atomic´
#[test]
fn mixed_coordinate_restoration_overflow_is_atomic() -> TestResult {
    let prepared = fixture::plan()?;
    let extent = crate::Extent::new(2, 1)?;
    let regions = [
        crate::ReadingRegion {
            extent,
            values: vec![0.0_f64, 0.25],
        },
        crate::ReadingRegion {
            extent,
            values: vec![f64::MAX, 0.0625],
        },
        crate::ReadingRegion {
            extent,
            values: vec![0.0, 0.015_625],
        },
    ];
    let mut heap = Heap::new(crate::HeapLimits::default())?;
    let imported = heap.import_readings(
        std::rc::Rc::new(prepared.case.cas),
        prepared.coverage,
        prepared.companions,
        &regions,
        crate::DependencyPolicy::Entries,
        &mut ledger()?,
    )?;
    let binding = imported.bindings().first().ok_or("binding")?;
    let context = ArithmeticContext::new()?;
    let normalized =
        heap.normalize_reading(binding, &prepared.contract, &context, &mut ledger()?)?;
    // This strict inequality makes the exact radius product exceed the finite restoration range.
    assert!(normalized.totals()[0] > f64::MAX / 5.0);
    assert!(matches!(
        heap.restore_reading(&normalized, &context, &mut ledger()?),
        Err(CoordinateError::Bound(
            nrz_accuracy::AccuracyFailure::Arithmetic(nrz_scalar::ArithmeticFailure::Overflow)
        ))
    ));
    assert_eq!(
        heap.reading_entries(binding)?[0].to_bits(),
        0.0_f64.to_bits()
    );
    assert_eq!(
        heap.reading_companion_entries(binding, 0)?[0].to_bits(),
        f64::MAX.to_bits()
    );
    assert_eq!(binding.state(), crate::SymbolState::Published);
    assert_eq!(normalized.direction(), CoordinateDirection::Normalize);
    Ok(())
}

/// The independent component box has radii one tenth and five eighths in the original metric.
fn norm_contains(upper: f64, kind: NormKind, divisor: i128) -> TestResult {
    let contains = match kind {
        NormKind::One => above(upper, 29, 40 * divisor)?,
        NormKind::Euclidean => square_above(upper, 641, 1600 * divisor * divisor)?,
        NormKind::Infinity => above(upper, 5, 8 * divisor)?,
        NormKind::Frobenius => return Err("unsupported oracle norm".into()),
    };
    assert!(contains);
    Ok(())
}

/// The complete retained snapshot chain and each norm result obey the requested output allowance.
/// ´test:unit:mixed-coordinate-output-boundaries´
#[test]
fn mixed_coordinate_output_boundaries() -> TestResult {
    for (index, [_, _, output]) in costs::profiles().into_iter().enumerate() {
        let operation = u32::try_from(index)?;
        let mut case = costs::Case::new(operation)?;
        case.attempt(
            operation,
            Limits {
                output_bytes: output,
                ..Limits::default()
            },
            0,
        )?;
        assert!(
            case.attempt(
                operation,
                Limits {
                    output_bytes: output - 1,
                    ..Limits::default()
                },
                0
            )
            .is_err()
        );
    }
    Ok(())
}

/// Affine point origins shift centers while both deterministic and numerical radii scale as differences.
/// ´test:unit:mixed-coordinate-point-restoration-keeps-difference-errors´
#[test]
fn mixed_coordinate_point_restoration_keeps_difference_errors() -> TestResult {
    point_roundtrip::<f32>()?;
    point_roundtrip::<f64>()
}

/// Independently specified Kelvin boxes do not acquire an origin term in either radius.
fn point_roundtrip<P: Scalar + 'static>() -> TestResult {
    let (heap, binding, contract, _) = fixture::point_imported::<P>()?;
    let context = ArithmeticContext::new()?;
    let normalized = heap.normalize_reading(&binding, &contract, &context, &mut ledger()?)?;
    check(
        &normalized,
        &[(527, 200), (547, 200), (15, 8), (25, 8)],
        &[(1069, 400), (1079, 400), (75, 32), (85, 32)],
    )?;
    let restored = heap.restore_reading(&normalized, &context, &mut ledger()?)?;
    check(
        &restored,
        &[(599, 2), (601, 2), (3, 16), (5, 16)],
        &[(2399, 8), (2401, 8), (15, 64), (17, 64)],
    )?;
    assert!(restored.totals()[0].to_f64() < 0.501);
    assert!(restored.errors()[0].to_f64() < 0.126);
    Ok(())
}

/// Required irrational coefficient export refuses by name while the exact descriptor and live source survive.
/// ´test:unit:mixed-coordinate-pi-export-refuses-before-result´
#[test]
fn mixed_coordinate_pi_export_refuses_before_result() -> TestResult {
    let (heap, binding, contract, _) = fixture::angle_imported()?;
    let context = ArithmeticContext::new()?;
    assert!(matches!(
        heap.normalize_reading(&binding, &contract, &context, &mut ledger()?),
        Err(CoordinateError::Reading(ReadingError::Units(
            nrz_bindings::UnitError::UnsupportedExactConstantExport
        )))
    ));
    assert_eq!(
        heap.reading_entries(&binding)?[0].to_bits(),
        3.0_f64.to_bits()
    );
    assert_eq!(binding.state(), crate::SymbolState::Published);
    assert!(contract.frame().inverse().witness().is_discharged());
    Ok(())
}
