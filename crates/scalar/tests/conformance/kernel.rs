// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`kernel_certificate_independent_oracles`] | scalar | Rational constants, reciprocals and positive square roots satisfy independent exact relations. |
//! | [`kernel_certificate_full_boxes`] | scalar | Full interval coverage includes mixed signs and quotient composition retains both laws. |
//! | [`kernel_certificate_dyadic_boundaries`] | scalar | Integer dyadic export preserves zero, subnormals, minimum normals and finite extrema. |
//! | [`kernel_certificate_work_boundaries`] | scalar | Every construction and export has a literal Work boundary with 17 prior units. |
//! | [`kernel_certificate_refusal_ledger`] | scalar | Domain and late decoding refusals retain Work and publish no mathematical records. |
//! | [`kernel_certificate_quotient_boundary`] | scalar | A quotient records reciprocal then product and both exports under one cumulative ceiling. |
//!

//! Independent exact rational replay of the closed exported scalar laws.

use crate::testing::{Binary, Rational, point};
use nrz_scalar::{
    ArithmeticContext, Dyadic, DyadicInterval, Enclosure, KernelBudget, KernelCertificate,
    KernelError, KernelEvaluation, KernelLaw, LiteralLimits,
};
use num_bigint::BigInt;
use num_traits::{One, Zero};

/// Fallible public certificate test.
type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Interpret integer certificate data without scalar arithmetic or machine conversion.
fn exact(value: Dyadic) -> Rational {
    let coefficient = BigInt::from(value.significand);
    let shift = usize::from(value.exponent.unsigned_abs());
    if value.exponent >= 0 {
        Rational::from_integer(coefficient << shift)
    } else {
        Rational::new(coefficient, BigInt::one() << shift)
    }
}

/// Decode a closed interval and independently check its endpoint order.
fn interval(value: DyadicInterval) -> [Rational; 2] {
    let lower = exact(value.lower);
    let upper = exact(value.upper);
    assert!(lower <= upper);
    [lower, upper]
}

/// Replay every inequality defining a candidate law, including its domain.
fn replay(certificate: KernelCertificate) -> bool {
    let [lower, upper] = interval(certificate.output);
    match certificate.law {
        KernelLaw::Rational {
            numerator,
            denominator,
        } => {
            let value = Rational::new(numerator.into(), denominator.into());
            let above = lower <= value;
            let below = value <= upper;
            above && below
        }
        KernelLaw::Sum { left, right } => {
            let [a, b] = interval(left);
            let [c, d] = interval(right);
            let above = lower <= a + c;
            let below = b + d <= upper;
            above && below
        }
        KernelLaw::Product { left, right } => {
            let first = interval(left);
            let second = interval(right);
            first.iter().all(|a| {
                second.iter().all(|b| {
                    let product = a * b;
                    let above = lower <= product;
                    let below = product <= upper;
                    above && below
                })
            })
        }
        KernelLaw::Reciprocal { input } => {
            let [a, b] = interval(input);
            let domain = a > Rational::zero() || b < Rational::zero();
            if !domain {
                return false;
            }
            let above = lower <= Rational::one() / b;
            let below = Rational::one() / a <= upper;
            above && below
        }
        KernelLaw::SquareRoot { input } => {
            let [a, b] = interval(input);
            let domain = a > Rational::zero();
            let positive = lower >= Rational::zero();
            let above = &lower * &lower <= a;
            let below = b <= &upper * &upper;
            domain && positive && above && below
        }
    }
}

/// Exercise exact rational import, reciprocal and squared-endpoint root oracles in both formats.
fn oracle<P: Binary>() -> TestResult {
    let context = ArithmeticContext::new()?;
    let mut budget = KernelBudget::new(4096);
    let constant = KernelEvaluation::<P>::rational(
        1,
        10,
        LiteralLimits::new(32, 32, 128),
        &context,
        &mut budget,
    )?;
    let constant_record = constant.export(&mut budget)?;
    assert!(replay(constant_record));
    assert_eq!(
        exact(constant_record.output.lower),
        constant.enclosure().lower().get().rational()
    );
    let three = P::ONE + P::ONE + P::ONE;
    let reciprocal = KernelEvaluation::reciprocal(point(three)?, &context, &mut budget)?;
    let record = reciprocal.export(&mut budget)?;
    assert!(replay(record));
    let one_third = Rational::new(1.into(), 3.into());
    assert!(exact(record.output.lower) <= one_third);
    assert!(exact(record.output.upper) >= one_third);
    let square_root =
        KernelEvaluation::square_root(point(P::ONE + P::ONE)?, &context, &mut budget)?;
    assert!(replay(square_root.export(&mut budget)?));
    // A singleton at either rounded endpoint cannot enclose an irrational root.
    let mut inward = square_root.export(&mut budget)?;
    inward.output.lower = inward.output.upper;
    assert!(!replay(inward));
    assert_eq!(budget.usage().constructed, 3);
    assert_eq!(budget.usage().exported, 4);
    Ok(())
}

