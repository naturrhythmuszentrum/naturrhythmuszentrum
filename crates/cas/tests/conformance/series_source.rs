// SPDX-License-Identifier: AGPL-3.0-only
//! Original source identity and independent exact finite origin oracles.

use crate::testing::common::{TestResult, default_budget};
use crate::testing::series_proof::rational;
use crate::testing::series_source::{setup, statement};
use nrz_cas::{FaultKind, InvalidCode};

/// Supplied rows cannot hide false full polynomials or unavailable original-source capabilities.
/// ´test:integration:series-source-supplied-polynomial-capabilities´
#[test]
fn source_supplied_polynomial_capabilities() -> TestResult {
    use crate::testing::series_source::{admit_expected, assume_source};
    use nrz_cas::syntax::Operator;
    let mut observed = Vec::new();
    for kind in 0..4 {
        let mut f = setup(
            if kind == 3 { 0 } else { 3 },
            if kind == 0 { 4 } else { 1 },
            false,
            false,
        )?;
        match kind {
            1 => {
                f.request.source = f
                    .base
                    .term(Operator::IntegerPower(12), &[f.base.x.clone()])?;
                f.certificate.coefficients = vec![rational("4096")?];
            }
            2 => {
                let quotient = f
                    .base
                    .term(Operator::Div, &[f.base.one.clone(), f.base.one.clone()])?;
                f.request.source = f
                    .base
                    .term(Operator::Mul, &[f.base.zero.clone(), quotient])?;
                f.certificate.coefficients = vec![rational("0")?];
            }
            3 => {
                let shifted = f
                    .base
                    .term(Operator::Sub, &[f.base.x.clone(), f.base.one.clone()])?;
                f.request.source = f.base.term(Operator::Sine, &[shifted])?;
                f.request.centre = f.base.one.clone();
            }
            _ => {}
        }
        assume_source(&mut f)?;
        let positive = if kind < 2 {
            Some(
                f.base
                    .cas
                    .expand_series_source(&f.request, &f.proofs, &mut default_budget()?)?,
            )
        } else {
            None
        };
        let bytes = positive
            .as_ref()
            .map(|w| f.base.cas.export_witness(w, &mut default_budget()?))
            .transpose()?;
        if kind == 0 {
            f.certificate.coefficients[0] = rational("8")?;
        }
        admit_expected(&mut f)?;
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        let mut budget = default_budget()?;
        let fault = f
            .base
            .cas
            .check_series_source(
                statement(f.expected.as_ref().ok_or("expected")?)?,
                &f.certificate,
                &f.proofs,
                &mut budget,
            )
            .expect_err("supplied original source is independently checked");
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(if kind < 2 {
                InvalidCode::Rule
            } else {
                InvalidCode::SeriesUnsupported
            })
        );
        observed.push(budget.usage().work);
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
        if let (Some(positive), Some(bytes)) = (positive, bytes) {
            assert_eq!(
                f.base
                    .cas
                    .export_witness(&positive, &mut default_budget()?)?,
                bytes
            );
            f.base
                .cas
                .replay(&bytes, positive.judgment(), &mut default_budget()?)?;
        }
    }
    assert_eq!(observed, [37_397, 240_438, 4_652, 4_151]);
    Ok(())
}

