// SPDX-License-Identifier: AGPL-3.0-only
//! Exact numeric laws against independent bounded integer arithmetic.

use num_bigint::{BigInt, BigUint};
use num_traits::One;

use crate::limits::{Budget, BudgetKind, Limits};
use crate::solving::numeric::{Gaussian, integer_root, rational_root};
use crate::testing::common::TestResult;
use crate::testing::solve_numeric::{parts, stored};
use crate::{Fault, FaultKind, InvalidCode, Rational};

/// Unreduced fractions for the independent small-integer oracle.
type Fraction = (i128, i128);
/// Real and imaginary fractions, with no library arithmetic in the oracle.
type Pair = [Fraction; 2];

/// Oracle addition; fixtures keep every exact product within `i128`.
const fn sum(left: Fraction, right: Fraction) -> Fraction {
    (left.0 * right.1 + right.0 * left.1, left.1 * right.1)
}

/// Oracle multiplication of bounded integer pairs.
const fn product(left: Fraction, right: Fraction) -> Fraction {
    (left.0 * right.0, left.1 * right.1)
}

/// Oracle sign change without rational normalization.
const fn negative(value: Fraction) -> Fraction {
    (-value.0, value.1)
}

/// Independent complex multiplication using only bounded integer products.
const fn pair_product(left: Pair, right: Pair) -> Pair {
    [
        sum(
            product(left[0], right[0]),
            negative(product(left[1], right[1])),
        ),
        sum(product(left[0], right[1]), product(left[1], right[0])),
    ]
}

/// Compare by integer cross products, independently of the rational equality implementation.
fn check_fraction(value: &Rational, expected: Fraction, budget: &mut Budget) -> TestResult {
    let (numerator, denominator) = value.small_ratio(budget)?.ok_or("small oracle overflow")?;
    assert_eq!(
        i128::from(numerator) * expected.1,
        i128::from(denominator) * expected.0
    );
    Ok(())
}

/// Extract both metered components and compare with the independent oracle.
fn check_pair(value: &Gaussian, expected: Pair, budget: &mut Budget) -> TestResult {
    check_fraction(&value.real(budget)?, expected[0], budget)?;
    check_fraction(&value.imaginary(budget)?, expected[1], budget)
}

/// Build a scratch Gaussian from the shared public rational fixture.
fn gaussian(real: &str, imaginary: &str, budget: &mut Budget) -> TestResult<Gaussian> {
    let (real, imaginary) = parts(real, imaginary)?;
    Ok(Gaussian::from_parts(&real, &imaginary, budget)?)
}

/// Exhaustive small inputs agree with independently computed exact field products and quotients.
/// ´test:crate:gaussian-field-oracle´
#[test]
fn gaussian_field_oracle() -> TestResult {
    for numerator in -2..=2 {
        for imaginary in -2..=2 {
            for denominator in 1..=3 {
                field_case(numerator, imaginary, denominator)?;
            }
        }
    }
    Ok(())
}

/// One bounded oracle case includes nonreal divisors and unreduced input fractions.
fn field_case(numerator: i64, imaginary: i64, denominator: u64) -> TestResult {
    let mut budget = Budget::new(Limits::default())?;
    let left = Gaussian::from_parts(
        &Rational::from_ratio(numerator, denominator, &mut budget)?,
        &Rational::from_ratio(imaginary, denominator, &mut budget)?,
        &mut budget,
    )?;
    let right = gaussian("-2/3", "3/5", &mut budget)?;
    let expected = [
        (i128::from(numerator), i128::from(denominator)),
        (i128::from(imaginary), i128::from(denominator)),
    ];
    let operand = [(-2, 3), (3, 5)];
    check_pair(
        &left.add(&right, &mut budget)?,
        [sum(expected[0], operand[0]), sum(expected[1], operand[1])],
        &mut budget,
    )?;
    check_pair(
        &left.sub(&right, &mut budget)?,
        [
            sum(expected[0], negative(operand[0])),
            sum(expected[1], negative(operand[1])),
        ],
        &mut budget,
    )?;
    check_pair(
        &left.mul(&right, &mut budget)?,
        pair_product(expected, operand),
        &mut budget,
    )?;
    // (-2/3 + 3i/5)^-1 = -150/181 - 135i/181, derived by integer cross products.
    check_pair(
        &right.inverse(&mut budget)?,
        [(-150, 181), (-135, 181)],
        &mut budget,
    )?;
    check_pair(
        &left.div(&right, &mut budget)?,
        pair_product(expected, [(-150, 181), (-135, 181)]),
        &mut budget,
    )?;
    inverse_case(&left, numerator, imaginary, denominator, &mut budget)?;
    let round_trip = left.div(&right, &mut budget)?.mul(&right, &mut budget)?;
    assert!(round_trip.equals(&left, &mut budget)?);
    Ok(())
}

