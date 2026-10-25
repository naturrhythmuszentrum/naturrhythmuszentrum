// SPDX-License-Identifier: AGPL-3.0-only
//! Certified pi coefficient owners, typed conversions and complete companion propagation.

use crate::conversion_testing::{TestResult, execute, shape};
use nrz_bindings::{
    Budget, CheckedDeclarations, Context, ExponentPolicy, Limits, LimitsOverride, Registry,
};
use nrz_matrix::{ConversionContract, ConversionMode, Extent, Heap, HeapLimits, PiConstant};
use nrz_scalar::{ArithmeticContext, Finite, LiteralLimits, PiLimits, Precision, Scalar};
use std::rc::Rc;

#[path = "pi_conversion/budgets.rs"]
mod budgets;
#[path = "pi_conversion/lifecycle.rs"]
mod lifecycle;
#[path = "pi_conversion/oracle.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "independent pi bounds serve multiple public integration fixtures"
)]
pub(crate) mod oracle;
#[path = "pi_conversion/roundtrip.rs"]
mod roundtrip;
#[path = "pi_conversion/statistics.rs"]
mod statistics;

/// Complete owner measurements for one sealed format.
type Measurement = (Precision, u64, usize, u64, usize, u64, usize);

/// Frozen owner with angle measures and the explicit full-cycle relation.
fn owner() -> TestResult<Rc<CheckedDeclarations>> {
    Ok(Rc::new(
        Context::with_real_symbols(
            Registry::with_angle_frequency(ExponentPolicy::Rational)?,
            Limits::default(),
            &[],
        )?
        .freeze()?,
    ))
}

/// An explicit bounded allowance for exact factor recognition.
pub(super) fn exact_policy() -> LimitsOverride {
    LimitsOverride {
        staged_payload_bytes: Some(65_536),
        output_bytes: Some(16_384),
        ..LimitsOverride::default()
    }
}

/// Materialize the measured scalar enclosure and its reciprocal exactly once.
pub(super) fn pi<P: Scalar + 'static>(
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> TestResult<PiConstant<P>> {
    let (tolerance, limits) = match P::PRECISION {
        Precision::F32 => (
            "0.0000003",
            PiLimits {
                work: 668,
                steps: 5,
                staged_bytes: 240,
                output_bytes: 64,
            },
        ),
        Precision::F64 => (
            "0.0000000000000005",
            PiLimits {
                work: 2054,
                steps: 11,
                staged_bytes: 256,
                output_bytes: 80,
            },
        ),
    };
    Ok(PiConstant::new(
        Finite::new(P::parse_nearest(tolerance).ok_or("tolerance")?)?,
        limits,
        context,
        budget,
    )?)
}

/// Measure separate constant, coefficient and cache-hit lifecycles.
fn measure<P: Scalar + 'static>() -> TestResult<Measurement> {
    let declarations = owner()?;
    let radian = declarations
        .registry()
        .measure("rad")
        .ok_or("radian")?
        .coherent();
    let degree = declarations.registry().measure("degree").ok_or("degree")?;
    let map = declarations.conversion_map(&radian, degree)?;
    let conversion = ConversionContract::new(
        shape(&declarations, &radian, 1)?,
        shape(&declarations, degree, 1)?,
        map,
        LiteralLimits::default(),
        ConversionMode::Deterministic,
    )?;
    let context = ArithmeticContext::new()?;
    let mut budget = Budget::new(Limits::default())?;
    let circle = pi::<P>(&context, &mut budget)?;
    let first_work = budget.usage().work;
    let cold = conversion.certify_pi(&circle, &exact_policy(), &context, &mut budget)?;
    let cloned = conversion.clone();
    let warm = cloned.certify_pi(&circle, &exact_policy(), &context, &mut budget)?;
    assert!(!cold.reused());
    assert!(warm.reused());
    let mut heap = Heap::new(HeapLimits::default())?;
    let input = heap.import_represented(Extent::new(1, 1)?, &radian, &[P::ONE])?;
    let result = execute(&mut heap, conversion, input.value())?;
    assert!(heap.value(result.value())?.region().get(0, 0)? > P::ZERO);
    Ok((
        P::PRECISION,
        first_work,
        circle.owned_bytes(),
        cold.work(),
        cold.new_owned_bytes(),
        warm.work(),
        warm.new_owned_bytes(),
    ))
}

/// Separate pi, coefficient and warm-cache owners retain their measured costs in both formats.
/// ´test:integration:pi-conversion-owner-costs´
#[test]
fn pi_conversion_owner_costs() -> TestResult {
    let observed = [measure::<f32>()?, measure::<f64>()?];
    assert_eq!(
        observed,
        [
            (Precision::F32, 669, 96, 84_572, 128, 2, 0),
            (Precision::F64, 2055, 128, 84_572, 160, 2, 0)
        ]
    );
    Ok(())
}

/// Prepare a checked map and certify only coefficients that require pi.
fn conversion<P: Scalar + 'static>(
    declarations: &Rc<CheckedDeclarations>,
    endpoints: [&nrz_bindings::Descriptor; 2],
    coordinates: [&nrz_matrix::CheckedShape; 2],
    circle: &PiConstant<P>,
    frequency: bool,
) -> TestResult<ConversionContract> {
    let [source, target] = endpoints;
    let map = if frequency {
        declarations.frequency_map(source, target)?
    } else {
        declarations.conversion_map(source, target)?
    };
    let needs_pi =
        map.scale().exact().as_rational().is_none() || map.shift().exact().as_rational().is_none();
    let result = ConversionContract::new(
        coordinates[0].clone(),
        coordinates[1].clone(),
        map,
        LiteralLimits::default(),
        ConversionMode::Deterministic,
    )?;
    if needs_pi {
        result.certify_pi(
            circle,
            &exact_policy(),
            &ArithmeticContext::new()?,
            &mut Budget::new(Limits::default())?,
        )?;
    }
    Ok(result)
}

#[path = "pi_conversion/coordinates.rs"]
mod coordinates;

#[path = "pi_conversion/unit.rs"]
mod unit;
