// SPDX-License-Identifier: AGPL-3.0-only
//! Original-expression, binding, normalization and publication boundaries.

use nrz_algebra::{Limits, Rational, Symbol, parse};
use nrz_descriptor_prototype::{
    CoordinateMap, Coverage, Error, OutputContract, ResistorQuantities, Scalar, point_view,
    resistor_fixture, solve,
};
use nrz_units::{Descriptor, Kind, Measure, Role};

/// Preserve every fallible setup error.
type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Execute both precisions through exactly the same descriptor path.
fn run<P: Scalar>() -> Result<[P; 6], Error> {
    let mut fixture = resistor_fixture::<P>()?;
    let plan = fixture.plan.check()?;
    assert_eq!(plan.slots().len(), 6);
    let contract = OutputContract::from(plan.layout());
    let binding = plan.bind(&fixture.heap, fixture.inputs, fixture.maps)?;
    let bound = binding.validate(&mut fixture.heap)?;
    let view = bound.normalized()?;
    assert_eq!(view.matrix(), &[[P::ONE, -P::ONE], [P::ONE, P::ZERO]]);
    let two = P::rational(Rational::integer(2));
    assert_eq!(view.rhs(), &[P::ZERO, two]);
    let candidate = solve(&view)?;
    let published = bound.publish(&candidate, contract)?;
    assert_eq!(published.normalized_center(), &[two; 2]);
    assert_eq!(
        published.normalized_covariance(),
        &[P::rational(Rational::new(1, 16)?); 4]
    );
    let center = fixture.heap.values(published.value())?;
    let covariance = fixture.heap.companion_values(published.companion())?;
    Ok([
        center[0],
        center[1],
        covariance[0],
        covariance[1],
        covariance[2],
        covariance[3],
    ])
}

/// Dyadic f32 results match the physical and paired-covariance oracle bit for bit.
///
/// ´test:integration:resistor-f32-exact-fixture´
#[test]
fn resistor_f32_exact_fixture() -> TestResult {
    assert_eq!(
        run::<f32>()?.map(f32::to_bits),
        [4.0_f32, 2.0, 0.25, 0.125, 0.125, 0.0625].map(f32::to_bits),
    );
    Ok(())
}

/// Dyadic f64 results match the same independently derived physical contract.
///
/// ´test:integration:resistor-f64-exact-fixture´
#[test]
fn resistor_f64_exact_fixture() -> TestResult {
    assert_eq!(
        run::<f64>()?.map(f64::to_bits),
        [4.0_f64, 2.0, 0.25, 0.125, 0.125, 0.0625].map(f64::to_bits),
    );
    Ok(())
}

/// Exact elimination and covariance scaling establish an independent rational oracle.
///
/// ´test:integration:rational-resistor-oracle´
#[test]
fn rational_resistor_oracle() -> TestResult {
    let resistance = Rational::integer(2);
    let source = Rational::integer(4);
    let scales = [Rational::integer(2), Rational::ONE];
    let current = source.checked_div(resistance)?;
    assert_eq!(current, Rational::integer(2));
    assert_eq!(
        source.checked_sub(resistance.checked_mul(current)?)?,
        Rational::ZERO
    );
    let sensitivity = [Rational::ONE, Rational::ONE.checked_div(resistance)?];
    let variance = Rational::new(1, 4)?;
    let expected = [
        Rational::new(1, 4)?,
        Rational::new(1, 8)?,
        Rational::new(1, 8)?,
        Rational::new(1, 16)?,
    ];
    for column in 0..2 {
        for row in 0..2 {
            let covariance = sensitivity[row]
                .checked_mul(sensitivity[column])?
                .checked_mul(variance)?;
            assert_eq!(covariance, expected[row + 2 * column]);
            assert_eq!(
                covariance.checked_div(scales[row].checked_mul(scales[column])?)?,
                Rational::new(1, 16)?,
            );
        }
    }
    Ok(())
}

/// Original invalid children survive zero multiplication and apparent cancellation.
///
/// ´test:integration:original-errors-precede-recipe-selection´
#[test]
fn original_errors_precede_recipe_selection() -> TestResult {
    for (left, right) in [
        ("v - R * i", "0"),
        ("0 * (v + i)", "zero_v"),
        ("(v + i) - (v + i)", "zero_v"),
    ] {
        let fixture = resistor_fixture::<f64>()?;
        let plan = fixture.plan.with_equation(
            0,
            parse(left, Limits::default())?,
            parse(right, Limits::default())?,
        )?;
        let error = plan.check();
        assert!(matches!(error, Err(Error::Check { node: 0, .. })));
    }
    let fixture = resistor_fixture::<f64>()?;
    let plan = fixture.plan.with_equation(
        1,
        parse("v", Limits::default())?,
        parse("s", Limits::default())?,
    )?;
    assert!(matches!(plan.check(), Err(Error::Unsupported)));
    Ok(())
}

/// Proposed bindings expire when coefficients change without changing the slot.
///
/// ´test:integration:binding-versions-and-companion-coverage´
#[test]
fn binding_versions_and_companion_coverage() -> TestResult {
    let mut fixture = resistor_fixture::<f64>()?;
    let plan = fixture.plan.check()?;
    let binding = plan.bind(&fixture.heap, fixture.inputs, fixture.maps.clone())?;
    fixture.heap.replace(fixture.inputs.resistance, vec![4.0])?;
    assert!(matches!(
        binding.validate(&mut fixture.heap),
        Err(Error::Version)
    ));
    fixture.heap.replace(fixture.inputs.source, vec![8.0])?;
    let binding = plan.bind(&fixture.heap, fixture.inputs, fixture.maps)?;
    assert!(matches!(
        binding.validate(&mut fixture.heap),
        Err(Error::Version)
    ));
    Ok(())
}

