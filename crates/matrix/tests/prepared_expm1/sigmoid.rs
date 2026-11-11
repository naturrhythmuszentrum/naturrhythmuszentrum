// SPDX-License-Identifier: AGPL-3.0-only
//! Scalar composition evidence uses existing test dependencies, without a matrix owner.
use super::*;
use nrz_bindings::readings::algebra::enclosure::prepared_expm1::{Outcome, Prepared};
use nrz_scalar::{
    ExpProgram, Expm1Parameters, Expm1Program, SigmoidFailure, SigmoidInput, SigmoidProgram,
};

/// Lower one checked CAS authority into the conditional numerical schedule.
fn basis() -> TestResult<(Expm1Program, Box<Prepared>)> {
    let context = ArithmeticContext::new()?;
    let mut f = Fixture::new()?;
    let r = broad(&mut f, 44)?;
    let Outcome::Ready(authority) = f.cas.prepare_expm1(
        &r,
        Policy::default(),
        &mut ledger()?,
        &mut Counts::default(),
    )?
    else {
        return Err("checked sigmoid basis unavailable".into());
    };
    let proof = authority.proof();
    let coefficients = proof
        .coefficients
        .iter()
        .map(|c| exact_float(&c.exact(&mut ledger()?)?))
        .collect::<TestResult<Vec<_>>>()?;
    let absolute = proof
        .stages
        .last()
        .ok_or("stage")?
        .error
        .add(&proof.absolute_tail, &mut ledger()?)?;
    let mut exponent = -1074;
    while power(exponent)?.compare(&absolute, &mut ledger()?)? == std::cmp::Ordering::Less {
        exponent += 1;
    }
    let absolute_error = exact_float(&power(exponent)?)?;
    let program = Expm1Program::new(
        &coefficients,
        Expm1Parameters {
            domain: [-2.0, 0.0],
            bits: 44,
            radius_bits: proof.radius_bits,
            absolute_error,
            environment: 1,
        },
        &context,
    )?;
    Ok((program, authority))
}

/// Require an exact binary64 encoding by integer-ordered search; never round an operand.
fn exact_float(q: &Rational) -> TestResult<f64> {
    let mut b = ledger()?;
    let mut lo = 0_u64;
    let mut hi = 4.0_f64.to_bits();
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if oracles::value(mid, &mut b)?.compare(q, &mut b)? == std::cmp::Ordering::Greater {
            hi = mid - 1;
        } else {
            lo = mid;
        }
    }
    if oracles::value(lo, &mut b)? != *q {
        return Err("operand is not exactly binary64".into());
    }
    Ok(f64::from_bits(lo))
}

/// Every represented sigmoid row contains its entire golden interval and meets its own absolute target.
/// Refusal rows preserve their named cause; the nonrepresented mathematical box is not rounded into a native claim.
/// # Panics
/// Panics if containment, slack, row counts or a typed refusal changes.
/// ´claim:matrix:scalar-sigmoid-golden-values´
/// ´test:integration:scalar-sigmoid-golden-values´
#[test]
fn scalar_sigmoid_golden_values() -> TestResult {
    let payload = nrz_docgen::golden::read(
        include_bytes!("../../../../data/golden/prepared-binary64.json").as_slice(),
        nrz_docgen::golden::Limits::default(),
        &mut ledger()?,
    )?;
    let context = ArithmeticContext::new()?;
    let (native, _authority) = basis()?;
    let exp = ExpProgram::new(&native, 44);
    let mut successes = 0;
    let mut refusals = 0;
    let mut reference = 0;
    for row in payload.value()["entries"].as_array().ok_or("entries")? {
        if row["operation"] != "sigma" {
            continue;
        }
        let id = row["id"].as_str().ok_or("id")?;
        if id == "sigma-ln-two-box" {
            reference += 1;
            continue;
        }
        let bits = u32::try_from(row["target"]["bits"].as_u64().ok_or("bits")?)?;
        let program = SigmoidProgram::new(&exp, [0.0, 2.0], bits)?;
        let texts = if let Some(point) = row["input"]["point"].as_str() {
            [point, point]
        } else {
            [
                row["input"]["box"][0].as_str().ok_or("lower")?,
                row["input"]["box"][1].as_str().ok_or("upper")?,
            ]
        };
        let mut operands = [0.0; 2];
        for (slot, text) in operands.iter_mut().zip(texts) {
            *slot = exact_float(&Rational::parse(text, &mut ledger()?)?)?;
        }
        let input = SigmoidInput {
            endpoints: operands,
            center: operands[0] + (operands[1] - operands[0]) / 2.0,
            bits,
        };
        let result = program.evaluate(input, &context);
        if let Some(reason) = row["expected"]["refusal"].as_str() {
            let error = result.expect_err("golden refusal");
            assert!(
                match reason {
                    "below-normal-operand" => error == SigmoidFailure::BelowNormalOperand,
                    "outside-domain" => error == SigmoidFailure::OutsideDomain,
                    "target-not-certifiable" =>
                        matches!(error, SigmoidFailure::TargetNotCertifiable { .. }),
                    _ => false,
                },
                "{id}: {error:?}"
            );
            refusals += 1;
            continue;
        }
        let output = result?;
        for (index, name) in if row["input"]["point"].is_string() {
            [(0, "point"), (1, "point")]
        } else {
            [(0, "lower_endpoint"), (1, "upper_endpoint")]
        } {
            let pair = row["golden"][name].as_array().ok_or("golden")?;
            let mut b = ledger()?;
            let lo = Rational::parse(pair[0].as_str().ok_or("low")?, &mut b)?;
            let hi = Rational::parse(pair[1].as_str().ok_or("high")?, &mut b)?;
            let endpoint = represented(output.endpoints[index], &mut b)?;
            let slack = if index == 0 {
                oracles::le(&endpoint, &lo, &mut b)?;
                hi.sub(&endpoint, &mut b)?
            } else {
                oracles::le(&hi, &endpoint, &mut b)?;
                endpoint.sub(&lo, &mut b)?
            };
            oracles::le(&slack, &power(-i64::from(bits))?, &mut b)?;
        }
        successes += 1;
    }
    assert_eq!((successes, refusals, reference), (10, 4, 1));
    Ok(())
}

