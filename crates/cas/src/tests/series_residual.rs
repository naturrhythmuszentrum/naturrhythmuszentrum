// SPDX-License-Identifier: AGPL-3.0-only
//! Typed prerequisite gates and simultaneous owned payloads need kernel-visible access.

use crate::budget::{Budget, BudgetKind, Limits};
use crate::error::{FaultKind, InvalidCode};
use crate::model::Operator;
use crate::proof_model::{ApplicationDraft, Certificate, CheckedApplication, Statement};
use crate::solving::{SolveCertificate, SolveStatement};
use crate::testing::common::{TestResult, default_budget};
use crate::testing::series_residual::{Fixture, Term, binary, member, rational};

/// Reconstruct ordinary supplied syntax from an independently checked application.
fn draft(application: &CheckedApplication) -> ApplicationDraft {
    ApplicationDraft {
        rule: application.rule,
        context: application.judgment.context.clone(),
        conclusion: application.judgment.statement.clone(),
        premises: (0..application.premises.len())
            .map(|index| u32::try_from(index).unwrap_or(u32::MAX))
            .collect(),
        substitutions: application.substitutions.clone(),
        certificate: application.certificate.clone(),
    }
}

/// Duplicate and misplaced residuals, mismatched problems, matrices, regions and tuples refuse.
/// ´test:crate:residual-solving-gate´
#[test]
fn residual_solving_gate() -> TestResult {
    for linear in [false, true] {
        let mut f = Fixture::new(&["1"], "0", false, false)?;
        let subject = f.compose()?;
        let term = binary(Operator::Sub, Term::Y(0), Term::Q("1"));
        let (expression, rows) = f.term(&term, 1)?;
        let supplied = if linear {
            f.linear(&[rational("0")?])?
        } else {
            member()
        };
        let (residual, solve) = f.cas.check_evaluated_residual(
            &subject,
            &expression,
            &f.y,
            &rows,
            Some(&supplied),
            &mut default_budget()?,
        )?;
        let root = crate::proof_view::application(
            &f.cas,
            solve.ok_or("solve")?.proof().logical_id(),
            &mut default_budget()?,
        )?;
        let residual = crate::proof_view::application(
            &f.cas,
            residual.proof().logical_id(),
            &mut default_budget()?,
        )?;
        let subject = crate::proof_view::application(
            &f.cas,
            subject.proof().logical_id(),
            &mut default_budget()?,
        )?;
        let ordinary = draft(&root);
        crate::series::residual_adapter::gate(
            &f.cas,
            &ordinary,
            std::slice::from_ref(&residual),
            &mut default_budget()?,
        )?;
        // Old solving applications with no residual premise retain their existing path.
        crate::series::residual_adapter::gate(&f.cas, &ordinary, &[], &mut default_budget()?)?;
        for mutation in 0..6 {
            let mut changed = ordinary.clone();
            let mut premises = vec![residual.clone()];
            match mutation {
                0 => premises.push(residual.clone()),
                1 => premises.insert(0, subject.clone()),
                2 => {
                    let Statement::Solve(value) = &mut changed.conclusion else {
                        return Err("solve".into());
                    };
                    match value.as_mut() {
                        SolveStatement::Member(v) => v.problem = f.x.clone(),
                        SolveStatement::LinearSystem(v) => v.problem = f.x.clone(),
                        _ => return Err("kind".into()),
                    }
                }
                3 => {
                    let Certificate::Solve(value) = &mut changed.certificate else {
                        return Err("certificate".into());
                    };
                    match value.as_mut() {
                        SolveCertificate::Membership(v) => v.tuple = f.x.clone(),
                        SolveCertificate::ExactLinearEquivalence(v) => v.matrix = f.x.clone(),
                        _ => return Err("kind".into()),
                    }
                }
                4 => {
                    let Statement::Solve(value) = &mut changed.conclusion else {
                        return Err("solve".into());
                    };
                    match value.as_mut() {
                        SolveStatement::Member(v) => v.region = f.x.clone(),
                        SolveStatement::LinearSystem(v) => v.region = f.x.clone(),
                        _ => return Err("kind".into()),
                    }
                }
                _ => premises[0].judgment.context = f.x.clone(),
            }
            let fault = crate::series::residual_adapter::gate(
                &f.cas,
                &changed,
                &premises,
                &mut default_budget()?,
            )
            .expect_err("typed residual prerequisite");
            assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Rule));
        }
    }
    Ok(())
}