/// Supported syntax with an exact ordinary domain still cannot supply an unregistered source bridge.
/// ´test:integration:series-source-capability-exclusions´
#[test]
fn source_capability_exclusions() -> TestResult {
    use nrz_cas::series::SourceClause;
    use nrz_cas::syntax::{Constant, Operator};
    for kind in 0..13 {
        let mut f = setup(if kind == 1 { 2 } else { 0 }, 1, false, false)?;
        let x = f.base.x.clone();
        let source = f.request.source.clone();
        match kind {
            0 | 1 => f.request.centre = f.base.one.clone(),
            2 => {
                f.request.centre = f.base.one.clone();
                let shifted = f.base.term(Operator::Sub, &[x, f.base.one.clone()])?;
                f.request.source = f.base.term(Operator::Sine, &[shifted])?;
            }
            3 => {
                let square = f.base.term(Operator::IntegerPower(2), &[x])?;
                f.request.source = f.base.term(Operator::Sine, &[square])?;
            }
            4 => {
                let cosine = f.base.term(Operator::Cosine, &[x])?;
                f.request.source = f.base.term(Operator::Div, &[source, cosine])?;
            }
            5 => f.request.source = f.base.term(Operator::Div, &[source, x])?,
            6 => f.request.centre = f.base.term(Operator::Constant(Constant::Pi), &[])?,
            7 | 8 => {
                let pi = f.base.term(Operator::Constant(Constant::Pi), &[])?;
                let divisor = f.base.number(if kind == 7 { "6" } else { "2" })?;
                f.request.centre = f.base.term(Operator::Div, &[pi, divisor])?;
                if kind == 8 {
                    f.request.clause = SourceClause::CosineOrigin;
                    f.request.source = f.base.term(Operator::Cosine, &[x])?;
                }
            }
            9..=12 => {
                f.request.clause = SourceClause::PolynomialSeed;
                let unsupported = f
                    .base
                    .term(Operator::Div, &[f.base.one.clone(), f.base.one.clone()])?;
                f.request.source = match kind {
                    9 => f
                        .base
                        .term(Operator::Mul, &[f.base.zero.clone(), unsupported])?,
                    10 => f
                        .base
                        .term(Operator::Sub, &[unsupported.clone(), unsupported])?,
                    11 => f.base.term(Operator::IntegerPower(0), &[unsupported])?,
                    _ => f.base.term(Operator::Constant(Constant::Pi), &[])?,
                };
            }
            _ => unreachable!(),
        }
        crate::testing::series_source::assume_source(&mut f)?;
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.base
                .cas
                .expand_series_source(&f.request, &f.proofs, &mut default_budget()?)
                .expect_err("unavailable bridge")
                .kind,
            FaultKind::Invalid(InvalidCode::SeriesUnsupported),
            "case {kind}"
        );
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
    }
    Ok(())
}

/// Full centred degree survives a one-coefficient request; strict exponent zero retains its premise.
/// ´test:integration:series-source-full-polynomial-degree´
#[test]
fn source_full_polynomial_degree() -> TestResult {
    use nrz_cas::limits::{Budget, BudgetKind};
    use nrz_cas::syntax::Operator;
    for exponent in [0, 12] {
        let mut f = setup(3, 1, false, false)?;
        f.request.source = f
            .base
            .term(Operator::IntegerPower(exponent), &[f.base.x.clone()])?;
        crate::testing::series_source::assume_source(&mut f)?;
        let positive =
            f.base
                .cas
                .expand_series_source(&f.request, &f.proofs, &mut default_budget()?)?;
        assert_eq!(positive.hypotheses().len(), 1);
        let source = statement(positive.judgment())?;
        let coefficients = f
            .base
            .cas
            .series_object(
                &source.output,
                &mut crate::testing::budgets::graph_budget()?,
            )?
            .coefficients
            .clone();
        let expected = f.base.number(if exponent == 0 { "1" } else { "4096" })?;
        assert_eq!(coefficients, [expected]);
        for ((dimension, low), expected_work) in [
            BudgetKind::JetEntries,
            BudgetKind::ComponentBits,
            BudgetKind::IntermediateBits,
        ]
        .into_iter()
        .zip(if exponent == 0 {
            [8, 2, 2]
        } else {
            [188, 17, 18]
        })
        .zip(if exponent == 0 {
            [[61_263, 19_001], [51_023, 3475], [51_023, 3475]]
        } else {
            [[557_313, 497_961], [363_513, 129_510], [363_513, 130_598]]
        }) {
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            let mut measurements = Vec::new();
            for (limit, success) in [(low, true), (low - 1, false)] {
                let mut limits = crate::testing::half_pi_limits::ceiling(dimension, limit);
                if dimension == BudgetKind::JetEntries {
                    limits.jet_entries = limit;
                }
                let mut b = Budget::new(limits)?;
                let at = nrz_cas::Location::new("complete centred source");
                b.work(37, at)?;
                let result = b.with_retained_payload(113, at, |b| {
                    f.base.cas.expand_series_source(&f.request, &f.proofs, b)
                });
                assert_eq!(result.is_ok(), success);
                if let Err(fault) = result {
                    assert!(
                        matches!(fault.kind,FaultKind::Exhausted{budget,..} if budget==dimension || (dimension==BudgetKind::IntermediateBits && budget==BudgetKind::ComponentBits))
                    );
                }
                measurements.push(b.usage().work);
                assert_eq!(
                    f.base.cas.canonical_snapshot(&mut default_budget()?)?,
                    before
                );
            }
            assert_eq!(measurements, expected_work, "{exponent} {dimension:?}");
        }
    }
    Ok(())
}

