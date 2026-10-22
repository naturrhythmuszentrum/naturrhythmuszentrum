// SPDX-License-Identifier: AGPL-3.0-only
//! Exact represented imports and explicit refusal boundaries.

use nrz_bindings::{
    CountDomain, Descriptor, Kind, Measure, Role, Scale, UncertaintyMode, uncertainty,
};
use nrz_matrix::{Coverage, Extent, Heap, HeapError, HeapLimits};

use crate::testing::{TestResult, bound, voltage};

/// Binary32 admission preserves subnormals and finite extrema and canonicalizes zero.
/// ´test:integration:represented-binary32-bits´
#[test]
fn represented_binary32_bits() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let data = [f32::from_bits(1), f32::MAX, -0.0, -f32::MAX, 3.0, 4.0];
    let bundle = heap.import_represented(Extent::new(2, 3)?, &voltage()?, &data)?;
    let view = heap.value(bundle.value())?;
    assert_eq!(view.coverage(), Coverage::RepresentedInputs);
    assert!(view.bound().is_none());
    assert_eq!(view.region().get(1, 1)?.to_bits(), (-f32::MAX).to_bits());
    assert_eq!(view.region().get(0, 1)?.to_bits(), 0.0_f32.to_bits());
    let expected = [f32::from_bits(1), f32::MAX, 0.0, -f32::MAX, 3.0, 4.0];
    assert_eq!(
        view.region()
            .export()?
            .iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>(),
        expected.map(f32::to_bits)
    );
    assert!(matches!(
        view.region().get(2, 0),
        Err(HeapError::InvalidLayout)
    ));
    assert!(matches!(
        view.region().get(0, 3),
        Err(HeapError::InvalidLayout)
    ));
    Ok(())
}

/// Binary64 bound regions preserve their own exact payload, descriptor and target.
/// ´test:integration:declared-bound-region-contract´
#[test]
fn declared_bound_region_contract() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let quantity = voltage()?;
    let bundle = heap.import_declared_bounds(
        Extent::new(2, 1)?,
        &quantity,
        &[2.0_f64, 3.0],
        &[0.125, 0.25],
    )?;
    let value = heap.value(bundle.value())?;
    let companion = heap.companion(bound(bundle)?)?;
    assert_eq!(value.coverage(), Coverage::DeclaredBounds);
    assert_eq!(companion.target(), bundle.value());
    assert_eq!(companion.target_stamp(), value.stamp());
    assert_eq!(companion.region().extent(), value.region().extent());
    assert_ne!(companion.region().schema_id(), value.region().schema_id());
    assert_eq!(
        companion.region().descriptor(),
        &uncertainty(UncertaintyMode::Radius, &quantity, None, None)?
    );
    assert_eq!(companion.region().get(0, 0)?.to_bits(), 0.125_f64.to_bits());
    assert_eq!(companion.region().get(1, 0)?.to_bits(), 0.25_f64.to_bits());
    assert_eq!(value.region().get(0, 0)?.to_bits(), 2.0_f64.to_bits());
    Ok(())
}

/// Invalid extents, lengths and payloads fail before consuming a one-entry heap.
/// ´test:integration:invalid-imports-leave-capacity-available´
#[test]
fn invalid_imports_leave_capacity_available() -> TestResult {
    assert_eq!(Extent::new(usize::MAX, 2), Err(HeapError::ExtentOverflow));
    let mut heap = Heap::new(HeapLimits::new(2, 2, 2))?;
    let extent = Extent::new(1, 1)?;
    let quantity = Descriptor::ratio();
    assert!(matches!(
        heap.import_represented(extent, &quantity, &[] as &[f64]),
        Err(HeapError::LengthMismatch)
    ));
    for center in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(matches!(
            heap.import_represented(extent, &quantity, std::slice::from_ref(&center)),
            Err(HeapError::NonFinite { index: 0 })
        ));
    }
    for radius in [-1.0_f64, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(matches!(
            heap.import_declared_bounds(extent, &quantity, &[1.0], std::slice::from_ref(&radius)),
            Err(HeapError::InvalidBound { index: 0 })
        ));
    }
    assert!(matches!(
        heap.import_declared_bounds(extent, &quantity, &[1.0_f64], &[]),
        Err(HeapError::LengthMismatch)
    ));
    let bundle = heap.import_declared_bounds(extent, &quantity, &[1.0_f64], &[-0.0])?;
    assert_eq!(
        heap.companion(bound(bundle)?)?
            .region()
            .get(0, 0)?
            .to_bits(),
        0.0_f64.to_bits()
    );
    Ok(())
}