/// The independent inverse oracle also covers purely real and purely imaginary values.
fn inverse_case(
    value: &Gaussian,
    real: i64,
    imaginary: i64,
    denominator: u64,
    budget: &mut Budget,
) -> TestResult {
    let real = i128::from(real);
    let imaginary = i128::from(imaginary);
    let scale = i128::from(denominator);
    let norm = real * real + imaginary * imaginary;
    if norm == 0 {
        return Ok(());
    }
    check_pair(
        &value.inverse(budget)?,
        [(real * scale, norm), (-imaginary * scale, norm)],
        budget,
    )
}

/// Associativity, distributivity, identities and Bombelli's cube use exact independent values.
/// ´test:crate:gaussian-field-identities´
#[test]
fn gaussian_field_identities() -> TestResult {
    let mut budget = Budget::new(Limits::default())?;
    let a = gaussian("2", "1", &mut budget)?;
    let b = gaussian("-1/3", "2/5", &mut budget)?;
    let c = gaussian("3/7", "-4/9", &mut budget)?;
    let zero = Gaussian::zero(&mut budget)?;
    let one = Gaussian::one(&mut budget)?;
    assert!(a.add(&zero, &mut budget)?.equals(&a, &mut budget)?);
    assert!(a.mul(&one, &mut budget)?.equals(&a, &mut budget)?);
    assert!(
        a.add(&a.negated(&mut budget)?, &mut budget)?
            .is_zero(&mut budget)?
    );
    let left = a.add(&b, &mut budget)?.add(&c, &mut budget)?;
    let right = a.add(&b.add(&c, &mut budget)?, &mut budget)?;
    assert!(left.equals(&right, &mut budget)?);
    let left = a.mul(&b, &mut budget)?.mul(&c, &mut budget)?;
    let right = a.mul(&b.mul(&c, &mut budget)?, &mut budget)?;
    assert!(left.equals(&right, &mut budget)?);
    let left = a.mul(&b.add(&c, &mut budget)?, &mut budget)?;
    let right = a
        .mul(&b, &mut budget)?
        .add(&a.mul(&c, &mut budget)?, &mut budget)?;
    assert!(left.equals(&right, &mut budget)?);
    check_pair(
        &a.mul(&a, &mut budget)?.mul(&a, &mut budget)?,
        [(2, 1), (11, 1)],
        &mut budget,
    )?;
    assert!(!a.equals(&b, &mut budget)?);
    Ok(())
}

/// Conjugation is involutive and respects sums, products and inverses; its product is the norm.
/// ´test:crate:gaussian-conjugation-and-norm´
#[test]
fn gaussian_conjugation_and_norm() -> TestResult {
    let mut budget = Budget::new(Limits::default())?;
    let a = gaussian("2/3", "-5/7", &mut budget)?;
    let b = gaussian("-3/4", "2/5", &mut budget)?;
    let conjugate = a.conjugate(&mut budget)?;
    check_pair(&conjugate, [(2, 3), (5, 7)], &mut budget)?;
    assert!(conjugate.conjugate(&mut budget)?.equals(&a, &mut budget)?);
    check_fraction(&a.norm(&mut budget)?, (421, 441), &mut budget)?;
    check_pair(
        &a.mul(&conjugate, &mut budget)?,
        [(421, 441), (0, 1)],
        &mut budget,
    )?;
    let left = a.add(&b, &mut budget)?.conjugate(&mut budget)?;
    let right = conjugate.add(&b.conjugate(&mut budget)?, &mut budget)?;
    assert!(left.equals(&right, &mut budget)?);
    let left = a.mul(&b, &mut budget)?.conjugate(&mut budget)?;
    let right = conjugate.mul(&b.conjugate(&mut budget)?, &mut budget)?;
    assert!(left.equals(&right, &mut budget)?);
    let left = a.inverse(&mut budget)?.conjugate(&mut budget)?;
    assert!(left.equals(&conjugate.inverse(&mut budget)?, &mut budget)?);
    let product_norm = a
        .norm(&mut budget)?
        .mul(&b.norm(&mut budget)?, &mut budget)?;
    assert_eq!(a.mul(&b, &mut budget)?.norm(&mut budget)?, product_norm);
    Ok(())
}