/// Exact generated equations cannot be replaced by a tautology or a shorter valid problem.
/// ´test:crate:residual-derived-problem-and-premise-controls´
#[test]
fn residual_derived_problem_and_premise_controls() -> TestResult {
    use crate::series::{ResidualSubject, SeriesStatement};
    let mut f = Fixture::new(&["1", "0", "0"], "0", false, false)?;
    let subject = f.compose()?;
    let term = binary(Operator::Sub, Term::Y(0), Term::Q("1"));
    let (expression, rows) = f.term(&term, 3)?;
    let (residual, _) = f.cas.check_evaluated_residual(
        &subject,
        &expression,
        &f.y,
        &rows,
        None,
        &mut default_budget()?,
    )?;
    let application = crate::proof_view::application(
        &f.cas,
        residual.proof().logical_id(),
        &mut default_budget()?,
    )?;
    let original = draft(&application);
    let one = f.node(Operator::Rational(rational("1")?), &[])?;
    let different = f.defined(&one)?;
    f.cas.check_proof(
        &f.context,
        std::slice::from_ref(&different),
        &mut default_budget()?,
    )?;
    let Statement::Solve(different) = different.conclusion else {
        return Err("solve".into());
    };
    let SolveStatement::Member(different) = *different else {
        return Err("member".into());
    };
    let (short, _) = f.expectations(&term, &[rational("0")?, rational("0")?], 0)?;
    let Statement::Series(short) = short.statement else {
        return Err("series".into());
    };
    let SeriesStatement::EvaluatedResidual { problem: short, .. } = *short else {
        return Err("residual".into());
    };
    for problem in [different.problem, short] {
        let mut changed = original.clone();
        let Statement::Series(value) = &mut changed.conclusion else {
            return Err("series".into());
        };
        let SeriesStatement::EvaluatedResidual {
            problem: target, ..
        } = value.as_mut()
        else {
            return Err("residual".into());
        };
        *target = problem;
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        let fault = crate::scoped_check::check_supplied(
            &mut f.cas,
            &changed,
            std::slice::from_ref(subject.proof()),
            &mut default_budget()?,
        )
        .expect_err("exact derived predicate");
        assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Rule));
        assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);
    }
    for slots in [vec![], vec![0, 0], vec![0, 1]] {
        let mut changed = original.clone();
        changed.premises = slots;
        let fault = crate::scoped_check::check_supplied(
            &mut f.cas,
            &changed,
            &[subject.proof().clone(), residual.proof().clone()],
            &mut default_budget()?,
        )
        .expect_err("exact one premise");
        assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Rule));
    }
    let mut changed = original;
    let Statement::Series(value) = &mut changed.conclusion else {
        return Err("series".into());
    };
    let SeriesStatement::EvaluatedResidual {
        subject: ResidualSubject::Compose { default_order, .. },
        ..
    } = value.as_mut()
    else {
        return Err("compose".into());
    };
    *default_order = false;
    let fault = crate::scoped_check::check_supplied(
        &mut f.cas,
        &changed,
        std::slice::from_ref(subject.proof()),
        &mut default_budget()?,
    )
    .expect_err("retained subject flag");
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Rule));
    Ok(())
}

/// Independently valid solving roots cannot borrow a residual for a different derived problem.
/// ´test:crate:residual-gate-valid-problem-twins´
#[test]
fn residual_gate_valid_problem_twins() -> TestResult {
    let mut f = Fixture::new(&["1"], "0", false, false)?;
    let subject = f.compose()?;
    let (expression, rows) = f.term(&binary(Operator::Sub, Term::Y(0), Term::Q("1")), 1)?;
    let (residual, _) = f.cas.check_evaluated_residual(
        &subject,
        &expression,
        &f.y,
        &rows,
        None,
        &mut default_budget()?,
    )?;
    let one = f.node(Operator::Rational(rational("1")?), &[])?;
    let mut different_member = f.defined(&one)?;
    f.cas.check_proof(
        &f.context,
        std::slice::from_ref(&different_member),
        &mut default_budget()?,
    )?;
    different_member.premises = vec![0];
    let (expression, rows) = f.term(&binary(Operator::Sub, Term::Y(0), Term::Q("2")), 1)?;
    let linear = f.linear(&[rational("-1")?])?;
    let (_, different_linear) = f.cas.check_evaluated_residual(
        &subject,
        &expression,
        &f.y,
        &rows,
        Some(&linear),
        &mut default_budget()?,
    )?;
    let different_linear = crate::proof_view::application(
        &f.cas,
        different_linear.ok_or("linear")?.proof().logical_id(),
        &mut default_budget()?,
    )?;
    for changed in [different_member, draft(&different_linear)] {
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        let fault = crate::scoped_check::check_supplied(
            &mut f.cas,
            &changed,
            std::slice::from_ref(residual.proof()),
            &mut default_budget()?,
        )
        .expect_err("valid different problem");
        assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Rule));
        assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);
    }
    Ok(())
}

