// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed-candidate construction preserves regional meaning and exact provenance.

use crate::testing::common::{TestResult, default_budget};
use crate::testing::series_residual::{Fixture, Term, binary, member, rational};
use nrz_cas::proof::Statement;
use nrz_cas::series::{SeriesStatement, SourceClause};
use nrz_cas::syntax::Operator;
use nrz_cas::{FaultKind, InvalidCode};

/// Independent original addition.
fn add(a: Term, b: Term) -> Term {
    binary(Operator::Add, a, b)
}
/// Independent original subtraction.
fn sub(a: Term, b: Term) -> Term {
    binary(Operator::Sub, a, b)
}
/// Independent original multiplication.
fn mul(a: Term, b: Term) -> Term {
    binary(Operator::Mul, a, b)
}
/// Explicit original power syntax.
fn pow(a: Term, q: u32) -> Term {
    Term::Power(Box::new(a), q)
}

/// All eleven confirmed rows are checked against literal hand coefficients.
/// ´test:integration:residual-eleven-hand-fixtures´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "all eleven independent arithmetic fixtures remain adjacent"
)]
fn residual_eleven_hand_fixtures() -> TestResult {
    use Term::{Q, X, Y};
    let cases = [
        (
            vec!["0", "1", "0", "-1/6"],
            "0",
            X,
            Some(SourceClause::SineOrigin),
            add(Y(2), Y(0)),
            vec!["0", "0"],
        ),
        (
            vec!["0", "1", "0", "-1/3"],
            "0",
            X,
            Some(SourceClause::ArctangentOrigin),
            sub(mul(add(Q("1"), pow(X, 2)), Y(1)), Q("1")),
            vec!["0", "0", "0"],
        ),
        (
            vec!["0", "1", "0", "-2"],
            "0",
            Q("0"),
            None,
            add(sub(Y(1), Q("1")), mul(Q("6"), pow(X, 2))),
            vec!["0", "0", "0"],
        ),
        (
            vec!["9", "6", "1", "0"],
            "2",
            add(add(pow(X, 2), mul(Q("2"), X)), Q("1")),
            Some(SourceClause::PolynomialSeed),
            sub(Y(1), add(mul(Q("2"), X), Q("2"))),
            vec!["0", "0", "0"],
        ),
        (vec!["7"], "0", Q("7"), None, sub(Y(0), Q("7")), vec!["0"]),
        (
            vec!["1", "1", "1/2", "1/6"],
            "0",
            add(
                add(add(Q("1"), X), mul(Q("1/2"), pow(X, 2))),
                mul(Q("1/6"), pow(X, 3)),
            ),
            Some(SourceClause::PolynomialSeed),
            sub(Y(1), Y(0)),
            vec!["0", "0", "0"],
        ),
        (
            vec!["1", "0", "1/2", "0", "1/24"],
            "0",
            add(
                add(Q("1"), mul(Q("1/2"), pow(X, 2))),
                mul(Q("1/24"), pow(X, 4)),
            ),
            Some(SourceClause::PolynomialSeed),
            sub(Y(2), Y(0)),
            vec!["0", "0", "0"],
        ),
        (
            vec!["0", "2", "1/2"],
            "2",
            mul(Q("1/2"), sub(pow(X, 2), Q("4"))),
            Some(SourceClause::PolynomialSeed),
            sub(Y(1), X),
            vec!["0", "0"],
        ),
        (
            vec!["0", "7", "0", "9"],
            "0",
            add(mul(Q("7"), X), mul(Q("9"), pow(X, 3))),
            Some(SourceClause::PolynomialSeed),
            sub(mul(X, Y(1)), Y(0)),
            vec!["0", "0", "0"],
        ),
        (
            vec!["0", "1", "0"],
            "0",
            X,
            Some(SourceClause::PolynomialSeed),
            mul(Y(0), Y(1)),
            vec!["0", "1"],
        ),
        (
            vec!["0", "0"],
            "0",
            Q("0"),
            Some(SourceClause::PolynomialSeed),
            sub(Y(1), Q("1")),
            vec!["-1"],
        ),
    ];
    for reversed in [false, true] {
        for (values, centre, source, clause, residual, expected) in &cases {
            let mut f = Fixture::new(values, centre, reversed, false)?;
            let subject = if values.as_slice() == ["0", "1", "0", "-2"] {
                f.compose_pair(&["0", "1", "1", "0"], &["0", "1", "-1", "0"])?
            } else if let Some(clause) = clause {
                f.source(source, *clause)?
            } else {
                f.compose()?
            };
            let (expression, rows) = f.term(residual, expected.len())?;
            let expected = expected
                .iter()
                .map(|v| rational(v))
                .collect::<TestResult<Vec<_>>>()?;
            assert_eq!(rows.last().ok_or("root")?.coefficients, expected);
            let (computed, _) = f.cas.check_evaluated_residual(
                &subject,
                &expression,
                &f.y,
                &rows,
                None,
                &mut default_budget()?,
            )?;
            let Statement::Series(statement) = &computed.judgment().statement else {
                return Err("series".into());
            };
            let SeriesStatement::EvaluatedResidual { output, order, .. } = statement.as_ref()
            else {
                return Err("residual".into());
            };
            assert_eq!(usize::try_from(*order)?, expected.len());
            assert_eq!(
                f.cas
                    .series_object(output, &mut crate::testing::budgets::graph_budget()?)?
                    .order,
                *order
            );
            let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
            let result = f.cas.check_evaluated_residual(
                &subject,
                &expression,
                &f.y,
                &rows,
                Some(&member()),
                &mut default_budget()?,
            );
            if expected.iter().all(nrz_cas::Rational::is_zero) {
                let (_, solve) = result?;
                assert_eq!(solve.ok_or("Member")?.hypotheses(), subject.hypotheses());
            } else {
                assert_eq!(
                    result.expect_err("nonzero ambient Member").kind,
                    FaultKind::Invalid(InvalidCode::Rule)
                );
                assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);
            }
            let bytes = f.cas.export_witness(&computed, &mut default_budget()?)?;
            f.cas
                .replay(&bytes, computed.judgment(), &mut default_budget()?)?;
            let linear = f.linear(&expected)?;
            let (_, solved) = f.cas.check_evaluated_residual(
                &subject,
                &expression,
                &f.y,
                &rows,
                Some(&linear),
                &mut default_budget()?,
            )?;
            assert!(solved.is_some());
        }
    }
    Ok(())
}