/// Rational constants, reciprocals and positive square roots satisfy independent exact relations.
/// ´claim:scalar:kernel-certificate-independent-oracles´
/// ´test:integration:kernel-certificate-independent-oracles´
#[test]
fn kernel_certificate_independent_oracles() -> TestResult {
    oracle::<f32>()?;
    oracle::<f64>()
}

/// Full interval coverage includes mixed signs and quotient composition retains both laws.
/// ´claim:scalar:kernel-certificate-full-boxes´
/// ´test:integration:kernel-certificate-full-boxes´
#[test]
fn kernel_certificate_full_boxes() -> TestResult {
    let context = ArithmeticContext::new()?;
    let mut budget = KernelBudget::new(4096);
    for (left, right) in [((-3.0, 2.0), (-2.0, 4.0)), ((-3.0, -2.0), (2.0, 4.0))] {
        let lhs = Enclosure::new(left.0, left.1)?;
        let rhs = Enclosure::new(right.0, right.1)?;
        assert!(replay(
            KernelEvaluation::sum(lhs, rhs, &context, &mut budget)?.export(&mut budget)?
        ));
        assert!(replay(
            KernelEvaluation::product(lhs, rhs, &context, &mut budget)?.export(&mut budget)?
        ));
    }
    let before = budget.usage();
    let divisor =
        KernelEvaluation::reciprocal(Enclosure::new(3.0_f64, 4.0)?, &context, &mut budget)?;
    let quotient = KernelEvaluation::product(
        Enclosure::new(1.0, 2.0)?,
        divisor.enclosure(),
        &context,
        &mut budget,
    )?;
    let reciprocal = divisor.export(&mut budget)?;
    let product = quotient.export(&mut budget)?;
    assert!(replay(reciprocal));
    assert!(replay(product));
    let KernelLaw::Product { right, .. } = product.law else {
        panic!("quotient must record its product");
    };
    assert_eq!(right, reciprocal.output);
    assert_eq!(budget.usage().work - before.work, 360);
    assert_eq!(budget.usage().constructed - before.constructed, 2);
    assert_eq!(budget.usage().exported - before.exported, 2);
    Ok(())
}

/// Export every exponent boundary and compare with an independent primitive-bit decoder.
fn endpoints<P: Binary>(values: &[P]) -> TestResult {
    let context = ArithmeticContext::new()?;
    let mut budget = KernelBudget::new(4096);
    for &value in values {
        let evaluated =
            KernelEvaluation::sum(point(value)?, point(P::ZERO)?, &context, &mut budget)?;
        let record = evaluated.export(&mut budget)?;
        let expected = value.rational();
        assert_eq!(exact(record.output.lower), expected);
        assert_eq!(exact(record.output.upper), expected);
        for endpoint in [record.output.lower, record.output.upper] {
            if endpoint.significand == 0 {
                assert_eq!(endpoint.exponent, 0);
            } else {
                assert_ne!(endpoint.significand % 2, 0);
            }
        }
    }
    Ok(())
}

/// Integer dyadic export preserves zero, subnormals, minimum normals and finite extrema.
/// ´claim:scalar:kernel-certificate-dyadic-boundaries´
/// ´test:integration:kernel-certificate-dyadic-boundaries´
#[test]
fn kernel_certificate_dyadic_boundaries() -> TestResult {
    endpoints(&[
        0.0_f32,
        -0.0,
        f32::from_bits(1),
        -f32::from_bits(1),
        f32::MIN_POSITIVE,
        f32::MAX,
        -f32::MAX,
    ])?;
    endpoints(&[
        0.0_f64,
        -0.0,
        f64::from_bits(1),
        -f64::from_bits(1),
        f64::MIN_POSITIVE,
        f64::MAX,
        -f64::MAX,
    ])
}

/// Construct one fixed fixture for each law on the supplied cumulative ledger.
fn evaluated(kind: u8, budget: &mut KernelBudget) -> Result<KernelEvaluation<f64>, KernelError> {
    let context = ArithmeticContext::new()?;
    let two = point(2.0)?;
    let three = point(3.0)?;
    match kind {
        0 => KernelEvaluation::rational(1, 10, LiteralLimits::new(32, 32, 128), &context, budget),
        1 => KernelEvaluation::sum(two, three, &context, budget),
        2 => KernelEvaluation::product(two, three, &context, budget),
        3 => KernelEvaluation::reciprocal(three, &context, budget),
        _ => KernelEvaluation::square_root(two, &context, budget),
    }
}