/// A separate retained definedness proof is consistent and unnecessary for ground pivot inversion.
fn owned_setup(reversed: bool) -> TestResult<(Fixture, crate::proof_model::VerifiedWitness)> {
    use crate::domain_model::{ContextDraft, HypothesisRef};
    use crate::graph::{AssumptionDraft, Draft, Reference};
    use crate::model::PredicateKind;
    use crate::testing::series_residual::admit;
    let mut f = Fixture::new(&["1/2"], "0", reversed, false)?;
    let pivot = f.node(Operator::Rational(rational("1/2")?), &[])?;
    let predicate = admit(
        &mut f.cas,
        Draft::Condition {
            header: Reference::Existing(f.header.clone()),
            scope: Vec::new(),
            kind: PredicateKind::Defined,
            arguments: vec![Reference::Existing(pivot)],
        },
    )?;
    f.context = admit(
        &mut f.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(f.header.clone()),
            admissible: Reference::Existing(f.region.clone()),
            assumptions: vec![AssumptionDraft {
                key: "pivot-defined".into(),
                condition: Reference::Existing(predicate.clone()),
            }],
        }),
    )?;
    let ordinary = f.cas.check_proof(
        &f.context,
        &[ApplicationDraft {
            rule: crate::rule::Primitive::ContextHypothesis,
            context: f.context.clone(),
            conclusion: Statement::PredicateFact {
                region: f.region.clone(),
                predicate,
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::Hypothesis(HypothesisRef {
                context: f.context.clone(),
                slot: 0,
            }),
        }],
        &mut default_budget()?,
    )?;
    Ok((f, ordinary))
}

/// The local checker fixture exposes each already-owned premise payload separately.
fn owned_fixture(
    reversed: bool,
) -> TestResult<(Fixture, ApplicationDraft, Vec<CheckedApplication>, [u64; 2])> {
    let (mut f, ordinary) = owned_setup(reversed)?;
    let subject = f.compose()?;
    let (expression, rows) = f.term(&binary(Operator::Sub, Term::Y(0), Term::Q("1")), 1)?;
    let mut linear = f.linear(&[rational("-1/2")?])?;
    linear.premises.push(ordinary.proof().clone());
    let (residual, solve) = f.cas.check_evaluated_residual(
        &subject,
        &expression,
        &f.y,
        &rows,
        Some(&linear),
        &mut default_budget()?,
    )?;
    let solve = solve.ok_or("linear")?;
    assert_eq!(solve.hypotheses(), ordinary.hypotheses());
    let application =
        crate::proof_view::application(&f.cas, solve.proof().logical_id(), &mut default_budget()?)?;
    let ids = [residual.proof().logical_id(), ordinary.proof().logical_id()];
    let mut premises = Vec::new();
    let mut payload = [0; 2];
    for (index, id) in ids.into_iter().enumerate() {
        payload[index] = crate::proof_view::application_bytes(&f.cas, id, &mut default_budget()?)?;
        premises.push(crate::proof_view::application(
            &f.cas,
            id,
            &mut default_budget()?,
        )?);
    }
    Ok((f, draft(&application), premises, payload))
}

/// Local-checker subtest only: both payloads remain present under a hand-supplied reservation.
/// ´test:crate:residual-aggregate-owned-payload´
#[test]
fn residual_aggregate_owned_payload() -> TestResult {
    aggregate_boundary(false)?;
    aggregate_boundary(true)
}

/// Opposite allocation orders retain the same exact summed reservation and rollback.
fn aggregate_boundary(reversed: bool) -> TestResult {
    let (mut f, draft, premises, payload) = owned_fixture(reversed)?;
    let at = crate::error::Location::new("residual aggregate reservation");
    let mut attempt = |allowance, ownership| -> TestResult<(bool, u64)> {
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        let mut budget = Budget::new(Limits {
            staged_payload_bytes: allowance,
            ..Limits::default()
        })?;
        budget.work(37, at)?;
        let result = budget.with_retained_payload(113 + ownership, at, |budget| {
            crate::scoped_check::check_application(&mut f.cas, &draft, &premises, budget)
        });
        if let Err(fault) = &result {
            assert!(
                matches!(
                    fault.kind,
                    FaultKind::Exhausted {
                        budget: BudgetKind::StagedPayloadBytes,
                        limit: 511,
                        needed: 512,
                        ..
                    }
                ),
                "{fault:?}"
            );
            assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);
        }
        Ok((result.is_ok(), budget.usage().work))
    };
    // E=113, W=512, P0=1120 and P1=700 describe only this local-checker subtest.
    assert_eq!(payload, [1120, 700]);
    let low = 625;
    assert!(attempt(low, 0)?.0);
    assert_eq!(attempt(low - 1, 0)?, (false, 37));
    let allowance = 2444;
    assert!(attempt(allowance, payload[0])?.0);
    assert!(attempt(allowance, payload[1])?.0);
    let failure = attempt(allowance, payload.iter().sum())?;
    assert_eq!(failure, (false, 37));
    assert!(attempt(low + payload.iter().sum::<u64>(), payload.iter().sum())?.0);
    Ok(())
}