/// Empty-region satisfaction is regional even when the complete residual is nonzero.
/// ´test:integration:residual-empty-guarded-region´
#[test]
fn residual_empty_guarded_region() -> TestResult {
    let mut f = Fixture::new(&["0"], "0", false, true)?;
    let subject = f.compose()?;
    let (expression, rows) = f.term(&Term::Q("1"), 1)?;
    let (residual, solve) = f.cas.check_evaluated_residual(
        &subject,
        &expression,
        &f.y,
        &rows,
        Some(&member()),
        &mut default_budget()?,
    )?;
    assert_eq!(rows[0].coefficients, [rational("1")?]);
    assert!(solve.is_some());
    assert_eq!(residual.judgment().residuals, []);
    Ok(())
}

/// Polynomial zero powers supply neither strict definedness nor scalar correspondence.
/// ´test:integration:residual-zero-power-boundary´
#[test]
fn residual_zero_power_boundary() -> TestResult {
    let mut f = Fixture::new(&["0"], "0", false, false)?;
    let subject = f.compose()?;
    let (expression, rows) = f.term(&sub(pow(Term::Q("0"), 0), Term::Q("1")), 1)?;
    assert_eq!(rows.len(), 4);
    assert_eq!(rows.last().ok_or("root")?.coefficients, [rational("0")?]);
    let (_, solve) = f.cas.check_evaluated_residual(
        &subject,
        &expression,
        &f.y,
        &rows,
        Some(&member()),
        &mut default_budget()?,
    )?;
    assert!(solve.is_some());
    let one = f.node(Operator::Rational(rational("1")?), &[])?;
    let defined = f.defined(&one)?;
    f.cas
        .check_proof(&f.context, &[defined], &mut default_budget()?)?;
    let undefined = f.defined(&expression)?;
    assert_eq!(
        f.cas
            .check_proof(&f.context, &[undefined], &mut default_budget()?)
            .expect_err("strict zero power is undefined")
            .kind,
        FaultKind::Invalid(InvalidCode::Rule)
    );
    Ok(())
}