/// Reconstruct nearest rounding followed by the upper binary neighbour.
fn upward(q: &Rational, b: &mut Budget) -> TestResult<Rational> {
    oracles::neighbour(&oracles::nearest(q, b)?, true, b)
}
/// Reconstruct one signed outward binary neighbour independently.
fn outward(q: &Rational, up: bool, b: &mut Budget) -> TestResult<Rational> {
    oracles::neighbour(&oracles::nearest(q, b)?, up, b)
}
/// Replay the complete schedule using rational arithmetic and independently decoded encodings.
fn reconstruction(
    x: f64,
    authority: &Prepared,
    b: &mut Budget,
) -> TestResult<([Rational; 2], Rational)> {
    if x == 0.0 {
        let half = number(1, 2)?;
        return Ok(([half.clone(), half.clone()], half));
    }
    let argument = represented(-x, b)?;
    let proof = authority.proof();
    let mut schedule = Rational::zero();
    for coefficient in proof.coefficients.iter().rev() {
        schedule = oracles::nearest(&schedule.mul(&argument, b)?, b)?;
        schedule = oracles::nearest(&schedule.add(&coefficient.exact(b)?, b)?, b)?;
    }
    schedule = oracles::nearest(&schedule.mul(&argument, b)?, b)?;
    let radius = if x < f64::MIN_POSITIVE {
        let absolute = proof
            .stages
            .last()
            .ok_or("stage")?
            .error
            .add(&proof.absolute_tail, b)?;
        let mut exponent = -1074;
        while power(exponent)?.compare(&absolute, b)? == std::cmp::Ordering::Less {
            exponent += 1;
        }
        power(exponent)?
    } else {
        upward(
            &represented(x, b)?.mul(&power(-i64::from(proof.radius_bits))?, b)?,
            b,
        )?
    };
    let basis_image = [
        outward(&schedule.sub(&radius, b)?, false, b)?,
        outward(&schedule.add(&radius, b)?, true, b)?,
    ];
    let mut denominators = [Rational::zero(), Rational::zero()];
    for (i, up) in [(0, false), (1, true)] {
        let exponential = if basis_image[i].is_zero() {
            Rational::one()
        } else {
            outward(&basis_image[i].add(&Rational::one(), b)?, up, b)?
        };
        denominators[i] = outward(&exponential.add(&Rational::one(), b)?, up, b)?;
    }
    let nominal_exp = oracles::nearest(&schedule.add(&Rational::one(), b)?, b)?;
    let nominal_d = oracles::nearest(&nominal_exp.add(&Rational::one(), b)?, b)?;
    let nominal = oracles::nearest(&Rational::one().div(&nominal_d, b)?, b)?;
    Ok((
        [
            outward(&Rational::one().div(&denominators[1], b)?, false, b)?,
            outward(&Rational::one().div(&denominators[0], b)?, true, b)?,
        ],
        nominal,
    ))
}
/// Reconstruct an outward displacement, preserving the exact equal-value identity.
fn distance(c: &Rational, q: &Rational, b: &mut Budget) -> TestResult<Rational> {
    if c == q {
        Ok(Rational::zero())
    } else {
        upward(&oracles::absolute(&c.sub(q, b)?, b)?, b)
    }
}
/// Compare exact readings without rounding or adding their bounds.
fn maximum(a: Rational, c: Rational, b: &mut Budget) -> TestResult<Rational> {
    Ok(if a.compare(&c, b)? == std::cmp::Ordering::Less {
        c
    } else {
        a
    })
}