/// Matching serials never authenticate a foreign proof; rollback never revives a stale proof.
/// ´test:crate:residual-prerequisite-handles´
#[test]
fn residual_prerequisite_handles() -> TestResult {
    for stale in [false, true] {
        let mut f = Fixture::new(&["0"], "0", false, false)?;
        let mut foreign = Fixture::new(&["0"], "0", false, false)?;
        let one = f.node(Operator::Rational(rational("1")?), &[])?;
        let step = f.defined(&one)?;
        let mut proof = None;
        if stale {
            let fault = f
                .cas
                .transaction(&mut default_budget()?, |cas, budget| {
                    proof = Some(
                        cas.check_proof(&f.context, std::slice::from_ref(&step), budget)?
                            .proof()
                            .clone(),
                    );
                    Err::<(), _>(crate::error::Fault::invalid(
                        InvalidCode::Rule,
                        crate::error::Location::new("rollback fixture"),
                    ))
                })
                .expect_err("deliberate rollback");
            assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Rule));
        } else {
            let one = foreign.node(Operator::Rational(rational("1")?), &[])?;
            let step = foreign.defined(&one)?;
            proof = Some(
                foreign
                    .cas
                    .check_proof(&foreign.context, &[step], &mut default_budget()?)?
                    .proof()
                    .clone(),
            );
        }
        let proof = proof.ok_or("proof")?;
        let local = f
            .cas
            .check_proof(&f.context, &[step], &mut default_budget()?)?;
        assert_eq!(local.proof().logical_id() == proof.logical_id(), !stale);
        let subject = f.compose()?;
        let (expression, rows) = f.term(&Term::Y(0), 1)?;
        let mut supplied = member();
        supplied.premises.push(proof);
        let SolveCertificate::Membership(value) = &mut supplied.certificate else {
            return Err("member".into());
        };
        value.definedness.push(1);
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
            .expect_err("unauthenticated prerequisite");
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(if stale {
                InvalidCode::Reference
            } else {
                InvalidCode::ForeignHandle
            })
        );
        assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);
    }
    Ok(())
}

/// A tiny residual cannot authorize an unbudgeted 32 MiB definedness copy or operation walk.
/// ´test:crate:residual-supplied-inventory´
#[test]
fn residual_supplied_inventory() -> TestResult {
    for linear in [false, true] {
        let mut f = Fixture::new(&["0"], "0", false, false)?;
        let one = f.node(Operator::Rational(rational("1")?), &[])?;
        let step = f.defined(&one)?;
        let ordinary = f
            .cas
            .check_proof(&f.context, &[step], &mut default_budget()?)?;
        let subject = f.compose()?;
        let (expression, rows) = f.term(&Term::Y(0), 1)?;
        let mut supplied = if linear {
            f.linear(&[rational("0")?])?
        } else {
            member()
        };
        supplied.premises.push(ordinary.proof().clone());
        match &mut supplied.certificate {
            SolveCertificate::Membership(value) => value.definedness = vec![1; 8_388_608],
            SolveCertificate::ExactLinearEquivalence(value) => {
                value.operations =
                    vec![crate::solving::RowOperation::Swap { left: 0, right: 0 }; 524_288];
            }
            _ => return Err("certificate".into()),
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
                &mut Budget::new(Limits {
                    staged_payload_bytes: 1_048_576,
                    ..Limits::default()
                })?,
            )
            .expect_err("supplied inventory must be reserved first");
        assert_eq!(
            fault.kind,
            FaultKind::Exhausted {
                reason: crate::error::Exhaustion::Limit,
                budget: BudgetKind::StagedPayloadBytes,
                limit: 1_047_680,
                needed: 33_560_700,
            }
        );
        assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);
    }
    Ok(())
}