/// Integer exponent ceilings cover the full original syntax before the power schedule exists.
/// ´test:integration:residual-integer-exponent-pins´
#[test]
fn residual_integer_exponent_pins() -> TestResult {
    use nrz_cas::limits::{Budget, BudgetKind, Limits};
    for dead in [false, true] {
        let mut f = Fixture::new(&["1"], "0", false, false)?;
        let subject = f.compose()?;
        let term = sub(pow(Term::Y(0), 2), Term::Q("1"));
        let term = if dead { mul(Term::Q("0"), term) } else { term };
        let (expression, rows) = f.term(&term, 1)?;
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        let fault = f
            .cas
            .check_evaluated_residual(
                &subject,
                &expression,
                &f.y,
                &rows,
                Some(&member()),
                &mut Budget::new(Limits {
                    integer_exponent: 1,
                    ..Limits::default()
                })?,
            )
            .expect_err("q-1");
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::IntegerExponent,
                limit: 1,
                needed: 2,
                ..
            }
        ));
        assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);
        let (residual, _) = f.cas.check_evaluated_residual(
            &subject,
            &expression,
            &f.y,
            &rows,
            Some(&member()),
            &mut Budget::new(Limits {
                integer_exponent: 2,
                ..Limits::default()
            })?,
        )?;
        let bytes = f.cas.export_witness(&residual, &mut default_budget()?)?;
        let mut receiver = Fixture::new(&["1"], "0", true, false)?;
        let before = receiver.cas.canonical_snapshot(&mut default_budget()?)?;
        let fault = receiver
            .cas
            .inspect(
                &bytes,
                &mut Budget::new(Limits {
                    integer_exponent: 1,
                    ..Limits::default()
                })?,
            )
            .expect_err("replay q-1");
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::IntegerExponent,
                limit: 1,
                needed: 2,
                ..
            }
        ));
        assert_eq!(
            before,
            receiver.cas.canonical_snapshot(&mut default_budget()?)?
        );
        receiver.cas.inspect(
            &bytes,
            &mut Budget::new(Limits {
                integer_exponent: 2,
                ..Limits::default()
            })?,
        )?;
    }
    Ok(())
}

/// Equal equations do not identify the original template; paired replay checks it second.
/// ´test:integration:residual-paired-second-expectation´
#[test]
fn residual_paired_second_expectation() -> TestResult {
    let mut sender = Fixture::new(&["0"], "0", false, false)?;
    let subject = sender.compose()?;
    let (expression, rows) = sender.term(&Term::Y(0), 1)?;
    let (residual, solve) = sender.cas.check_evaluated_residual(
        &subject,
        &expression,
        &sender.y,
        &rows,
        Some(&member()),
        &mut default_budget()?,
    )?;
    let solve = solve.ok_or("solve")?;
    let bytes = sender.cas.export_witness(&solve, &mut default_budget()?)?;
    let mut receiver = Fixture::new(&["0"], "0", true, false)?;
    let (wrong, expected) = receiver.expectations(&Term::Q("0"), &[rational("0")?], 0)?;
    let before = receiver.cas.canonical_snapshot(&mut default_budget()?)?;
    let fault = receiver
        .cas
        .replay_evaluated_residual(&bytes, &wrong, &expected, &mut default_budget()?)
        .expect_err("different original R");
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(
        before,
        receiver.cas.canonical_snapshot(&mut default_budget()?)?
    );
    // The identical solving endpoint passes ordinary replay: only the second comparison differs.
    receiver
        .cas
        .replay(&bytes, &expected, &mut default_budget()?)?;
    sender.cas.replay_evaluated_residual(
        &bytes,
        residual.judgment(),
        solve.judgment(),
        &mut default_budget()?,
    )?;
    Ok(())
}