/// Division and inversion reject zero after the charged zero scan, before norm arithmetic.
/// ´test:crate:gaussian-zero-divisor-refused´
#[test]
fn gaussian_zero_divisor_refused() -> TestResult {
    let mut setup = Budget::new(Limits::default())?;
    let zero = gaussian("-0/7", "0", &mut setup)?;
    let value = gaussian("1", "2", &mut setup)?;
    for inverse in [false, true] {
        let mut budget = Budget::new(Limits::default())?;
        let fault = if inverse {
            zero.inverse(&mut budget)
        } else {
            value.div(&zero, &mut budget)
        }
        .err()
        .ok_or("zero divisor accepted")?;
        assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::ZeroDenominator));
        assert_eq!(budget.usage().arithmetic_steps, 2);
        assert_eq!(budget.usage().gcd_steps, 0);
        assert_eq!(budget.usage().coefficient_ops, 1);
    }
    Ok(())
}

/// Adjust exactly one finite resource for an isolated refusal fixture.
fn limits(kind: BudgetKind, ceiling: u64, profile: &Limits) -> Limits {
    let mut limits = profile.clone();
    match kind {
        BudgetKind::ComponentBits => limits.component_bits = ceiling,
        BudgetKind::IntermediateBits => limits.intermediate_bits = ceiling,
        BudgetKind::Work => limits.work = ceiling,
        BudgetKind::ArithmeticSteps => limits.arithmetic_steps = ceiling,
        BudgetKind::GcdSteps => limits.gcd_steps = ceiling,
        BudgetKind::CoefficientOps => limits.coefficient_ops = ceiling,
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = ceiling,
        _ => {}
    }
    limits
}

/// Discover the required preflight ceiling from structured refusals, then test `N` and `N - 1`.
fn boundary(
    kind: BudgetKind,
    profile: &Limits,
    operation: impl Fn(&mut Budget) -> Result<(), Fault>,
) -> TestResult {
    let mut ceiling = 1;
    for _ in 0..64 {
        let mut budget = Budget::new(limits(kind, ceiling, profile))?;
        match operation(&mut budget) {
            Ok(()) => {
                let mut short = Budget::new(limits(kind, ceiling - 1, profile))?;
                let error = operation(&mut short).err().ok_or("one-below accepted")?;
                let FaultKind::Exhausted {
                    budget: resource,
                    limit,
                    needed,
                    ..
                } = error.kind
                else {
                    return Err("expected resource exhaustion".into());
                };
                assert_eq!((resource, limit, needed), (kind, ceiling - 1, ceiling));
                return Ok(());
            }
            Err(error) => {
                let FaultKind::Exhausted {
                    budget: resource,
                    needed,
                    ..
                } = error.kind
                else {
                    return Err(error.into());
                };
                assert_eq!(resource, kind);
                assert!(needed > ceiling);
                ceiling = needed;
            }
        }
    }
    Err("preflight threshold did not converge".into())
}

/// Full product reservations refuse at each work, step, gcd, storage and intermediate boundary.
/// ´test:crate:gaussian-preflight-boundaries´
#[test]
fn gaussian_preflight_boundaries() -> TestResult {
    let mut setup = Budget::new(Limits::default())?;
    let value = gaussian("1", "0", &mut setup)?;
    for kind in [
        BudgetKind::Work,
        BudgetKind::ArithmeticSteps,
        BudgetKind::GcdSteps,
        BudgetKind::CoefficientOps,
        BudgetKind::StagedPayloadBytes,
        BudgetKind::IntermediateBits,
    ] {
        boundary(
            kind,
            &Limits {
                component_bits: 1,
                ..Limits::default()
            },
            |budget| value.mul(&value, budget).map(drop),
        )?;
    }
    let mut short = Budget::new(Limits {
        intermediate_bits: 4,
        component_bits: 1,
        ..Limits::default()
    })?;
    let before = short.usage();
    assert!(value.mul(&value, &mut short).is_err());
    assert_eq!(short.usage().arithmetic_steps, before.arithmetic_steps);
    assert_eq!(short.usage().gcd_steps, before.gcd_steps);
    Ok(())
}