/// The shared checker and a fresh decoder require the derived header on the literal zero.
/// ´test:crate:residual-derived-zero-frame´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "independent malformed problem and fresh replay stay together"
)]
fn residual_derived_zero_frame() -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    use crate::graph::{Draft, ExpressionDraft, Reference};
    use crate::model::PredicateKind;
    use crate::series::SeriesStatement;
    use crate::solving::SolveDraft;
    use crate::testing::series_residual::admit;
    for values in [&["0"][..], &["0", "0"][..]] {
        let mut f = Fixture::with_extension(values)?;
        let parent = f
            .cas
            .state
            .header(f.header.logical_id(), &mut budget)?
            .parent
            .ok_or("extension")?;
        let ancestor = f.cas.handle(parent);
        let zero = admit(
            &mut f.cas,
            Draft::Expression(ExpressionDraft {
                header: Reference::Existing(ancestor),
                space: None,
                operator: Operator::Rational(rational("0")?),
                sort: crate::testing::common::real(),
                scope: Vec::new(),
                binding: None,
                operands: Vec::new(),
            }),
        )?;
        let subject = f.compose()?;
        let (expression, rows) = f.term(&Term::Y(0), values.len())?;
        let (residual, _) = f.cas.check_evaluated_residual(
            &subject,
            &expression,
            &f.y,
            &rows,
            None,
            &mut default_budget()?,
        )?;
        let valid_bytes = f.cas.export_witness(&residual, &mut default_budget()?)?;
        let mut application = crate::proof_view::application(
            &f.cas,
            residual.proof().logical_id(),
            &mut default_budget()?,
        )?;
        let Statement::Series(value) = &mut application.judgment.statement else {
            return Err("series".into());
        };
        let SeriesStatement::EvaluatedResidual {
            problem, output, ..
        } = value.as_mut()
        else {
            return Err("residual".into());
        };
        let SolveDraft::SolveProblem(original) =
            crate::solving::graph::object(&f.cas.state, problem.logical_id(), &mut budget)?
        else {
            return Err("problem".into());
        };
        let mut original = original.clone();
        let coefficients = f
            .cas
            .series_object(output, &mut budget)?
            .coefficients
            .clone();
        let (kind, arguments) = if values.len() == 1 {
            (PredicateKind::Equal, vec![coefficients[0].clone(), zero])
        } else {
            let mut equations = Vec::new();
            for coefficient in coefficients {
                equations.push(admit(
                    &mut f.cas,
                    Draft::Expression(ExpressionDraft {
                        header: Reference::Existing(f.header.clone()),
                        space: None,
                        operator: Operator::Predicate(PredicateKind::Equal),
                        sort: crate::model::Sort::Value(crate::model::AtomicSort::Boolean),
                        scope: Vec::new(),
                        binding: None,
                        operands: vec![
                            Reference::Existing(coefficient),
                            Reference::Existing(zero.clone()),
                        ],
                    }),
                )?);
            }
            (PredicateKind::And, equations)
        };
        original.predicate = admit(
            &mut f.cas,
            Draft::Condition {
                header: Reference::Existing(f.header.clone()),
                scope: Vec::new(),
                kind,
                arguments: arguments.into_iter().map(Reference::Existing).collect(),
            },
        )?;
        *problem = admit(
            &mut f.cas,
            Draft::Solve(SolveDraft::SolveProblem(crate::solving::SolveProblemData {
                header: Reference::Existing(original.header),
                scope: original.scope,
                predicate: Reference::Existing(original.predicate),
                coefficients: Reference::Existing(original.coefficients),
                unknowns: Reference::Existing(original.unknowns),
                region: Reference::Existing(original.region),
                domain: Reference::Existing(original.domain),
                coefficient_domain: Reference::Existing(original.coefficient_domain),
                ambient: original.ambient,
                denominators: original
                    .denominators
                    .into_iter()
                    .map(Reference::Existing)
                    .collect(),
            })),
        )?;
        let changed = draft(&application);
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        let fault = crate::scoped_check::check_supplied(
            &mut f.cas,
            &changed,
            std::slice::from_ref(subject.proof()),
            &mut default_budget()?,
        )
        .expect_err("ancestor zero");
        assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Rule));
        assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);
        // Test-only publication supplies a hostile wire DAG without passing the checker under test.
        let forged = f.cas.publish_scoped(application, &mut default_budget()?)?;
        let bytes = crate::wire_build::encode(&f.cas, forged.logical_id(), &mut default_budget()?)?;
        let mut receiver = crate::graph::Cas::new(Limits::default())?;
        receiver.inspect(&valid_bytes, &mut default_budget()?)?;
        let mut receiver = crate::graph::Cas::new(Limits::default())?;
        let before = receiver.canonical_snapshot(&mut default_budget()?)?;
        let fault = receiver
            .inspect(&bytes, &mut default_budget()?)
            .expect_err("fresh ancestor zero replay");
        assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Rule));
        assert_eq!(before, receiver.canonical_snapshot(&mut default_budget()?)?);
    }
    Ok(())
}

/// False row inventories and prior ledger spend are charged before coefficient allocation.
/// ´test:crate:residual-decoded-inventory´
#[test]
fn residual_decoded_inventory() -> TestResult {
    for (rows, width, prior, limit, needed) in [(64, 64, 0, 64, 128), (1, 1, 64, 64, 65)] {
        let certificate = crate::series::SeriesCertificate::EvaluatedResidual {
            form: 0,
            stage: 0,
            rows: (0..rows)
                .map(|ordinal| crate::series::ResidualRow {
                    ordinal,
                    coefficients: vec![crate::number::Rational::zero(); width],
                })
                .collect(),
        };
        let mut encoding_budget = default_budget()?;
        let mut encoder = crate::encoding::Encoder::new(
            &mut encoding_budget,
            crate::error::Location::new("residual decoder fixture"),
        );
        crate::series::codec::certificate(&certificate, &mut encoder)?;
        let bytes = encoder.finish();
        let mut budget = Budget::new(Limits {
            jet_entries: limit,
            ..Limits::default()
        })?;
        budget.specialized(
            BudgetKind::JetEntries,
            prior,
            true,
            crate::error::Location::new("prior entries"),
        )?;
        let mut reader = crate::wire_read::Reader::new(&bytes, &mut budget)?;
        let fault = crate::series::codec::read_certificate(&mut reader)
            .expect_err("cumulative decoded coefficients");
        assert_eq!(
            fault.kind,
            FaultKind::Exhausted {
                reason: crate::error::Exhaustion::Limit,
                budget: BudgetKind::JetEntries,
                limit,
                needed
            }
        );
        assert_eq!(budget.usage().jet_entries, 64);
    }
    Ok(())
}