/// Every row, ordinal and zero occurrence belongs to the certificate, including dead powers.
/// ´test:integration:residual-trace-and-grammar-refusals´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the closed grammar and complete trace mutations remain adjacent"
)]
fn residual_trace_and_grammar_refusals() -> TestResult {
    let mut f = Fixture::new(&["1", "0", "0"], "0", false, false)?;
    let subject = f.compose()?;
    let term = sub(pow(Term::Y(0), 2), Term::Q("1"));
    let (expression, rows) = f.term(&term, 3)?;
    for mutation in 0..7 {
        let mut changed = rows.clone();
        match mutation {
            0 => {
                changed.pop();
            }
            1 => changed.push(rows[0].clone()),
            2 => changed.swap(0, 1),
            3 => changed[0].ordinal = 1,
            4 => changed[0].coefficients[2] = rational("1")?,
            5 => {
                changed[0].coefficients.pop();
            }
            _ => changed.last_mut().ok_or("root")?.coefficients[0] = rational("1")?,
        }
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        let fault = f
            .cas
            .check_evaluated_residual(
                &subject,
                &expression,
                &f.y,
                &changed,
                None,
                &mut default_budget()?,
            )
            .expect_err("false trace");
        assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Rule));
        assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);
    }
    let y = f.y.clone();
    let x = f.x.clone();
    let extra = f.node(Operator::Free("z".into()), &[])?;
    let compound = f.node(Operator::Add, &[x.clone(), y.clone()])?;
    let unsupported = [
        f.node(Operator::Div, &[y.clone(), x.clone()])?,
        f.node(Operator::IntegerPower(-1), std::slice::from_ref(&y))?,
        f.node(Operator::Derivative("x".into()), &[compound])?,
        f.node(Operator::Derivative("y".into()), std::slice::from_ref(&y))?,
        f.node(Operator::Sine, std::slice::from_ref(&y))?,
        extra,
    ];
    for expression in unsupported {
        // Neither exponent zero nor zero multiplication may hide unsupported syntax.
        let dead = f.node(Operator::IntegerPower(0), std::slice::from_ref(&expression))?;
        let zero = f.node(Operator::Rational(rational("0")?), &[])?;
        let annihilated = f.node(Operator::Mul, &[zero, expression.clone()])?;
        for expression in [&expression, &dead, &annihilated] {
            let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
            let fault = f
                .cas
                .check_evaluated_residual(
                    &subject,
                    expression,
                    &f.y,
                    &rows,
                    None,
                    &mut default_budget()?,
                )
                .expect_err("unsupported original syntax");
            assert_eq!(
                fault.kind,
                FaultKind::Invalid(InvalidCode::SeriesUnsupported)
            );
            assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);
        }
    }
    for depth in [3, 4] {
        let (expression, _) = f.term(&pow(Term::Y(depth), 0), 1)?;
        let fault = f
            .cas
            .check_evaluated_residual(
                &subject,
                &expression,
                &f.y,
                &rows,
                None,
                &mut default_budget()?,
            )
            .expect_err("dead high derivative");
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::SeriesUnsupported)
        );
    }
    assert_eq!(
        f.cas
            .check_evaluated_residual(
                &subject,
                &expression,
                &x,
                &rows,
                None,
                &mut default_budget()?
            )
            .expect_err("y equals x")
            .kind,
        FaultKind::Invalid(InvalidCode::SeriesUnsupported)
    );
    Ok(())
}

