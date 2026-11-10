// SPDX-License-Identifier: AGPL-3.0-only
//! Independent finite inverses and complete public-operation mutation controls.

use crate::testing::common::{TestResult, default_budget};
use crate::testing::series_proof::{fixture, object, rational};
use nrz_cas::proof::{ApplicationDraft, Certificate, Primitive, Statement};
use nrz_cas::series::{
    SeriesCertificate as C, SeriesOperation as O, SeriesProductRow, SeriesStatement as S,
};
use nrz_cas::{FaultKind, InvalidCode};

use crate::testing::series_residual::{Fixture as ResidualFixture, Term, admit, binary, member};
use nrz_cas::domain::{AssumptionDraft, ContextDraft, HypothesisRef};
use nrz_cas::proof::{Judgment, VerifiedWitness};
use nrz_cas::series::{ResidualSubject, SourceClause, SourceRequest, SourceStatement};
use nrz_cas::syntax::{Draft, Operator, PredicateKind};
use nrz_cas::{Handle, Reference};

/// Original syntax, explicit guards, independent tangent coefficients and optional reciprocal slots.
struct QuotientFixture {
    /// Owning graph and independent coefficient frame.
    frame: ResidualFixture,
    /// Original sine, cosine, reciprocal and quotient expressions.
    sources: Vec<Handle>,
    /// Ordinary guards in their explicit hypothesis order.
    predicates: Vec<Handle>,
    /// Independently constructed quotient statement.
    expected: SourceStatement,
    /// Exact source reciprocal and its four ordered proof slots.
    reciprocal: Option<(SourceStatement, Vec<Handle>)>,
}

/// Independently admit the original quotient, its guards and the hand tangent vector.
fn quotient_fixture(reversed: bool) -> TestResult<QuotientFixture> {
    quotient_frame(reversed, false)
}

/// Choose distinct numerator/denominator sources or genuine repeated source handles.
fn quotient_frame(reversed: bool, equal: bool) -> TestResult<QuotientFixture> {
    quotient_named_frame(reversed, equal, false)
}

/// Build the complete original domain and independent coefficient expectation before checking proofs.
fn quotient_named_frame(reversed: bool, equal: bool, renamed: bool) -> TestResult<QuotientFixture> {
    let mut frame = ResidualFixture::new(
        if equal {
            &["1", "0", "0", "0", "0", "0"]
        } else {
            &["0", "1", "0", "1/3", "0", "2/15"]
        },
        "0",
        reversed,
        false,
    )?;
    if renamed {
        frame = ResidualFixture::renamed(
            if equal {
                &["1", "0", "0", "0", "0", "0"]
            } else {
                &["0", "1", "0", "1/3", "0", "2/15"]
            },
            reversed,
        )?;
    }
    let x = frame.x.clone();
    let sine = frame.node(Operator::Sine, std::slice::from_ref(&x))?;
    let cosine = frame.node(Operator::Cosine, std::slice::from_ref(&x))?;
    let sine = if equal { cosine.clone() } else { sine };
    let one = frame.node(Operator::Rational(rational("1")?), &[])?;
    let secant = frame.node(Operator::Div, &[one, cosine.clone()])?;
    let tangent = frame.node(Operator::Div, &[sine.clone(), cosine.clone()])?;
    let sources = vec![sine, cosine.clone(), secant, tangent.clone()];
    let mut predicates = Vec::new();
    for (index, source) in sources.iter().chain(std::iter::once(&cosine)).enumerate() {
        predicates.push(admit(
            &mut frame.cas,
            Draft::Condition {
                header: Reference::Existing(frame.header.clone()),
                scope: Vec::new(),
                kind: if index == 4 {
                    PredicateKind::NonZero
                } else {
                    PredicateKind::Defined
                },
                arguments: vec![Reference::Existing(source.clone())],
            },
        )?);
    }
    frame.context = admit(
        &mut frame.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(frame.header.clone()),
            admissible: Reference::Existing(frame.region.clone()),
            assumptions: predicates
                .iter()
                .enumerate()
                .map(|(i, p)| AssumptionDraft {
                    key: format!("quotient-guard-{i}"),
                    condition: Reference::Existing(p.clone()),
                })
                .collect(),
        }),
    )?;
    let candidate = frame.candidate.clone();
    let output = frame.object(&candidate)?;
    let expected = SourceStatement {
        clause: SourceClause::Quotient,
        source: tangent,
        domain: frame.region.clone(),
        output,
    };
    Ok(QuotientFixture {
        frame,
        sources,
        predicates,
        expected,
        reciprocal: None,
    })
}

/// Retain all source interpretation axes and the independently selected positive order.
fn request(f: &QuotientFixture, clause: SourceClause, source: &Handle) -> SourceRequest {
    SourceRequest {
        clause,
        source: source.clone(),
        domain: f.frame.region.clone(),
        field: f.frame.field.clone(),
        centre: f.frame.centre.clone(),
        variable: f.frame.x.clone(),
        scope: Vec::new(),
        context: f.frame.context.clone(),
        order: u32::try_from(f.frame.candidate.len()).unwrap_or(u32::MAX),
    }
}

/// Extract only the typed source output from checked evidence.
fn source_output(witness: &VerifiedWitness) -> TestResult<Handle> {
    let Statement::Series(s) = &witness.judgment().statement else {
        return Err("series".into());
    };
    let S::Source(s) = s.as_ref() else {
        return Err("source".into());
    };
    Ok(s.output.clone())
}

/// Assemble exact ordered premises through public checking, publication and sealing.
fn quotient_recipe(f: &mut QuotientFixture) -> TestResult<(VerifiedWitness, Vec<Handle>)> {
    quotient_recipe_with_budget(f, &mut quotient_budget()?)
}

/// Run every public step of the quotient recipe on one continuing caller ledger.
#[expect(
    clippy::too_many_lines,
    reason = "The complete fixture and exact boundary checks remain adjacent"
)]
fn quotient_recipe_with_budget(
    f: &mut QuotientFixture,
    budget: &mut nrz_cas::limits::Budget,
) -> TestResult<(VerifiedWitness, Vec<Handle>)> {
    // Each public call owns its rollback; completed prerequisites remain committed.
    macro_rules! checked_step {
        ($call:expr) => {{
            let before = f.frame.cas.canonical_snapshot(&mut quotient_budget()?)?;
            let result = $call;
            if result.is_err() {
                assert_eq!(
                    f.frame.cas.canonical_snapshot(&mut quotient_budget()?)?,
                    before
                );
            }
            result?
        }};
    }
    let mut guards = Vec::new();
    for (slot, predicate) in f.predicates.iter().enumerate() {
        let guard = ApplicationDraft {
            rule: Primitive::ContextHypothesis,
            context: f.frame.context.clone(),
            conclusion: Statement::PredicateFact {
                region: f.frame.region.clone(),
                predicate: predicate.clone(),
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::Hypothesis(HypothesisRef {
                context: f.frame.context.clone(),
                slot: u32::try_from(slot)?,
            }),
        };
        guards.push(
            checked_step!(f.frame.cas.check_proof(&f.frame.context, &[guard], budget))
                .proof()
                .clone(),
        );
    }
    let same = f.sources[0] == f.sources[1];
    let sine_request = request(
        f,
        if same {
            SourceClause::CosineOrigin
        } else {
            SourceClause::SineOrigin
        },
        &f.sources[0],
    );
    let cosine_request = request(f, SourceClause::CosineOrigin, &f.sources[1]);
    let sine = checked_step!(
        f.frame
            .cas
            .expand_series_source(&sine_request, &guards[..1], budget)
    );
    let cosine = if same {
        sine.clone()
    } else {
        checked_step!(
            f.frame
                .cas
                .expand_series_source(&cosine_request, &guards[1..2], budget)
        )
    };
    let inverse = checked_step!(f.frame.cas.compute_series(
        &O::Reciprocal {
            input: source_output(&cosine)?,
        },
        &[],
        budget,
    ));
    let reciprocal_request = request(f, SourceClause::Reciprocal, &f.sources[2]);
    let reciprocal = checked_step!(f.frame.cas.expand_series_source(
        &reciprocal_request,
        &[
            cosine.proof().clone(),
            inverse.proof().clone(),
            guards[4].clone(),
            guards[2].clone(),
        ],
        budget,
    ));
    let product = checked_step!(f.frame.cas.compute_series(
        &O::Mul {
            left: source_output(&sine)?,
            right: source_output(&reciprocal)?,
            order: None,
        },
        &[],
        budget,
    ));
    f.reciprocal = Some((
        SourceStatement {
            clause: SourceClause::Reciprocal,
            source: f.sources[2].clone(),
            domain: f.frame.region.clone(),
            output: source_output(&reciprocal)?,
        },
        vec![
            cosine.proof().clone(),
            inverse.proof().clone(),
            guards[4].clone(),
            guards[2].clone(),
        ],
    ));
    let slots = vec![
        sine.proof().clone(),
        cosine.proof().clone(),
        reciprocal.proof().clone(),
        product.proof().clone(),
        guards[4].clone(),
        guards[3].clone(),
    ];
    let quotient_request = request(f, SourceClause::Quotient, &f.sources[3]);
    let quotient = checked_step!(f.frame.cas.expand_series_source(
        &quotient_request,
        &slots,
        budget
    ));
    assert_eq!(
        quotient.judgment().statement,
        Statement::Series(Box::new(S::Source(f.expected.clone())))
    );
    assert_eq!(quotient.hypotheses().len(), if same { 4 } else { 5 });
    Ok((quotient, slots))
}

/// Exact source quotient survives fresh replay and rejects missing, reordered and wrong typed slots.
/// ´test:integration:series-reciprocal-quotient-source´
#[test]
fn series_reciprocal_quotient_source() -> TestResult {
    let mut f = quotient_fixture(false)?;
    let (quotient, slots) = quotient_recipe(&mut f)?;
    let bytes = f
        .frame
        .cas
        .export_witness(&quotient, &mut quotient_budget()?)?;
    for reversed in [false, true] {
        let mut receiver = quotient_fixture(reversed)?;
        let expected = Judgment {
            context: receiver.frame.context.clone(),
            layer: nrz_cas::domain::Layer::Logic,
            space: None,
            statement: Statement::Series(Box::new(S::Source(receiver.expected.clone()))),
            residuals: Vec::new(),
        };
        let imported = receiver
            .frame
            .cas
            .replay(&bytes, &expected, &mut quotient_budget()?)?;
        assert_eq!(imported.hypotheses().len(), 5);
    }
    for change in 0..4 {
        let mut bad = slots.clone();
        match change {
            0 => {
                bad.remove(4);
            }
            1 => bad.swap(0, 1),
            2 => bad[2] = bad[0].clone(),
            _ => bad[4] = bad[5].clone(),
        }
        let before = f.frame.cas.canonical_snapshot(&mut quotient_budget()?)?;
        let request = request(&f, SourceClause::Quotient, &f.sources[3]);
        assert_eq!(
            f.frame
                .cas
                .expand_series_source(&request, &bad, &mut quotient_budget()?)
                .expect_err("bad source slot")
                .kind,
            FaultKind::Invalid(InvalidCode::Rule)
        );
        assert_eq!(
            f.frame.cas.canonical_snapshot(&mut quotient_budget()?)?,
            before
        );
    }
    Ok(())
}

/// Independent wire oracle uses a receiver built from original syntax and literal coefficients.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Independent integration oracles share this internal fixture across test modules"
)]
pub(crate) fn quotient_payload_control(bytes: &[u8], renamed: bool) -> TestResult {
    for reversed in [false, true] {
        let mut f = quotient_named_frame(reversed, false, renamed)?;
        let expected = Judgment {
            context: f.frame.context.clone(),
            layer: nrz_cas::domain::Layer::Logic,
            space: None,
            statement: Statement::Series(Box::new(S::Source(f.expected.clone()))),
            residuals: Vec::new(),
        };
        let imported = f
            .frame
            .cas
            .replay(bytes, &expected, &mut quotient_budget()?)?;
        assert_eq!(imported.hypotheses().len(), 5);
        let (produced, _) = quotient_recipe(&mut f)?;
        assert_eq!(
            f.frame
                .cas
                .export_witness(&produced, &mut quotient_budget()?)?,
            bytes
        );
    }
    Ok(())
}

/// Paired wire expectations are constructed from original syntax and five explicit zero coefficients.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Independent integration oracles share this internal fixture across test modules"
)]
pub(crate) fn paired_payload_control(bytes: &[u8]) -> TestResult {
    let term = binary(
        Operator::Sub,
        binary(Operator::Sub, Term::Y(1), Term::Q("1")),
        Term::Power(Box::new(Term::Y(0)), 2),
    );
    for reversed in [false, true] {
        let mut f = quotient_fixture(reversed)?;
        let (mut residual, solve) = f.frame.expectations(&term, &vec![rational("0")?; 5], 1)?;
        let Statement::Series(value) = &mut residual.statement else {
            return Err("residual".into());
        };
        let S::EvaluatedResidual { subject, .. } = value.as_mut() else {
            return Err("residual".into());
        };
        *subject = ResidualSubject::Source(f.expected.clone());
        let alternate = binary(Operator::Add, term.clone(), Term::Q("0"));
        let (mut wrong, same_solve) =
            f.frame
                .expectations(&alternate, &vec![rational("0")?; 5], 1)?;
        assert_eq!(same_solve, solve);
        let Statement::Series(value) = &mut wrong.statement else {
            return Err("alternate residual".into());
        };
        let S::EvaluatedResidual { subject, .. } = value.as_mut() else {
            return Err("alternate residual".into());
        };
        *subject = ResidualSubject::Source(f.expected.clone());
        let before = f.frame.cas.canonical_snapshot(&mut quotient_budget()?)?;
        let mut retry_budget = quotient_budget()?;
        assert_eq!(
            f.frame
                .cas
                .replay_evaluated_residual(bytes, &wrong, &solve, &mut retry_budget)
                .expect_err("second independent expectation")
                .kind,
            FaultKind::Invalid(InvalidCode::EndpointMismatch)
        );
        assert_eq!(
            f.frame.cas.canonical_snapshot(&mut quotient_budget()?)?,
            before
        );
        let spent = retry_budget.usage().work;
        let (_, imported) =
            f.frame
                .cas
                .replay_evaluated_residual(bytes, &residual, &solve, &mut retry_budget)?;
        assert!(retry_budget.usage().work > spent);
        assert_eq!(imported.hypotheses().len(), 5);
        let (subject, _) = quotient_recipe(&mut f)?;
        let (expression, rows) = f.frame.term(&term, 5)?;
        let (_, solve) = f.frame.cas.check_evaluated_residual(
            &subject,
            &expression,
            &f.frame.y,
            &rows,
            Some(&member()),
            &mut quotient_budget()?,
        )?;
        assert_eq!(
            f.frame
                .cas
                .export_witness(&solve.ok_or("Member")?, &mut quotient_budget()?)?,
            bytes
        );
    }
    Ok(())
}

