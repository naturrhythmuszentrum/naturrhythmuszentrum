// SPDX-License-Identifier: AGPL-3.0-only
//! Independent supplied Real signs, Sturm chains and isolated selections.

use crate::testing::common::TestResult;
use crate::testing::solve_real;
use nrz_cas::domain::Layer;
use nrz_cas::limits::{Budget, Limits};
use nrz_cas::proof::{Certificate, Judgment, Statement};
use nrz_cas::solving::{SolveCertificate, SolveStatement};

/// A hand-authored chain counts three roots and replays in a fresh original Real frame.
/// ´test:integration:real-sturm-cubic-replay´
#[test]
fn real_sturm_cubic_replay() -> TestResult {
    replay(solve_real::cubic()?, solve_real::cubic()?)
}

/// Count one supplies existence of the actual isolated selection, without a Complex relation.
/// ´test:integration:real-cell-existence-replay´
#[test]
fn real_cell_existence_replay() -> TestResult {
    replay(solve_real::existence()?, solve_real::existence()?)
}

/// Independently construct the expected full judgment before replay and deterministic re-export.
fn replay(mut source: solve_real::Fixture, mut target: solve_real::Fixture) -> TestResult {
    let expected = Judgment {
        context: target.base.context.clone(),
        layer: Layer::Logic,
        space: None,
        statement: target
            .steps
            .last()
            .ok_or("expected final theorem")?
            .conclusion
            .clone(),
        residuals: Vec::new(),
    };
    let proof = source.base.cas.check_proof(
        &source.base.context,
        &source.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert!(proof.is_discharged());
    let bytes = source
        .base
        .cas
        .export_witness(&proof, &mut Budget::new(Limits::blind_cubic())?)?;
    let replayed =
        target
            .base
            .cas
            .replay(&bytes, &expected, &mut Budget::new(Limits::blind_cubic())?)?;
    assert!(replayed.is_discharged());
    assert_eq!(
        target
            .base
            .cas
            .export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}

/// Every mutation begins with an accepted chain and pins the field being changed.
/// ´test:integration:real-sturm-tampering´
#[test]
fn real_sturm_tampering() -> TestResult {
    for mutation in 0..5 {
        let mut fixture = solve_real::cubic()?;
        fixture.base.cas.check_proof(
            &fixture.base.context,
            &fixture.steps,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let last = fixture.steps.last_mut().ok_or("count application")?;
        let Certificate::Solve(certificate) = &mut last.certificate else {
            return Err("certificate".into());
        };
        let SolveCertificate::RealRootCount(certificate) = certificate.as_mut() else {
            return Err("count certificate".into());
        };
        mutate(certificate, mutation)?;
        assert!(
            fixture
                .base
                .cas
                .check_proof(
                    &fixture.base.context,
                    &fixture.steps,
                    &mut Budget::new(Limits::blind_cubic())?
                )
                .is_err()
        );
    }
    Ok(())
}

/// Remainders, infinity evidence, cells, endpoint signs and totals are independently constrained.
fn mutate(
    certificate: &mut nrz_cas::solving::CertificateRealRootCount,
    mutation: usize,
) -> TestResult {
    match mutation {
        0 => {
            assert_ne!(certificate.quotients[3], certificate.quotients[4]);
            certificate.quotients[3] = certificate.quotients[4].clone();
        }
        1 => {
            assert_eq!(certificate.signs.len(), 29);
            certificate.signs.remove(0);
        }
        2 => {
            assert_ne!(certificate.cells[0], certificate.cells[1]);
            certificate.cells[1] = certificate.cells[0].clone();
        }
        3 => {
            assert_ne!(certificate.signs[8], certificate.signs[18]);
            certificate.signs[8] = certificate.signs[18];
        }
        4 => {
            assert_eq!(certificate.exterior_left, 0);
            certificate.exterior_left = 1;
            assert_ne!(certificate.exterior_left, 0);
        }
        _ => return Err("mutation index".into()),
    }
    Ok(())
}

/// False rational signs cannot become ordered-field facts.
/// ´test:integration:real-sign-tampering´
#[test]
fn real_sign_tampering() -> TestResult {
    let mut fixture = solve_real::Fixture::new()?;
    fixture.sign(-1, 1)?;
    fixture.base.cas.check_proof(
        &fixture.base.context,
        &fixture.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let last = fixture.steps.last_mut().ok_or("sign application")?;
    let Statement::Solve(statement) = &mut last.conclusion else {
        return Err("sign statement".into());
    };
    let SolveStatement::RealSign(statement) = statement.as_mut() else {
        return Err("sign shape".into());
    };
    assert_eq!(
        statement.conclusion,
        nrz_cas::solving::RealSignConclusion::Negative
    );
    statement.conclusion = nrz_cas::solving::RealSignConclusion::Positive;
    assert_ne!(
        statement.conclusion,
        nrz_cas::solving::RealSignConclusion::Negative
    );
    assert!(
        fixture
            .base
            .cas
            .check_proof(
                &fixture.base.context,
                &fixture.steps,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    Ok(())
}

/// Exact ordered-field arithmetic, weakening and identity transport share one bounded derivation.
/// ´test:integration:real-sign-laws´
#[test]
fn real_sign_laws() -> TestResult {
    use nrz_cas::solving::{RealSignConclusion as Sign, RealSignRule as Rule};
    use nrz_cas::syntax::Operator;
    let mut fixture = solve_real::Fixture::new()?;
    let negative = fixture.literal(-2, 1)?;
    let cube_value = fixture.literal(-8, 1)?;
    let square = fixture.base.expression(
        Operator::IntegerPower(2),
        std::slice::from_ref(&negative),
        false,
    )?;
    let cube = fixture.base.expression(
        Operator::IntegerPower(3),
        std::slice::from_ref(&negative),
        false,
    )?;
    let identity = fixture.equality(&cube, &cube_value)?;
    let sum =
        fixture
            .base
            .expression(Operator::Add, &[negative.clone(), negative.clone()], false)?;
    let product =
        fixture
            .base
            .expression(Operator::Mul, &[negative.clone(), negative.clone()], false)?;
    let opposite =
        fixture
            .base
            .expression(Operator::Neg, std::slice::from_ref(&negative), false)?;
    let mut steps = vec![
        node(Rule::Rational, &negative, Sign::Negative, &[]),
        node(Rule::Sum, &sum, Sign::NonPositive, &[0, 0]),
        node(Rule::Product, &product, Sign::Positive, &[0, 0]),
        node(Rule::Negation, &opposite, Sign::Positive, &[0]),
        node(Rule::EvenPower, &square, Sign::NonNegative, &[]),
        node(Rule::Rational, &cube_value, Sign::Negative, &[]),
        node(Rule::Identity, &cube, Sign::Negative, &[5]),
        node(Rule::OddPowerReflection, &negative, Sign::NonZero, &[6]),
    ];
    steps[4].exponent = 2;
    steps[6].premises = vec![identity];
    steps[7].exponent = 3;
    fixture.sign_steps(steps)?;
    fixture.base.cas.check_proof(
        &fixture.base.context,
        &fixture.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    Ok(())
}

/// Author one finite sign node without concealing rule data or earlier-node indices.
fn node(
    rule: nrz_cas::solving::RealSignRule,
    subject: &nrz_cas::Handle,
    conclusion: nrz_cas::solving::RealSignConclusion,
    arguments: &[u32],
) -> nrz_cas::solving::RealSignStep {
    nrz_cas::solving::RealSignStep {
        rule,
        subject: subject.clone(),
        conclusion,
        arguments: arguments.to_vec(),
        premises: Vec::new(),
        exponent: 0,
    }
}

/// A square of an undefined quotient is not a total nonnegative Real expression.
/// ´test:integration:real-sign-strict-totality´
#[test]
fn real_sign_strict_totality() -> TestResult {
    use nrz_cas::solving::{RealSignConclusion as Sign, RealSignRule as Rule};
    use nrz_cas::syntax::Operator;
    let mut fixture = solve_real::Fixture::new()?;
    let one = fixture.literal(1, 1)?;
    let zero = fixture.literal(0, 1)?;
    let quotient = fixture
        .base
        .expression(Operator::Div, &[one, zero], false)?;
    let square = fixture
        .base
        .expression(Operator::IntegerPower(2), &[quotient], false)?;
    let mut step = node(Rule::EvenPower, &square, Sign::NonNegative, &[]);
    step.exponent = 2;
    fixture.sign_steps(vec![step])?;
    assert!(
        fixture
            .base
            .cas
            .check_proof(
                &fixture.base.context,
                &fixture.steps,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    Ok(())
}

/// An otherwise valid negation rejects a node edge changed from earlier to forward.
/// ´test:integration:real-sign-forward-edge´
#[test]
fn real_sign_forward_edge() -> TestResult {
    use nrz_cas::solving::{RealSignConclusion as Sign, RealSignRule as Rule};
    let mut fixture = solve_real::Fixture::new()?;
    let negative = fixture.literal(-1, 1)?;
    let opposite = fixture.base.expression(
        nrz_cas::syntax::Operator::Neg,
        std::slice::from_ref(&negative),
        false,
    )?;
    fixture.sign_steps(vec![
        node(Rule::Rational, &negative, Sign::Negative, &[]),
        node(Rule::Negation, &opposite, Sign::Positive, &[0]),
    ])?;
    fixture.base.cas.check_proof(
        &fixture.base.context,
        &fixture.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let Certificate::Solve(certificate) = &mut fixture.steps[0].certificate else {
        return Err("sign certificate".into());
    };
    let SolveCertificate::RealSign(certificate) = certificate.as_mut() else {
        return Err("sign shape".into());
    };
    assert_eq!(certificate.steps[1].arguments, [0]);
    certificate.steps[1].arguments[0] = 1;
    assert_ne!(certificate.steps[1].arguments, [0]);
    assert!(
        fixture
            .base
            .cas
            .check_proof(
                &fixture.base.context,
                &fixture.steps,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    Ok(())
}

/// Square-free reduction preserves repeated original roots; endpoint singletons account for them exactly once.
/// ´test:integration:real-sturm-repeated-singletons´
#[test]
fn real_sturm_repeated_singletons() -> TestResult {
    replay(solve_real::repeated()?, solve_real::repeated()?)
}

/// A checked quotient sign carries its definedness through an exact same-region premise import.
/// ´test:integration:real-sign-checked-division-import´
#[test]
fn real_sign_checked_division_import() -> TestResult {
    use nrz_cas::solving::{RealSignConclusion as Sign, RealSignRule as Rule};
    let mut fixture = solve_real::Fixture::new()?;
    let one = fixture.literal(1, 1)?;
    let two = fixture.literal(2, 1)?;
    let quotient = fixture.base.expression(
        nrz_cas::syntax::Operator::Div,
        &[one.clone(), two.clone()],
        false,
    )?;
    let proof = fixture.sign_steps(vec![
        node(Rule::Rational, &one, Sign::Positive, &[]),
        node(Rule::Rational, &two, Sign::Positive, &[]),
        node(Rule::Product, &quotient, Sign::Positive, &[0, 1]),
    ])?;
    let mut import = node(Rule::Hypothesis, &quotient, Sign::Positive, &[]);
    import.premises = vec![proof];
    fixture.sign_steps(vec![import])?;
    let mut target = solve_real::Fixture::new()?;
    let first = target.literal(1, 1)?;
    let second = target.literal(2, 1)?;
    let value = target
        .base
        .expression(nrz_cas::syntax::Operator::Div, &[first, second], false)?;
    let mut expected = node(Rule::Hypothesis, &value, Sign::Positive, &[]);
    expected.premises = vec![0];
    target.sign_steps(vec![expected])?;
    replay(fixture, target)
}

/// A one-root Real stratum accepts real radicals; the three-root stratum retains complete exact coverage.
/// ´test:integration:real-radicals-presentation-boundary´
#[test]
fn real_radicals_presentation_boundary() -> TestResult {
    use nrz_cas::request::Outcome;
    use nrz_cas::solving::{SolveForm, SolveReason};
    let (mut fixture, request) =
        crate::testing::solve_blind::real_polynomial(&[8, 0, 0, 1], SolveForm::Radicals)?;
    let outcome = fixture.cas.run(&request);
    let Outcome::Solved { answer, .. } = outcome else {
        return Err(format!("Real radical: {outcome:?}").into());
    };
    assert_eq!(answer.form, SolveForm::Radicals);
    assert!(
        answer
            .coverage
            .ok_or("Real radical coverage")?
            .is_discharged()
    );
    assert!(
        answer
            .facts
            .last()
            .ok_or("Real radical presentation")?
            .is_discharged()
    );
    let (mut fixture, request) =
        crate::testing::solve_blind::real_polynomial(&[1, -3, 0, 1], SolveForm::Radicals)?;
    let outcome = fixture.cas.run(&request);
    let Outcome::SolvePartial { answer, reason, .. } = outcome else {
        return Err(format!("Real cell radical refusal: {outcome:?}").into());
    };
    assert_eq!(reason, SolveReason::RequestedRealRadicalsUnavailable);
    assert_eq!(answer.form, SolveForm::AnyExact);
    assert_eq!(answer.facts.len(), 0);
    let coverage = answer.coverage.ok_or("retained Real cell coverage")?;
    assert!(coverage.is_discharged());
    assert_eq!(
        coverage.hypotheses(),
        [] as [nrz_cas::domain::HypothesisRef; 0]
    );
    Ok(())
}

/// A cell root is exact Real syntax but cannot be presented as a real radical.
/// ´test:integration:real-cell-refuses-radicals´
#[test]
fn real_cell_refuses_radicals() -> TestResult {
    use nrz_cas::solving::SolveForm;
    let mut fixture = solve_real::existence()?;
    let root = fixture.selected_value()?;
    let set = fixture.finite_set(&root)?;
    fixture.present(&set, SolveForm::RealExact)?;
    fixture.base.cas.check_proof(
        &fixture.base.context,
        &fixture.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let step = fixture.steps.last_mut().ok_or("Real cell presentation")?;
    let Statement::Solve(statement) = &mut step.conclusion else {
        return Err("Real presentation statement".into());
    };
    let SolveStatement::Presentation(claim) = statement.as_mut() else {
        return Err("Real presentation claim".into());
    };
    assert_eq!(claim.form, SolveForm::RealExact);
    claim.form = SolveForm::Radicals;
    let Certificate::Solve(certificate) = &mut step.certificate else {
        return Err("Real presentation certificate".into());
    };
    let SolveCertificate::RootSelectionRefinement(value) = certificate.as_mut() else {
        return Err("Real refinement".into());
    };
    assert_eq!(value.form, Some(SolveForm::RealExact));
    value.form = Some(SolveForm::Radicals);
    assert!(
        fixture
            .base
            .cas
            .check_proof(
                &fixture.base.context,
                &fixture.steps,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    Ok(())
}

/// A checked Complex equality is still a forbidden dependency of a `RealExact` presentation.
/// ´test:integration:real-presentation-refuses-complex-intermediate´
#[test]
fn real_presentation_refuses_complex_intermediate() -> TestResult {
    use nrz_cas::solving::SolveForm;
    let mut fixture = solve_real::Fixture::new()?;
    let one = fixture.literal(1, 1)?;
    let set = fixture.finite_set(&one)?;
    fixture.present(&set, SolveForm::RealExact)?;
    fixture.base.cas.check_proof(
        &fixture.base.context,
        &fixture.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    fixture.steps.pop().ok_or("prior Real presentation")?;
    fixture.complex_intermediate()?;
    fixture.base.cas.check_proof(
        &fixture.base.context,
        &fixture.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    fixture.present(&set, SolveForm::RealExact)?;
    assert!(
        fixture
            .base
            .cas
            .check_proof(
                &fixture.base.context,
                &fixture.steps,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    Ok(())
}

/// Count-one cells refuse overlapping endpoint ownership and endpoints depending on the counted relation.
/// ´test:integration:real-cells-overlap-and-dependency´
#[test]
fn real_cells_overlap_and_dependency() -> TestResult {
    for root_dependent in [false, true] {
        let mut fixture = solve_real::altered_cells(root_dependent)?;
        assert!(
            fixture
                .base
                .cas
                .check_proof(
                    &fixture.base.context,
                    &fixture.steps,
                    &mut Budget::new(Limits::blind_cubic())?
                )
                .is_err()
        );
    }
    Ok(())
}

/// Exact normalization of the difference proves a ground equality with the same strict domain.
/// ´test:integration:real-zero-difference-equality´
#[test]
fn real_zero_difference_equality() -> TestResult {
    let mut fixture = solve_real::Fixture::new()?;
    let one = fixture.literal(1, 1)?;
    let two = fixture.literal(2, 1)?;
    let three = fixture.literal(3, 1)?;
    let sum = fixture
        .base
        .expression(nrz_cas::syntax::Operator::Add, &[one, two], false)?;
    fixture.sign_equality(&sum, &three)?;
    fixture.base.cas.check_proof(
        &fixture.base.context,
        &fixture.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let Statement::Equality { left, right, .. } = &mut fixture
        .steps
        .last_mut()
        .ok_or("ground equality")?
        .conclusion
    else {
        return Err("ground equality shape".into());
    };
    assert_ne!(left, right);
    std::mem::swap(left, right);
    assert!(
        fixture
            .base
            .cas
            .check_proof(
                &fixture.base.context,
                &fixture.steps,
                &mut Budget::new(Limits::blind_cubic())?
            )
            .is_err()
    );
    Ok(())
}

/// The public Real request refuses reachable Complex originals before any coefficient or orbit work.
/// ´test:integration:real-original-complex-refusal´
#[test]
fn real_original_complex_refusal() -> TestResult {
    for cancel in [false, true] {
        let (mut cas, request) = solve_real::complex_original(cancel)?;
        let outcome = cas.run(&request);
        let nrz_cas::request::Outcome::Unresolved {
            solve_reason,
            usage,
            ..
        } = outcome
        else {
            return Err(format!("Complex original cancellation {cancel}: {outcome:?}").into());
        };
        assert_eq!(
            solve_reason,
            Some(nrz_cas::solving::SolveReason::RealProblemHasComplexSymbol)
        );
        assert_eq!(
            [
                usage.coefficient_ops,
                usage.orbit_permutations,
                usage.gcd_steps
            ],
            [0; 3]
        );
    }
    Ok(())
}