/// Repeated DAG occurrences and nonintegral centres retain the complete ordered trace.
/// ´test:conformance:residual-occurrences-and-rational-centre´
#[test]
fn residual_occurrences_and_rational_centre() -> TestResult {
    for reversed in [false, true] {
        let mut f = Fixture::new(&["1/2", "1", "0"], "1/2", reversed, false)?;
        let subject = f.compose()?;
        let repeated = sub(add(Term::X, Term::X), add(Term::Y(0), Term::Y(0)));
        let (expression, rows) = f.term(&repeated, 3)?;
        assert_eq!(rows.len(), 7);
        assert_eq!(
            rows[0].coefficients,
            vec![rational("1/2")?, rational("1")?, rational("0")?]
        );
        let (_, solve) = f.cas.check_evaluated_residual(
            &subject,
            &expression,
            &f.y,
            &rows,
            Some(&member()),
            &mut default_budget()?,
        )?;
        assert!(solve.is_some());
        let mut changed = rows.clone();
        changed.remove(1);
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.cas
                .check_evaluated_residual(
                    &subject,
                    &expression,
                    &f.y,
                    &changed,
                    None,
                    &mut default_budget()?
                )
                .expect_err("no occurrence deduplication")
                .kind,
            FaultKind::Invalid(InvalidCode::Rule)
        );
        assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);
    }
    Ok(())
}

/// Declaration keys may be renamed consistently, while coefficient roles remain exact.
/// ´test:conformance:residual-coordinate-frame´
#[test]
fn residual_coordinate_frame() -> TestResult {
    use crate::testing::series_residual::admit;
    use nrz_cas::Reference;
    use nrz_cas::domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
    use nrz_cas::solving::{Coordinate, CoordinateRole, CoordinateSpaceData, SolveDraft};
    use nrz_cas::syntax::{Branch, Declaration, Draft, Environment, Field};
    for wrong_role in [false, true] {
        let mut f = Fixture::new(&["1/2", "1"], "1/2", false, false)?;
        f.header = admit(
            &mut f.cas,
            Draft::Header(HeaderDraft::Core(Environment {
                field: Field::Real,
                branch: Branch::StrictReal,
                declarations: ["t", "u"]
                    .into_iter()
                    .map(|key| Declaration {
                        key: key.into(),
                        display_name: "same-display".into(),
                        sort: crate::testing::common::real(),
                    })
                    .collect(),
            })),
        )?;
        f.x = f.node(Operator::Free("t".into()), &[])?;
        f.y = f.node(Operator::Free("u".into()), &[])?;
        f.centre = f.node(Operator::Rational(rational("1/2")?), &[])?;
        let product = admit(
            &mut f.cas,
            Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
                header: Reference::Existing(f.header.clone()),
                scalar_spaces: Vec::new(),
                coordinates: vec![Coordinate {
                    key: "t".into(),
                    field: Field::Real,
                    role: if wrong_role {
                        CoordinateRole::Unknown
                    } else {
                        CoordinateRole::Coefficient
                    },
                }],
            })),
        )?;
        f.region = admit(
            &mut f.cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(f.header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Product {
                    coordinates: product,
                },
                expression: RegionExpr::Ambient,
            }),
        )?;
        f.context = admit(
            &mut f.cas,
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(f.header.clone()),
                admissible: Reference::Existing(f.region.clone()),
                assumptions: Vec::new(),
            }),
        )?;
        let subject = f.compose()?;
        let (expression, rows) = f.term(&sub(Term::Y(0), Term::X), 2)?;
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        let result = f.cas.check_evaluated_residual(
            &subject,
            &expression,
            &f.y,
            &rows,
            Some(&member()),
            &mut default_budget()?,
        );
        if wrong_role {
            assert_eq!(
                result.expect_err("coefficient role").kind,
                FaultKind::Invalid(InvalidCode::SeriesUnsupported)
            );
            assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);
        } else {
            let (_, solve) = result?;
            let bytes = f
                .cas
                .export_witness(&solve.ok_or("Member")?, &mut default_budget()?)?;
            let mut receiver = Fixture::new(&["0"], "0", true, false)?;
            receiver.cas.inspect(&bytes, &mut default_budget()?)?;
        }
    }
    Ok(())
}