/// Original tangent quotient supplies the residual subject; genuine atan gives the nonzero control.
/// ´test:integration:series-reciprocal-tangent-residual´
#[test]
fn series_reciprocal_tangent_residual() -> TestResult {
    let term = binary(
        Operator::Sub,
        binary(Operator::Sub, Term::Y(1), Term::Q("1")),
        Term::Power(Box::new(Term::Y(0)), 2),
    );
    for reversed in [false, true] {
        let mut f = quotient_fixture(reversed)?;
        let (subject, _) = quotient_recipe(&mut f)?;
        let (expression, rows) = f.frame.term(&term, 5)?;
        assert_eq!(
            rows.last().ok_or("root")?.coefficients,
            vec![rational("0")?; 5]
        );
        let (_, solved) = f.frame.cas.check_evaluated_residual(
            &subject,
            &expression,
            &f.frame.y,
            &rows,
            Some(&member()),
            &mut quotient_budget()?,
        )?;
        let solved = solved.ok_or("Member")?;
        let bytes = f
            .frame
            .cas
            .export_witness(&solved, &mut quotient_budget()?)?;
        let mut receiver = quotient_fixture(!reversed)?;
        let (mut residual, solve) =
            receiver
                .frame
                .expectations(&term, &vec![rational("0")?; 5], 1)?;
        let Statement::Series(statement) = &mut residual.statement else {
            return Err("residual".into());
        };
        let S::EvaluatedResidual { subject, .. } = statement.as_mut() else {
            return Err("residual".into());
        };
        *subject = ResidualSubject::Source(receiver.expected.clone());
        receiver.frame.cas.replay_evaluated_residual(
            &bytes,
            &residual,
            &solve,
            &mut quotient_budget()?,
        )?;
        let mut atan =
            ResidualFixture::new(&["0", "1", "0", "-1/3", "0", "1/5"], "0", reversed, false)?;
        let subject = atan.source(&Term::X, SourceClause::ArctangentOrigin)?;
        let (expression, rows) = atan.term(&term, 5)?;
        let expected = ["0", "0", "-2", "0", "5/3"]
            .iter()
            .map(|v| rational(v))
            .collect::<TestResult<Vec<_>>>()?;
        assert_eq!(rows.last().ok_or("atan residual")?.coefficients, expected);
        atan.cas.check_evaluated_residual(
            &subject,
            &expression,
            &atan.y,
            &rows,
            None,
            &mut quotient_budget()?,
        )?;
        let before = atan.cas.canonical_snapshot(&mut quotient_budget()?)?;
        assert_eq!(
            atan.cas
                .check_evaluated_residual(
                    &subject,
                    &expression,
                    &atan.y,
                    &rows,
                    Some(&member()),
                    &mut quotient_budget()?
                )
                .expect_err("nonzero residual")
                .kind,
            FaultKind::Invalid(InvalidCode::Rule)
        );
        assert_eq!(
            atan.cas.canonical_snapshot(&mut quotient_budget()?)?,
            before
        );
    }
    Ok(())
}

/// Leave independent headroom for complete mixed source and residual replay.
fn quotient_budget() -> Result<nrz_cas::limits::Budget, nrz_cas::Fault> {
    nrz_cas::limits::Budget::new(nrz_cas::limits::Limits {
        work: 20_000_000,
        replay_work: 20_000_000,
        ..nrz_cas::limits::Limits::default()
    })
}

/// The fifth coefficient contributes exactly five times its perturbation at residual degree four.
/// ´test:integration:series-reciprocal-residual-sensitivity´
#[test]
fn series_reciprocal_residual_sensitivity() -> TestResult {
    let term = binary(
        Operator::Sub,
        binary(Operator::Sub, Term::Y(1), Term::Q("1")),
        Term::Power(Box::new(Term::Y(0)), 2),
    );
    for (last, residual) in [("7/30", "1/2"), ("1/30", "-1/2")] {
        let mut f = ResidualFixture::new(&["0", "1", "0", "1/3", "0", last], "0", false, false)?;
        let subject = f.compose()?;
        let (expression, rows) = f.term(&term, 5)?;
        let expected = ["0", "0", "0", "0", residual]
            .iter()
            .map(|v| rational(v))
            .collect::<TestResult<Vec<_>>>()?;
        assert_eq!(rows.last().ok_or("root")?.coefficients, expected);
        f.cas.check_evaluated_residual(
            &subject,
            &expression,
            &f.y,
            &rows,
            None,
            &mut quotient_budget()?,
        )?;
        assert_eq!(
            f.cas
                .check_evaluated_residual(
                    &subject,
                    &expression,
                    &f.y,
                    &rows,
                    Some(&member()),
                    &mut quotient_budget()?
                )
                .expect_err("perturbed residual")
                .kind,
            FaultKind::Invalid(InvalidCode::Rule)
        );
    }
    Ok(())
}

/// Run one complete source or residual client with separately fixed setup.
///
/// # Panics
/// Panics only if the local fixture omits its paired evidence or returns an untyped recipe error,
/// which supplied proof inputs cannot arrange.
#[expect(
    clippy::too_many_lines,
    reason = "The complete fixture and exact boundary checks remain adjacent"
)]
fn quotient_resource_attempt(
    mode: usize,
    reversed: bool,
    equal: bool,
    kind: nrz_cas::limits::BudgetKind,
    amount: u64,
) -> TestResult<(bool, u64, Option<nrz_cas::Fault>)> {
    use nrz_cas::limits::{Budget, BudgetKind};
    let mut f = quotient_frame(reversed, equal)?;
    let (original, slots) = quotient_recipe(&mut f)?;
    let original_bytes = f
        .frame
        .cas
        .export_witness(&original, &mut quotient_budget()?)?;
    let mut receiver = quotient_frame(!reversed, equal)?;
    let expected = Judgment {
        context: receiver.frame.context.clone(),
        layer: nrz_cas::domain::Layer::Logic,
        space: None,
        statement: Statement::Series(Box::new(S::Source(receiver.expected.clone()))),
        residuals: Vec::new(),
    };
    let term = binary(
        Operator::Sub,
        binary(Operator::Sub, Term::Y(1), Term::Q("1")),
        Term::Power(Box::new(Term::Y(0)), 2),
    );
    let (expression, rows) = if matches!(mode, 4 | 5) {
        f.frame.term(&term, 5)?
    } else {
        (f.frame.y.clone(), Vec::new())
    };
    let mut bytes = original_bytes.clone();
    let paired = if mode == 5 {
        let (_, solved) = f.frame.cas.check_evaluated_residual(
            &original,
            &expression,
            &f.frame.y,
            &rows,
            Some(&member()),
            &mut quotient_budget()?,
        )?;
        bytes = f
            .frame
            .cas
            .export_witness(&solved.ok_or("Member")?, &mut quotient_budget()?)?;
        let (mut residual, solve) =
            receiver
                .frame
                .expectations(&term, &vec![rational("0")?; 5], 1)?;
        let Statement::Series(value) = &mut residual.statement else {
            return Err("residual".into());
        };
        let S::EvaluatedResidual { subject, .. } = value.as_mut() else {
            return Err("residual".into());
        };
        *subject = ResidualSubject::Source(receiver.expected.clone());
        Some((residual, solve))
    } else {
        None
    };
    let imported = matches!(mode, 2 | 3 | 5);
    let before = if imported {
        receiver
            .frame
            .cas
            .canonical_snapshot(&mut quotient_budget()?)?
    } else {
        f.frame.cas.canonical_snapshot(&mut quotient_budget()?)?
    };
    let mut limits = crate::testing::half_pi_limits::ceiling(kind, amount);
    if kind != BudgetKind::Work {
        limits.work = 20_000_000;
    }
    if kind != BudgetKind::ReplayWork {
        limits.replay_work = 20_000_000;
    }
    if kind == BudgetKind::JetEntries {
        limits.jet_entries = amount;
    }
    if kind == BudgetKind::JetOrder {
        limits.jet_order = amount;
    }
    if kind == BudgetKind::SolveIdentities {
        limits.solve_identities = amount;
    }
    if kind == BudgetKind::IntegerExponent {
        limits.integer_exponent = amount;
    }
    let mut budget = Budget::new(limits)?;
    budget.work(37, nrz_cas::Location::new("quotient prior spend"))?;
    let req = request(&f, SourceClause::Quotient, &f.sources[3]);
    let empty = |clause| nrz_cas::series::SourceCertificate {
        clause,
        coefficients: Vec::new(),
        companion: Vec::new(),
        units: Vec::new(),
    };
    let mut published = None;
    let result = budget.with_retained_payload(
        31 + u64::try_from(bytes.len())?,
        nrz_cas::Location::new("quotient caller bytes"),
        |b| match mode {
            0 => f
                .frame
                .cas
                .check_series_source(&f.expected, &empty(SourceClause::Quotient), &slots, b)
                .map(|witness| {
                    published = Some(witness);
                }),
            1 => f
                .frame
                .cas
                .expand_series_source(&req, &slots, b)
                .map(|witness| {
                    published = Some(witness);
                }),
            2 => receiver.frame.cas.replay(&bytes, &expected, b).map(|_| ()),
            3 => quotient_recipe_with_budget(&mut receiver, b)
                .map(|_| ())
                .map_err(|e| {
                    *e.downcast::<nrz_cas::Fault>()
                        .expect("recipe faults are typed")
                }),
            4 => f
                .frame
                .cas
                .check_evaluated_residual(
                    &original,
                    &expression,
                    &f.frame.y,
                    &rows,
                    Some(&member()),
                    b,
                )
                .map(|_| ()),
            5 => {
                let (residual, solve) = paired.as_ref().expect("paired fixture");
                receiver
                    .frame
                    .cas
                    .replay_evaluated_residual(&bytes, residual, solve, b)
                    .map(|_| ())
            }
            _ => {
                let (statement, proofs) = f.reciprocal.as_ref().expect("reciprocal fixture");
                f.frame
                    .cas
                    .check_series_source(statement, &empty(SourceClause::Reciprocal), proofs, b)
                    .map(|_| ())
            }
        },
    );
    if let Some(witness) = published {
        let exported = f
            .frame
            .cas
            .export_witness(&witness, &mut quotient_budget()?)?;
        assert_eq!(exported, original_bytes);
        receiver
            .frame
            .cas
            .replay(&exported, &expected, &mut quotient_budget()?)?;
    }
    if result.is_err() {
        let after = if imported {
            receiver
                .frame
                .cas
                .canonical_snapshot(&mut quotient_budget()?)?
        } else {
            f.frame.cas.canonical_snapshot(&mut quotient_budget()?)?
        };
        // A recipe is a sequence of independently atomic public operations; its successful prefix stays.
        if mode != 3 {
            assert_eq!(before, after);
        }
    }
    assert_eq!(
        f.frame
            .cas
            .export_witness(&original, &mut quotient_budget()?)?,
        original_bytes
    );
    Ok((result.is_ok(), budget.usage().work, result.err()))
}

