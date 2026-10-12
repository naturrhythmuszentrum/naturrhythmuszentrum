// SPDX-License-Identifier: AGPL-3.0-only
//! Shared semantic cases for the descriptor authority and the integral static probe.

use nrz_algebra::{Rational, Symbol};
use nrz_descriptor_prototype::{
    OutputContract, ResistorQuantities, resistor_fixture, solve,
    static_probe::{Exponents, ResultWitness, Typed},
};
use nrz_units::{
    BaseDimension, BinaryOp, CountDomain, Descriptor, Dimension, ExponentPolicy, Kind, Measure,
    Registry, Role, Scale, UncertaintyMode, uncertainty,
};

/// Every setup and arithmetic failure is visible to the test runner.
type TestResult = Result<(), Box<dyn std::error::Error>>;
/// Compile-time voltage dimension; semantic voltage kinds remain dynamic.
type Voltage = Exponents<2, 1, -3, -1, 0, 0, 0>;
/// Compile-time current dimension.
type Current = Exponents<0, 0, 0, 1, 0, 0, 0>;
/// Compile-time resistance dimension.
type Resistance = Exponents<2, 1, -3, -2, 0, 0, 0>;
/// Dimensionless marker, shared by multiple incompatible semantic kinds.
type Zero = Exponents<0, 0, 0, 0, 0, 0, 0>;
/// Integral length marker.
type Length = Exponents<1, 0, 0, 0, 0, 0, 0>;
/// Voltage variance dimension; not a semantic permission to replace variance by energy.
type VoltageSquared = Exponents<4, 2, -6, -2, 0, 0, 0>;

/// Addition and explicitly annotated products and quotients agree with the same CAS.
///
/// ´test:integration:static-and-descriptor-rules-agree´
#[test]
fn static_and_descriptor_rules_agree() -> TestResult {
    let quantities = ResistorQuantities::new()?;
    let registry = quantities.registry()?;
    let voltage = Typed::<Voltage>::lift(quantities.voltage.clone(), &registry)?;
    let current = Typed::<Current>::lift(quantities.current.clone(), &registry)?;
    let resistance = Typed::<Resistance>::lift(quantities.resistance.clone(), &registry)?;
    assert_eq!(
        voltage.add(&voltage, &registry)?.descriptor(),
        &registry.binary(BinaryOp::Add, &quantities.voltage, &quantities.voltage,)?
    );
    let voltage_result = ResultWitness::<Voltage>::new(quantities.voltage.clone(), &registry)?;
    let current_result = ResultWitness::<Current>::new(quantities.current.clone(), &registry)?;
    assert_eq!(
        resistance
            .product(&current, &voltage_result, &registry)?
            .descriptor(),
        &quantities.voltage,
    );
    assert_eq!(
        voltage
            .quotient(&resistance, &current_result, &registry)?
            .descriptor(),
        &quantities.current,
    );
    assert!(
        resistance
            .product(&current, &current_result, &registry)
            .is_err()
    );
    assert!(Typed::<Current>::lift(quantities.voltage, &registry).is_err());
    Ok(())
}

/// Equal static exponents cannot establish semantic kind, affine role or count closure.
///
/// ´test:integration:equal-static-dimensions-still-need-semantics´
#[test]
fn equal_static_dimensions_still_need_semantics() -> TestResult {
    let registry = Registry::new(ExponentPolicy::Rational);
    let ratio = Typed::<Zero>::lift(Descriptor::ratio(), &registry)?;
    let angle = Typed::<Zero>::lift(
        Descriptor::new(Kind::Angle, Measure::coherent(), Role::Difference)?,
        &registry,
    )?;
    assert!(ratio.add(&angle, &registry).is_err());
    let count = Descriptor::new(
        Kind::Count(
            Symbol::new("bounded_particles")?,
            CountDomain::new(Some(0), Some(10))?,
        ),
        Measure::coherent(),
        Role::Difference,
    )?;
    let count = Typed::<Zero>::lift(count, &registry)?;
    assert!(count.add(&count, &registry).is_err());
    let quantities = ResistorQuantities::new()?;
    let point = Descriptor::new(
        quantities.voltage.kind().clone(),
        Measure::coherent(),
        Role::Point,
    )?;
    let point = Typed::<Voltage>::lift(point, &registry)?;
    assert!(point.add(&point, &registry).is_err());
    Ok(())
}