/// Supplied row operations cannot conceal the constant contradiction or change its matrix sign.
/// ´test:conformance:residual-linear-corruptions´
#[test]
fn residual_linear_corruptions() -> TestResult {
    use nrz_cas::solving::{SolveCertificate, SolveStatement};
    let mut f = Fixture::new(&["0", "0"], "0", false, false)?;
    let subject = f.source(&Term::Q("0"), SourceClause::PolynomialSeed)?;
    let (expression, rows) = f.term(&sub(Term::Y(1), Term::Q("1")), 1)?;
    let supplied = f.linear(&[rational("-1")?])?;
    let wrong_sign = f.linear(&[rational("1")?])?;
    for mutation in 0..4 {
        let mut changed = if mutation == 0 {
            wrong_sign.clone()
        } else {
            supplied.clone()
        };
        let SolveStatement::LinearSystem(statement) = &mut changed.statement else {
            return Err("linear".into());
        };
        let SolveCertificate::ExactLinearEquivalence(certificate) = &mut changed.certificate else {
            return Err("certificate".into());
        };
        match mutation {
            0 => {}
            1 => statement.augmented_rank = 0,
            2 => certificate.pivots.clear(),
            _ => statement.free_columns.push(0),
        }
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.cas
                .check_evaluated_residual(
                    &subject,
                    &expression,
                    &f.y,
                    &rows,
                    Some(&changed),
                    &mut default_budget()?
                )
                .expect_err("false linear certificate")
                .kind,
            FaultKind::Invalid(InvalidCode::Rule)
        );
        assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);
    }
    f.cas.check_evaluated_residual(
        &subject,
        &expression,
        &f.y,
        &rows,
        Some(&supplied),
        &mut default_budget()?,
    )?;
    Ok(())
}

/// Independent valid candidates distinguish actual derived obligations from convenient tautologies.
/// ´test:integration:residual-valid-wrong-premise-twins´
#[test]
fn residual_valid_wrong_premise_twins() -> TestResult {
    use Term::{Q, X, Y};
    for (candidate, clause, expected) in [
        (
            &["0", "1", "0", "-1/3"][..],
            SourceClause::ArctangentOrigin,
            vec!["0", "0", "0"],
        ),
        (
            &["0", "1", "0", "-1/6"][..],
            SourceClause::SineOrigin,
            vec!["0", "0", "1/2"],
        ),
    ] {
        let mut f = Fixture::new(candidate, "0", false, false)?;
        let subject = f.source(&X, clause)?;
        let (expression, rows) = f.term(&sub(mul(add(Q("1"), pow(X, 2)), Y(1)), Q("1")), 3)?;
        assert_eq!(
            rows.last().ok_or("root")?.coefficients,
            expected
                .iter()
                .map(|v| rational(v))
                .collect::<TestResult<Vec<_>>>()?
        );
        f.cas.check_evaluated_residual(
            &subject,
            &expression,
            &f.y,
            &rows,
            None,
            &mut default_budget()?,
        )?;
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        let result = f.cas.check_evaluated_residual(
            &subject,
            &expression,
            &f.y,
            &rows,
            Some(&member()),
            &mut default_budget()?,
        );
        if clause == SourceClause::ArctangentOrigin {
            result?;
        } else {
            assert_eq!(
                result.expect_err("sine jet is not a member").kind,
                FaultKind::Invalid(InvalidCode::Rule)
            );
            assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);
        }
    }
    let mut f = Fixture::new(&["0", "1", "0", "-2"], "0", false, false)?;
    let subject = f.compose_pair(&["0", "1", "1", "0"], &["0", "1", "-1", "0"])?;
    let (expression, rows) = f.term(&sub(sub(Y(1), Q("1")), mul(Q("6"), pow(X, 2))), 3)?;
    assert_eq!(
        rows.last().ok_or("root")?.coefficients,
        [rational("0")?, rational("0")?, rational("-12")?]
    );
    f.cas.check_evaluated_residual(
        &subject,
        &expression,
        &f.y,
        &rows,
        None,
        &mut default_budget()?,
    )?;
    Ok(())
}