/// Factorial and hand rational oracles distinguish all three origin laws at bounded orders.
/// ´test:integration:series-source-origin-oracles´
#[test]
fn source_origin_oracles() -> TestResult {
    for kind in 0..4 {
        for order in [1, 4, 6] {
            let mut f = setup(kind, order, false, true)?;
            let expected = f.expected.as_ref().ok_or("expectation")?;
            let supplied = f.base.cas.check_series_source(
                statement(expected)?,
                &f.certificate,
                &f.proofs,
                &mut default_budget()?,
            )?;
            assert_eq!(supplied.judgment(), expected);
            let mut producer = setup(kind, order, false, false)?;
            let generated = producer.base.cas.expand_series_source(
                &producer.request,
                &producer.proofs,
                &mut default_budget()?,
            )?;
            let bytes = producer
                .base
                .cas
                .export_witness(&generated, &mut default_budget()?)?;
            for reordered in [false, true] {
                let mut receiver = crate::testing::series_source::receiver(kind, order, reordered)?;
                receiver.base.cas.replay(
                    &bytes,
                    receiver.expected.as_ref().ok_or("expectation")?,
                    &mut default_budget()?,
                )?;
            }
        }
    }
    Ok(())
}

/// Paired companion and inverse rows remain authoritative even when the selected output is unchanged.
/// ´test:integration:series-source-row-mutations´
#[test]
fn source_row_mutations() -> TestResult {
    for kind in 0..3 {
        for change in 0..10 {
            let mut f = setup(kind, 6, false, true)?;
            let expected = f.expected.as_ref().ok_or("expectation")?;
            let positive = f.base.cas.check_series_source(
                statement(expected)?,
                &f.certificate,
                &f.proofs,
                &mut default_budget()?,
            )?;
            let bytes = f
                .base
                .cas
                .export_witness(&positive, &mut default_budget()?)?;
            match change {
                0 => f.certificate.coefficients[0] = rational("1")?,
                1 => f.certificate.coefficients[3] = rational("1/6")?,
                2 => {
                    f.certificate.units.pop();
                }
                3 => f.certificate.units.push(f.certificate.units[0].clone()),
                4 => f.certificate.units.swap(0, 1),
                5 => f.certificate.units[0].index += 1,
                6 => f.certificate.units[0].integer = rational("9")?,
                7 => f.certificate.units[0].inverse = rational("9")?,
                8 if kind < 2 => f.certificate.companion[5] = rational("9")?,
                9 if kind < 2 => {
                    f.certificate.companion.pop();
                }
                _ => f.certificate.coefficients[5] = rational("2/15")?,
            }
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            assert_eq!(
                f.base
                    .cas
                    .check_series_source(
                        statement(expected)?,
                        &f.certificate,
                        &f.proofs,
                        &mut default_budget()?
                    )
                    .expect_err("false row")
                    .kind,
                FaultKind::Invalid(InvalidCode::Rule)
            );
            assert_eq!(
                f.base.cas.canonical_snapshot(&mut default_budget()?)?,
                before
            );
            assert_eq!(
                f.base
                    .cas
                    .export_witness(&positive, &mut default_budget()?)?,
                bytes
            );
        }
    }
    Ok(())
}

/// Extra and missing proof slots refuse before dynamic proof sizing or any output publication.
/// ´test:integration:series-source-typed-arity´
#[test]
fn source_typed_arity() -> TestResult {
    for kind in 0..4 {
        let mut f = setup(kind, 4, false, false)?;
        for proofs in [Vec::new(), vec![f.proofs[0].clone(), f.proofs[0].clone()]] {
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            assert_eq!(
                f.base
                    .cas
                    .expand_series_source(&f.request, &proofs, &mut default_budget()?)
                    .expect_err("arity")
                    .kind,
                FaultKind::Invalid(InvalidCode::Rule)
            );
            assert_eq!(
                f.base.cas.canonical_snapshot(&mut default_budget()?)?,
                before
            );
        }
    }
    Ok(())
}