/// Exact rational nearest/tie and neighbour reconstruction checks every rounding stage and the once-counted nominal error.
/// Independent alternating sums establish truth containment and absolute endpoint slack on unsorted points and boxes.
/// # Panics
/// Panics if argument, endpoint association, nominal schedule, export or either error reading changes.
/// ´claim:matrix:scalar-sigmoid-independent-rounding´
/// ´test:integration:scalar-sigmoid-independent-rounding´
#[test]
fn scalar_sigmoid_independent_rounding() -> TestResult {
    let (native, authority) = basis()?;
    let exp = ExpProgram::new(&native, 44);
    let program = SigmoidProgram::new(&exp, [0.0, 2.0], 40)?;
    let context = ArithmeticContext::new()?;
    let cases = [
        ([1.0, 1.0], 1.0),
        ([0.0, f64::MIN_POSITIVE], f64::MIN_POSITIVE / 2.0),
        ([2.0, 2.0], 2.0),
        ([2.0_f64.powi(-600); 2], 2.0_f64.powi(-600)),
        ([0.0, 2.0], 0.25),
        ([2.0_f64.powi(-60); 2], 2.0_f64.powi(-60)),
        ([0.0, 0.0], 0.0),
    ];
    for (endpoints, center) in cases {
        let output = program.evaluate(
            SigmoidInput {
                endpoints,
                center,
                bits: 40,
            },
            &context,
        )?;
        let mut b = ledger()?;
        let left = reconstruction(endpoints[0], &authority, &mut b)?;
        let right = reconstruction(endpoints[1], &authority, &mut b)?;
        let nominal = reconstruction(center, &authority, &mut b)?;
        let lo = represented(output.endpoints[0], &mut b)?;
        let hi = represented(output.endpoints[1], &mut b)?;
        let c = represented(output.center, &mut b)?;
        assert_eq!(
            (&lo, &hi, &c),
            (&left.0[0], &right.0[1], &nominal.1),
            "exact schedule reconstruction"
        );
        let error = maximum(
            distance(&c, &nominal.0[0], &mut b)?,
            distance(&c, &nominal.0[1], &mut b)?,
            &mut b,
        )?;
        let total = maximum(
            maximum(
                distance(&c, &lo, &mut b)?,
                distance(&c, &hi, &mut b)?,
                &mut b,
            )?,
            error.clone(),
            &mut b,
        )?;
        assert_eq!(
            represented(output.numerical_error, &mut b)?,
            error,
            "nominal error once"
        );
        assert_eq!(
            represented(output.total_radius, &mut b)?,
            total,
            "total radius is a maximum"
        );
        for (x, endpoint, index) in [
            (endpoints[0], &lo, 0),
            (endpoints[1], &hi, 1),
            (center, &c, 2),
        ] {
            let truth = oracles::truth(&represented(x, &mut b)?, &mut b)?;
            let lower = Rational::one().div(&number(2, 1)?.add(&truth[1], &mut b)?, &mut b)?;
            let upper = Rational::one().div(&number(2, 1)?.add(&truth[0], &mut b)?, &mut b)?;
            if index == 0 {
                oracles::le(endpoint, &lower, &mut b)?;
                oracles::le(&upper.sub(endpoint, &mut b)?, &power(-40)?, &mut b)?;
            } else if index == 1 {
                oracles::le(&upper, endpoint, &mut b)?;
                oracles::le(&endpoint.sub(&lower, &mut b)?, &power(-40)?, &mut b)?;
            } else {
                for truth in [lower, upper] {
                    oracles::le(
                        &oracles::absolute(&c.sub(&truth, &mut b)?, &mut b)?,
                        &error,
                        &mut b,
                    )?;
                }
            }
        }
    }
    Ok(())
}