/// Full source, complete recipe, residual/Member and paired replay boundary records.
/// All minima and failed prefixes were measured on the frozen compiler in both allocation orders.
/// ´test:integration:series-reciprocal-quotient-resource-measurement´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Each independent operation and dimension retains its exact measured boundary"
)]
fn series_reciprocal_quotient_resource_measurement() -> TestResult {
    use nrz_cas::limits::BudgetKind as K;
    const DIMENSIONS: [K; 14] = [
        K::Work,
        K::StagedPayloadBytes,
        K::JetEntries,
        K::WitnessBytes,
        K::OutputBytes,
        K::JetOrder,
        K::ArithmeticSteps,
        K::ComponentBits,
        K::IntermediateBits,
        K::ProofNodes,
        K::Depth,
        K::ConditionSteps,
        K::PayloadBytes,
        K::ReplayWork,
    ];
    // Each row: N, successful Work, failed Work, reason (0 Limit / 1 ArithmeticRange),
    // exhausted dimension index in DIMENSIONS, reported limit, reported needed.
    static CASES: [(usize, bool, bool, [[u64; 7]; 14]); 24] = [
        (
            0,
            false,
            false,
            [
                [529_281, 529_281, 529_277, 0, 0, 529_280, 529_281],
                [156_323, 529_281, 505_122, 0, 1, 138_063, 138_064],
                [6, 529_281, 36_430, 0, 2, 5, 6],
                [6200, 529_281, 529_277, 0, 3, 6199, 6200],
                [15_393, 529_281, 38_556, 0, 4, 15_392, 15_393],
                [5, 529_281, 36_430, 0, 5, 4, 5],
                [1771, 529_281, 463_517, 0, 6, 1770, 1771],
                [7, 529_281, 621, 1, 7, 6, 7],
                [7, 529_281, 621, 1, 7, 6, 7],
                [11, 529_281, 39_078, 0, 9, 10, 11],
                [4, 529_281, 36_930, 0, 10, 3, 4],
                [12, 529_281, 36_929, 0, 11, 11, 12],
                [1240, 529_281, 236_835, 0, 12, 1239, 1240],
                [0, 529_281, 0, 0, 13, 0, 0],
            ],
        ),
        (
            0,
            false,
            true,
            [
                [461_845, 461_845, 461_841, 0, 0, 461_844, 461_845],
                [133_257, 461_845, 442_588, 0, 1, 115_887, 115_888],
                [6, 461_845, 36_440, 0, 2, 5, 6],
                [5306, 461_845, 461_841, 0, 3, 5305, 5306],
                [13_661, 461_845, 38_454, 0, 4, 13_660, 13_661],
                [5, 461_845, 36_440, 0, 5, 4, 5],
                [1613, 461_845, 380_794, 0, 6, 1612, 1613],
                [7, 461_845, 621, 1, 7, 6, 7],
                [7, 461_845, 621, 1, 7, 6, 7],
                [10, 461_845, 38_942, 0, 9, 9, 10],
                [4, 461_845, 36_896, 0, 10, 3, 4],
                [11, 461_845, 36_895, 0, 11, 10, 11],
                [1240, 461_845, 230_301, 0, 12, 1239, 1240],
                [0, 461_845, 0, 0, 13, 0, 0],
            ],
        ),
        (
            0,
            true,
            false,
            [
                [529_281, 529_281, 529_277, 0, 0, 529_280, 529_281],
                [156_323, 529_281, 505_122, 0, 1, 138_063, 138_064],
                [6, 529_281, 36_430, 0, 2, 5, 6],
                [6200, 529_281, 529_277, 0, 3, 6199, 6200],
                [15_393, 529_281, 38_556, 0, 4, 15_392, 15_393],
                [5, 529_281, 36_430, 0, 5, 4, 5],
                [1771, 529_281, 463_517, 0, 6, 1770, 1771],
                [7, 529_281, 621, 1, 7, 6, 7],
                [7, 529_281, 621, 1, 7, 6, 7],
                [11, 529_281, 39_078, 0, 9, 10, 11],
                [4, 529_281, 36_930, 0, 10, 3, 4],
                [12, 529_281, 36_929, 0, 11, 11, 12],
                [1240, 529_281, 236_835, 0, 12, 1239, 1240],
                [0, 529_281, 0, 0, 13, 0, 0],
            ],
        ),
        (
            0,
            true,
            true,
            [
                [461_845, 461_845, 461_841, 0, 0, 461_844, 461_845],
                [133_257, 461_845, 442_588, 0, 1, 115_887, 115_888],
                [6, 461_845, 36_440, 0, 2, 5, 6],
                [5306, 461_845, 461_841, 0, 3, 5305, 5306],
                [13_661, 461_845, 38_454, 0, 4, 13_660, 13_661],
                [5, 461_845, 36_440, 0, 5, 4, 5],
                [1613, 461_845, 380_794, 0, 6, 1612, 1613],
                [7, 461_845, 621, 1, 7, 6, 7],
                [7, 461_845, 621, 1, 7, 6, 7],
                [10, 461_845, 38_942, 0, 9, 9, 10],
                [4, 461_845, 36_896, 0, 10, 3, 4],
                [11, 461_845, 36_895, 0, 11, 10, 11],
                [1240, 461_845, 230_301, 0, 12, 1239, 1240],
                [0, 461_845, 0, 0, 13, 0, 0],
            ],
        ),
        (
            1,
            false,
            false,
            [
                [575_250, 575_250, 575_246, 0, 0, 575_249, 575_250],
                [193_187, 575_250, 551_091, 0, 1, 138_063, 138_064],
                [24, 575_250, 299, 0, 2, 23, 24],
                [6200, 575_250, 575_246, 0, 3, 6199, 6200],
                [15_393, 575_250, 84_525, 0, 4, 15_392, 15_393],
                [5, 575_250, 299, 0, 5, 4, 5],
                [1855, 575_250, 509_486, 0, 6, 1854, 1855],
                [7, 550_698, 608, 1, 7, 6, 7],
                [7, 550_698, 608, 1, 7, 6, 7],
                [11, 575_250, 85_047, 0, 9, 10, 11],
                [4, 575_250, 82_899, 0, 10, 3, 4],
                [12, 575_250, 82_898, 0, 11, 11, 12],
                [1240, 575_250, 282_804, 0, 12, 1239, 1240],
                [0, 575_250, 0, 0, 13, 0, 0],
            ],
        ),
        (
            1,
            false,
            true,
            [
                [507_819, 507_819, 507_815, 0, 0, 507_818, 507_819],
                [170_121, 507_819, 488_562, 0, 1, 115_887, 115_888],
                [24, 507_819, 299, 0, 2, 23, 24],
                [5306, 507_819, 507_815, 0, 3, 5305, 5306],
                [13_661, 507_819, 84_428, 0, 4, 13_660, 13_661],
                [5, 507_819, 299, 0, 5, 4, 5],
                [1697, 507_819, 426_768, 0, 6, 1696, 1697],
                [7, 483_267, 608, 1, 7, 6, 7],
                [7, 483_267, 608, 1, 7, 6, 7],
                [10, 507_819, 84_916, 0, 9, 9, 10],
                [4, 507_819, 82_870, 0, 10, 3, 4],
                [11, 507_819, 82_869, 0, 11, 10, 11],
                [1240, 507_819, 276_275, 0, 12, 1239, 1240],
                [0, 507_819, 0, 0, 13, 0, 0],
            ],
        ),
        (
            1,
            true,
            false,
            [
                [575_250, 575_250, 575_246, 0, 0, 575_249, 575_250],
                [193_187, 575_250, 551_091, 0, 1, 138_063, 138_064],
                [24, 575_250, 299, 0, 2, 23, 24],
                [6200, 575_250, 575_246, 0, 3, 6199, 6200],
                [15_393, 575_250, 84_525, 0, 4, 15_392, 15_393],
                [5, 575_250, 299, 0, 5, 4, 5],
                [1855, 575_250, 509_486, 0, 6, 1854, 1855],
                [7, 550_698, 608, 1, 7, 6, 7],
                [7, 550_698, 608, 1, 7, 6, 7],
                [11, 575_250, 85_047, 0, 9, 10, 11],
                [4, 575_250, 82_899, 0, 10, 3, 4],
                [12, 575_250, 82_898, 0, 11, 11, 12],
                [1240, 575_250, 282_804, 0, 12, 1239, 1240],
                [0, 575_250, 0, 0, 13, 0, 0],
            ],
        ),
        (
            1,
            true,
            true,
            [
                [507_819, 507_819, 507_815, 0, 0, 507_818, 507_819],
                [170_121, 507_819, 488_562, 0, 1, 115_887, 115_888],
                [24, 507_819, 299, 0, 2, 23, 24],
                [5306, 507_819, 507_815, 0, 3, 5305, 5306],
                [13_661, 507_819, 84_428, 0, 4, 13_660, 13_661],
                [5, 507_819, 299, 0, 5, 4, 5],
                [1697, 507_819, 426_768, 0, 6, 1696, 1697],
                [7, 483_267, 608, 1, 7, 6, 7],
                [7, 483_267, 608, 1, 7, 6, 7],
                [10, 507_819, 84_916, 0, 9, 9, 10],
                [4, 507_819, 82_870, 0, 10, 3, 4],
                [11, 507_819, 82_869, 0, 11, 10, 11],
                [1240, 507_819, 276_275, 0, 12, 1239, 1240],
                [0, 507_819, 0, 0, 13, 0, 0],
            ],
        ),
        (
            2,
            false,
            false,
            [
                [2_017_196, 2_017_196, 2_017_149, 0, 0, 2_017_195, 2_017_196],
                [258_066, 2_017_196, 540_789, 0, 1, 7, 8],
                [152, 2_017_196, 585_848, 0, 2, 151, 152],
                [6_200, 2_017_196, 315, 0, 3, 6_199, 6_200],
                [14_038, 2_017_196, 1_506_676, 0, 4, 14_037, 14_038],
                [5, 2_017_196, 38, 0, 5, 4, 5],
                [10_736, 2_017_196, 1_951_334, 0, 6, 10_735, 10_736],
                [7, 2_017_196, 56_239, 1, 7, 6, 7],
                [8, 2_017_196, 420_322, 1, 8, 7, 8],
                [11, 2_017_196, 1_507_198, 0, 9, 10, 11],
                [4, 2_017_196, 1_505_050, 0, 10, 3, 4],
                [58, 2_017_196, 1_505_049, 0, 11, 57, 58],
                [38_615, 2_017_196, 1_507_198, 0, 12, 38_614, 38_615],
                [2_017_159, 2_017_196, 2_017_149, 0, 13, 2_017_158, 2_017_159],
            ],
        ),
        (
            2,
            false,
            true,
            [
                [1_707_466, 1_707_466, 1_707_419, 0, 0, 1_707_465, 1_707_466],
                [255_542, 1_707_466, 412_090, 0, 1, 7, 8],
                [130, 1_707_466, 457_149, 0, 2, 129, 130],
                [5_306, 1_707_466, 315, 0, 3, 5_305, 5_306],
                [12_357, 1_707_466, 1_267_447, 0, 4, 12_356, 12_357],
                [5, 1_707_466, 38, 0, 5, 4, 5],
                [9_625, 1_707_466, 1_626_317, 0, 6, 9_624, 9_625],
                [7, 1_707_466, 132_774, 1, 7, 6, 7],
                [7, 1_707_466, 132_774, 1, 7, 6, 7],
                [9, 1_707_466, 1_267_935, 0, 9, 8, 9],
                [4, 1_707_466, 1_265_889, 0, 10, 3, 4],
                [51, 1_707_466, 1_265_888, 0, 11, 50, 51],
                [32_411, 1_707_466, 1_267_935, 0, 12, 32_410, 32_411],
                [1_707_429, 1_707_466, 1_707_419, 0, 13, 1_707_428, 1_707_429],
            ],
        ),
        (
            2,
            true,
            false,
            [
                [2_017_196, 2_017_196, 2_017_149, 0, 0, 2_017_195, 2_017_196],
                [258_066, 2_017_196, 540_789, 0, 1, 7, 8],
                [152, 2_017_196, 585_848, 0, 2, 151, 152],
                [6_200, 2_017_196, 315, 0, 3, 6_199, 6_200],
                [14_038, 2_017_196, 1_506_676, 0, 4, 14_037, 14_038],
                [5, 2_017_196, 38, 0, 5, 4, 5],
                [10_736, 2_017_196, 1_951_334, 0, 6, 10_735, 10_736],
                [7, 2_017_196, 56_239, 1, 7, 6, 7],
                [8, 2_017_196, 420_322, 1, 8, 7, 8],
                [11, 2_017_196, 1_507_198, 0, 9, 10, 11],
                [4, 2_017_196, 1_505_050, 0, 10, 3, 4],
                [58, 2_017_196, 1_505_049, 0, 11, 57, 58],
                [38_615, 2_017_196, 1_507_198, 0, 12, 38_614, 38_615],
                [2_017_159, 2_017_196, 2_017_149, 0, 13, 2_017_158, 2_017_159],
            ],
        ),
        (
            2,
            true,
            true,
            [
                [1_707_466, 1_707_466, 1_707_419, 0, 0, 1_707_465, 1_707_466],
                [255_542, 1_707_466, 412_090, 0, 1, 7, 8],
                [130, 1_707_466, 457_149, 0, 2, 129, 130],
                [5_306, 1_707_466, 315, 0, 3, 5_305, 5_306],
                [12_357, 1_707_466, 1_267_447, 0, 4, 12_356, 12_357],
                [5, 1_707_466, 38, 0, 5, 4, 5],
                [9_625, 1_707_466, 1_626_317, 0, 6, 9_624, 9_625],
                [7, 1_707_466, 132_774, 1, 7, 6, 7],
                [7, 1_707_466, 132_774, 1, 7, 6, 7],
                [9, 1_707_466, 1_267_935, 0, 9, 8, 9],
                [4, 1_707_466, 1_265_889, 0, 10, 3, 4],
                [51, 1_707_466, 1_265_888, 0, 11, 50, 51],
                [32_411, 1_707_466, 1_267_935, 0, 12, 32_410, 32_411],
                [1_707_429, 1_707_466, 1_707_419, 0, 13, 1_707_428, 1_707_429],
            ],
        ),
        (
            3,
            false,
            false,
            [
                [2_010_584, 2_010_584, 2_010_580, 0, 0, 2_010_583, 2_010_584],
                [494_528, 2_010_584, 593_535, 0, 1, 7, 8],
                [212, 2_010_584, 1_245_645, 0, 2, 211, 212],
                [6200, 2_010_584, 2_010_580, 0, 3, 6199, 6200],
                [14_038, 2_010_584, 1_519_859, 0, 4, 14_037, 14_038],
                [5, 2_010_584, 155_123, 0, 5, 4, 5],
                [11_732, 2_010_584, 1_944_820, 0, 6, 11_731, 11_732],
                [7, 1_887_824, 177_684, 1, 7, 6, 7],
                [8, 1_887_944, 177_492, 1, 8, 7, 8],
                [11, 2_010_584, 1_520_381, 0, 9, 10, 11],
                [4, 2_010_584, 1_518_233, 0, 10, 3, 4],
                [43, 2_010_584, 1_518_232, 0, 11, 42, 43],
                [38_615, 2_010_584, 1_520_381, 0, 12, 38_614, 38_615],
                [0, 2_010_584, 0, 0, 13, 0, 0],
            ],
        ),
        (
            3,
            false,
            true,
            [
                [1_731_275, 1_731_275, 1_731_271, 0, 0, 1_731_274, 1_731_275],
                [492_001, 1_731_275, 392_440, 0, 1, 7, 8],
                [178, 1_731_275, 1_039_111, 0, 2, 177, 178],
                [5306, 1_731_275, 1_731_271, 0, 3, 5305, 5306],
                [12_357, 1_731_275, 1_307_884, 0, 4, 12_356, 12_357],
                [5, 1_731_275, 143_108, 0, 5, 4, 5],
                [10_499, 1_731_275, 1_650_224, 0, 6, 10_498, 10_499],
                [7, 1_645_343, 165_669, 1, 7, 6, 7],
                [8, 1_645_427, 165_477, 1, 8, 7, 8],
                [10, 1_731_275, 1_308_372, 0, 9, 9, 10],
                [4, 1_731_275, 1_306_326, 0, 10, 3, 4],
                [39, 1_731_275, 1_306_325, 0, 11, 38, 39],
                [33_420, 1_731_275, 1_308_372, 0, 12, 33_419, 33_420],
                [0, 1_731_275, 0, 0, 13, 0, 0],
            ],
        ),
        (
            3,
            true,
            false,
            [
                [2_010_584, 2_010_584, 2_010_580, 0, 0, 2_010_583, 2_010_584],
                [494_528, 2_010_584, 593_535, 0, 1, 7, 8],
                [212, 2_010_584, 1_245_645, 0, 2, 211, 212],
                [6200, 2_010_584, 2_010_580, 0, 3, 6199, 6200],
                [14_038, 2_010_584, 1_519_859, 0, 4, 14_037, 14_038],
                [5, 2_010_584, 155_123, 0, 5, 4, 5],
                [11_732, 2_010_584, 1_944_820, 0, 6, 11_731, 11_732],
                [7, 1_887_824, 177_684, 1, 7, 6, 7],
                [8, 1_887_944, 177_492, 1, 8, 7, 8],
                [11, 2_010_584, 1_520_381, 0, 9, 10, 11],
                [4, 2_010_584, 1_518_233, 0, 10, 3, 4],
                [43, 2_010_584, 1_518_232, 0, 11, 42, 43],
                [38_615, 2_010_584, 1_520_381, 0, 12, 38_614, 38_615],
                [0, 2_010_584, 0, 0, 13, 0, 0],
            ],
        ),
        (
            3,
            true,
            true,
            [
                [1_731_275, 1_731_275, 1_731_271, 0, 0, 1_731_274, 1_731_275],
                [492_001, 1_731_275, 392_440, 0, 1, 7, 8],
                [178, 1_731_275, 1_039_111, 0, 2, 177, 178],
                [5306, 1_731_275, 1_731_271, 0, 3, 5305, 5306],
                [12_357, 1_731_275, 1_307_884, 0, 4, 12_356, 12_357],
                [5, 1_731_275, 143_108, 0, 5, 4, 5],
                [10_499, 1_731_275, 1_650_224, 0, 6, 10_498, 10_499],
                [7, 1_645_343, 165_669, 1, 7, 6, 7],
                [8, 1_645_427, 165_477, 1, 8, 7, 8],
                [10, 1_731_275, 1_308_372, 0, 9, 9, 10],
                [4, 1_731_275, 1_306_326, 0, 10, 3, 4],
                [39, 1_731_275, 1_306_325, 0, 11, 38, 39],
                [33_420, 1_731_275, 1_308_372, 0, 12, 33_419, 33_420],
                [0, 1_731_275, 0, 0, 13, 0, 0],
            ],
        ),
        (
            4,
            false,
            false,
            [
                [1_424_037, 1_424_037, 1_424_033, 0, 0, 1_424_036, 1_424_037],
                [286_202, 1_424_037, 97_902, 0, 1, 2943, 2944],
                [115, 1_424_037, 143_665, 0, 2, 114, 115],
                [7878, 1_424_037, 1_424_033, 0, 3, 7877, 7878],
                [16_975, 1_424_037, 151_942, 0, 4, 16_974, 16_975],
                [5, 1_424_037, 293, 0, 5, 4, 5],
                [5644, 1_424_037, 1_359_075, 0, 6, 5643, 5644],
                [7, 1_424_037, 242_164, 1, 7, 6, 7],
                [7, 1_424_037, 242_164, 1, 7, 6, 7],
                [13, 1_424_037, 794_420, 0, 9, 12, 13],
                [6, 1_424_037, 793_904, 0, 10, 5, 6],
                [14, 1_424_037, 793_903, 0, 11, 13, 14],
                [49_234, 1_424_037, 794_420, 0, 12, 49_233, 49_234],
                [0, 1_424_037, 0, 0, 13, 0, 0],
            ],
        ),
        (
            4,
            true,
            false,
            [
                [1_424_037, 1_424_037, 1_424_033, 0, 0, 1_424_036, 1_424_037],
                [286_202, 1_424_037, 97_902, 0, 1, 2943, 2944],
                [115, 1_424_037, 143_665, 0, 2, 114, 115],
                [7878, 1_424_037, 1_424_033, 0, 3, 7877, 7878],
                [16_975, 1_424_037, 151_942, 0, 4, 16_974, 16_975],
                [5, 1_424_037, 293, 0, 5, 4, 5],
                [5644, 1_424_037, 1_359_075, 0, 6, 5643, 5644],
                [7, 1_424_037, 242_164, 1, 7, 6, 7],
                [7, 1_424_037, 242_164, 1, 7, 6, 7],
                [13, 1_424_037, 794_420, 0, 9, 12, 13],
                [6, 1_424_037, 793_904, 0, 10, 5, 6],
                [14, 1_424_037, 793_903, 0, 11, 13, 14],
                [49_234, 1_424_037, 794_420, 0, 12, 49_233, 49_234],
                [0, 1_424_037, 0, 0, 13, 0, 0],
            ],
        ),
        (
            5,
            false,
            false,
            [
                [4_417_387, 4_417_387, 4_417_383, 0, 0, 4_417_386, 4_417_387],
                [259_872, 4_417_387, 666_297, 0, 1, 7, 8],
                [312, 4_417_387, 2_281_625, 0, 2, 311, 312],
                [7_878, 4_417_387, 3_186, 0, 3, 7_877, 7_878],
                [16_975, 4_417_387, 2_289_902, 0, 4, 16_974, 16_975],
                [5, 4_417_387, 38, 0, 5, 4, 5],
                [18_644, 4_417_387, 4_407_969, 0, 6, 18_643, 18_644],
                [7, 4_417_387, 77_735, 1, 7, 6, 7],
                [8, 4_417_387, 529_422, 1, 8, 7, 8],
                [13, 4_417_387, 2_964_264, 0, 9, 12, 13],
                [6, 4_417_387, 2_963_748, 0, 10, 5, 6],
                [72, 4_417_387, 2_963_747, 0, 11, 71, 72],
                [50_122, 4_417_387, 2_964_264, 0, 12, 50_121, 50_122],
                [4_417_350, 4_417_387, 4_417_383, 0, 13, 4_417_349, 4_417_350],
            ],
        ),
        (
            5,
            true,
            false,
            [
                [4_417_387, 4_417_387, 4_417_383, 0, 0, 4_417_386, 4_417_387],
                [259_872, 4_417_387, 666_297, 0, 1, 7, 8],
                [312, 4_417_387, 2_281_625, 0, 2, 311, 312],
                [7_878, 4_417_387, 3_186, 0, 3, 7_877, 7_878],
                [16_975, 4_417_387, 2_289_902, 0, 4, 16_974, 16_975],
                [5, 4_417_387, 38, 0, 5, 4, 5],
                [18_644, 4_417_387, 4_407_969, 0, 6, 18_643, 18_644],
                [7, 4_417_387, 77_735, 1, 7, 6, 7],
                [8, 4_417_387, 529_422, 1, 8, 7, 8],
                [13, 4_417_387, 2_964_264, 0, 9, 12, 13],
                [6, 4_417_387, 2_963_748, 0, 10, 5, 6],
                [72, 4_417_387, 2_963_747, 0, 11, 71, 72],
                [50_122, 4_417_387, 2_964_264, 0, 12, 50_121, 50_122],
                [4_417_350, 4_417_387, 4_417_383, 0, 13, 4_417_349, 4_417_350],
            ],
        ),
        (
            6,
            false,
            false,
            [
                [282_215, 282_215, 282_211, 0, 0, 282_214, 282_215],
                [94_699, 282_215, 268_991, 0, 1, 79_231, 79_232],
                [6, 282_215, 28_422, 0, 2, 5, 6],
                [3560, 282_215, 282_211, 0, 3, 3559, 3560],
                [15_194, 282_215, 30_152, 0, 4, 15_193, 15_194],
                [5, 282_215, 28_422, 0, 5, 4, 5],
                [985, 282_215, 238_477, 0, 6, 984, 985],
                [7, 282_215, 619, 1, 7, 6, 7],
                [7, 282_215, 619, 1, 7, 6, 7],
                [11, 282_215, 30_584, 0, 9, 10, 11],
                [3, 282_215, 28_770, 0, 10, 2, 3],
                [8, 282_215, 28_769, 0, 11, 7, 8],
                [1240, 282_215, 7188, 0, 12, 1239, 1240],
                [0, 282_215, 0, 0, 13, 0, 0],
            ],
        ),
        (
            6,
            false,
            true,
            [
                [279_652, 279_652, 279_648, 0, 0, 279_651, 279_652],
                [91_741, 279_652, 266_878, 0, 1, 77_167, 77_168],
                [6, 279_652, 28_422, 0, 2, 5, 6],
                [3489, 279_652, 279_648, 0, 3, 3488, 3489],
                [13_513, 279_652, 30_152, 0, 4, 13_512, 13_513],
                [5, 279_652, 28_422, 0, 5, 4, 5],
                [985, 279_652, 236_364, 0, 6, 984, 985],
                [7, 279_652, 619, 1, 7, 6, 7],
                [7, 279_652, 619, 1, 7, 6, 7],
                [10, 279_652, 30_584, 0, 9, 9, 10],
                [3, 279_652, 28_770, 0, 10, 2, 3],
                [8, 279_652, 28_769, 0, 11, 7, 8],
                [1240, 279_652, 7188, 0, 12, 1239, 1240],
                [0, 279_652, 0, 0, 13, 0, 0],
            ],
        ),
        (
            6,
            true,
            false,
            [
                [282_215, 282_215, 282_211, 0, 0, 282_214, 282_215],
                [94_699, 282_215, 268_991, 0, 1, 79_231, 79_232],
                [6, 282_215, 28_422, 0, 2, 5, 6],
                [3560, 282_215, 282_211, 0, 3, 3559, 3560],
                [15_194, 282_215, 30_152, 0, 4, 15_193, 15_194],
                [5, 282_215, 28_422, 0, 5, 4, 5],
                [985, 282_215, 238_477, 0, 6, 984, 985],
                [7, 282_215, 619, 1, 7, 6, 7],
                [7, 282_215, 619, 1, 7, 6, 7],
                [11, 282_215, 30_584, 0, 9, 10, 11],
                [3, 282_215, 28_770, 0, 10, 2, 3],
                [8, 282_215, 28_769, 0, 11, 7, 8],
                [1240, 282_215, 7188, 0, 12, 1239, 1240],
                [0, 282_215, 0, 0, 13, 0, 0],
            ],
        ),
        (
            6,
            true,
            true,
            [
                [279_652, 279_652, 279_648, 0, 0, 279_651, 279_652],
                [91_741, 279_652, 266_878, 0, 1, 77_167, 77_168],
                [6, 279_652, 28_422, 0, 2, 5, 6],
                [3489, 279_652, 279_648, 0, 3, 3488, 3489],
                [13_513, 279_652, 30_152, 0, 4, 13_512, 13_513],
                [5, 279_652, 28_422, 0, 5, 4, 5],
                [985, 279_652, 236_364, 0, 6, 984, 985],
                [7, 279_652, 619, 1, 7, 6, 7],
                [7, 279_652, 619, 1, 7, 6, 7],
                [10, 279_652, 30_584, 0, 9, 9, 10],
                [3, 279_652, 28_770, 0, 10, 2, 3],
                [8, 279_652, 28_769, 0, 11, 7, 8],
                [1240, 279_652, 7188, 0, 12, 1239, 1240],
                [0, 279_652, 0, 0, 13, 0, 0],
            ],
        ),
    ];
    for (mode, reversed, equal, rows) in CASES.iter().copied() {
        for (kind, [n, work, prefix, reason, exhausted, limit, needed]) in
            DIMENSIONS.into_iter().zip(rows)
        {
            let success = quotient_resource_attempt(mode, reversed, equal, kind, n)?;
            assert!(
                success.0,
                "{mode} {reversed} {equal} {kind:?} N={n}: {:?}",
                success.2
            );
            assert_eq!(success.1, work);
            if n > 0 {
                let failure = quotient_resource_attempt(mode, reversed, equal, kind, n - 1)?;
                assert!(
                    !failure.0,
                    "mode={mode} reversed={reversed} equal={equal} dimension={kind:?} N={n}: {failure:?}"
                );
                assert_eq!(failure.1, prefix);
                let reason = if reason == 0 {
                    nrz_cas::Exhaustion::Limit
                } else {
                    nrz_cas::Exhaustion::ArithmeticRange
                };
                let budget = *DIMENSIONS
                    .get(usize::try_from(exhausted)?)
                    .ok_or("dimension")?;
                assert_eq!(
                    failure.2.ok_or("missing exhaustion")?.kind,
                    FaultKind::Exhausted {
                        reason,
                        budget,
                        limit,
                        needed
                    }
                );
            }
        }
    }
    Ok(())
}