/// All coefficient operations share the caller's monotone ledger, including copies and decisions.
/// ´test:crate:gaussian-shared-ledger´
#[test]
fn gaussian_shared_ledger() -> TestResult {
    let mut setup = Budget::new(Limits::default())?;
    let value = gaussian("2/3", "-5/7", &mut setup)?;
    let mut budget = Budget::new(Limits {
        coefficient_ops: 4,
        ..Limits::default()
    })?;
    let copied = value.copied(&mut budget)?;
    assert!(value.equals(&copied, &mut budget)?);
    let real = value.real(&mut budget)?;
    let imaginary = value.imaginary(&mut budget)?;
    let before = budget.usage();
    let error = value
        .is_zero(&mut budget)
        .err()
        .ok_or("spent ledger accepted")?;
    assert!(matches!(
        error.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::CoefficientOps,
            limit: 4,
            needed: 5,
            ..
        }
    ));
    assert_eq!(budget.usage().arithmetic_steps, before.arithmetic_steps);
    assert_eq!(budget.usage().coefficient_ops, 5);
    let (cas, _, object) = stored(&real, &imaginary)?;
    let crate::solving::SolveView::CoefficientValue(data) = cas.solve_view(&object)? else {
        return Err("Gaussian value missing".into());
    };
    assert_eq!(
        data.value,
        crate::solving::CoefficientValueData::Gaussian { real, imaginary }
    );
    Ok(())
}

/// Small integer powers are checked against direct repeated integer multiplication.
/// ´test:crate:integer-perfect-powers´
#[test]
fn integer_perfect_powers() -> TestResult {
    for order in 1..=9 {
        for root in -7_i64..=7 {
            let expected = root.pow(order);
            let mut budget = Budget::new(Limits::default())?;
            let result = integer_root(&BigInt::from(expected), order, &mut budget)?;
            let canonical = if order.is_multiple_of(2) {
                root.abs()
            } else {
                root
            };
            assert_eq!(result, Some(BigInt::from(canonical)));
        }
    }
    for (value, order) in [
        (2, 2),
        (15, 2),
        (17, 2),
        (26, 3),
        (28, 3),
        (-9, 3),
        (-16, 2),
    ] {
        assert_eq!(
            integer_root(
                &BigInt::from(value),
                order,
                &mut Budget::new(Limits::default())?
            )?,
            None
        );
    }
    Ok(())
}

/// Reduced numerator and denominator must both be powers; negative even roots remain absent.
/// ´test:crate:rational-perfect-powers´
#[test]
fn rational_perfect_powers() -> TestResult {
    for (text, order, expected) in [
        ("0", 7, Some("0")),
        ("1", 8, Some("1")),
        ("-1", 7, Some("-1")),
        ("-1", 8, None),
        ("64/729", 3, Some("4/9")),
        ("-64/729", 3, Some("-4/9")),
        ("144/324", 2, Some("2/3")),
        ("4/3", 2, None),
        ("3/4", 2, None),
        ("-4/9", 2, None),
        ("17/19", 1, Some("17/19")),
    ] {
        let mut budget = Budget::new(Limits::default())?;
        let value = Rational::parse(text, &mut budget)?;
        let result = rational_root(&value, order, &mut budget)?;
        let expected = expected
            .map(|text| Rational::parse(text, &mut budget))
            .transpose()?;
        assert_eq!(result, expected);
    }
    Ok(())
}

/// Undefined root order and finite exponent ceilings produce structured refusals.
/// ´test:crate:perfect-power-order-refusals´
#[test]
fn perfect_power_order_refusals() -> TestResult {
    let mut budget = Budget::new(Limits::default())?;
    for fault in [
        integer_root(&BigInt::from(8), 0, &mut budget).err(),
        rational_root(&Rational::one(), 0, &mut budget).err(),
    ] {
        assert_eq!(
            fault.ok_or("zero order accepted")?.kind,
            FaultKind::Invalid(InvalidCode::UndefinedPower)
        );
    }
    let mut limited = Budget::new(Limits {
        integer_exponent: 2,
        ..Limits::default()
    })?;
    let fault = integer_root(&BigInt::from(-8), 3, &mut limited)
        .err()
        .ok_or("order ceiling ignored")?;
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::IntegerExponent,
            limit: 2,
            needed: 3,
            ..
        }
    ));
    Ok(())
}

