// SPDX-License-Identifier: AGPL-3.0-only
//! Shape-owned pi references retain exact identities through certified native execution.

use super::{exact_policy, oracle, owner, pi, roundtrip, statistics};
use crate::conversion_testing::{TestResult, contract, execute, shape};
use nrz_bindings::{Budget, Descriptor, ExactConstant, Kind, Limits, Measure, Role, Scale};
use nrz_matrix::{
    ConversionContract, CoordinateMap, ExactCoefficient, Extent, Heap, HeapLimits,
    NormalizationMaps, NormalizationSchema, ReferenceScale, ShapeLimits,
};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Exact positive constants stay independent of any supplied numerical coordinate.
fn descriptor(scale: Scale) -> TestResult<Descriptor> {
    Ok(Descriptor::new(
        Kind::Ratio,
        Measure::new("reference", scale, Measure::coherent().offset().clone())?,
        Role::Difference,
    )?)
}

/// The shape-owned pi definition reaches checked bindings maps without a rational extraction.
type Schema = (
    std::rc::Rc<nrz_bindings::CheckedDeclarations>,
    NormalizationMaps,
);

fn maps(definition: &Descriptor) -> TestResult<Schema> {
    let declarations = owner()?;
    let operator = shape(&declarations, &Descriptor::ratio(), 1)?;
    let limits = ShapeLimits::default();
    let axis =
        |entries, reference| CoordinateMap::check(entries, vec![reference], vec![None], limits);
    let variable = ReferenceScale::new("pi-reference", definition)?;
    let unit = ReferenceScale::new("unit", &Descriptor::ratio())?;
    let schema = NormalizationSchema::check(
        &operator,
        &operator,
        axis(operator.domain(), variable)?,
        axis(operator.codomain(), unit.clone())?,
        axis(operator.domain(), unit)?,
        limits,
    )?;
    Ok((
        declarations,
        ConversionContract::from_normalization_schema(&schema, &exact_policy())?,
    ))
}

/// Both directions borrow the original exported reference definition.
fn contracts(schema: &Schema, rows: usize) -> TestResult<[ConversionContract; 2]> {
    let (declarations, maps) = schema;
    let map = maps.domain().first().ok_or("variable map")?;
    let coordinates = shape(declarations, &Descriptor::ratio(), rows)?;
    Ok([
        contract(coordinates.clone(), coordinates.clone(), map.clone())?,
        contract(coordinates.clone(), coordinates, map.inverse()?)?,
    ])
}

/// Independent rational endpoint boxes enclose normalization and restoration in both formats.
/// ´test:integration:pi-coordinate-schema-roundtrip´
#[test]
fn pi_coordinate_schema_roundtrip() -> TestResult {
    roundtrip::<f32>()?;
    roundtrip::<f64>()
}

/// Exact unit coefficients agree with the same certified restoration scale.
fn roundtrip<P: Scalar + 'static>() -> TestResult {
    let definition = descriptor(Scale::pi()?)?;
    let exact = maps(&definition)?;
    let [normalization, restoration] = contracts(&exact, 1)?;
    let coefficient = ExactCoefficient::unit(&definition)?;
    assert_eq!(
        &coefficient.exact_map().scale,
        restoration.map().scale().exact()
    );
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut Budget::new(Limits::default())?)?;
    for conversion in [&normalization, &restoration] {
        conversion.certify_pi(
            &circle,
            &exact_policy(),
            &context,
            &mut Budget::new(Limits::default())?,
        )?;
    }
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[P::ONE],
        &[P::ONE / P::from_u8(8)],
    )?;
    let normalized = execute(&mut heap, normalization, source.value())?;
    roundtrip::check(&heap, normalized, oracle::factor(1, 1, true, 56)?)?;
    let restored = execute(&mut heap, restoration.clone(), normalized.value())?;
    roundtrip::check(
        &heap,
        restored,
        [crate::kernel_oracle::Rational::new(1, 1)?; 2],
    )?;
    let unit_scaled = execute(&mut heap, restoration, source.value())?;
    roundtrip::check(&heap, unit_scaled, oracle::factor(1, 1, false, 54)?)?;
    Ok(())
}

/// Shape-origin maps transform both statistical axes and the cross block on one pi owner.
/// ´test:integration:pi-coordinate-schema-full-covariance´
#[test]
fn pi_coordinate_schema_full_covariance() -> TestResult {
    covariance::<f32>()?;
    covariance::<f64>()
}