/// A declared coordinate at centre two works in a genuinely nonempty admitted binder frame.
/// ´test:integration:series-reciprocal-scoped-centred-source´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "The complete fixture and exact boundary checks remain adjacent"
)]
fn series_reciprocal_scoped_centred_source() -> TestResult {
    use nrz_cas::domain::{AmbientDomain, RegionDraft, RegionExpr};
    use nrz_cas::syntax::{AtomicSort, ExpressionDraft, Field, Sort};
    for reversed in [false, true] {
        let mut f = ResidualFixture::new(&["1/2", "-1/4", "1/8", "-1/16"], "2", reversed, false)?;
        let scope = if reversed {
            vec![vec![AtomicSort::Scalar(Field::Real)]]
        } else {
            Vec::new()
        };
        let make =
            |cas: &mut nrz_cas::Cas, operator, operands: Vec<Handle>| -> TestResult<Handle> {
                admit(
                    cas,
                    Draft::Expression(ExpressionDraft {
                        header: Reference::Existing(f.header.clone()),
                        space: None,
                        scope: scope.clone(),
                        binding: None,
                        sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
                        operator,
                        operands: operands.into_iter().map(Reference::Existing).collect(),
                    }),
                )
            };
        let x = make(&mut f.cas, Operator::Free("x".into()), Vec::new())?;
        let centre = make(
            &mut f.cas,
            Operator::Rational(rational(if reversed { "0" } else { "2" })?),
            Vec::new(),
        )?;
        let one = make(&mut f.cas, Operator::Rational(rational("1")?), Vec::new())?;
        let denominator_source = if reversed {
            make(&mut f.cas, Operator::Cosine, vec![x.clone()])?
        } else {
            x.clone()
        };
        let source = make(
            &mut f.cas,
            Operator::Div,
            vec![one, denominator_source.clone()],
        )?;
        let domain = admit(
            &mut f.cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(f.header.clone()),
                scope: scope.clone(),
                ambient: AmbientDomain::Real("x".into()),
                expression: RegionExpr::Ambient,
            }),
        )?;
        let mut predicates = Vec::new();
        for (kind, expression) in [
            (PredicateKind::Defined, denominator_source.clone()),
            (PredicateKind::NonZero, denominator_source.clone()),
            (PredicateKind::Defined, source.clone()),
        ] {
            predicates.push(admit(
                &mut f.cas,
                Draft::Condition {
                    header: Reference::Existing(f.header.clone()),
                    scope: scope.clone(),
                    kind,
                    arguments: vec![Reference::Existing(expression)],
                },
            )?);
        }
        let context = admit(
            &mut f.cas,
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(f.header.clone()),
                admissible: Reference::Existing(domain.clone()),
                assumptions: predicates
                    .iter()
                    .enumerate()
                    .map(|(i, p)| AssumptionDraft {
                        key: format!("centred-{i}"),
                        condition: Reference::Existing(p.clone()),
                    })
                    .collect(),
            }),
        )?;
        let mut guards = Vec::new();
        for (i, predicate) in predicates.into_iter().enumerate() {
            guards.push(
                f.cas
                    .check_proof(
                        &context,
                        &[ApplicationDraft {
                            rule: Primitive::ContextHypothesis,
                            context: context.clone(),
                            conclusion: Statement::PredicateFact {
                                region: domain.clone(),
                                predicate,
                            },
                            premises: Vec::new(),
                            substitutions: Vec::new(),
                            certificate: Certificate::Hypothesis(HypothesisRef {
                                context: context.clone(),
                                slot: u32::try_from(i)?,
                            }),
                        }],
                        &mut quotient_budget()?,
                    )?
                    .proof()
                    .clone(),
            );
        }
        let mut request = SourceRequest {
            clause: if reversed {
                SourceClause::CosineOrigin
            } else {
                SourceClause::PolynomialSeed
            },
            source: denominator_source,
            domain,
            field: f.field.clone(),
            centre,
            variable: x,
            scope: scope.clone(),
            context,
            order: 4,
        };
        let denominator = if reversed {
            None
        } else {
            Some(
                f.cas
                    .expand_series_source(&request, &guards[..1], &mut quotient_budget()?)?,
            )
        };
        let input = if let Some(denominator) = &denominator {
            source_output(denominator)?
        } else {
            let mut coefficients = Vec::new();
            for value in ["1", "0", "-1/2", "0"] {
                coefficients.push(Reference::Existing(make(
                    &mut f.cas,
                    Operator::Rational(rational(value)?),
                    Vec::new(),
                )?));
            }
            admit(
                &mut f.cas,
                Draft::SeriesObject(nrz_cas::series::SeriesObject {
                    field: Reference::Existing(request.field.clone()),
                    centre: Reference::Existing(request.centre.clone()),
                    variable: Reference::Existing(request.variable.clone()),
                    scope: scope.clone(),
                    context: Reference::Existing(request.context.clone()),
                    order: 4,
                    coefficients,
                }),
            )?
        };
        let inverse =
            f.cas
                .compute_series(&O::Reciprocal { input }, &[], &mut quotient_budget()?)?;
        let Statement::Series(s) = &inverse.judgment().statement else {
            return Err("inverse".into());
        };
        let S::Reciprocal { output, .. } = s.as_ref() else {
            return Err("inverse".into());
        };
        let object = f.cas.series_object(output)?;
        assert_eq!(object.scope, scope);
        for (coefficient, expected) in object.coefficients.iter().zip(if reversed {
            ["1", "0", "1/2", "0"]
        } else {
            ["1/2", "-1/4", "1/8", "-1/16"]
        }) {
            assert_eq!(
                f.cas.expression(coefficient)?.0,
                &Operator::Rational(rational(expected)?)
            );
        }
        if reversed {
            let bytes = f.cas.export_witness(&inverse, &mut quotient_budget()?)?;
            let mut receiver = ResidualFixture::new(&["0"], "0", false, false)?;
            receiver.cas.inspect(&bytes, &mut quotient_budget()?)?;
            continue;
        }
        let denominator = denominator.ok_or("centred source")?;
        request.clause = SourceClause::Reciprocal;
        request.source = source;
        let result = f.cas.expand_series_source(
            &request,
            &[
                denominator.proof().clone(),
                inverse.proof().clone(),
                guards[1].clone(),
                guards[2].clone(),
            ],
            &mut quotient_budget()?,
        )?;
        assert_eq!(result.hypotheses().len(), 3);
        let bytes = f.cas.export_witness(&result, &mut quotient_budget()?)?;
        let mut receiver = ResidualFixture::new(&["0"], "0", !reversed, false)?;
        let imported = receiver.cas.inspect(&bytes, &mut quotient_budget()?)?;
        assert_eq!(imported.hypotheses().len(), 3);
    }
    Ok(())
}

/// Equal finite vectors preserve the original one versus G/G and atan versus tan subjects.
/// ´test:integration:series-reciprocal-provenance-twins´
#[test]
fn series_reciprocal_provenance_twins() -> TestResult {
    for equal in [false, true] {
        let mut f = quotient_frame(false, equal)?;
        if !equal {
            f.frame.candidate = vec![rational("0")?];
        }
        let alternate = if equal {
            f.frame.node(Operator::Rational(rational("1")?), &[])?
        } else {
            f.frame.node(Operator::Arctangent, &[f.frame.x.clone()])?
        };
        let predicate = admit(
            &mut f.frame.cas,
            Draft::Condition {
                header: Reference::Existing(f.frame.header.clone()),
                scope: Vec::new(),
                kind: PredicateKind::Defined,
                arguments: vec![Reference::Existing(alternate.clone())],
            },
        )?;
        f.predicates.push(predicate.clone());
        f.frame.context = admit(
            &mut f.frame.cas,
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(f.frame.header.clone()),
                admissible: Reference::Existing(f.frame.region.clone()),
                assumptions: f
                    .predicates
                    .iter()
                    .enumerate()
                    .map(|(i, p)| AssumptionDraft {
                        key: format!("twin-guard-{i}"),
                        condition: Reference::Existing(p.clone()),
                    })
                    .collect(),
            }),
        )?;
        f.expected.output = f.frame.object(&f.frame.candidate.clone())?;
        let (quotient, _) = quotient_recipe(&mut f)?;
        let guard = f.frame.cas.check_proof(
            &f.frame.context,
            &[ApplicationDraft {
                rule: Primitive::ContextHypothesis,
                context: f.frame.context.clone(),
                conclusion: Statement::PredicateFact {
                    region: f.frame.region.clone(),
                    predicate,
                },
                premises: Vec::new(),
                substitutions: Vec::new(),
                certificate: Certificate::Hypothesis(HypothesisRef {
                    context: f.frame.context.clone(),
                    slot: 5,
                }),
            }],
            &mut quotient_budget()?,
        )?;
        let alternate_request = request(
            &f,
            if equal {
                SourceClause::PolynomialSeed
            } else {
                SourceClause::ArctangentOrigin
            },
            &alternate,
        );
        let twin = f.frame.cas.expand_series_source(
            &alternate_request,
            &[guard.proof().clone()],
            &mut quotient_budget()?,
        )?;
        assert_eq!(source_output(&quotient)?, source_output(&twin)?);
        assert_ne!(quotient.judgment(), twin.judgment());
        let bytes = f
            .frame
            .cas
            .export_witness(&quotient, &mut quotient_budget()?)?;
        let before = f.frame.cas.canonical_snapshot(&mut quotient_budget()?)?;
        assert_eq!(
            f.frame
                .cas
                .replay(&bytes, twin.judgment(), &mut quotient_budget()?)
                .expect_err("original source matters")
                .kind,
            FaultKind::Invalid(InvalidCode::EndpointMismatch)
        );
        assert_eq!(
            f.frame.cas.canonical_snapshot(&mut quotient_budget()?)?,
            before
        );
    }
    Ok(())
}