/// Every construction and export has a literal Work boundary with 17 prior units.
/// ´claim:scalar:kernel-certificate-work-boundaries´
/// ´test:integration:kernel-certificate-work-boundaries´
#[test]
fn kernel_certificate_work_boundaries() -> TestResult {
    for (kind, total) in [(0, 234), (1, 75), (2, 77), (3, 61), (4, 60)] {
        let mut budget = KernelBudget::new(total);
        budget.charge(17)?;
        let value = evaluated(kind, &mut budget)?;
        assert_eq!(budget.usage().work, total);
        assert_eq!(budget.usage().constructed, 1);
        assert_eq!(budget.usage().exported, 0);
        let mut refused = KernelBudget::new(total - 1);
        refused.charge(17)?;
        let Err(fault) = evaluated(kind, &mut refused) else {
            panic!("one-below construction Work must refuse");
        };
        assert_eq!(fault, KernelError::WorkLimit);
        assert!(refused.usage().work >= 17);
        assert_eq!(refused.usage().constructed, 0);
        assert_eq!(refused.usage().exported, 0);
        let mut export = KernelBudget::new(145);
        export.charge(17)?;
        assert!(replay(value.export(&mut export)?));
        assert_eq!(export.usage().work, 145);
        assert_eq!(export.usage().exported, 1);
        let mut export = KernelBudget::new(144);
        export.charge(17)?;
        let Err(fault) = value.export(&mut export) else {
            panic!("one-below export Work must refuse");
        };
        assert_eq!(fault, KernelError::WorkLimit);
        assert_eq!(export.usage().work, 17);
        assert_eq!(export.usage().exported, 0);
    }
    Ok(())
}

/// Domain and late decoding refusals retain Work and publish no mathematical records.
/// ´claim:scalar:kernel-certificate-refusal-ledger´
/// ´test:integration:kernel-certificate-refusal-ledger´
#[test]
fn kernel_certificate_refusal_ledger() -> TestResult {
    let context = ArithmeticContext::new()?;
    for (kind, spent, expected) in [
        (
            0,
            21,
            KernelError::Arithmetic(nrz_scalar::ArithmeticFailure::ZeroInDivisor),
        ),
        (1, 18, KernelError::PositiveRoot),
        (2, 18, KernelError::ZeroDenominator),
        (
            3,
            121,
            KernelError::Arithmetic(nrz_scalar::ArithmeticFailure::ResourceLimit),
        ),
    ] {
        let mut budget = KernelBudget::new(1024);
        budget.charge(17)?;
        let result = match kind {
            0 => {
                KernelEvaluation::reciprocal(Enclosure::new(-1.0_f64, 1.0)?, &context, &mut budget)
            }
            1 => KernelEvaluation::square_root(Enclosure::new(0.0, 1.0)?, &context, &mut budget),
            2 => KernelEvaluation::rational(
                1,
                0,
                LiteralLimits::new(32, 32, 40),
                &context,
                &mut budget,
            ),
            _ => KernelEvaluation::rational(
                1,
                10,
                LiteralLimits::new(32, 32, 39),
                &context,
                &mut budget,
            ),
        };
        let Err(fault) = result else {
            panic!("invalid kernel certificate attempt must refuse");
        };
        assert_eq!(fault, expected);
        assert_eq!(budget.usage().work, spent);
        assert_eq!(budget.usage().constructed, 0);
        assert_eq!(budget.usage().exported, 0);
    }
    Ok(())
}

/// A quotient records reciprocal then product and both exports under one cumulative ceiling.
/// ´claim:scalar:kernel-certificate-quotient-boundary´
/// ´test:integration:kernel-certificate-quotient-boundary´
#[test]
fn kernel_certificate_quotient_boundary() -> TestResult {
    let context = ArithmeticContext::new()?;
    for (limit, accepted) in [(377, true), (376, false)] {
        let mut budget = KernelBudget::new(limit);
        budget.charge(17)?;
        let reciprocal = KernelEvaluation::reciprocal(point(3.0_f64)?, &context, &mut budget)?;
        let product =
            KernelEvaluation::product(point(2.0)?, reciprocal.enclosure(), &context, &mut budget)?;
        assert!(replay(reciprocal.export(&mut budget)?));
        let result = product.export(&mut budget);
        if accepted {
            assert!(replay(result?));
            assert_eq!(budget.usage().work, 377);
            assert_eq!(budget.usage().exported, 2);
        } else {
            let Err(fault) = result else {
                panic!("one-below quotient Work must refuse the final export");
            };
            assert_eq!(fault, KernelError::WorkLimit);
            assert_eq!(budget.usage().work, 249);
            assert_eq!(budget.usage().exported, 1);
        }
        assert_eq!(budget.usage().constructed, 2);
    }
    Ok(())
}