/// Shape identity, normalization identity and uncertainty model are separate checks.
///
/// ´test:integration:binding-refuses-shape-map-and-model-mismatches´
#[test]
fn binding_refuses_shape_map_and_model_mismatches() -> TestResult {
    let mut fixture = resistor_fixture::<f64>()?;
    let plan = fixture.plan.check()?;
    let mut wrong = fixture.inputs;
    wrong.source = wrong.resistance;
    let binding = plan.bind(&fixture.heap, wrong, fixture.maps.clone())?;
    assert!(matches!(
        binding.validate(&mut fixture.heap),
        Err(Error::Shape)
    ));
    let quantities = ResistorQuantities::new()?;
    let mut maps = fixture.maps.clone();
    maps.solution[0] = CoordinateMap::new(
        quantities.voltage.clone(),
        (&quantities.voltage, Rational::integer(4)),
        None,
    )?;
    let binding = plan.bind(&fixture.heap, fixture.inputs, maps)?;
    assert!(matches!(
        binding.validate(&mut fixture.heap),
        Err(Error::Map)
    ));
    wrong = fixture.inputs;
    wrong.coverage = Coverage::DeterministicRadius;
    let binding = plan.bind(&fixture.heap, wrong, fixture.maps)?;
    assert!(matches!(
        binding.validate(&mut fixture.heap),
        Err(Error::Unsupported)
    ));
    Ok(())
}

/// Restoration failures and wrong output shapes publish neither half of a result.
///
/// ´test:integration:publication-is-atomic-and-binding-specific´
#[test]
fn publication_is_atomic_and_binding_specific() -> TestResult {
    let mut first = resistor_fixture::<f64>()?;
    let first_plan = first.plan.check()?;
    let candidate = {
        let binding = first_plan.bind(&first.heap, first.inputs, first.maps)?;
        let bound = binding.validate(&mut first.heap)?;
        solve(&bound.normalized()?)?
    };
    let mut second = resistor_fixture::<f64>()?;
    let second_plan = second.plan.check()?;
    let count = second.heap.live_count();
    let contract = OutputContract::from(second_plan.layout());
    let binding = second_plan.bind(&second.heap, second.inputs, second.maps.clone())?;
    let bound = binding.validate(&mut second.heap)?;
    assert!(matches!(
        bound.publish(&candidate, contract),
        Err(Error::Map)
    ));
    assert_eq!(second.heap.live_count(), count);
    let binding = second_plan.bind(&second.heap, second.inputs, second.maps)?;
    let bound = binding.validate(&mut second.heap)?;
    let candidate = solve(&bound.normalized()?)?;
    let wrong = OutputContract {
        solution: contract.solution,
        covariance: second_plan.layout().source,
    };
    assert!(matches!(
        bound.publish(&candidate, wrong),
        Err(Error::Shape)
    ));
    assert_eq!(second.heap.live_count(), count);
    Ok(())
}

/// Maps check kind and role even when dimensions agree, and affine origins are explicit.
///
/// ´test:integration:map-kind-and-affine-origin-checks´
#[test]
fn map_kind_and_affine_origin_checks() -> TestResult {
    let quantities = ResistorQuantities::new()?;
    let other = Descriptor::new(
        Kind::Named(
            Symbol::new("other_voltage_kind")?,
            quantities.voltage.dimension(),
        ),
        Measure::coherent(),
        Role::Difference,
    )?;
    assert!(CoordinateMap::new(quantities.voltage.clone(), (&other, Rational::ONE), None).is_err());
    let point = Descriptor::new(
        quantities.voltage.kind().clone(),
        Measure::coherent(),
        Role::Point,
    )?;
    assert!(matches!(
        CoordinateMap::new(point.clone(), (&quantities.voltage, Rational::ONE), None),
        Err(Error::Map),
    ));
    let map = CoordinateMap::new(
        point.clone(),
        (&quantities.voltage, Rational::integer(2)),
        Some((&point, Rational::integer(10))),
    )?;
    assert_eq!(
        map.normalize_exact(Rational::integer(14))?,
        Rational::integer(2)
    );
    let fixture = resistor_fixture::<f64>()?;
    assert!(matches!(
        point_view(&fixture.heap, fixture.inputs.source),
        Err(Error::Map)
    ));
    Ok(())
}

/// Overflow in a valid but extreme finite input cannot publish a nonfinite output.
///
/// ´test:integration:nonfinite-kernel-results-refuse´
#[test]
fn nonfinite_kernel_results_refuse() -> TestResult {
    let mut fixture = resistor_fixture::<f64>()?;
    let plan = fixture.plan.check()?;
    fixture
        .heap
        .replace(fixture.inputs.resistance, vec![f64::MIN_POSITIVE])?;
    let count = fixture.heap.live_count();
    {
        let binding = plan.bind(&fixture.heap, fixture.inputs, fixture.maps)?;
        let bound = binding.validate(&mut fixture.heap)?;
        assert!(matches!(solve(&bound.normalized()?), Err(Error::Numerical)));
    }
    assert_eq!(fixture.heap.live_count(), count);
    Ok(())
}

/// Scalar CAS success cannot erase an incompatible ordered solution shape.
///
/// ´test:integration:plan-checks-ordered-shape-snapshots´
#[test]
fn plan_checks_ordered_shape_snapshots() -> TestResult {
    let fixture = resistor_fixture::<f64>()?;
    let mut shape = fixture
        .heap
        .schemas()
        .shape_definition(fixture.plan.layout().solution)?
        .clone();
    shape.rows.reverse();
    assert!(matches!(
        fixture.plan.with_solution_shape(shape).check(),
        Err(Error::Shape)
    ));
    Ok(())
}