/// Large signed cubes and nonpowers exercise candidate refinement at the admitted bit ceiling.
/// ´test:crate:large-perfect-powers-at-bit-limit´
#[test]
fn large_perfect_powers_at_bit_limit() -> TestResult {
    let root = (BigUint::one() << 120_usize) + BigUint::from(12_345_u32);
    let cube = root.pow(3);
    let limits = Limits {
        component_bits: cube.bits(),
        intermediate_bits: cube.bits() + 6,
        work: 50_000_000,
        arithmetic_steps: 1_000_000,
        ..Limits::default()
    };
    for negative in [false, true] {
        let sign = if negative {
            num_bigint::Sign::Minus
        } else {
            num_bigint::Sign::Plus
        };
        let value = BigInt::from_biguint(sign, cube.clone());
        let expected = BigInt::from_biguint(sign, root.clone());
        assert_eq!(
            integer_root(&value, 3, &mut Budget::new(limits.clone())?)?,
            Some(expected)
        );
        let mut short = Budget::new(Limits {
            component_bits: cube.bits() - 1,
            ..limits.clone()
        })?;
        let fault = integer_root(&value, 3, &mut short)
            .err()
            .ok_or("oversized radicand accepted")?;
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::ComponentBits,
                ..
            }
        ));
    }
    let adjacent = BigInt::from(&cube + BigUint::one());
    assert_eq!(
        integer_root(&adjacent, 3, &mut Budget::new(limits.clone())?)?,
        None
    );
    let even = -BigInt::from(root.pow(2));
    assert_eq!(
        integer_root(&even, 2, &mut Budget::new(limits.clone())?)?,
        None
    );
    let high_order = BigInt::from(BigUint::from(3_u8).pow(65));
    assert_eq!(
        integer_root(
            &high_order,
            65,
            &mut Budget::new(Limits {
                intermediate_bits: 512,
                ..limits
            })?
        )?,
        Some(BigInt::from(3))
    );
    Ok(())
}

/// Large rational cubes verify both component roots and preserve the negative odd-root sign.
/// ´test:crate:large-rational-perfect-powers´
#[test]
fn large_rational_perfect_powers() -> TestResult {
    let root = (BigUint::one() << 100_usize) + BigUint::one();
    let numerator = root.pow(3);
    let denominator = BigUint::from(7_u8).pow(3);
    let limits = Limits {
        component_bits: numerator.bits(),
        intermediate_bits: 1024,
        work: 50_000_000,
        arithmetic_steps: 1_000_000,
        ..Limits::default()
    };
    let mut setup = Budget::new(limits.clone())?;
    let value = Rational::new(true, &numerator, &denominator, &mut setup)?;
    let expected = Rational::new(true, &root, &BigUint::from(7_u8), &mut setup)?;
    assert_eq!(
        rational_root(&value, 3, &mut Budget::new(limits.clone())?)?,
        Some(expected)
    );
    let nonpower = Rational::new(false, &numerator, &BigUint::from(17_u8), &mut setup)?;
    assert_eq!(
        rational_root(&nonpower, 3, &mut Budget::new(limits.clone())?)?,
        None
    );
    assert_eq!(rational_root(&value, 2, &mut Budget::new(limits)?)?, None);
    Ok(())
}

/// Candidate generation and verification share work; the complete integer-root envelope is checked.
/// ´test:crate:perfect-power-preflight-boundaries´
#[test]
fn perfect_power_preflight_boundaries() -> TestResult {
    let value = BigInt::from(729);
    let profile = Limits {
        component_bits: 16,
        ..Limits::default()
    };
    for kind in [
        BudgetKind::Work,
        BudgetKind::ArithmeticSteps,
        BudgetKind::StagedPayloadBytes,
    ] {
        boundary(kind, &profile, |budget| {
            integer_root(&value, 3, budget).map(drop)
        })?;
    }
    let profile = Limits {
        component_bits: 10,
        intermediate_bits: 16,
        ..Limits::default()
    };
    assert_eq!(
        integer_root(&value, 3, &mut Budget::new(profile.clone())?)?,
        Some(BigInt::from(9))
    );
    let mut short = Budget::new(Limits {
        intermediate_bits: 15,
        ..profile
    })?;
    assert!(matches!(
        integer_root(&value, 3, &mut short),
        Err(Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::IntermediateBits,
                needed: 16,
                ..
            },
            ..
        })
    ));
    assert_eq!(short.usage().arithmetic_steps, 1);
    Ok(())
}

