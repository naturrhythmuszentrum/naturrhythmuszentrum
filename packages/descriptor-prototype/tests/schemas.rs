// SPDX-License-Identifier: AGPL-3.0-only
//! Schema equality and generational identity regression cases.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use nrz_algebra::{Rational, Symbol};
use nrz_descriptor_prototype::{Axis, Error, Heap, QuantityKey, SchemaPool, Shape};
use nrz_units::{
    BaseDimension, CountDomain, Descriptor, Dimension, ExponentPolicy, Kind, Measure, Registry,
    Role, Scale, UncertaintyMode, uncertainty,
};

/// Test failures propagate rather than hiding fallible setup.
type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A deterministic coarse hash for comparing deliberately colliding keys.
fn hash(value: &QuantityKey) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// A minimal full dense scalar schema.
fn scalar(
    pool: &mut SchemaPool,
    descriptor: Descriptor,
) -> Result<nrz_descriptor_prototype::ShapeId, Error> {
    let quantity = pool.quantity(descriptor)?;
    pool.shape(Shape {
        rows: vec![Axis {
            meaning: "value".into(),
            quantity,
        }],
        columns: vec![Axis {
            meaning: "scalar".into(),
            quantity,
        }],
        entries: vec![quantity],
    })
}

/// Full equality resolves coarse collisions and conversion does not merge names.
///
/// ´test:integration:exact-equality-and-coarse-collisions´
#[test]
fn exact_equality_and_coarse_collisions() -> TestResult {
    let mut pool = SchemaPool::new(Registry::new(ExponentPolicy::Rational))?;
    let ratio = Descriptor::ratio();
    let angle = Descriptor::new(Kind::Angle, Measure::coherent(), Role::Difference)?;
    assert_eq!(
        hash(&QuantityKey(ratio.clone())),
        hash(&QuantityKey(angle.clone()))
    );
    let ratio_id = pool.quantity(ratio.clone())?;
    assert_eq!(ratio_id, pool.quantity(ratio.clone())?);
    assert_ne!(ratio_id, pool.quantity(angle)?);
    let alias = Descriptor::new(
        Kind::Ratio,
        Measure::new("alias", Scale::Exact(Rational::ONE), Rational::ZERO)?,
        Role::Difference,
    )?;
    assert_eq!(
        ratio.conversion_to(&alias)?.apply(Rational::ONE)?,
        Rational::ONE
    );
    assert_ne!(ratio_id, pool.quantity(alias)?);
    let name = Symbol::new("particles")?;
    let signed = Descriptor::new(
        Kind::Count(name.clone(), CountDomain::SIGNED),
        Measure::coherent(),
        Role::Difference,
    )?;
    let positive = Descriptor::new(
        Kind::Count(name, CountDomain::NONNEGATIVE),
        Measure::coherent(),
        Role::Difference,
    )?;
    assert_eq!(
        hash(&QuantityKey(signed.clone())),
        hash(&QuantityKey(positive.clone()))
    );
    assert_ne!(pool.quantity(signed)?, pool.quantity(positive)?);
    let mut foreign = SchemaPool::new(Registry::new(ExponentPolicy::Rational))?;
    assert_ne!(ratio_id, foreign.quantity(ratio)?);
    assert!(matches!(foreign.descriptor(ratio_id), Err(Error::Schema)));
    Ok(())
}

/// Reduced rational exponents and explicit integer admission use the CAS.
///
/// ´test:integration:rational-policy-and-affine-conversions´
#[test]
fn rational_policy_and_affine_conversions() -> TestResult {
    let half = Rational::new(1, 2)?;
    assert_eq!(half, Rational::new(2, 4)?);
    let dimension = Dimension::base(BaseDimension::Length).power(half, ExponentPolicy::Rational)?;
    let root = Descriptor::new(
        Kind::Named(Symbol::new("root_length")?, dimension),
        Measure::coherent(),
        Role::Difference,
    )?;
    let mut integral = SchemaPool::new(Registry::new(ExponentPolicy::Integral))?;
    assert!(integral.quantity(root.clone()).is_err());
    let mut rational = SchemaPool::new(Registry::new(ExponentPolicy::Rational))?;
    rational.quantity(root)?;
    let temperature = Kind::Named(
        Symbol::new("temperature")?,
        Dimension::base(BaseDimension::Temperature),
    );
    let kelvin = Descriptor::new(temperature.clone(), Measure::coherent(), Role::Point)?;
    let celsius = Descriptor::new(
        temperature,
        Measure::new("C", Scale::Exact(Rational::ONE), Rational::new(27315, 100)?)?,
        Role::Point,
    )?;
    assert_eq!(
        celsius.conversion_to(&kelvin)?.apply(Rational::ZERO)?,
        Rational::new(27315, 100)?
    );
    assert_eq!(
        celsius
            .difference()
            .conversion_to(&kelvin.difference())?
            .shift,
        Rational::ZERO
    );
    assert_ne!(
        rational.quantity(celsius.clone())?,
        rational.quantity(celsius.difference())?
    );
    let length = Kind::Named(
        Symbol::new("length")?,
        Dimension::base(BaseDimension::Length),
    );
    let metre = Descriptor::new(length.clone(), Measure::coherent(), Role::Difference)?;
    let centimetre = Descriptor::new(
        length,
        Measure::new("cm", Scale::Exact(Rational::new(1, 100)?), Rational::ZERO)?,
        Role::Difference,
    )?;
    assert_eq!(
        centimetre
            .conversion_to(&metre)?
            .apply(Rational::integer(100))?,
        Rational::ONE
    );
    let mixed_covariance =
        uncertainty(UncertaintyMode::Covariance, &centimetre, Some(&metre), None)?;
    let coherent_covariance = uncertainty(UncertaintyMode::Covariance, &metre, Some(&metre), None)?;
    assert_eq!(
        mixed_covariance.conversion_to(&coherent_covariance)?.scale,
        Rational::new(1, 100)?
    );
    Ok(())
}