/// Complete construction, fresh import and paired replay own both actual premise payloads.
/// ´test:crate:residual-complete-owned-payload´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "each measured operation keeps setup, publication, buffers and rollback visible"
)]
fn residual_complete_owned_payload() -> TestResult {
    for reversed in [false, true] {
        let (mut f, ordinary) = owned_setup(reversed)?;
        let one = f.node(Operator::Rational(rational("1")?), &[])?;
        let mut step = f.defined(&one)?;
        step.premises = vec![0];
        let Certificate::Solve(value) = &mut step.certificate else {
            return Err("solve".into());
        };
        let SolveCertificate::Membership(value) = value.as_mut() else {
            return Err("member".into());
        };
        value.definedness = vec![0; 256];
        let ordinary_proof = crate::scoped_check::check_supplied(
            &mut f.cas,
            &step,
            std::slice::from_ref(ordinary.proof()),
            &mut default_budget()?,
        )?;
        let ordinary = f.cas.seal_scoped(&ordinary_proof, &mut default_budget()?)?;
        let subject = f.compose()?;
        let term = binary(Operator::Sub, Term::Y(0), Term::Q("1"));
        let (expression, rows) = f.term(&term, 1)?;
        let mut supplied = f.linear(&[rational("-1/2")?])?;
        supplied.premises = vec![ordinary.proof().clone(); 16];
        let mut producer = owned_copy(&f.cas);
        let (residual, solve) = producer.check_evaluated_residual(
            &subject,
            &expression,
            &f.y,
            &rows,
            Some(&supplied),
            &mut default_budget()?,
        )?;
        let solve = solve.ok_or("solve")?;
        assert_eq!(solve.hypotheses(), ordinary.hypotheses());
        let bytes = producer.export_witness(&solve, &mut default_budget()?)?;
        let mut payload = Vec::new();
        for proof in [residual.proof(), ordinary.proof()] {
            payload.push(crate::proof_view::application_bytes(
                &producer,
                proof.logical_id(),
                &mut default_budget()?,
            )?);
        }
        assert_eq!(payload, [1120, 4940]);
        let application = crate::proof_view::application(
            &producer,
            solve.proof().logical_id(),
            &mut default_budget()?,
        )?;
        assert_eq!(application.premises.len(), 17);
        // Sixteen actual ordinary copies make publication, rather than sealing, the peak.
        assert_eq!(payload[0] + 16 * payload[1], 80_160);
        let (mut receiver, _) = owned_setup(!reversed)?;
        let (expected_residual, mut expected_solve) =
            receiver.expectations(&term, &[rational("-1/2")?], 0)?;
        let Statement::Solve(value) = &expected_solve.statement else {
            return Err("solve".into());
        };
        let SolveStatement::Member(member) = value.as_ref() else {
            return Err("member".into());
        };
        let linear = receiver.linear(&[rational("-1/2")?])?;
        let SolveStatement::LinearSystem(linear) = linear.statement else {
            return Err("linear".into());
        };
        let crate::series::ResidualReference::Existing(reduced) = linear.reduced else {
            return Err("reduced".into());
        };
        expected_solve.statement = Statement::Solve(Box::new(SolveStatement::LinearSystem(
            crate::solving::StatementLinearSystem {
                problem: member.problem.clone(),
                reduced,
                rank: 0,
                augmented_rank: 1,
                region: receiver.region.clone(),
                free_columns: Vec::new(),
            },
        )));
        for mode in 0..3 {
            let attempt = |allowance| -> TestResult<(Result<(), crate::error::Fault>, u64)> {
                let mut cas = if mode == 0 {
                    owned_copy(&f.cas)
                } else if mode == 1 {
                    crate::graph::Cas::new(Limits::default())?
                } else {
                    owned_copy(&receiver.cas)
                };
                let before = cas.canonical_snapshot(&mut default_budget()?)?;
                let mut budget = Budget::new(Limits {
                    staged_payload_bytes: allowance,
                    component_bits: 64,
                    intermediate_bits: 128,
                    ..Limits::default()
                })?;
                budget.work(37, crate::error::Location::new("owned prior work"))?;
                let retained = 113
                    + u64::try_from(
                        bytes.len() + subject.exported_byte_len() + ordinary.exported_byte_len(),
                    )?;
                let result = budget.with_retained_payload(
                    retained,
                    crate::error::Location::new("owned caller buffers"),
                    |budget| {
                        if mode == 0 {
                            cas.check_evaluated_residual(
                                &subject,
                                &expression,
                                &f.y,
                                &rows,
                                Some(&supplied),
                                budget,
                            )
                            .map(|(_, solve)| assert!(solve.is_some()))
                        } else if mode == 1 {
                            cas.inspect(&bytes, budget).map(|_| ())
                        } else {
                            cas.replay_evaluated_residual(
                                &bytes,
                                &expected_residual,
                                &expected_solve,
                                budget,
                            )
                            .map(|_| ())
                        }
                    },
                );
                if result.is_err() {
                    assert_eq!(before, cas.canonical_snapshot(&mut default_budget()?)?);
                }
                Ok((result, budget.usage().work))
            };
            let (ceiling, needed, success_work, failure_work) = [
                (122_450, 24_603, 459_638, 319_991),
                (108_920, 22_887, 649_125, 507_868),
                (110_306, 23_441, 849_109, 507_061),
            ][mode];
            let (failure, work) = attempt(ceiling - 1)?;
            assert_eq!(
                failure
                    .expect_err("production owns every premise through publication")
                    .kind,
                FaultKind::Exhausted {
                    reason: crate::error::Exhaustion::Limit,
                    budget: BudgetKind::StagedPayloadBytes,
                    limit: needed - 1,
                    needed,
                }
            );
            assert_eq!(work, failure_work);
            let (success, work) = attempt(ceiling)?;
            success?;
            assert_eq!(work, success_work);
        }
    }
    Ok(())
}