/// Measures survive lifting and the result still follows CAS coherent coordinates.
///
/// ´test:integration:static-measures-remain-dynamic´
#[test]
fn static_measures_remain_dynamic() -> TestResult {
    let registry = Registry::new(ExponentPolicy::Rational);
    let kind = Kind::Named(
        Symbol::new("length")?,
        Dimension::base(BaseDimension::Length),
    );
    let metre = Descriptor::new(kind.clone(), Measure::coherent(), Role::Difference)?;
    let centimetre = Descriptor::new(
        kind,
        Measure::new("cm", Scale::Exact(Rational::new(1, 100)?), Rational::ZERO)?,
        Role::Difference,
    )?;
    let metres = Typed::<Length>::lift(metre.clone(), &registry)?;
    let centimetres = Typed::<Length>::lift(centimetre.clone(), &registry)?;
    assert_ne!(metres.descriptor(), centimetres.descriptor());
    assert_eq!(
        metres.add(&centimetres, &registry)?.descriptor(),
        &registry.binary(BinaryOp::Add, &metre, &centimetre,)?
    );
    assert_eq!(
        centimetre
            .conversion_to(&metre)?
            .apply(Rational::integer(100))?,
        Rational::ONE
    );
    Ok(())
}

/// Roots use CAS rules and an explicit result witness, not inferred exponent division.
///
/// ´test:integration:static-root-witness-and-rational-refusal´
#[test]
fn static_root_witness_and_rational_refusal() -> TestResult {
    let quantities = ResistorQuantities::new()?;
    let mut registry = quantities.registry()?;
    let variance = uncertainty(UncertaintyMode::Variance, &quantities.voltage, None, None)?;
    let standard = uncertainty(UncertaintyMode::Standard, &quantities.voltage, None, None)?;
    let variance = Typed::<VoltageSquared>::lift(variance, &registry)?;
    let result = ResultWitness::<Voltage>::new(standard.clone(), &registry)?;
    assert_eq!(
        variance.root(&result, &registry)?.descriptor(),
        &standard.coherent()
    );
    let wrong_kind = ResultWitness::<Voltage>::new(quantities.voltage, &registry)?;
    assert!(variance.root(&wrong_kind, &registry).is_err());
    let length_dimension = Dimension::base(BaseDimension::Length);
    let length_kind = Kind::Named(Symbol::new("length")?, length_dimension);
    let root_kind = Kind::Named(
        Symbol::new("root_length")?,
        length_dimension.power(Rational::new(1, 2)?, ExponentPolicy::Rational)?,
    );
    registry.register_power(length_kind.clone(), Rational::new(1, 2)?, root_kind)?;
    let length = Descriptor::new(length_kind, Measure::coherent(), Role::Difference)?;
    let root = registry.power(&length, Rational::new(1, 2)?)?;
    assert_eq!(root.dimension().exponents()[0], Rational::new(1, 2)?);
    assert!(Typed::<Length>::lift(root, &registry).is_err());
    Ok(())
}

/// Physical static annotations do not change the normalized kernel or its result.
///
/// ´test:integration:static-probe-reuses-normalized-descriptor-pipeline´
#[test]
fn static_probe_reuses_normalized_descriptor_pipeline() -> TestResult {
    let quantities = ResistorQuantities::new()?;
    let registry = quantities.registry()?;
    let voltage = Typed::<Voltage>::lift(quantities.voltage, &registry)?;
    let resistance = Typed::<Resistance>::lift(quantities.resistance, &registry)?;
    let expected = ResultWitness::<Current>::new(quantities.current, &registry)?;
    voltage.quotient(&resistance, &expected, &registry)?;
    let mut fixture = resistor_fixture::<f64>()?;
    let plan = fixture.plan.check()?;
    let output = OutputContract::from(plan.layout());
    let binding = plan.bind(&fixture.heap, fixture.inputs, fixture.maps)?;
    let bound = binding.validate(&mut fixture.heap)?;
    let candidate = solve(&bound.normalized()?)?;
    let published = bound.publish(&candidate, output)?;
    let values = fixture.heap.values(published.value())?;
    assert_eq!(
        [values[0].to_bits(), values[1].to_bits()],
        [4.0_f64.to_bits(), 2.0_f64.to_bits()]
    );
    Ok(())
}