/// Equal shape keys ignore values but preserve ordered coordinate meanings.
///
/// ´test:integration:shapes-preserve-axes-and-distinct-extents´
#[test]
fn shapes_preserve_axes_and_distinct_extents() -> TestResult {
    let mut pool = SchemaPool::new(Registry::new(ExponentPolicy::Rational))?;
    let quantity = pool.quantity(Descriptor::ratio())?;
    let first = Axis {
        meaning: "first".into(),
        quantity,
    };
    let second = Axis {
        meaning: "second".into(),
        quantity,
    };
    let column = Axis {
        meaning: "scalar".into(),
        quantity,
    };
    let shape = Shape {
        rows: vec![first.clone(), second.clone()],
        columns: vec![column],
        entries: vec![quantity; 2],
    };
    let id = pool.shape(shape.clone())?;
    assert_eq!(id, pool.shape(shape.clone())?);
    let mut swapped = shape;
    swapped.rows.reverse();
    assert_ne!(id, pool.shape(swapped)?);
    let covariance = pool.shape(Shape {
        rows: vec![first.clone(), second.clone()],
        columns: vec![first, second],
        entries: vec![quantity; 4],
    })?;
    let norm = scalar(&mut pool, Descriptor::ratio())?;
    assert_ne!(id, covariance);
    assert_ne!(id, norm);
    let mut heap = Heap::<f64>::new(pool)?;
    let a = heap.value(id, vec![1.0, 2.0])?;
    let b = heap.value(id, vec![1.0, 2.0])?;
    assert_ne!(a, b);
    assert_eq!(heap.contract(a)?.0, heap.contract(b)?.0);
    Ok(())
}

/// Resolution distinguishes value changes, slot retirement and arena replacement.
///
/// ´test:integration:references-versions-and-companions´
#[test]
fn references_versions_and_companions() -> TestResult {
    let mut pool = SchemaPool::new(Registry::new(ExponentPolicy::Rational))?;
    let shape = scalar(&mut pool, Descriptor::ratio())?;
    let covariance_quantity = pool.quantity(uncertainty(
        UncertaintyMode::Covariance,
        &Descriptor::ratio(),
        Some(&Descriptor::ratio()),
        None,
    )?)?;
    let axes = pool.shape_definition(shape)?.rows.clone();
    let covariance = pool.shape(Shape {
        rows: axes.clone(),
        columns: axes,
        entries: vec![covariance_quantity],
    })?;
    let mut heap = Heap::<f64>::new(pool)?;
    let value = heap.value(shape, vec![2.0])?;
    let equal_schema = heap.value(shape, vec![2.0])?;
    let target = heap.target(value, vec![0])?;
    let companion = heap.covariance(covariance, vec![0.25], target.clone(), target.clone())?;
    let other_target = heap.target(equal_schema, vec![0])?;
    assert!(
        heap.covariance_for(companion, &other_target, &target)
            .is_err()
    );
    let version = heap.contract(value)?.1;
    heap.replace(value, vec![3.0])?;
    assert_eq!(heap.contract(value)?.1, version + 1);
    assert!(matches!(
        heap.companion_values(companion),
        Err(Error::Version)
    ));
    let foreign = Heap::<f64>::new(SchemaPool::new(Registry::new(ExponentPolicy::Rational))?)?;
    assert!(matches!(foreign.values(value), Err(Error::Reference)));
    heap.retire(value)?;
    assert!(matches!(heap.values(value), Err(Error::Reference)));
    heap.invalidate_arena()?;
    assert!(matches!(heap.values(equal_schema), Err(Error::Reference)));
    Ok(())
}

/// Exact count admission cannot silently become rounded floating storage.
///
/// ´test:integration:counts-and-nonfinite-values-refuse´
#[test]
fn counts_and_nonfinite_values_refuse() -> TestResult {
    let mut pool = SchemaPool::new(Registry::new(ExponentPolicy::Rational))?;
    let count = Descriptor::new(
        Kind::Count(Symbol::new("particles")?, CountDomain::NONNEGATIVE),
        Measure::coherent(),
        Role::Difference,
    )?;
    count.check_count_literal(Rational::integer(2))?;
    assert!(count.check_count_literal(Rational::new(1, 2)?).is_err());
    let count_shape = scalar(&mut pool, count)?;
    let ratio_shape = scalar(&mut pool, Descriptor::ratio())?;
    let mut heap = Heap::<f32>::new(pool)?;
    assert!(matches!(
        heap.value(count_shape, vec![2.0]),
        Err(Error::Unsupported)
    ));
    assert!(matches!(
        heap.value(ratio_shape, vec![f32::INFINITY]),
        Err(Error::Numerical)
    ));
    assert_eq!(heap.live_count(), 0);
    Ok(())
}