/// An unmetered test checkpoint retains the authentication of its existing series evidence.
fn owned_copy(cas: &crate::graph::Cas) -> crate::graph::Cas {
    let mut copy = cas.full_copy_for_test();
    copy.memo_owner.clone_from(&cas.memo_owner);
    copy.memo_epoch = cas.memo_epoch;
    copy
}

/// A complete malformed import exhausts the coefficient inventory and rolls back all records.
/// ´test:crate:residual-decoded-inventory-replay´
#[test]
fn residual_decoded_inventory_replay() -> TestResult {
    let mut f = Fixture::new(&["0"], "0", false, false)?;
    let subject = f.compose()?;
    let (expression, rows) = f.term(&Term::Y(0), 1)?;
    let (residual, _) = f.cas.check_evaluated_residual(
        &subject,
        &expression,
        &f.y,
        &rows,
        None,
        &mut default_budget()?,
    )?;
    let mut application = crate::proof_view::application(
        &f.cas,
        residual.proof().logical_id(),
        &mut default_budget()?,
    )?;
    application.certificate = Certificate::Series(Box::new(
        crate::series::SeriesCertificate::EvaluatedResidual {
            form: 0,
            stage: 0,
            rows: (0..64)
                .map(|ordinal| crate::series::ResidualRow {
                    ordinal,
                    coefficients: vec![crate::number::Rational::zero(); 64],
                })
                .collect(),
        },
    ));
    let mut setup = Budget::new(Limits {
        work: 20_000_000,
        ..Limits::default()
    })?;
    let forged = f.cas.publish_scoped(application, &mut setup)?;
    let bytes = crate::wire_build::encode(&f.cas, forged.logical_id(), &mut setup)?;
    let mut receiver = crate::graph::Cas::new(Limits::default())?;
    let before = receiver.canonical_snapshot(&mut default_budget()?)?;
    let fault = receiver
        .inspect(
            &bytes,
            &mut Budget::new(Limits {
                jet_entries: 64,
                ..Limits::default()
            })?,
        )
        .expect_err("malformed many-row import");
    assert_eq!(
        fault.kind,
        FaultKind::Exhausted {
            reason: crate::error::Exhaustion::Limit,
            budget: BudgetKind::JetEntries,
            limit: 64,
            needed: 70,
        }
    );
    assert_eq!(before, receiver.canonical_snapshot(&mut default_budget()?)?);
    Ok(())
}

/// Supplied rows are prepaid even when empty coefficient vectors hide their inventory.
/// ´test:crate:residual-supplied-row-prepayment´
#[test]
fn residual_supplied_row_prepayment() -> TestResult {
    for reversed in [false, true] {
        let mut f = Fixture::new(&["0"], "0", reversed, false)?;
        let subject = f.compose()?;
        let (expression, rows) = f.term(&Term::Y(0), 1)?;
        let (residual, solve) = f.cas.check_evaluated_residual(
            &subject,
            &expression,
            &f.y,
            &rows,
            Some(&member()),
            &mut default_budget()?,
        )?;
        let solve = solve.ok_or("solve")?;
        let mut application = crate::proof_view::application(
            &f.cas,
            residual.proof().logical_id(),
            &mut default_budget()?,
        )?;
        let compose = crate::proof_view::application(
            &f.cas,
            subject.proof().logical_id(),
            &mut default_budget()?,
        )?;
        let valid = [draft(&compose), draft(&application)];
        f.cas
            .check_proof(&f.context, &valid, &mut default_budget()?)?;
        application.certificate = Certificate::Series(Box::new(
            crate::series::SeriesCertificate::EvaluatedResidual {
                form: 0,
                stage: 0,
                rows: (0..1_048_576)
                    .map(|ordinal| crate::series::ResidualRow {
                        ordinal,
                        coefficients: Vec::new(),
                    })
                    .collect(),
            },
        ));
        let steps = [draft(&compose), draft(&application)];
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        let fault = f
            .cas
            .check_proof(
                &f.context,
                &steps,
                &mut Budget::new(Limits {
                    work: 100_000,
                    ..Limits::default()
                })?,
            )
            .expect_err("row scan must be prepaid");
        assert_eq!(
            fault.kind,
            FaultKind::Exhausted {
                reason: crate::error::Exhaustion::Limit,
                budget: BudgetKind::Work,
                limit: 100_000,
                needed: 1_048_579,
            }
        );
        assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);

        supplied_inventory_replay(&mut f, application, &solve, reversed)?;
    }
    Ok(())
}