/// Valid ordinary facts cannot replace the exact original source and domain premise.
/// ´test:integration:series-source-ordinary-slot-twins´
#[test]
fn source_ordinary_slot_twins() -> TestResult {
    use nrz_cas::Reference;
    use nrz_cas::domain::ContextDraft;
    use nrz_cas::syntax::{Draft, PredicateKind};
    for other_domain in [false, true] {
        let mut f = setup(0, 4, false, true)?;
        let expected = f.expected.as_ref().ok_or("expected")?;
        let original = f.base.cas.check_series_source(
            statement(expected)?,
            &f.certificate,
            &f.proofs,
            &mut default_budget()?,
        )?;
        if other_domain {
            let predicate = f
                .base
                .predicate(PredicateKind::Positive, &[f.base.x.clone()])?;
            f.base.domain = f.base.region(&predicate, true)?;
            f.base.context = f
                .base
                .cas
                .admit(
                    &[Draft::Context(ContextDraft::Root {
                        header: Reference::Existing(f.base.header.clone()),
                        admissible: Reference::Existing(f.base.domain.clone()),
                        assumptions: Vec::new(),
                    })],
                    &mut default_budget()?,
                )?
                .pop()
                .ok_or("context")?;
        }
        let x = f.base.x.clone();
        let mut ordinary = if other_domain {
            let request = f.base.request(
                nrz_cas::request::Task::Coverage {
                    domain: f.base.domain.clone(),
                },
                &[x],
            )?;
            f.base.proved(&request)?.witness
        } else {
            let fact = crate::testing::series_proof::defined(&mut f.base, &x)?;
            f.base
                .cas
                .check_proof(&f.base.context, &[fact], &mut default_budget()?)?
        };
        if other_domain {
            ordinary = f.base.cas.trig_defined(
                &f.request.source,
                ordinary.proof(),
                &mut default_budget()?,
            )?;
        }
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.base
                .cas
                .check_series_source(
                    statement(expected)?,
                    &f.certificate,
                    &[ordinary.proof().clone()],
                    &mut default_budget()?
                )
                .expect_err("exact ordinary slot")
                .kind,
            FaultKind::Invalid(InvalidCode::Rule)
        );
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
        f.base
            .cas
            .export_witness(&original, &mut default_budget()?)?;
    }
    Ok(())
}

/// Source candidates use their own typed aggregate path even inside an ordinary supplied proof.
/// ´test:integration:series-source-supplied-steps´
#[test]
fn source_supplied_steps() -> TestResult {
    use nrz_cas::proof::{ApplicationDraft, Certificate, Primitive, Statement};
    use nrz_cas::series::SeriesCertificate;
    for kind in 0..4 {
        let mut f = if kind == 3 {
            crate::testing::series_source::linear_seed(false)?
        } else {
            setup(kind, 4, false, true)?
        };
        let x = f.base.x.clone();
        let mut steps = vec![crate::testing::series_proof::defined(&mut f.base, &x)?];
        if kind != 3 {
            let predicate = f.base.predicate(
                nrz_cas::syntax::PredicateKind::Defined,
                &[f.request.source.clone()],
            )?;
            steps.push(ApplicationDraft {
                rule: Primitive::TrigDefined,
                context: f.base.context.clone(),
                conclusion: Statement::PredicateFact {
                    region: f.base.domain.clone(),
                    predicate,
                },
                premises: vec![0],
                substitutions: Vec::new(),
                certificate: Certificate::None,
            });
        }
        let expected = f.expected.as_ref().ok_or("expected")?;
        steps.push(ApplicationDraft {
            rule: f.request.clause.primitive(),
            context: f.base.context.clone(),
            conclusion: expected.statement.clone(),
            premises: vec![u32::try_from(steps.len() - 1)?],
            substitutions: Vec::new(),
            certificate: Certificate::Series(Box::new(SeriesCertificate::Source(
                f.certificate.clone(),
            ))),
        });
        let checked = f
            .base
            .cas
            .check_proof(&f.base.context, &steps, &mut default_budget()?)?;
        assert_eq!(checked.judgment(), expected);
        for extra in [false, true] {
            let mut bad = steps.clone();
            let root = bad.last_mut().ok_or("root")?;
            if extra {
                root.premises.push(0);
            } else {
                root.premises.clear();
            }
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            assert_eq!(
                f.base
                    .cas
                    .check_proof(&f.base.context, &bad, &mut default_budget()?)
                    .expect_err("source arity")
                    .kind,
                FaultKind::Invalid(InvalidCode::Rule)
            );
            assert_eq!(
                f.base.cas.canonical_snapshot(&mut default_budget()?)?,
                before
            );
        }
    }
    Ok(())
}