/// Floating count and offset-coordinate imports refuse; coherent points retain their role.
/// ´test:integration:count-and-presentation-refusals´
#[test]
fn count_and_presentation_refusals() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let extent = Extent::new(1, 1)?;
    let count = Descriptor::new(
        Kind::Count("events".to_owned(), CountDomain::NONNEGATIVE),
        Measure::coherent(),
        Role::Difference,
    )?;
    assert!(matches!(
        heap.import_represented(extent, &count, &[1.0_f64]),
        Err(HeapError::CountStorage)
    ));
    let coherent = Measure::coherent();
    let point = Descriptor::new(
        voltage()?.kind().clone(),
        Measure::new(
            "shifted",
            Scale::Exact(coherent.scale().clone()),
            coherent.scale().clone(),
        )?,
        Role::Point,
    )?;
    assert!(matches!(
        heap.import_represented(extent, &point, &[1.0_f64]),
        Err(HeapError::NonCoherentMeasure)
    ));
    let bundle = heap.import_declared_bounds(extent, &point.coherent(), &[1.0_f64], &[0.25])?;
    assert_eq!(
        heap.value(bundle.value())?.region().descriptor().role(),
        Role::Point
    );
    assert_eq!(
        heap.companion(bound(bundle)?)?.region().descriptor().role(),
        Role::Difference
    );
    Ok(())
}

/// Unsupported nested relative radius derivation preserves the bindings cause and publishes nothing.
/// ´test:integration:relative-bound-derivation-refuses´
#[test]
fn relative_bound_derivation_refuses() -> TestResult {
    let mut heap = Heap::new(HeapLimits::new(1, 1, 1))?;
    let ratio = Descriptor::ratio();
    let relative = uncertainty(
        UncertaintyMode::RelativeRadius,
        &ratio,
        None,
        Some((&ratio, true)),
    )?;
    let extent = Extent::new(1, 1)?;
    assert!(matches!(
        heap.import_declared_bounds(extent, &relative, &[1.0_f64], &[0.1]),
        Err(HeapError::Units(nrz_bindings::UnitError::RelativeReference))
    ));
    let bundle = heap.import_represented(extent, &relative, &[1.0_f64])?;
    assert_eq!(heap.value(bundle.value())?.region().descriptor(), &relative);
    Ok(())
}

/// Empty contiguous regions have no addressable entry and export an empty copy.
/// ´test:integration:empty-regions-have-no-address´
#[test]
fn empty_regions_have_no_address() -> TestResult {
    let mut heap = Heap::new(HeapLimits::new(2, 0, 2))?;
    let bundle =
        heap.import_declared_bounds(Extent::new(0, 3)?, &Descriptor::ratio(), &[] as &[f64], &[])?;
    let value = heap.value(bundle.value())?;
    assert_eq!(value.region().export()?, []);
    assert_eq!(heap.companion(bound(bundle)?)?.region().export()?, []);
    assert!(matches!(
        value.region().get(0, 0),
        Err(HeapError::InvalidLayout)
    ));
    Ok(())
}

/// Binary64 copies need no numerical oracle: represented bits are the exact target.
/// ´test:integration:represented-binary64-bits´
#[test]
fn represented_binary64_bits() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let data = [f64::from_bits(1), f64::MAX, -0.0, -f64::MAX];
    let bundle = heap.import_represented(Extent::new(2, 2)?, &Descriptor::ratio(), &data)?;
    let view = heap.value(bundle.value())?;
    let expected = [f64::from_bits(1), f64::MAX, 0.0, -f64::MAX];
    assert_eq!(
        view.region()
            .export()?
            .iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>(),
        expected.map(f64::to_bits)
    );
    assert_eq!(view.stamp().version().number(), 1);
    Ok(())
}

/// Coherent reporting names remain part of exact schema equality.
/// ´test:integration:coherent-measure-names-stay-distinct´
#[test]
fn coherent_measure_names_stay_distinct() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let coherent = Measure::coherent();
    let alias = Descriptor::new(
        Kind::Ratio,
        Measure::new(
            "another name",
            Scale::Exact(coherent.scale().clone()),
            coherent.offset().clone(),
        )?,
        Role::Difference,
    )?;
    let extent = Extent::new(1, 1)?;
    let first = heap.import_represented(extent, &Descriptor::ratio(), &[1.0_f64])?;
    let second = heap.import_represented(extent, &alias, &[1.0_f64])?;
    assert_ne!(
        heap.value(first.value())?.region().schema_id(),
        heap.value(second.value())?.region().schema_id()
    );
    assert_eq!(heap.value(second.value())?.region().descriptor(), &alias);
    Ok(())
}