/// Normalization and unit restoration keep the original statistical model and its source IDs.
fn covariance<P: Scalar + 'static>() -> TestResult {
    let exact = maps(&descriptor(Scale::pi()?)?)?;
    let conversions = contracts(&exact, 2)?;
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut Budget::new(Limits::default())?)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let (source, declared) = statistics::declared_source::<P>(&mut heap, &Descriptor::ratio())?;
    let original = heap.statistical(declared)?;
    let model = original.model().id();
    let ids = original
        .model()
        .sources()
        .iter()
        .map(nrz_matrix::Source::id)
        .collect::<Vec<_>>();
    let mut outputs = Vec::new();
    let factors = [
        oracle::factor(1, 1, true, 56)?,
        oracle::factor(1, 1, false, 54)?,
    ];
    for (conversion, factor) in conversions.into_iter().zip(factors) {
        conversion.certify_pi(
            &circle,
            &exact_policy(),
            &context,
            &mut Budget::new(Limits::default())?,
        )?;
        let output = execute(&mut heap, conversion, source.value())?;
        let view = heap.moments(heap.moment_companion(output.value())?)?;
        statistics::covariance(view.covariance(), factor, factor)?;
        statistics::means(view.means().ok_or("means")?, factor)?;
        assert_eq!(view.model(), model);
        assert_eq!(view.source_ids(), ids);
        outputs.push(output);
    }
    let cross = heap.materialize_cross_covariance(
        outputs[0].value(),
        outputs[1].value(),
        nrz_matrix::JointLimits::default(),
    )?;
    let view = heap.moments(cross)?;
    statistics::covariance(view.covariance(), factors[0], factors[1])?;
    assert_eq!(view.model(), model);
    assert_eq!(view.source_ids(), ids);
    Ok(())
}

/// Every new coefficient owner succeeds at literal measured limits and refuses atomically below them.
/// ´test:integration:pi-coordinate-schema-owner-boundaries´
#[test]
fn pi_coordinate_schema_owner_boundaries() -> TestResult {
    boundaries::<f32>(82_168, 152)?;
    boundaries::<f64>(82_232, 184)
}

/// Exact limits cover each cold direction and prior caller Work; valid retries remain cold.
fn boundaries<P: Scalar + 'static>(staged: u64, output: u64) -> TestResult {
    let exact = maps(&descriptor(Scale::pi()?)?)?;
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut Budget::new(Limits::default())?)?;
    for (direction, work) in [84_517, 28_192].into_iter().enumerate() {
        let ceiling = Limits {
            work: work + 17,
            staged_payload_bytes: staged,
            output_bytes: output,
            ..Limits::default()
        };
        for resource in 0..3 {
            let conversions = contracts(&exact, 1)?;
            let conversion = &conversions[direction];
            let mut limits = ceiling.clone();
            match resource {
                0 => limits.work -= 1,
                1 => limits.staged_payload_bytes -= 1,
                _ => limits.output_bytes -= 1,
            }
            let mut budget = prior(limits)?;
            let before = budget.usage();
            assert!(
                conversion
                    .certify_pi(&circle, &exact_policy(), &context, &mut budget)
                    .is_err()
            );
            assert_eq!(budget.usage(), before);
            let mut budget = prior(ceiling.clone())?;
            let cold = conversion.certify_pi(&circle, &exact_policy(), &context, &mut budget)?;
            assert_eq!(cold.work(), work);
            assert!(!cold.reused());
            assert_eq!(budget.usage().work, work + 17);
            let mut warm = Budget::new(Limits {
                work: 2,
                staged_payload_bytes: 24,
                output_bytes: 24,
                ..Limits::default()
            })?;
            let reused =
                conversion
                    .clone()
                    .certify_pi(&circle, &exact_policy(), &context, &mut warm)?;
            assert!(reused.reused());
            assert_eq!(reused.new_owned_bytes(), 0);
        }
    }
    Ok(())
}

/// Distinct successful caller usage must remain identical after any refused attempt.
fn prior(limits: Limits) -> TestResult<Budget> {
    let mut budget = Budget::new(limits)?;
    budget.work(
        17,
        nrz_bindings::readings::algebra::Location::new("prior coordinate caller"),
    )?;
    Ok(budget)
}

/// Valid exact schemas outside the numerical grammar still refuse without an approximation.
/// ´test:integration:pi-coordinate-schema-unsupported-constants´
#[test]
fn pi_coordinate_schema_unsupported_constants() -> TestResult {
    use nrz_bindings::readings::algebra::syntax::Constant;
    let mut setup = Budget::new(Limits::default())?;
    let circle = ExactConstant::named(Constant::Pi, &mut setup)?;
    let values = [
        ExactConstant::named(Constant::E, &mut setup)?,
        circle.add(&ExactConstant::one(), &mut setup)?,
        circle.mul(&circle, &mut setup)?,
    ];
    let context = ArithmeticContext::new()?;
    let numerical = pi::<f64>(&context, &mut Budget::new(Limits::default())?)?;
    for value in values {
        let exact = maps(&descriptor(Scale::Expression(value))?)?;
        for conversion in contracts(&exact, 1)? {
            let mut budget = prior(Limits::default())?;
            let before = budget.usage();
            assert!(matches!(
                conversion.certify_pi(&numerical, &exact_policy(), &context, &mut budget),
                Err(nrz_matrix::ConversionError::Map(
                    nrz_bindings::MapError::Units(
                        nrz_bindings::UnitError::UnsupportedExactConstantExport
                    )
                ))
            ));
            assert_eq!(budget.usage(), before);
        }
    }
    Ok(())
}
