// SPDX-License-Identifier: AGPL-3.0-only
//! Positive Pi monomial roots preserve original domains and the checked Machin interval route.

use super::{
    Cas, Constant, Costs, ExactConstant, FaultKind, Handle, InvalidCode, Operator, Rational,
    RationalEnclosure, TestResult, enclosure_for, ledger, pin, rational, refused, root, rooted,
};

/// Preserve the original rational coefficient, Pi power and multiplication before the root.
fn original_pi(numerator: i64, denominator: u64, power: i64) -> TestResult<(Cas, Handle)> {
    rooted(vec![
        (
            Operator::Rational(Rational::from_ratio(
                numerator,
                denominator,
                &mut ledger()?,
            )?),
            vec![],
        ),
        (Operator::Constant(Constant::Pi), vec![]),
        (Operator::IntegerPower(power), vec![1]),
        (Operator::Mul, vec![0, 2]),
    ])
}

/// Independently admit the original positive Pi monomial fixture.
fn pi_root(numerator: i64, denominator: u64, power: i64) -> TestResult<ExactConstant> {
    let (cas, expression) = original_pi(numerator, denominator, power)?;
    Ok(ExactConstant::from_expression(
        &cas,
        &expression,
        &mut ledger()?,
    )?)
}

/// Positive Pi powers leave roots exactly, and root products use only proved positive factors.
/// ´test:integration:constant-square-root-pi-rules´
/// ´claim:cas:constant-square-root-pi-rules´
#[test]
fn constant_square_root_pi_rules() -> TestResult {
    let pi = ExactConstant::named(Constant::Pi, &mut ledger()?)?;
    let square_root = pi_root(1, 1, 1)?;
    assert!(square_root.positive().is_some());
    assert_eq!(square_root.pow(2, &mut ledger()?)?, pi);
    assert_eq!(
        square_root.div(&square_root, &mut ledger()?)?,
        ExactConstant::one()
    );
    assert_eq!(
        square_root.mul(&root(2, 1)?, &mut ledger()?)?,
        pi_root(2, 1, 1)?
    );
    assert_eq!(
        square_root.mul(&pi_root(2, 1, 1)?, &mut ledger()?)?,
        pi.mul(&root(2, 1)?, &mut ledger()?)?
    );
    assert_eq!(
        square_root.pow(3, &mut ledger()?)?,
        pi.mul(&square_root, &mut ledger()?)?
    );
    assert_eq!(
        pi_root(9, 49, 2)?,
        rational(3, 7)?.mul(&pi, &mut ledger()?)?
    );
    assert_eq!(pi_root(1, 1, 6)?, pi.pow(3, &mut ledger()?)?);
    assert_eq!(
        pi_root(1, 1, -1)?.pow(2, &mut ledger()?)?,
        ExactConstant::one().div(&pi, &mut ledger()?)?
    );
    assert_eq!(
        pi_root(1, 1, -2)?,
        ExactConstant::one().div(&pi, &mut ledger()?)?
    );
    assert_eq!(
        pi.positive().ok_or("positive Pi")?.sqrt(&mut ledger()?)?,
        square_root
    );
    Ok(())
}

/// A strictly positive sum still lies outside the explicitly bounded Pi monomial fragment.
/// ´test:integration:constant-square-root-pi-boundary´
/// ´claim:cas:constant-square-root-pi-boundary´
#[test]
fn constant_square_root_pi_boundary() -> TestResult {
    let (cas, expression) = rooted(vec![
        (Operator::Constant(Constant::Pi), vec![]),
        (Operator::Rational(Rational::one()), vec![]),
        (Operator::Add, vec![0, 1]),
    ])?;
    refused(
        &cas,
        &expression,
        InvalidCode::UnsupportedFragment,
        "finite coefficient jets",
    )?;
    let (cas, expression) = original_pi(-1, 1, 1)?;
    refused(
        &cas,
        &expression,
        InvalidCode::RefutedDomain,
        "exact constant expression",
    )?;
    Ok(())
}

/// Direct Pi and the original two-Pi product retain their own root and enclosure identities.
fn enclosure(two_pi: bool) -> TestResult<(Cas, RationalEnclosure)> {
    let mut operators = vec![(Operator::Constant(Constant::Pi), vec![])];
    if two_pi {
        operators.push((
            Operator::Rational(Rational::from_ratio(2, 1, &mut ledger()?)?),
            vec![],
        ));
        operators.push((Operator::Mul, vec![1, 0]));
    }
    let (cas, expression) = rooted(operators)?;
    let (cas, mut contract) = enclosure_for(cas, expression)?;
    if two_pi {
        contract.lower = Rational::from_ratio(2, 1, &mut ledger()?)?;
        contract.upper = Rational::from_ratio(3, 1, &mut ledger()?)?;
    }
    Ok((cas, contract))
}