/// Equal-spelled declared functions are valid syntax but cannot inherit a closed origin law.
/// ´test:integration:series-source-opaque-call-refused´
#[test]
fn source_opaque_call_refused() -> TestResult {
    use nrz_cas::Reference;
    use nrz_cas::domain::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
    use nrz_cas::series::{SourceCertificate, SourceClause, SourceRequest};
    use nrz_cas::syntax::{Draft, Operator};
    for (name, clause) in [
        ("sine", SourceClause::SineOrigin),
        ("cosine", SourceClause::CosineOrigin),
        ("arctangent", SourceClause::ArctangentOrigin),
    ] {
        let mut t = crate::testing::trig::Fixture::new()?;
        let source = t.term(Operator::Call(name.into()), &[t.x.clone()])?;
        let zero = t.term(Operator::Rational(rational("0")?), &[])?;
        let one = t.term(Operator::Rational(rational("1")?), &[])?;
        let nodes = t.cas.admit(
            &[
                Draft::Region(RegionDraft {
                    header: Reference::Existing(t.header.clone()),
                    scope: Vec::new(),
                    ambient: AmbientDomain::Real("x".into()),
                    expression: RegionExpr::Ambient,
                }),
                Draft::Context(ContextDraft::Root {
                    header: Reference::Existing(t.header.clone()),
                    admissible: Reference::Local(0),
                    assumptions: Vec::new(),
                }),
                Draft::Solve(nrz_cas::solving::SolveDraft::CoefficientDomain(
                    nrz_cas::solving::CoefficientDomainData::Rational,
                )),
            ],
            &mut default_budget()?,
        )?;
        let request = SourceRequest {
            clause,
            source: source.clone(),
            domain: nodes[0].clone(),
            field: nodes[2].clone(),
            centre: zero.clone(),
            variable: t.x.clone(),
            scope: Vec::new(),
            context: nodes[1].clone(),
            order: 1,
        };
        let base = crate::testing::cases::Fixture {
            cas: t.cas,
            header: t.header,
            context: nodes[1].clone(),
            domain: nodes[0].clone(),
            source,
            x: t.x,
            zero,
            one,
        };
        let mut f = crate::testing::series_source::Fixture {
            base,
            request,
            certificate: SourceCertificate {
                clause,
                coefficients: Vec::new(),
                companion: Vec::new(),
                units: Vec::new(),
            },
            proofs: Vec::new(),
            expected: None,
        };
        crate::testing::series_source::assume_source(&mut f)?;
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.base
                .cas
                .expand_series_source(&f.request, &f.proofs, &mut default_budget()?)
                .expect_err("opaque source")
                .kind,
            FaultKind::Invalid(InvalidCode::SeriesUnsupported)
        );
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
    }
    Ok(())
}

/// Full source metadata refuses implicit scope, context, field and original-domain transport.
/// ´test:integration:series-source-metadata-twins´
#[test]
fn source_metadata_twins() -> TestResult {
    use nrz_cas::Reference;
    use nrz_cas::domain::{AssumptionDraft, ContextDraft};
    use nrz_cas::syntax::{AtomicSort, Draft, Field, PredicateKind};
    for kind in 0..6 {
        let mut f = setup(0, 4, false, false)?;
        let code = match kind {
            0 => {
                f.request.centre = f.base.one.clone();
                InvalidCode::SeriesUnsupported
            }
            1 => {
                f.request.scope = vec![vec![AtomicSort::Scalar(Field::Real)]];
                InvalidCode::Scope
            }
            2 => {
                f.request.variable = f.base.one.clone();
                InvalidCode::SeriesUnsupported
            }
            3 => {
                f.request.field = f
                    .base
                    .cas
                    .admit(
                        &[Draft::Solve(
                            nrz_cas::solving::SolveDraft::CoefficientDomain(
                                nrz_cas::solving::CoefficientDomainData::Gaussian,
                            ),
                        )],
                        &mut default_budget()?,
                    )?
                    .pop()
                    .ok_or("field")?;
                InvalidCode::SeriesUnsupported
            }
            4 => {
                let predicate = f
                    .base
                    .predicate(PredicateKind::Positive, &[f.base.x.clone()])?;
                f.request.domain = f.base.region(&predicate, true)?;
                InvalidCode::Rule
            }
            _ => {
                let predicate = f
                    .base
                    .predicate(PredicateKind::Positive, &[f.base.x.clone()])?;
                f.request.context = f
                    .base
                    .cas
                    .admit(
                        &[Draft::Context(ContextDraft::Root {
                            header: Reference::Existing(f.base.header.clone()),
                            admissible: Reference::Existing(f.base.domain.clone()),
                            assumptions: vec![AssumptionDraft {
                                key: "positive-coordinate".into(),
                                condition: Reference::Existing(predicate),
                            }],
                        })],
                        &mut default_budget()?,
                    )?
                    .pop()
                    .ok_or("context")?;
                InvalidCode::Rule
            }
        };
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.base
                .cas
                .expand_series_source(&f.request, &f.proofs, &mut default_budget()?)
                .expect_err("metadata twin")
                .kind,
            FaultKind::Invalid(code),
            "case {kind}"
        );
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
    }
    Ok(())
}