/// Original operator, ordered children, numerator literal and exact domain cannot be replaced by equal coefficients.
/// ´test:integration:series-reciprocal-original-joins´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Original syntax, wrong guard and domain mutations share the same checked fixture"
)]
fn series_reciprocal_original_joins() -> TestResult {
    for reciprocal in [false, true] {
        let mut f = quotient_fixture(false)?;
        let wrong_nonzero = admit(
            &mut f.frame.cas,
            Draft::Condition {
                header: Reference::Existing(f.frame.header.clone()),
                scope: Vec::new(),
                kind: PredicateKind::NonZero,
                arguments: vec![Reference::Existing(f.sources[0].clone())],
            },
        )?;
        f.predicates.push(wrong_nonzero.clone());
        f.frame.context = admit(
            &mut f.frame.cas,
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(f.frame.header.clone()),
                admissible: Reference::Existing(f.frame.region.clone()),
                assumptions: f
                    .predicates
                    .iter()
                    .enumerate()
                    .map(|(i, p)| AssumptionDraft {
                        key: format!("quotient-guard-{i}"),
                        condition: Reference::Existing(p.clone()),
                    })
                    .collect(),
            }),
        )?;
        f.expected.output = f.frame.object(&f.frame.candidate.clone())?;
        let (_, quotient_slots) = quotient_recipe(&mut f)?;
        let wrong_guard = f.frame.cas.check_proof(
            &f.frame.context,
            &[ApplicationDraft {
                rule: Primitive::ContextHypothesis,
                context: f.frame.context.clone(),
                conclusion: Statement::PredicateFact {
                    region: f.frame.region.clone(),
                    predicate: wrong_nonzero,
                },
                premises: Vec::new(),
                substitutions: Vec::new(),
                certificate: Certificate::Hypothesis(HypothesisRef {
                    context: f.frame.context.clone(),
                    slot: 5,
                }),
            }],
            &mut quotient_budget()?,
        )?;
        let (original, slots) = if reciprocal {
            f.reciprocal.clone().ok_or("reciprocal")?
        } else {
            (f.expected.clone(), quotient_slots)
        };
        let two = f.frame.node(Operator::Rational(rational("2")?), &[])?;
        let one = f.frame.node(Operator::Rational(rational("1")?), &[])?;
        let numerator = if reciprocal {
            one
        } else {
            f.sources[0].clone()
        };
        let denominator = f.sources[1].clone();
        let equal_one = f
            .frame
            .node(Operator::Div, &[denominator.clone(), denominator.clone()])?;
        let product = admit(
            &mut f.frame.cas,
            Draft::Solve(nrz_cas::solving::SolveDraft::CoordinateSpace(
                nrz_cas::solving::CoordinateSpaceData {
                    header: Reference::Existing(f.frame.header.clone()),
                    scalar_spaces: Vec::new(),
                    coordinates: vec![nrz_cas::solving::Coordinate {
                        key: "x".into(),
                        field: nrz_cas::syntax::Field::Real,
                        role: nrz_cas::solving::CoordinateRole::Coefficient,
                    }],
                },
            )),
        )?;
        let equivalent_domain = admit(
            &mut f.frame.cas,
            Draft::Region(nrz_cas::domain::RegionDraft {
                header: Reference::Existing(f.frame.header.clone()),
                scope: Vec::new(),
                ambient: nrz_cas::domain::AmbientDomain::Product {
                    coordinates: product,
                },
                expression: nrz_cas::domain::RegionExpr::Meet(vec![Reference::Existing(
                    f.frame.region.clone(),
                )]),
            }),
        )?;
        for change in 0..9 {
            let mut statement = original.clone();
            statement.source = match change {
                0 => f
                    .frame
                    .node(Operator::Mul, &[numerator.clone(), denominator.clone()])?,
                1 => f
                    .frame
                    .node(Operator::Div, &[denominator.clone(), numerator.clone()])?,
                2 => f
                    .frame
                    .node(Operator::Div, &[two.clone(), denominator.clone()])?,
                3 => f
                    .frame
                    .node(Operator::Div, &[equal_one.clone(), denominator.clone()])?,
                _ => original.source.clone(),
            };
            let mut proofs = slots.clone();
            if change == 4 {
                proofs.push(proofs[0].clone());
            }
            if change == 5 {
                let n = proofs.len();
                proofs.swap(n - 1, n - 2);
            }
            if change == 6 {
                statement.domain = equivalent_domain.clone();
            }
            if change == 7 {
                let nonzero_slot = proofs.len() - 2;
                proofs[nonzero_slot] = wrong_guard.proof().clone();
            }
            if change == 8 {
                proofs[0] = f.frame.centre.clone();
            }
            let before = f.frame.cas.canonical_snapshot(&mut quotient_budget()?)?;
            assert_eq!(
                f.frame
                    .cas
                    .check_series_source(
                        &statement,
                        &nrz_cas::series::SourceCertificate {
                            clause: statement.clause,
                            coefficients: Vec::new(),
                            companion: Vec::new(),
                            units: Vec::new()
                        },
                        &proofs,
                        &mut quotient_budget()?
                    )
                    .expect_err("original join")
                    .kind,
                FaultKind::Invalid(InvalidCode::Rule)
            );
            assert_eq!(
                f.frame.cas.canonical_snapshot(&mut quotient_budget()?)?,
                before
            );
        }
    }
    Ok(())
}

/// A polynomial unit at zero still needs its original nonzero-domain hypothesis.
/// ´test:integration:series-reciprocal-polynomial-domain´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Explicit polynomial syntax, three domain guards and independent fresh expectation"
)]
fn series_reciprocal_polynomial_domain() -> TestResult {
    let mut payload: Option<Vec<u8>> = None;
    for reversed in [false, true] {
        let mut f = ResidualFixture::new(&["1", "1", "1", "1"], "0", reversed, false)?;
        let one = f.node(Operator::Rational(rational("1")?), &[])?;
        let denominator = f.node(Operator::Sub, &[one.clone(), f.x.clone()])?;
        let reciprocal = f.node(Operator::Div, &[one, denominator.clone()])?;
        let mut predicates = Vec::new();
        for (kind, source) in [
            (PredicateKind::Defined, denominator.clone()),
            (PredicateKind::NonZero, denominator.clone()),
            (PredicateKind::Defined, reciprocal.clone()),
        ] {
            predicates.push(admit(
                &mut f.cas,
                Draft::Condition {
                    header: Reference::Existing(f.header.clone()),
                    scope: Vec::new(),
                    kind,
                    arguments: vec![Reference::Existing(source)],
                },
            )?);
        }
        f.context = admit(
            &mut f.cas,
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(f.header.clone()),
                admissible: Reference::Existing(f.region.clone()),
                assumptions: predicates
                    .iter()
                    .enumerate()
                    .map(|(i, p)| AssumptionDraft {
                        key: format!("polynomial-guard-{i}"),
                        condition: Reference::Existing(p.clone()),
                    })
                    .collect(),
            }),
        )?;
        let output = f.object(&f.candidate.clone())?;
        let expected = Judgment {
            context: f.context.clone(),
            layer: nrz_cas::domain::Layer::Logic,
            space: None,
            statement: Statement::Series(Box::new(S::Source(SourceStatement {
                clause: SourceClause::Reciprocal,
                source: reciprocal.clone(),
                domain: f.region.clone(),
                output,
            }))),
            residuals: Vec::new(),
        };
        if let Some(bytes) = &payload {
            let replayed = f.cas.replay(bytes, &expected, &mut quotient_budget()?)?;
            assert_eq!(replayed.hypotheses().len(), 3);
            continue;
        }
        let mut guards = Vec::new();
        for (i, predicate) in predicates.into_iter().enumerate() {
            guards.push(f.cas.check_proof(
                &f.context,
                &[ApplicationDraft {
                    rule: Primitive::ContextHypothesis,
                    context: f.context.clone(),
                    conclusion: Statement::PredicateFact {
                        region: f.region.clone(),
                        predicate,
                    },
                    premises: Vec::new(),
                    substitutions: Vec::new(),
                    certificate: Certificate::Hypothesis(HypothesisRef {
                        context: f.context.clone(),
                        slot: u32::try_from(i)?,
                    }),
                }],
                &mut quotient_budget()?,
            )?);
        }
        let mut request = SourceRequest {
            clause: SourceClause::PolynomialSeed,
            source: denominator,
            domain: f.region.clone(),
            field: f.field.clone(),
            centre: f.centre.clone(),
            variable: f.x.clone(),
            scope: Vec::new(),
            context: f.context.clone(),
            order: 4,
        };
        let source = f.cas.expand_series_source(
            &request,
            &[guards[0].proof().clone()],
            &mut quotient_budget()?,
        )?;
        let input = f.object(&[
            rational("1")?,
            rational("-1")?,
            rational("0")?,
            rational("0")?,
        ])?;
        assert_eq!(source_output(&source)?, input);
        let inverse =
            f.cas
                .compute_series(&O::Reciprocal { input }, &[], &mut quotient_budget()?)?;
        request.clause = SourceClause::Reciprocal;
        request.source = reciprocal;
        let slots = [
            source.proof().clone(),
            inverse.proof().clone(),
            guards[1].proof().clone(),
            guards[2].proof().clone(),
        ];
        let before = f.cas.canonical_snapshot(&mut quotient_budget()?)?;
        assert_eq!(
            f.cas
                .expand_series_source(
                    &request,
                    &[slots[0].clone(), slots[1].clone(), slots[3].clone()],
                    &mut quotient_budget()?
                )
                .expect_err("formal unit cannot provide a NonZero slot")
                .kind,
            FaultKind::Invalid(InvalidCode::Rule)
        );
        assert_eq!(f.cas.canonical_snapshot(&mut quotient_budget()?)?, before);
        let checked = f
            .cas
            .expand_series_source(&request, &slots, &mut quotient_budget()?)?;
        assert_eq!(checked.judgment(), &expected);
        assert_eq!(checked.hypotheses().len(), 3);
        payload = Some(f.cas.export_witness(&checked, &mut quotient_budget()?)?);
    }
    Ok(())
}

/// Independent exact finite inverses, including negative units and all zero tails.
const fn vectors(kind: usize) -> (&'static [&'static str], &'static [&'static str]) {
    match kind {
        0 => (&["2"], &["1/2"]),
        1 => (&["1", "-1", "0", "0"], &["1", "1", "1", "1"]),
        2 => (&["2", "1", "0", "0"], &["1/2", "-1/4", "1/8", "-1/16"]),
        3 => (&["1", "-2", "1", "0"], &["1", "2", "3", "4"]),
        4 => (&["-2", "-1", "0", "0"], &["-1/2", "1/4", "-1/8", "1/16"]),
        _ => (
            &["1", "0", "-1/2", "0", "1/24", "0"],
            &["1", "0", "1/2", "0", "5/24", "0"],
        ),
    }
}

/// Build supplied rows from independent literals rather than the reciprocal producer.
fn supplied(kind: usize, reordered: bool) -> TestResult<crate::testing::series_proof::Fixture> {
    let mut f = fixture(0, reordered)?;
    let (p, q) = vectors(kind);
    let input = object(&mut f.base, &f.field, p)?;
    let output = object(&mut f.base, &f.field, q)?;
    let mut rows = Vec::new();
    for degree in 0..p.len() {
        let mut sum = rational("0")?;
        for i in 0..=degree {
            let left = rational(p[i])?;
            let right = rational(q[degree - i])?;
            let product = left.mul(&right, &mut default_budget()?)?;
            sum = sum.add(&product, &mut default_budget()?)?;
            rows.push(SeriesProductRow {
                degree: u32::try_from(degree)?,
                left_index: u32::try_from(i)?,
                right_index: u32::try_from(degree - i)?,
                left,
                right,
                product,
                sum: sum.clone(),
            });
        }
        assert_eq!(sum, rational(if degree == 0 { "1" } else { "0" })?);
    }
    let statement = Statement::Series(Box::new(S::Reciprocal { input, output }));
    f.expected.statement = statement.clone();
    f.steps = vec![ApplicationDraft {
        rule: Primitive::SeriesReciprocal,
        context: f.base.context.clone(),
        conclusion: statement,
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Series(Box::new(C::Reciprocal {
            form: 0,
            constant: rational(p[0])?,
            inverse: rational(q[0])?,
            rows,
        })),
    }];
    Ok(f)
}

/// Supplied proofs, production, sealing and fresh replay agree with independent finite vectors.
/// ´test:integration:series-reciprocal-independent-vectors´
#[test]
fn series_reciprocal_independent_vectors() -> TestResult {
    for kind in 0..6 {
        let mut f = supplied(kind, false)?;
        let witness = f
            .base
            .cas
            .check_proof(&f.base.context, &f.steps, &mut default_budget()?)?;
        let bytes = f
            .base
            .cas
            .export_witness(&witness, &mut default_budget()?)?;
        let Statement::Series(s) = &f.expected.statement else {
            return Err("series".into());
        };
        let S::Reciprocal { input, .. } = s.as_ref() else {
            return Err("reciprocal".into());
        };
        let produced = f.base.cas.compute_series(
            &O::Reciprocal {
                input: input.clone(),
            },
            &[],
            &mut default_budget()?,
        )?;
        assert_eq!(produced.judgment(), &f.expected);
        assert_eq!(
            f.base
                .cas
                .export_witness(&produced, &mut default_budget()?)?,
            bytes
        );
        for reordered in [false, true] {
            let mut receiver = supplied(kind, reordered)?;
            receiver
                .base
                .cas
                .replay(&bytes, &receiver.expected, &mut default_budget()?)?;
        }
    }
    Ok(())
}

/// Every scalar field and ordered zero row is checked, and failed publication rolls back.
/// ´test:integration:series-reciprocal-exact-row-faults´
#[test]
fn series_reciprocal_exact_row_faults() -> TestResult {
    for change in 0..17 {
        let mut f = supplied(2, false)?;
        let original = f
            .base
            .cas
            .check_proof(&f.base.context, &f.steps, &mut default_budget()?)?;
        let bytes = f
            .base
            .cas
            .export_witness(&original, &mut default_budget()?)?;
        let Certificate::Series(c) = &mut f.steps[0].certificate else {
            return Err("certificate".into());
        };
        let C::Reciprocal {
            constant,
            inverse,
            rows,
            ..
        } = c.as_mut()
        else {
            return Err("reciprocal".into());
        };
        match change {
            0 => *constant = rational("1")?,
            1 => *inverse = rational("-1/2")?,
            2 => rows[0].left = rational("1")?,
            3 => rows[0].right = rational("1")?,
            4 => rows[0].product = rational("2")?,
            5 => rows[0].sum = rational("2")?,
            6 => rows[0].degree = 1,
            7 => rows[0].left_index = 1,
            8 => rows[0].right_index = 1,
            9 => {
                rows.remove(5);
            }
            10 => rows.push(rows[0].clone()),
            11 => rows.swap(1, 2),
            12 => rows[9] = rows[8].clone(),
            _ => {
                let values: &[&str] = match change {
                    13 => &["-1/2", "-1/4", "1/8", "-1/16"],
                    14 => &["1/2", "-1/4", "1/8", "1/16"],
                    15 => &["1/2", "-1/4", "1/8"],
                    _ => &["1/2", "-1/4", "1/8", "-1/16", "0"],
                };
                let replacement = object(&mut f.base, &f.field, values)?;
                let Statement::Series(value) = &mut f.steps[0].conclusion else {
                    return Err("statement".into());
                };
                let S::Reciprocal { output, .. } = value.as_mut() else {
                    return Err("reciprocal".into());
                };
                *output = replacement;
            }
        }
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.base
                .cas
                .check_proof(&f.base.context, &f.steps, &mut default_budget()?)
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
                .export_witness(&original, &mut default_budget()?)?,
            bytes
        );
    }
    Ok(())
}