/// Independently checked Machin bounds propagate through positive roots and reject inward boxes.
/// ´test:integration:constant-square-root-pi-enclosures´
/// ´claim:cas:constant-square-root-pi-enclosures´
#[test]
fn constant_square_root_pi_enclosures() -> TestResult {
    for two_pi in [false, true] {
        let (cas, contract) = enclosure(two_pi)?;
        let before = cas.canonical_snapshot(&mut ledger()?)?;
        cas.check_rational_enclosure(&contract, &mut ledger()?)?;
        let mut inward = contract;
        inward.upper = inward.lower.clone();
        let Err(fault) = cas.check_rational_enclosure(&inward, &mut ledger()?) else {
            panic!("inward Pi root enclosure");
        };
        assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::DecisionBox));
        assert_eq!(fault.location.operation, "closed pi enclosure");
        assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    Ok(())
}

/// Literal Pi-root costs include N/N−1 refusals, prior Work and a spent monotone ledger.
/// ´test:integration:constant-square-root-pi-costs´
/// ´claim:cas:constant-square-root-pi-costs´
#[test]
fn constant_square_root_pi_costs() -> TestResult {
    let cases = [
        (
            enclosure(false)?,
            Costs {
                admission: 59_574,
                enclosure: 89_711,
            },
        ),
        (
            enclosure(true)?,
            Costs {
                admission: 81_440,
                enclosure: 119_183,
            },
        ),
    ];
    for ((cas, contract), costs) in cases {
        let before = cas.canonical_snapshot(&mut ledger()?)?;
        pin(
            |budget| ExactConstant::from_expression(&cas, &contract.target, budget),
            costs.admission,
        )?;
        pin(
            |budget| cas.check_rational_enclosure(&contract, budget),
            costs.enclosure,
        )?;
        assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    Ok(())
}

/// A forged Machin partial sum for a root target refuses at the finite certificate checker.
/// ´test:integration:constant-square-root-pi-certificate-refusal´
/// ´claim:cas:constant-square-root-pi-certificate-refusal´
#[test]
fn constant_square_root_pi_certificate_refusal() -> TestResult {
    let (cas, mut contract) = enclosure(true)?;
    let before = cas.canonical_snapshot(&mut ledger()?)?;
    let nrz_cas::decision::EnclosureOrigin::PiBound { certificate, .. } = &mut contract.origin
    else {
        panic!("Pi root certificate fixture");
    };
    certificate.sums[0] = Rational::zero();
    let Err(fault) = cas.check_rational_enclosure(&contract, &mut ledger()?) else {
        panic!("false Machin partial sum");
    };
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(fault.location.operation, "Machin pi bounds");
    assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, before);
    Ok(())
}

/// The original quotient and square retain their source-domain checks before root normalization.
/// ´test:integration:constant-square-root-pi-original-square´
/// ´claim:cas:constant-square-root-pi-original-square´
#[test]
fn constant_square_root_pi_original_square() -> TestResult {
    for denominator in [7, 0] {
        let (cas, expression) = rooted(vec![
            (
                Operator::Rational(Rational::from_ratio(3, 1, &mut ledger()?)?),
                vec![],
            ),
            (Operator::Constant(Constant::Pi), vec![]),
            (Operator::Mul, vec![0, 1]),
            (
                Operator::Rational(Rational::from_ratio(denominator, 1, &mut ledger()?)?),
                vec![],
            ),
            (Operator::Div, vec![2, 3]),
            (Operator::IntegerPower(2), vec![4]),
        ])?;
        if denominator == 0 {
            refused(
                &cas,
                &expression,
                InvalidCode::RefutedDomain,
                "exact constant expression",
            )?;
        } else {
            let before = cas.canonical_snapshot(&mut ledger()?)?;
            let pi = ExactConstant::named(Constant::Pi, &mut ledger()?)?;
            assert_eq!(
                ExactConstant::from_expression(&cas, &expression, &mut ledger()?)?,
                rational(3, 7)?.mul(&pi, &mut ledger()?)?
            );
            assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, before);
        }
    }
    Ok(())
}