/// Swapped original Compose operands agree through four coefficients but remain distinct subjects.
/// ´test:integration:residual-compose-provenance-twin´
#[test]
fn residual_compose_provenance_twin() -> TestResult {
    use Term::{Q, X, Y};
    use nrz_cas::series::{ResidualSubject, SeriesStatement as S};
    let candidate = ["0", "1", "0", "-2"];
    let p = ["0", "1", "1", "0"];
    let q = ["0", "1", "-1", "0"];
    let term = add(sub(Y(1), Q("1")), mul(Q("6"), pow(X, 2)));
    let mut sender = Fixture::new(&candidate, "0", false, false)?;
    let subject = sender.compose_pair(&p, &q)?;
    let (expression, rows) = sender.term(&term, 3)?;
    let (_, solve) = sender.cas.check_evaluated_residual(
        &subject,
        &expression,
        &sender.y,
        &rows,
        Some(&member()),
        &mut default_budget()?,
    )?;
    let bytes = sender
        .cas
        .export_witness(&solve.ok_or("solve")?, &mut default_budget()?)?;
    let mut receiver = Fixture::new(&candidate, "0", true, false)?;
    let (mut residual_expected, solve_expected) =
        receiver.expectations(&term, &[rational("0")?, rational("0")?, rational("0")?], 1)?;
    let left = receiver.object(
        &q.iter()
            .map(|v| rational(v))
            .collect::<TestResult<Vec<_>>>()?,
    )?;
    let right = receiver.object(
        &p.iter()
            .map(|v| rational(v))
            .collect::<TestResult<Vec<_>>>()?,
    )?;
    let Statement::Series(value) = &mut residual_expected.statement else {
        return Err("series".into());
    };
    let S::EvaluatedResidual {
        subject:
            ResidualSubject::Compose {
                left: old_left,
                right: old_right,
                ..
            },
        ..
    } = value.as_mut()
    else {
        return Err("subject".into());
    };
    *old_left = left;
    *old_right = right;
    let before = receiver.cas.canonical_snapshot(&mut default_budget()?)?;
    let mut budget = nrz_cas::limits::Budget::new(nrz_cas::limits::Limits {
        work: 20_000_000,
        replay_work: 20_000_000,
        ..nrz_cas::limits::Limits::default()
    })?;
    let fault = receiver
        .cas
        .replay_evaluated_residual(&bytes, &residual_expected, &solve_expected, &mut budget)
        .expect_err("same equations, swapped original inputs");
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(
        before,
        receiver.cas.canonical_snapshot(&mut default_budget()?)?
    );
    // Ordinary root replay accepts the same equations independently of the swapped subject.
    let mut budget = nrz_cas::limits::Budget::new(nrz_cas::limits::Limits {
        work: 20_000_000,
        replay_work: 20_000_000,
        ..nrz_cas::limits::Limits::default()
    })?;
    receiver.cas.replay(&bytes, &solve_expected, &mut budget)?;
    Ok(())
}