/// A zero input returns owned mathematical evidence; no inverse is evaluated by the producer.
/// ´test:integration:series-reciprocal-owned-nonunit´
#[test]
fn series_reciprocal_owned_nonunit() -> TestResult {
    for reordered in [false, true] {
        let mut f = supplied(1, reordered)?;
        let input = object(&mut f.base, &f.field, &["0", "1", "0"])?;
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        let fault = f
            .base
            .cas
            .compute_series(&O::Reciprocal { input }, &[], &mut default_budget()?)
            .expect_err("nonunit");
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::SeriesNonunitConstant)
        );
        assert!(fault.series_evidence.is_some());
        let retained = fault.clone();
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
        f.base.number("1234567")?;
        assert_eq!(fault.series_evidence, retained.series_evidence);
    }
    Ok(())
}

/// Fresh import retains descriptive identities across rollback and subsequent foreign attempts.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Independent integration oracles share this internal fixture across test modules"
)]
pub(crate) fn nonunit_wire_control(bytes: &[u8]) -> TestResult {
    let mut retained = None;
    for reversed in [false, true] {
        let mut f = supplied(0, reversed)?;
        let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
        let fault = f
            .base
            .cas
            .inspect(bytes, &mut default_budget()?)
            .expect_err("nonunit import");
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::SeriesNonunitConstant)
        );
        let evidence = fault.series_constant().ok_or("zero evidence")?;
        assert_eq!(evidence.index, 0);
        assert_eq!(evidence.value, rational("0")?);
        assert!(evidence.input.same_attempt(&evidence.coefficient));
        assert_eq!(
            f.base.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
        let clone = fault.clone();
        f.base.number("987654321")?;
        assert_eq!(fault, clone);
        if let Some(previous) = retained {
            assert_ne!(fault.series_evidence, Some(previous));
        }
        retained = fault.series_evidence;
        for kind in [
            nrz_cas::limits::BudgetKind::OutputBytes,
            nrz_cas::limits::BudgetKind::StagedPayloadBytes,
        ] {
            let (n, prefix, limit, needed) = if kind == nrz_cas::limits::BudgetKind::OutputBytes {
                (273, 25_614, 272, 273)
            } else if reversed {
                (13_624, 22_217, 13_111, 13_112)
            } else {
                (10_768, 22_217, 10_255, 10_256)
            };
            let mut success =
                nrz_cas::limits::Budget::new(crate::testing::half_pi_limits::ceiling(kind, n))?;
            assert_eq!(
                f.base
                    .cas
                    .inspect(bytes, &mut success)
                    .expect_err("nonunit at N")
                    .kind,
                FaultKind::Invalid(InvalidCode::SeriesNonunitConstant)
            );
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            let mut budget =
                nrz_cas::limits::Budget::new(crate::testing::half_pi_limits::ceiling(kind, n - 1))?;
            let fault = f
                .base
                .cas
                .inspect(bytes, &mut budget)
                .expect_err("diagnostic storage");
            assert_eq!(
                fault.kind,
                FaultKind::Exhausted {
                    reason: nrz_cas::Exhaustion::Limit,
                    budget: kind,
                    limit,
                    needed
                }
            );
            assert_eq!(budget.usage().work, prefix);
            assert!(fault.series_evidence.is_none());
            assert_eq!(
                f.base.cas.canonical_snapshot(&mut default_budget()?)?,
                before
            );
        }
    }
    Ok(())
}

/// Zero is refused before reading the deliberately false supplied inverse and rows.
/// ´test:integration:series-reciprocal-nonunit-ordering-budgets´
#[test]
fn series_reciprocal_nonunit_ordering_budgets() -> TestResult {
    use nrz_cas::limits::{Budget, BudgetKind as K};
    for reversed in [false, true] {
        for supplied_proof in [false, true] {
            let mut f = supplied(0, reversed)?;
            let input = object(&mut f.base, &f.field, &["0"])?;
            let Statement::Series(s) = &mut f.steps[0].conclusion else {
                return Err("inverse".into());
            };
            let S::Reciprocal { input: slot, .. } = s.as_mut() else {
                return Err("inverse".into());
            };
            *slot = input.clone();
            let before = f.base.cas.canonical_snapshot(&mut default_budget()?)?;
            for kind in [K::OutputBytes, K::StagedPayloadBytes] {
                let mut attempt = |amount| -> TestResult<(nrz_cas::Fault, u64)> {
                    let mut budget =
                        Budget::new(crate::testing::half_pi_limits::ceiling(kind, amount))?;
                    let fault = if supplied_proof {
                        f.base
                            .cas
                            .check_proof(&f.base.context, &f.steps, &mut budget)
                    } else {
                        f.base.cas.compute_series(
                            &O::Reciprocal {
                                input: input.clone(),
                            },
                            &[],
                            &mut budget,
                        )
                    }
                    .expect_err("zero constant");
                    Ok((fault, budget.usage().work))
                };
                let n = if kind == K::StagedPayloadBytes && !supplied_proof {
                    785
                } else {
                    273
                };
                let (fault, work) = attempt(n)?;
                assert_eq!(
                    fault.kind,
                    FaultKind::Invalid(InvalidCode::SeriesNonunitConstant)
                );
                assert_eq!(work, if supplied_proof { 277 } else { 533 });
                let evidence = fault.series_constant().ok_or("zero")?;
                assert_eq!(evidence.index, 0);
                assert_eq!(evidence.value, rational("0")?);
                let (failure, prefix) = attempt(n - 1)?;
                assert_eq!(
                    failure.kind,
                    FaultKind::Exhausted {
                        reason: nrz_cas::Exhaustion::Limit,
                        budget: kind,
                        limit: 272,
                        needed: 273
                    }
                );
                assert_eq!(prefix, if supplied_proof { 2 } else { 258 });
                assert!(failure.series_evidence.is_none());
            }
            assert_eq!(
                f.base.cas.canonical_snapshot(&mut default_budget()?)?,
                before
            );
        }
    }
    Ok(())
}

/// Measure complete public calls with fixed setup and rollback, retaining earlier witness bytes.
#[expect(
    clippy::too_many_lines,
    reason = "The complete fixture and exact boundary checks remain adjacent"
)]
fn resource_attempt(
    mode: usize,
    reversed: bool,
    kind: nrz_cas::limits::BudgetKind,
    amount: u64,
) -> TestResult<(bool, u64, Option<nrz_cas::Fault>)> {
    use nrz_cas::limits::{Budget, BudgetKind};
    let mut limits = crate::testing::half_pi_limits::ceiling(kind, amount);
    match kind {
        BudgetKind::JetEntries => limits.jet_entries = amount,
        BudgetKind::JetOrder => limits.jet_order = amount,
        _ => {}
    }
    let mut f = if mode == 5 {
        fixture(0, reversed)?
    } else {
        supplied(2, reversed)?
    };
    let Statement::Series(s) = &f.expected.statement else {
        return Err("series".into());
    };
    let operation = if mode == 5 {
        O::Reciprocal {
            input: object(&mut f.base, &f.field, &["2", "1", "0", "0"])?,
        }
    } else {
        let S::Reciprocal { input, .. } = s.as_ref() else {
            return Err("reciprocal".into());
        };
        O::Reciprocal {
            input: input.clone(),
        }
    };
    let saved = if (2..=4).contains(&mode) {
        Some(
            f.base
                .cas
                .check_proof(&f.base.context, &f.steps, &mut default_budget()?)?,
        )
    } else {
        None
    };
    let bytes = if let Some(saved) = &saved {
        f.base.cas.export_witness(saved, &mut default_budget()?)?
    } else {
        Vec::new()
    };
    let mut receiver = supplied(2, !reversed)?;
    let before = if mode == 4 {
        receiver
            .base
            .cas
            .canonical_snapshot(&mut default_budget()?)?
    } else {
        f.base.cas.canonical_snapshot(&mut default_budget()?)?
    };
    let mut budget = match Budget::new(limits) {
        Ok(budget) => budget,
        Err(fault) => return Ok((false, 0, Some(fault))),
    };
    budget.work(37, nrz_cas::Location::new("reciprocal prior spend"))?;
    let result = budget.with_retained_payload(
        31 + u64::try_from(bytes.len())?,
        nrz_cas::Location::new("reciprocal caller buffers"),
        |budget| match mode {
            0 => f
                .base
                .cas
                .check_proof(&f.base.context, &f.steps, budget)
                .map(|_| ()),
            1 | 2 | 5 => f
                .base
                .cas
                .compute_series(&operation, &[], budget)
                .map(|_| ()),
            3 => f
                .base
                .cas
                .export_witness(
                    saved.as_ref().ok_or_else(|| {
                        nrz_cas::Fault::invalid(
                            InvalidCode::Rule,
                            nrz_cas::Location::new("fixture"),
                        )
                    })?,
                    budget,
                )
                .map(|_| ()),
            _ => receiver
                .base
                .cas
                .replay(&bytes, &receiver.expected, budget)
                .map(|_| ()),
        },
    );
    if result.is_err() {
        let after = if mode == 4 {
            receiver
                .base
                .cas
                .canonical_snapshot(&mut default_budget()?)?
        } else {
            f.base.cas.canonical_snapshot(&mut default_budget()?)?
        };
        assert_eq!(before, after);
    }
    if let Some(saved) = &saved {
        assert_eq!(
            f.base.cas.export_witness(saved, &mut default_budget()?)?,
            bytes
        );
    }
    Ok((result.is_ok(), budget.usage().work, result.err()))
}

/// The exact arithmetic ceilings are independent of default-width workspace reservations.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Exact absent-output and residual solving boundary observations"
)]
fn series_reciprocal_remaining_resource_measurement() -> TestResult {
    use nrz_cas::limits::BudgetKind as K;
    for (mode, reversed, kind, [n, work, prefix, limit, needed], reason, exhausted) in [
        (
            4,
            false,
            K::SolveIdentities,
            [1, 1_424_037, 791_321, 0, 1],
            nrz_cas::Exhaustion::Limit,
            K::SolveIdentities,
        ),
        (
            4,
            false,
            K::IntegerExponent,
            [2, 1_424_037, 1134, 1, 2],
            nrz_cas::Exhaustion::Limit,
            K::IntegerExponent,
        ),
        (
            4,
            true,
            K::SolveIdentities,
            [1, 1_424_037, 791_321, 0, 1],
            nrz_cas::Exhaustion::Limit,
            K::SolveIdentities,
        ),
        (
            4,
            true,
            K::IntegerExponent,
            [2, 1_424_037, 1134, 1, 2],
            nrz_cas::Exhaustion::Limit,
            K::IntegerExponent,
        ),
        (
            5,
            false,
            K::SolveIdentities,
            [1, 4_417_387, 2_961_165, 0, 1],
            nrz_cas::Exhaustion::Limit,
            K::SolveIdentities,
        ),
        (
            5,
            false,
            K::IntegerExponent,
            [2, 4_417_387, 879, 1, 2],
            nrz_cas::Exhaustion::Limit,
            K::IntegerExponent,
        ),
        (
            5,
            true,
            K::SolveIdentities,
            [1, 4_417_387, 2_961_165, 0, 1],
            nrz_cas::Exhaustion::Limit,
            K::SolveIdentities,
        ),
        (
            5,
            true,
            K::IntegerExponent,
            [2, 4_417_387, 879, 1, 2],
            nrz_cas::Exhaustion::Limit,
            K::IntegerExponent,
        ),
        (
            6,
            false,
            K::Work,
            [159_660, 159_660, 159_656, 159_659, 159_660],
            nrz_cas::Exhaustion::Limit,
            K::Work,
        ),
        (
            6,
            false,
            K::JetEntries,
            [46, 159_660, 57_987, 45, 46],
            nrz_cas::Exhaustion::Limit,
            K::JetEntries,
        ),
        (
            6,
            false,
            K::JetOrder,
            [3, 159_660, 295, 2, 3],
            nrz_cas::Exhaustion::Limit,
            K::JetOrder,
        ),
        (
            6,
            false,
            K::ProofNodes,
            [1, 159_660, 63_766, 0, 1],
            nrz_cas::Exhaustion::Limit,
            K::ProofNodes,
        ),
        (
            6,
            false,
            K::Depth,
            [1, 159_660, 18_027, 0, 1],
            nrz_cas::Exhaustion::Limit,
            K::Depth,
        ),
        (
            6,
            false,
            K::ConditionSteps,
            [1, 159_660, 57_986, 0, 1],
            nrz_cas::Exhaustion::Limit,
            K::ConditionSteps,
        ),
        (
            6,
            false,
            K::StagedPayloadBytes,
            [268_535, 159_660, 34_700, 7, 8],
            nrz_cas::Exhaustion::Limit,
            K::StagedPayloadBytes,
        ),
        (
            6,
            false,
            K::PayloadBytes,
            [12_389, 159_660, 63_766, 12_388, 12_389],
            nrz_cas::Exhaustion::Limit,
            K::PayloadBytes,
        ),
        (
            6,
            false,
            K::WitnessBytes,
            [1464, 159_660, 159_656, 1463, 1464],
            nrz_cas::Exhaustion::Limit,
            K::WitnessBytes,
        ),
        (
            6,
            false,
            K::OutputBytes,
            [2388, 159_660, 62_406, 2387, 2388],
            nrz_cas::Exhaustion::Limit,
            K::OutputBytes,
        ),
        (
            6,
            false,
            K::ArithmeticSteps,
            [1818, 159_660, 154_187, 1817, 1818],
            nrz_cas::Exhaustion::Limit,
            K::ArithmeticSteps,
        ),
        (
            6,
            false,
            K::ComponentBits,
            [5, 159_660, 8090, 4, 5],
            nrz_cas::Exhaustion::ArithmeticRange,
            K::ComponentBits,
        ),
        (
            6,
            false,
            K::IntermediateBits,
            [6, 159_660, 7898, 5, 6],
            nrz_cas::Exhaustion::ArithmeticRange,
            K::IntermediateBits,
        ),
        (
            6,
            false,
            K::ReplayWork,
            [0, 159_660, 0, 0, 0],
            nrz_cas::Exhaustion::Limit,
            K::ReplayWork,
        ),
        (
            6,
            true,
            K::Work,
            [159_660, 159_660, 159_656, 159_659, 159_660],
            nrz_cas::Exhaustion::Limit,
            K::Work,
        ),
        (
            6,
            true,
            K::JetEntries,
            [46, 159_660, 57_987, 45, 46],
            nrz_cas::Exhaustion::Limit,
            K::JetEntries,
        ),
        (
            6,
            true,
            K::JetOrder,
            [3, 159_660, 295, 2, 3],
            nrz_cas::Exhaustion::Limit,
            K::JetOrder,
        ),
        (
            6,
            true,
            K::ProofNodes,
            [1, 159_660, 63_766, 0, 1],
            nrz_cas::Exhaustion::Limit,
            K::ProofNodes,
        ),
        (
            6,
            true,
            K::Depth,
            [1, 159_660, 18_027, 0, 1],
            nrz_cas::Exhaustion::Limit,
            K::Depth,
        ),
        (
            6,
            true,
            K::ConditionSteps,
            [1, 159_660, 57_986, 0, 1],
            nrz_cas::Exhaustion::Limit,
            K::ConditionSteps,
        ),
        (
            6,
            true,
            K::StagedPayloadBytes,
            [268_535, 159_660, 34_700, 7, 8],
            nrz_cas::Exhaustion::Limit,
            K::StagedPayloadBytes,
        ),
        (
            6,
            true,
            K::PayloadBytes,
            [15_245, 159_660, 63_766, 15_244, 15_245],
            nrz_cas::Exhaustion::Limit,
            K::PayloadBytes,
        ),
        (
            6,
            true,
            K::WitnessBytes,
            [1464, 159_660, 159_656, 1463, 1464],
            nrz_cas::Exhaustion::Limit,
            K::WitnessBytes,
        ),
        (
            6,
            true,
            K::OutputBytes,
            [2388, 159_660, 62_406, 2387, 2388],
            nrz_cas::Exhaustion::Limit,
            K::OutputBytes,
        ),
        (
            6,
            true,
            K::ArithmeticSteps,
            [1818, 159_660, 154_187, 1817, 1818],
            nrz_cas::Exhaustion::Limit,
            K::ArithmeticSteps,
        ),
        (
            6,
            true,
            K::ComponentBits,
            [5, 159_660, 8090, 4, 5],
            nrz_cas::Exhaustion::ArithmeticRange,
            K::ComponentBits,
        ),
        (
            6,
            true,
            K::IntermediateBits,
            [6, 159_660, 7898, 5, 6],
            nrz_cas::Exhaustion::ArithmeticRange,
            K::IntermediateBits,
        ),
        (
            6,
            true,
            K::ReplayWork,
            [0, 159_660, 0, 0, 0],
            nrz_cas::Exhaustion::Limit,
            K::ReplayWork,
        ),
    ] {
        let attempt = |n| {
            if mode == 6 {
                resource_attempt(5, reversed, kind, n)
            } else {
                quotient_resource_attempt(mode, reversed, false, kind, n)
            }
        };
        let success = attempt(n)?;
        assert!(success.0);
        assert_eq!(success.1, work);
        if n > 0 {
            let failure = attempt(n - 1)?;
            assert!(!failure.0);
            assert_eq!(failure.1, prefix);
            assert_eq!(
                failure.2.ok_or("exhaustion")?.kind,
                FaultKind::Exhausted {
                    reason,
                    budget: exhausted,
                    limit,
                    needed
                }
            );
        }
    }
    Ok(())
}