/// Uniform result erasure for independently isolated numeric operation budgets.
type NumericOperation = fn(&Gaussian, &mut Budget) -> Result<(), Fault>;

/// Norm, inverse, division and both sums reserve their complete sequences before execution.
/// ´test:crate:gaussian-composite-boundaries´
#[test]
fn gaussian_composite_boundaries() -> TestResult {
    let value = gaussian("1", "0", &mut Budget::new(Limits::default())?)?;
    let operations: [NumericOperation; 5] = [
        |value, budget| value.add(value, budget).map(drop),
        |value, budget| value.sub(value, budget).map(drop),
        |value, budget| value.norm(budget).map(drop),
        |value, budget| value.inverse(budget).map(drop),
        |value, budget| value.div(value, budget).map(drop),
    ];
    let profile = Limits {
        component_bits: 2,
        ..Limits::default()
    };
    for operation in operations {
        for kind in [
            BudgetKind::Work,
            BudgetKind::ArithmeticSteps,
            BudgetKind::GcdSteps,
            BudgetKind::StagedPayloadBytes,
        ] {
            boundary(kind, &profile, |budget| operation(&value, budget))?;
        }
    }
    Ok(())
}

/// Copies, components, conjugation, equality and zero decisions each consume their shared ledger.
/// ´test:crate:gaussian-component-operation-boundaries´
#[test]
fn gaussian_component_operation_boundaries() -> TestResult {
    let value = gaussian("1", "0", &mut Budget::new(Limits::default())?)?;
    let operations: [NumericOperation; 8] = [
        |value, budget| value.copied(budget).map(drop),
        |value, budget| value.negated(budget).map(drop),
        |value, budget| value.conjugate(budget).map(drop),
        |value, budget| value.real(budget).map(drop),
        |value, budget| value.imaginary(budget).map(drop),
        |value, budget| value.equals(value, budget).map(drop),
        |value, budget| value.is_zero(budget).map(drop),
        |_, budget| Gaussian::one(budget).map(drop),
    ];
    for operation in operations {
        for kind in [
            BudgetKind::Work,
            BudgetKind::ArithmeticSteps,
            BudgetKind::CoefficientOps,
        ] {
            boundary(kind, &Limits::default(), |budget| operation(&value, budget))?;
        }
    }
    Ok(())
}

/// Input checks precede copying, and reduced output components retain their separate ceiling.
/// ´test:crate:gaussian-component-bits-refused´
#[test]
fn gaussian_component_bits_refused() -> TestResult {
    let value = gaussian("15", "0", &mut Budget::new(Limits::default())?)?;
    let mut exact = Budget::new(Limits {
        component_bits: 4,
        ..Limits::default()
    })?;
    check_pair(&value.copied(&mut exact)?, [(15, 1), (0, 1)], &mut exact)?;
    let mut short = Budget::new(Limits {
        component_bits: 3,
        ..Limits::default()
    })?;
    let fault = value
        .copied(&mut short)
        .err()
        .ok_or("oversized copy accepted")?;
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::ComponentBits,
            limit: 3,
            needed: 4,
            ..
        }
    ));
    assert_eq!(short.usage().arithmetic_steps, 0);
    let fault = value
        .add(&value, &mut exact)
        .err()
        .ok_or("oversized sum accepted")?;
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::ComponentBits,
            limit: 4,
            needed: 5,
            ..
        }
    ));
    Ok(())
}

/// Rational extraction, both root tests and reconstruction retain every reservation on one ledger.
/// ´test:crate:rational-perfect-power-boundaries´
#[test]
fn rational_perfect_power_boundaries() -> TestResult {
    let value = Rational::parse("512/729", &mut Budget::new(Limits::default())?)?;
    let profile = Limits {
        component_bits: 16,
        ..Limits::default()
    };
    for kind in [
        BudgetKind::Work,
        BudgetKind::ArithmeticSteps,
        BudgetKind::GcdSteps,
        BudgetKind::StagedPayloadBytes,
        BudgetKind::CoefficientOps,
    ] {
        boundary(kind, &profile, |budget| {
            rational_root(&value, 3, budget).map(drop)
        })?;
    }
    Ok(())
}