/// Encode hostile evidence with ample setup resources, then pin both low-Work import routes.
fn supplied_inventory_replay(
    f: &mut Fixture,
    application: CheckedApplication,
    solve: &crate::proof_model::VerifiedWitness,
    reversed: bool,
) -> TestResult {
    // Deliberately publish hostile evidence only to exercise wire refusal routes.
    let limits = Limits {
        work: 1_000_000_000,
        replay_work: 1_000_000_000,
        staged_payload_bytes: 1_000_000_000,
        payload_bytes: 1_000_000_000,
        witness_bytes: 100_000_000,
        output_bytes: 100_000_000,
        jet_entries: 1_048_576,
        ..Limits::default()
    };
    let mut setup = Budget::new(limits.clone())?;
    f.cas.limits = limits.clone();
    let forged = f.cas.publish_scoped(application, &mut setup)?;
    let mut root = crate::proof_view::application(&f.cas, solve.proof().logical_id(), &mut setup)?;
    root.premises[0] = forged.logical_id();
    let forged = f.cas.publish_scoped(root, &mut setup)?;
    let bytes = crate::wire_build::encode(&f.cas, forged.logical_id(), &mut setup)?;
    for paired in [false, true] {
        let mut receiver = Fixture::new(&["0"], "0", !reversed, false)?;
        let (expected_residual, expected_solve) =
            receiver.expectations(&Term::Y(0), &[rational("0")?], 0)?;
        receiver.cas.limits = limits.clone();
        let before = receiver.cas.canonical_snapshot(&mut default_budget()?)?;
        let mut budget = Budget::new(Limits {
            work: 100_000,
            ..limits.clone()
        })?;
        let result = if paired {
            receiver
                .cas
                .replay_evaluated_residual(&bytes, &expected_residual, &expected_solve, &mut budget)
                .map(|_| ())
        } else {
            receiver.cas.inspect(&bytes, &mut budget).map(|_| ())
        };
        let fault = result.expect_err("large supplied wire inventory");
        assert_eq!(
            fault.kind,
            FaultKind::Exhausted {
                reason: crate::error::Exhaustion::Limit,
                budget: BudgetKind::Work,
                limit: 100_000,
                needed: if paired { 16_872_550 } else { 16_872_240 },
            }
        );
        assert_eq!(
            before,
            receiver.cas.canonical_snapshot(&mut default_budget()?)?
        );
    }
    Ok(())
}

/// Shape and rational-size passes both reserve visits before inspecting the power schedule.
/// ´test:crate:residual-constructor-inventory-prepayment´
#[test]
fn residual_constructor_inventory_prepayment() -> TestResult {
    for reversed in [false, true] {
        let mut f = Fixture::new(&["0"], "0", reversed, false)?;
        f.cas.limits.integer_exponent = 524_286;
        f.cas.limits.jet_entries = 524_288;
        let subject = f.compose()?;
        let y = f.y.clone();
        let expression = f.node(Operator::IntegerPower(524_286), &[y])?;
        let mut rows: Vec<_> = (0..524_288)
            .map(|ordinal| crate::series::ResidualRow {
                ordinal,
                coefficients: vec![crate::number::Rational::zero()],
            })
            .collect();
        for (empty_last, work, needed) in [
            (true, 100_000, 524_546),
            (false, 100_000, 524_546),
            (false, 600_000, 1_048_834),
        ] {
            rows.last_mut().ok_or("last row")?.coefficients = if empty_last {
                Vec::new()
            } else {
                vec![crate::number::Rational::zero()]
            };
            let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
            let fault = f
                .cas
                .check_evaluated_residual(
                    &subject,
                    &expression,
                    &f.y,
                    &rows,
                    None,
                    &mut Budget::new(Limits {
                        work,
                        integer_exponent: 524_286,
                        jet_entries: 524_288,
                        ..Limits::default()
                    })?,
                )
                .expect_err("inventory visits must be prepaid");
            assert_eq!(
                fault.kind,
                FaultKind::Exhausted {
                    reason: crate::error::Exhaustion::Limit,
                    budget: BudgetKind::Work,
                    limit: work,
                    needed,
                }
            );
            assert_eq!(before, f.cas.canonical_snapshot(&mut default_budget()?)?);
        }
    }
    Ok(())
}