/// Measure one fresh production with fixed order, rational width and allocation order.
fn growth_attempt(
    order: usize,
    width: usize,
    reversed: bool,
    kind: nrz_cas::limits::BudgetKind,
    n: u64,
) -> TestResult<(bool, u64, Option<nrz_cas::Fault>)> {
    use nrz_cas::limits::{Budget, BudgetKind};
    let (constant, inverse) = match width {
        0 => ("2/3", "3/2"),
        1 => (
            "18446744073709551617/18446744073709551619",
            "18446744073709551619/18446744073709551617",
        ),
        _ => (
            "340282366920938463463374607431768211457/340282366920938463463374607431768211459",
            "340282366920938463463374607431768211459/340282366920938463463374607431768211457",
        ),
    };
    let negative = format!("-{constant}");
    let mut values = vec!["0"; order];
    values[0] = constant;
    if order > 1 {
        values[1] = &negative;
    }
    let mut f = ResidualFixture::new(&values, "0", reversed, false)?;
    let input = f.object(&f.candidate.clone())?;
    let before = f.cas.canonical_snapshot(&mut quotient_budget()?)?;
    let mut limits = crate::testing::half_pi_limits::ceiling(kind, n);
    if kind != BudgetKind::Work {
        limits.work = 20_000_000;
    }
    let mut budget = Budget::new(limits)?;
    budget.work(37, nrz_cas::Location::new("growth prior spend"))?;
    let result =
        budget.with_retained_payload(31, nrz_cas::Location::new("growth caller payload"), |b| {
            f.cas.compute_series(
                &O::Reciprocal {
                    input: input.clone(),
                },
                &[],
                b,
            )
        });
    match result {
        Ok(witness) => {
            let expected_values = vec![rational(inverse)?; order];
            let output = f.object(&expected_values)?;
            assert_eq!(
                witness.judgment().statement,
                Statement::Series(Box::new(S::Reciprocal { input, output }))
            );
            let bytes = f.cas.export_witness(&witness, &mut quotient_budget()?)?;
            let mut receiver = ResidualFixture::new(&values, "0", !reversed, false)?;
            let input = receiver.object(&receiver.candidate.clone())?;
            let output = receiver.object(&expected_values)?;
            let expected = Judgment {
                context: receiver.context.clone(),
                layer: nrz_cas::domain::Layer::Logic,
                space: None,
                statement: Statement::Series(Box::new(S::Reciprocal { input, output })),
                residuals: Vec::new(),
            };
            receiver
                .cas
                .replay(&bytes, &expected, &mut quotient_budget()?)?;
            Ok((true, budget.usage().work, None))
        }
        Err(fault) => {
            assert_eq!(f.cas.canonical_snapshot(&mut quotient_budget()?)?, before);
            Ok((false, budget.usage().work, Some(fault)))
        }
    }
}

/// Vary only positive order or exact numerator/denominator width, with absent candidate syntax.
/// ´test:integration:series-reciprocal-resource-growth´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Measured order and rational-width boundaries"
)]
fn series_reciprocal_resource_growth() -> TestResult {
    use nrz_cas::limits::BudgetKind as K;
    for (order, width, reversed, kind, [n, work, prefix, limit, needed], reason, exhausted) in [
        (
            1,
            0,
            false,
            K::Work,
            [50_693, 50_693, 50_689, 50_692, 50_693],
            nrz_cas::Exhaustion::Limit,
            K::Work,
        ),
        (
            1,
            0,
            false,
            K::StagedPayloadBytes,
            [79_667, 50_693, 6_453, 7, 8],
            nrz_cas::Exhaustion::Limit,
            K::StagedPayloadBytes,
        ),
        (
            1,
            0,
            true,
            K::Work,
            [50_693, 50_693, 50_689, 50_692, 50_693],
            nrz_cas::Exhaustion::Limit,
            K::Work,
        ),
        (
            1,
            0,
            true,
            K::StagedPayloadBytes,
            [79_667, 50_693, 6_453, 7, 8],
            nrz_cas::Exhaustion::Limit,
            K::StagedPayloadBytes,
        ),
        (
            1,
            1,
            false,
            K::Work,
            [64_165, 64_165, 64_161, 64_164, 64_165],
            nrz_cas::Exhaustion::Limit,
            K::Work,
        ),
        (
            1,
            1,
            false,
            K::StagedPayloadBytes,
            [79_859, 64_165, 8_645, 7, 8],
            nrz_cas::Exhaustion::Limit,
            K::StagedPayloadBytes,
        ),
        (
            1,
            1,
            true,
            K::Work,
            [64_165, 64_165, 64_161, 64_164, 64_165],
            nrz_cas::Exhaustion::Limit,
            K::Work,
        ),
        (
            1,
            1,
            true,
            K::StagedPayloadBytes,
            [79_859, 64_165, 8_645, 7, 8],
            nrz_cas::Exhaustion::Limit,
            K::StagedPayloadBytes,
        ),
        (
            1,
            2,
            false,
            K::Work,
            [79_429, 79_429, 79_425, 79_428, 79_429],
            nrz_cas::Exhaustion::Limit,
            K::Work,
        ),
        (
            1,
            2,
            false,
            K::StagedPayloadBytes,
            [80_051, 79_429, 11_413, 7, 8],
            nrz_cas::Exhaustion::Limit,
            K::StagedPayloadBytes,
        ),
        (
            1,
            2,
            true,
            K::Work,
            [79_429, 79_429, 79_425, 79_428, 79_429],
            nrz_cas::Exhaustion::Limit,
            K::Work,
        ),
        (
            1,
            2,
            true,
            K::StagedPayloadBytes,
            [80_051, 79_429, 11_413, 7, 8],
            nrz_cas::Exhaustion::Limit,
            K::StagedPayloadBytes,
        ),
        (
            4,
            0,
            false,
            K::Work,
            [156_336, 156_336, 156_332, 156_335, 156_336],
            nrz_cas::Exhaustion::Limit,
            K::Work,
        ),
        (
            4,
            0,
            false,
            K::StagedPayloadBytes,
            [268_535, 156_336, 33_201, 7, 8],
            nrz_cas::Exhaustion::Limit,
            K::StagedPayloadBytes,
        ),
        (
            4,
            0,
            true,
            K::Work,
            [156_336, 156_336, 156_332, 156_335, 156_336],
            nrz_cas::Exhaustion::Limit,
            K::Work,
        ),
        (
            4,
            0,
            true,
            K::StagedPayloadBytes,
            [268_535, 156_336, 33_201, 7, 8],
            nrz_cas::Exhaustion::Limit,
            K::StagedPayloadBytes,
        ),
        (
            4,
            1,
            false,
            K::Work,
            [220_952, 220_952, 220_948, 220_951, 220_952],
            nrz_cas::Exhaustion::Limit,
            K::Work,
        ),
        (
            4,
            1,
            false,
            K::StagedPayloadBytes,
            [269_447, 220_952, 46_337, 7, 8],
            nrz_cas::Exhaustion::Limit,
            K::StagedPayloadBytes,
        ),
        (
            4,
            1,
            true,
            K::Work,
            [220_952, 220_952, 220_948, 220_951, 220_952],
            nrz_cas::Exhaustion::Limit,
            K::Work,
        ),
        (
            4,
            1,
            true,
            K::StagedPayloadBytes,
            [269_447, 220_952, 46_337, 7, 8],
            nrz_cas::Exhaustion::Limit,
            K::StagedPayloadBytes,
        ),
        (
            4,
            2,
            false,
            K::Work,
            [295_272, 295_272, 295_268, 295_271, 295_272],
            nrz_cas::Exhaustion::Limit,
            K::Work,
        ),
        (
            4,
            2,
            false,
            K::StagedPayloadBytes,
            [270_359, 295_272, 63_409, 7, 8],
            nrz_cas::Exhaustion::Limit,
            K::StagedPayloadBytes,
        ),
        (
            4,
            2,
            true,
            K::Work,
            [295_272, 295_272, 295_268, 295_271, 295_272],
            nrz_cas::Exhaustion::Limit,
            K::Work,
        ),
        (
            4,
            2,
            true,
            K::StagedPayloadBytes,
            [270_359, 295_272, 63_409, 7, 8],
            nrz_cas::Exhaustion::Limit,
            K::StagedPayloadBytes,
        ),
        (
            6,
            0,
            false,
            K::Work,
            [279_141, 279_141, 279_137, 279_140, 279_141],
            nrz_cas::Exhaustion::Limit,
            K::Work,
        ),
        (
            6,
            0,
            false,
            K::StagedPayloadBytes,
            [484_842, 279_141, 64_608, 7, 8],
            nrz_cas::Exhaustion::Limit,
            K::StagedPayloadBytes,
        ),
        (
            6,
            0,
            true,
            K::Work,
            [279_141, 279_141, 279_137, 279_140, 279_141],
            nrz_cas::Exhaustion::Limit,
            K::Work,
        ),
        (
            6,
            0,
            true,
            K::StagedPayloadBytes,
            [484_842, 279_141, 64_608, 7, 8],
            nrz_cas::Exhaustion::Limit,
            K::StagedPayloadBytes,
        ),
        (
            6,
            1,
            false,
            K::Work,
            [395_653, 395_653, 395_649, 395_652, 395_653],
            nrz_cas::Exhaustion::Limit,
            K::Work,
        ),
        (
            6,
            1,
            false,
            K::StagedPayloadBytes,
            [486_474, 395_653, 89_480, 7, 8],
            nrz_cas::Exhaustion::Limit,
            K::StagedPayloadBytes,
        ),
        (
            6,
            1,
            true,
            K::Work,
            [395_653, 395_653, 395_649, 395_652, 395_653],
            nrz_cas::Exhaustion::Limit,
            K::Work,
        ),
        (
            6,
            1,
            true,
            K::StagedPayloadBytes,
            [486_474, 395_653, 89_480, 7, 8],
            nrz_cas::Exhaustion::Limit,
            K::StagedPayloadBytes,
        ),
        (
            6,
            2,
            false,
            K::Work,
            [530_293, 530_293, 530_289, 530_292, 530_293],
            nrz_cas::Exhaustion::Limit,
            K::Work,
        ),
        (
            6,
            2,
            false,
            K::StagedPayloadBytes,
            [488_106, 530_293, 122_048, 7, 8],
            nrz_cas::Exhaustion::Limit,
            K::StagedPayloadBytes,
        ),
        (
            6,
            2,
            true,
            K::Work,
            [530_293, 530_293, 530_289, 530_292, 530_293],
            nrz_cas::Exhaustion::Limit,
            K::Work,
        ),
        (
            6,
            2,
            true,
            K::StagedPayloadBytes,
            [488_106, 530_293, 122_048, 7, 8],
            nrz_cas::Exhaustion::Limit,
            K::StagedPayloadBytes,
        ),
    ] {
        let success = growth_attempt(order, width, reversed, kind, n)?;
        assert!(success.0);
        assert_eq!(success.1, work);
        let failure = growth_attempt(order, width, reversed, kind, n - 1)?;
        assert!(!failure.0);
        assert_eq!(failure.1, prefix);
        assert_eq!(
            failure.2.ok_or("growth exhaustion")?.kind,
            FaultKind::Exhausted {
                reason,
                budget: exhausted,
                limit,
                needed
            }
        );
    }
    Ok(())
}