/// Ordinary derivatives accept matching complex scalar declarations; residual syntax remains Real.
/// ´test:integration:residual-complex-derivative-boundary´
#[test]
fn residual_complex_derivative_boundary() -> TestResult {
    use nrz_cas::Reference;
    use nrz_cas::syntax::{AtomicSort, Draft, ExpressionDraft, Field, Sort};
    let mut f = Fixture::with_complex(&["0", "0"], "0", false, false, true)?;
    let subject = f.compose()?;
    let mut handles = f.cas.admit(
        &[
            Draft::Expression(ExpressionDraft {
                header: Reference::Existing(f.header.clone()),
                space: None,
                scope: Vec::new(),
                binding: None,
                sort: Sort::Value(AtomicSort::Scalar(Field::Complex)),
                operator: Operator::Free("c".into()),
                operands: Vec::new(),
            }),
            Draft::Expression(ExpressionDraft {
                header: Reference::Existing(f.header.clone()),
                space: None,
                scope: Vec::new(),
                binding: None,
                sort: Sort::Value(AtomicSort::Scalar(Field::Complex)),
                operator: Operator::Derivative("c".into()),
                operands: vec![Reference::Local(0)],
            }),
        ],
        &mut default_budget()?,
    )?;
    let derivative = handles.pop().ok_or("complex derivative")?;
    assert_eq!(
        f.cas
            .expression(&derivative, &mut crate::testing::budgets::graph_budget()?)?
            .1,
        &Sort::Value(AtomicSort::Scalar(Field::Complex))
    );
    let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
    let fault = f
        .cas
        .check_evaluated_residual(
            &subject,
            &derivative,
            &f.y,
            &[],
            None,
            &mut default_budget()?,
        )
        .expect_err("Real residual grammar");
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::SeriesUnsupported)
    );
    assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);
    Ok(())
}

/// The nonzero centre is part of substitution, not an optional coordinate display.
/// ´test:integration:residual-centre-omission-control´
#[test]
fn residual_centre_omission_control() -> TestResult {
    use Term::{Q, X, Y};
    let mut f = Fixture::new(&["9", "6", "1", "0"], "2", false, false)?;
    let subject = f.source(
        &add(add(pow(X, 2), mul(Q("2"), X)), Q("1")),
        SourceClause::PolynomialSeed,
    )?;
    let term = sub(Y(1), add(mul(Q("2"), X), Q("2")));
    f.centre_value = rational("0")?;
    let (expression, wrong) = f.term(&term, 3)?;
    assert_eq!(
        wrong.last().ok_or("root")?.coefficients,
        [rational("4")?, rational("0")?, rational("0")?]
    );
    let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
    let fault = f
        .cas
        .check_evaluated_residual(
            &subject,
            &expression,
            &f.y,
            &wrong,
            None,
            &mut default_budget()?,
        )
        .expect_err("x must mean a+h");
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Rule));
    assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);
    Ok(())
}

/// Derived placeholders cannot be bypassed with existing handles or used as ordinary evidence.
/// ´test:integration:residual-placeholder-bindings´
#[test]
fn residual_placeholder_bindings() -> TestResult {
    use nrz_cas::series::ResidualReference as R;
    use nrz_cas::solving::{SolveCertificate, SolveStatement};
    let mut f = Fixture::new(&["0"], "0", false, false)?;
    let subject = f.compose()?;
    let (expression, rows) = f.term(&Term::Y(0), 1)?;
    for mutation in 0..5 {
        let mut supplied = member();
        match (&mut supplied.statement, &mut supplied.certificate) {
            (SolveStatement::Member(statement), SolveCertificate::Membership(certificate)) => {
                match mutation {
                    0 => statement.problem = R::Existing(f.x.clone()),
                    1 => statement.tuple = R::Problem,
                    2 => certificate.problem = R::Existing(f.x.clone()),
                    3 => certificate.region = R::Tuple,
                    _ => certificate.tuple = R::Existing(f.y.clone()),
                }
            }
            _ => return Err("Member fixture".into()),
        }
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        let fault = f
            .cas
            .check_evaluated_residual(
                &subject,
                &expression,
                &f.y,
                &rows,
                Some(&supplied),
                &mut default_budget()?,
            )
            .expect_err("derived binding");
        assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Rule));
        assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);
    }
    Ok(())
}