/// Valid source sine/cosine, a rational unit and original quotient definedness do not assemble tangent.
/// ´test:integration:series-source-quotient-assembly-refused´
#[test]
fn source_quotient_assembly_refused() -> TestResult {
    let mut f = crate::testing::series_source::lifted(8, false)?;
    assert_eq!(f.proofs.len(), 4);
    let one = crate::testing::series_proof::rational("1")?;
    assert_eq!(one.mul(&one, &mut default_budget()?)?, one);
    let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
    assert_eq!(
        f.base
            .cas
            .expand_series_source(&f.request, &f.proofs, &mut default_budget()?)
            .expect_err("object product is not original quotient")
            .kind,
        FaultKind::Invalid(InvalidCode::Rule)
    );
    assert_eq!(
        f.base.cas.canonical_snapshot(&mut default_budget()?)?,
        before
    );
    f.request.clause = nrz_cas::series::SourceClause::PolynomialSeed;
    assert_eq!(
        f.base
            .cas
            .expand_series_source(&f.request, &[f.proofs[3].clone()], &mut default_budget()?)
            .expect_err("quotient has no source bridge")
            .kind,
        FaultKind::Invalid(InvalidCode::SeriesUnsupported)
    );
    assert_eq!(
        f.base.cas.canonical_snapshot(&mut default_budget()?)?,
        before
    );
    Ok(())
}

/// Every admitted lifted clause preserves original syntax, mixed premises and conditional domains.
/// ´test:integration:series-source-lifted-original-operators´
#[test]
fn source_lifted_original_operators() -> TestResult {
    for kind in 0..8 {
        let mut f = crate::testing::series_source::lifted(kind, false)?;
        let expected = f.expected.as_ref().ok_or("expected")?;
        let checked = f.base.cas.check_series_source(
            statement(expected)?,
            &f.certificate,
            &f.proofs,
            &mut default_budget()?,
        )?;
        assert_eq!(checked.hypotheses().len(), usize::from(kind != 0));
        let bytes = f
            .base
            .cas
            .export_witness(&checked, &mut default_budget()?)?;
        let produced =
            f.base
                .cas
                .expand_series_source(&f.request, &f.proofs, &mut default_budget()?)?;
        assert_eq!(produced.judgment(), expected);
        assert_eq!(produced.hypotheses(), checked.hypotheses());
        for reordered in [false, true] {
            let mut receiver = crate::testing::series_source::receiver(kind + 4, 4, reordered)?;
            let replayed = receiver.base.cas.replay(
                &bytes,
                receiver.expected.as_ref().ok_or("expected")?,
                &mut default_budget()?,
            )?;
            assert_eq!(replayed.hypotheses().len(), usize::from(kind != 0));
        }
        for change in 0..5 {
            let mut proofs = f.proofs.clone();
            match change {
                0 => {
                    proofs.pop();
                }
                1 => proofs.push(proofs[0].clone()),
                2 => proofs.swap(0, 1),
                4 => {
                    let input = statement(expected)?.output.clone();
                    let wrong = f.base.cas.compute_series(
                        &nrz_cas::series::SeriesOperation::Truncate { input, order: 1 },
                        &[],
                        &mut default_budget()?,
                    )?;
                    let slot = match kind {
                        0 | 3..=5 => 1,
                        2 => 3,
                        _ => 2,
                    };
                    proofs[slot] = wrong.proof().clone();
                }
                _ => proofs[0] = proofs[proofs.len() - 1].clone(),
            }
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            if kind == 7 && change == 2 {
                assert_eq!(proofs, f.proofs);
                f.base.cas.check_series_source(
                    statement(expected)?,
                    &f.certificate,
                    &proofs,
                    &mut default_budget()?,
                )?;
                continue;
            }
            assert_eq!(
                f.base
                    .cas
                    .check_series_source(
                        statement(expected)?,
                        &f.certificate,
                        &proofs,
                        &mut default_budget()?
                    )
                    .expect_err("exact typed source slots")
                    .kind,
                FaultKind::Invalid(InvalidCode::Rule)
            );
            assert_eq!(
                f.base.cas.canonical_snapshot(&mut default_budget()?)?,
                before
            );
        }
    }
    Ok(())
}