/// The exact arithmetic ceilings are independent of default-width workspace reservations.
/// ´test:integration:series-reciprocal-arithmetic-resource-measurement´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "All measured exact arithmetic boundaries remain explicit"
)]
fn series_reciprocal_arithmetic_resource_measurement() -> TestResult {
    use nrz_cas::limits::BudgetKind as K;
    for (mode, reversed, kind, n, work, prefix, expected) in [
        (
            0,
            false,
            K::ArithmeticSteps,
            1363,
            146_744,
            141_271,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::Limit,
                budget: K::ArithmeticSteps,
                limit: 1362,
                needed: 1363,
            }),
        ),
        (
            0,
            false,
            K::ComponentBits,
            5,
            146_744,
            1285,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::ArithmeticRange,
                budget: K::ComponentBits,
                limit: 4,
                needed: 5,
            }),
        ),
        (
            0,
            false,
            K::IntermediateBits,
            5,
            146_744,
            1285,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::ArithmeticRange,
                budget: K::ComponentBits,
                limit: 4,
                needed: 5,
            }),
        ),
        (0, false, K::ReplayWork, 0, 146_744, 0, None),
        (
            0,
            true,
            K::ArithmeticSteps,
            1363,
            146_744,
            141_271,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::Limit,
                budget: K::ArithmeticSteps,
                limit: 1362,
                needed: 1363,
            }),
        ),
        (
            0,
            true,
            K::ComponentBits,
            5,
            146_744,
            1285,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::ArithmeticRange,
                budget: K::ComponentBits,
                limit: 4,
                needed: 5,
            }),
        ),
        (
            0,
            true,
            K::IntermediateBits,
            5,
            146_744,
            1285,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::ArithmeticRange,
                budget: K::ComponentBits,
                limit: 4,
                needed: 5,
            }),
        ),
        (0, true, K::ReplayWork, 0, 146_744, 0, None),
        (
            1,
            false,
            K::ArithmeticSteps,
            1814,
            159_112,
            153_639,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::Limit,
                budget: K::ArithmeticSteps,
                limit: 1813,
                needed: 1814,
            }),
        ),
        (
            1,
            false,
            K::ComponentBits,
            5,
            159_112,
            8090,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::ArithmeticRange,
                budget: K::ComponentBits,
                limit: 4,
                needed: 5,
            }),
        ),
        (
            1,
            false,
            K::IntermediateBits,
            6,
            159_112,
            7898,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::ArithmeticRange,
                budget: K::IntermediateBits,
                limit: 5,
                needed: 6,
            }),
        ),
        (1, false, K::ReplayWork, 0, 159_112, 0, None),
        (
            1,
            true,
            K::ArithmeticSteps,
            1814,
            159_112,
            153_639,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::Limit,
                budget: K::ArithmeticSteps,
                limit: 1813,
                needed: 1814,
            }),
        ),
        (
            1,
            true,
            K::ComponentBits,
            5,
            159_112,
            8090,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::ArithmeticRange,
                budget: K::ComponentBits,
                limit: 4,
                needed: 5,
            }),
        ),
        (
            1,
            true,
            K::IntermediateBits,
            6,
            159_112,
            7898,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::ArithmeticRange,
                budget: K::IntermediateBits,
                limit: 5,
                needed: 6,
            }),
        ),
        (1, true, K::ReplayWork, 0, 159_112, 0, None),
        (
            2,
            false,
            K::ArithmeticSteps,
            1814,
            159_112,
            153_639,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::Limit,
                budget: K::ArithmeticSteps,
                limit: 1813,
                needed: 1814,
            }),
        ),
        (
            2,
            false,
            K::ComponentBits,
            5,
            159_112,
            8090,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::ArithmeticRange,
                budget: K::ComponentBits,
                limit: 4,
                needed: 5,
            }),
        ),
        (
            2,
            false,
            K::IntermediateBits,
            6,
            159_112,
            7898,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::ArithmeticRange,
                budget: K::IntermediateBits,
                limit: 5,
                needed: 6,
            }),
        ),
        (2, false, K::ReplayWork, 0, 159_112, 0, None),
        (
            2,
            true,
            K::ArithmeticSteps,
            1814,
            159_112,
            153_639,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::Limit,
                budget: K::ArithmeticSteps,
                limit: 1813,
                needed: 1814,
            }),
        ),
        (
            2,
            true,
            K::ComponentBits,
            5,
            159_112,
            8090,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::ArithmeticRange,
                budget: K::ComponentBits,
                limit: 4,
                needed: 5,
            }),
        ),
        (
            2,
            true,
            K::IntermediateBits,
            6,
            159_112,
            7898,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::ArithmeticRange,
                budget: K::IntermediateBits,
                limit: 5,
                needed: 6,
            }),
        ),
        (2, true, K::ReplayWork, 0, 159_112, 0, None),
        (3, false, K::ArithmeticSteps, 0, 1501, 0, None),
        (
            3,
            false,
            K::ComponentBits,
            1,
            1501,
            0,
            Some(FaultKind::Invalid(InvalidCode::Policy)),
        ),
        (
            3,
            false,
            K::IntermediateBits,
            1,
            1501,
            0,
            Some(FaultKind::Invalid(InvalidCode::Policy)),
        ),
        (3, false, K::ReplayWork, 0, 1501, 0, None),
        (3, true, K::ArithmeticSteps, 0, 1501, 0, None),
        (
            3,
            true,
            K::ComponentBits,
            1,
            1501,
            0,
            Some(FaultKind::Invalid(InvalidCode::Policy)),
        ),
        (
            3,
            true,
            K::IntermediateBits,
            1,
            1501,
            0,
            Some(FaultKind::Invalid(InvalidCode::Policy)),
        ),
        (3, true, K::ReplayWork, 0, 1501, 0, None),
        (
            4,
            false,
            K::ArithmeticSteps,
            1_604,
            204_103,
            198_552,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::Limit,
                budget: K::ArithmeticSteps,
                limit: 1_603,
                needed: 1_604,
            }),
        ),
        (
            4,
            false,
            K::ComponentBits,
            5,
            204_103,
            27_494,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::ArithmeticRange,
                budget: K::ComponentBits,
                limit: 4,
                needed: 5,
            }),
        ),
        (
            4,
            false,
            K::IntermediateBits,
            5,
            204_103,
            27_494,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::ArithmeticRange,
                budget: K::ComponentBits,
                limit: 4,
                needed: 5,
            }),
        ),
        (
            4,
            false,
            K::ReplayWork,
            204_066,
            204_103,
            204_066,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::Limit,
                budget: K::ReplayWork,
                limit: 204_065,
                needed: 204_066,
            }),
        ),
        (
            4,
            true,
            K::ArithmeticSteps,
            1_604,
            204_103,
            198_552,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::Limit,
                budget: K::ArithmeticSteps,
                limit: 1_603,
                needed: 1_604,
            }),
        ),
        (
            4,
            true,
            K::ComponentBits,
            5,
            204_103,
            27_494,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::ArithmeticRange,
                budget: K::ComponentBits,
                limit: 4,
                needed: 5,
            }),
        ),
        (
            4,
            true,
            K::IntermediateBits,
            5,
            204_103,
            27_494,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::ArithmeticRange,
                budget: K::ComponentBits,
                limit: 4,
                needed: 5,
            }),
        ),
        (
            4,
            true,
            K::ReplayWork,
            204_066,
            204_103,
            204_066,
            Some(FaultKind::Exhausted {
                reason: nrz_cas::Exhaustion::Limit,
                budget: K::ReplayWork,
                limit: 204_065,
                needed: 204_066,
            }),
        ),
    ] {
        let success = resource_attempt(mode, reversed, kind, n)?;
        assert!(success.0);
        assert_eq!(success.1, work);
        if n > 0 {
            let failure = resource_attempt(mode, reversed, kind, n - 1)?;
            assert!(!failure.0);
            assert_eq!(failure.1, prefix);
            assert_eq!(failure.2.map(|fault| fault.kind), expected);
        }
    }
    Ok(())
}

/// Complete-operation minima measured independently on the frozen compiler.
/// Each N succeeds and N-1 pins the full exhaustion and the consumed Work prefix.
/// ´test:integration:series-reciprocal-resource-measurement´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "The complete fixture and exact boundary checks remain adjacent"
)]
fn series_reciprocal_resource_measurement() -> TestResult {
    use nrz_cas::limits::BudgetKind as K;
    for (mode, reversed, kind, n, work, prefix, limit, needed) in [
        (
            0,
            false,
            K::Work,
            146_744,
            146_744,
            146_740,
            146_743,
            146_744,
        ),
        (0, false, K::JetEntries, 25, 146_744, 45_071, 24, 25),
        (0, false, K::JetOrder, 3, 146_744, 19_844, 2, 3),
        (0, false, K::ProofNodes, 1, 146_744, 37, 0, 1),
        (0, false, K::Depth, 1, 146_744, 45_070, 0, 1),
        (0, false, K::ConditionSteps, 1, 146_744, 45_070, 0, 1),
        (
            0,
            false,
            K::StagedPayloadBytes,
            135_399,
            146_744,
            21_784,
            7,
            8,
        ),
        (
            0,
            false,
            K::PayloadBytes,
            12_389,
            146_744,
            50_850,
            12_388,
            12_389,
        ),
        (
            0,
            false,
            K::WitnessBytes,
            1464,
            146_744,
            146_740,
            1463,
            1464,
        ),
        (0, false, K::OutputBytes, 2388, 146_744, 49_490, 2387, 2388),
        (
            0,
            true,
            K::Work,
            146_744,
            146_744,
            146_740,
            146_743,
            146_744,
        ),
        (0, true, K::JetEntries, 25, 146_744, 45_071, 24, 25),
        (0, true, K::JetOrder, 3, 146_744, 19_844, 2, 3),
        (0, true, K::ProofNodes, 1, 146_744, 37, 0, 1),
        (0, true, K::Depth, 1, 146_744, 45_070, 0, 1),
        (0, true, K::ConditionSteps, 1, 146_744, 45_070, 0, 1),
        (
            0,
            true,
            K::StagedPayloadBytes,
            135_399,
            146_744,
            21_784,
            7,
            8,
        ),
        (
            0,
            true,
            K::PayloadBytes,
            15_245,
            146_744,
            50_850,
            15_244,
            15_245,
        ),
        (0, true, K::WitnessBytes, 1464, 146_744, 146_740, 1463, 1464),
        (0, true, K::OutputBytes, 2388, 146_744, 49_490, 2387, 2388),
        (
            1,
            false,
            K::Work,
            159_112,
            159_112,
            159_108,
            159_111,
            159_112,
        ),
        (1, false, K::JetEntries, 46, 159_112, 57_439, 45, 46),
        (1, false, K::JetOrder, 3, 159_112, 295, 2, 3),
        (1, false, K::ProofNodes, 1, 159_112, 63_218, 0, 1),
        (1, false, K::Depth, 1, 159_112, 18_027, 0, 1),
        (1, false, K::ConditionSteps, 1, 159_112, 57_438, 0, 1),
        (
            1,
            false,
            K::StagedPayloadBytes,
            268_535,
            159_112,
            34_152,
            7,
            8,
        ),
        (
            1,
            false,
            K::PayloadBytes,
            12_389,
            159_112,
            63_218,
            12_388,
            12_389,
        ),
        (
            1,
            false,
            K::WitnessBytes,
            1464,
            159_112,
            159_108,
            1463,
            1464,
        ),
        (1, false, K::OutputBytes, 2388, 159_112, 61_858, 2387, 2388),
        (
            1,
            true,
            K::Work,
            159_112,
            159_112,
            159_108,
            159_111,
            159_112,
        ),
        (1, true, K::JetEntries, 46, 159_112, 57_439, 45, 46),
        (1, true, K::JetOrder, 3, 159_112, 295, 2, 3),
        (1, true, K::ProofNodes, 1, 159_112, 63_218, 0, 1),
        (1, true, K::Depth, 1, 159_112, 18_027, 0, 1),
        (1, true, K::ConditionSteps, 1, 159_112, 57_438, 0, 1),
        (
            1,
            true,
            K::StagedPayloadBytes,
            268_535,
            159_112,
            34_152,
            7,
            8,
        ),
        (
            1,
            true,
            K::PayloadBytes,
            15_245,
            159_112,
            63_218,
            15_244,
            15_245,
        ),
        (1, true, K::WitnessBytes, 1464, 159_112, 159_108, 1463, 1464),
        (1, true, K::OutputBytes, 2388, 159_112, 61_858, 2387, 2388),
        (
            2,
            false,
            K::Work,
            159_112,
            159_112,
            159_108,
            159_111,
            159_112,
        ),
        (2, false, K::JetEntries, 46, 159_112, 57_439, 45, 46),
        (2, false, K::JetOrder, 3, 159_112, 295, 2, 3),
        (2, false, K::ProofNodes, 1, 159_112, 63_218, 0, 1),
        (2, false, K::Depth, 1, 159_112, 18_027, 0, 1),
        (2, false, K::ConditionSteps, 1, 159_112, 57_438, 0, 1),
        (
            2,
            false,
            K::StagedPayloadBytes,
            272_515,
            159_112,
            34_152,
            7,
            8,
        ),
        (2, false, K::PayloadBytes, 669, 159_112, 61_859, 668, 669),
        (
            2,
            false,
            K::WitnessBytes,
            1464,
            159_112,
            159_108,
            1463,
            1464,
        ),
        (2, false, K::OutputBytes, 4776, 159_112, 61_858, 4775, 4776),
        (
            2,
            true,
            K::Work,
            159_112,
            159_112,
            159_108,
            159_111,
            159_112,
        ),
        (2, true, K::JetEntries, 46, 159_112, 57_439, 45, 46),
        (2, true, K::JetOrder, 3, 159_112, 295, 2, 3),
        (2, true, K::ProofNodes, 1, 159_112, 63_218, 0, 1),
        (2, true, K::Depth, 1, 159_112, 18_027, 0, 1),
        (2, true, K::ConditionSteps, 1, 159_112, 57_438, 0, 1),
        (
            2,
            true,
            K::StagedPayloadBytes,
            272_515,
            159_112,
            34_152,
            7,
            8,
        ),
        (2, true, K::PayloadBytes, 669, 159_112, 61_859, 668, 669),
        (2, true, K::WitnessBytes, 1464, 159_112, 159_108, 1463, 1464),
        (2, true, K::OutputBytes, 4776, 159_112, 61_858, 4775, 4776),
        (3, false, K::Work, 1501, 1501, 37, 1500, 1501),
        (3, false, K::JetEntries, 0, 1501, 0, 0, 0),
        (3, false, K::JetOrder, 0, 1501, 0, 0, 0),
        (3, false, K::ProofNodes, 0, 1501, 0, 0, 0),
        (3, false, K::Depth, 0, 1501, 0, 0, 0),
        (3, false, K::ConditionSteps, 0, 1501, 0, 0, 0),
        (3, false, K::StagedPayloadBytes, 2959, 1501, 37, 1463, 1464),
        (3, false, K::PayloadBytes, 0, 1501, 0, 0, 0),
        (3, false, K::WitnessBytes, 1464, 1501, 37, 1463, 1464),
        (3, false, K::OutputBytes, 1464, 1501, 37, 1463, 1464),
        (3, true, K::Work, 1501, 1501, 37, 1500, 1501),
        (3, true, K::JetEntries, 0, 1501, 0, 0, 0),
        (3, true, K::JetOrder, 0, 1501, 0, 0, 0),
        (3, true, K::ProofNodes, 0, 1501, 0, 0, 0),
        (3, true, K::Depth, 0, 1501, 0, 0, 0),
        (3, true, K::ConditionSteps, 0, 1501, 0, 0, 0),
        (3, true, K::StagedPayloadBytes, 2959, 1501, 37, 1463, 1464),
        (3, true, K::PayloadBytes, 0, 1501, 0, 0, 0),
        (3, true, K::WitnessBytes, 1464, 1501, 37, 1463, 1464),
        (3, true, K::OutputBytes, 1464, 1501, 37, 1463, 1464),
        (
            4,
            false,
            K::Work,
            204_103,
            204_103,
            204_066,
            204_102,
            204_103,
        ),
        (4, false, K::JetEntries, 35, 204_103, 95_484, 34, 35),
        (4, false, K::JetOrder, 3, 204_103, 39, 2, 3),
        (4, false, K::ProofNodes, 1, 204_103, 325, 0, 1),
        (4, false, K::Depth, 1, 204_103, 16_264, 0, 1),
        (4, false, K::ConditionSteps, 1, 204_103, 95_483, 0, 1),
        (
            4,
            false,
            K::StagedPayloadBytes,
            136_863,
            204_103,
            72_197,
            7,
            8,
        ),
        (
            4,
            false,
            K::PayloadBytes,
            15_245,
            204_103,
            101_263,
            15_244,
            15_245,
        ),
        (4, false, K::WitnessBytes, 1_464, 204_103, 313, 1_463, 1_464),
        (
            4,
            false,
            K::OutputBytes,
            2_388,
            204_103,
            99_903,
            2_387,
            2_388,
        ),
        (
            4,
            true,
            K::Work,
            204_103,
            204_103,
            204_066,
            204_102,
            204_103,
        ),
        (4, true, K::JetEntries, 35, 204_103, 95_484, 34, 35),
        (4, true, K::JetOrder, 3, 204_103, 39, 2, 3),
        (4, true, K::ProofNodes, 1, 204_103, 325, 0, 1),
        (4, true, K::Depth, 1, 204_103, 16_264, 0, 1),
        (4, true, K::ConditionSteps, 1, 204_103, 95_483, 0, 1),
        (
            4,
            true,
            K::StagedPayloadBytes,
            136_863,
            204_103,
            72_197,
            7,
            8,
        ),
        (
            4,
            true,
            K::PayloadBytes,
            12_389,
            204_103,
            101_263,
            12_388,
            12_389,
        ),
        (4, true, K::WitnessBytes, 1_464, 204_103, 313, 1_463, 1_464),
        (
            4,
            true,
            K::OutputBytes,
            2_388,
            204_103,
            99_903,
            2_387,
            2_388,
        ),
    ] {
        let success = resource_attempt(mode, reversed, kind, n)?;
        assert!(
            success.0,
            "{mode} {reversed} {kind:?} N={n}: {:?}",
            success.2
        );
        assert_eq!(success.1, work);
        if n > 0 {
            let failure = resource_attempt(mode, reversed, kind, n - 1)?;
            assert!(!failure.0);
            assert_eq!(failure.1, prefix);
            assert_eq!(
                failure.2.ok_or("missing exhaustion")?.kind,
                FaultKind::Exhausted {
                    reason: nrz_cas::Exhaustion::Limit,
                    budget: kind,
                    limit,
                    needed,
                }
            );
        }
    }
    Ok(())
}
