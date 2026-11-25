// SPDX-License-Identifier: AGPL-3.0-only
//! Totality has one original-domain premise and only three closed Real operators.

use crate::testing::common::{TestResult, default_budget};
use crate::testing::trig_defined::fixture;
use nrz_cas::proof::{HypothesisKind, Primitive};
use nrz_cas::syntax::Operator;
use nrz_cas::{FaultKind, InvalidCode};

/// Each closed node preserves the exact original guard and all its hypotheses.
/// ´test:integration:trig-defined-original-guard´
#[test]
fn trig_defined_original_guard() -> TestResult {
    assert_eq!(
        Primitive::TrigDefined
            .formal_schema()
            .premises
            .hypothesis_kind(),
        HypothesisKind::ExactPremises
    );
    for operator in [Operator::Sine, Operator::Cosine, Operator::Arctangent] {
        for composite in [false, true] {
            let (mut f, function) = fixture(operator.clone(), composite, false)?;
            let witness = f
                .cas
                .check_proof(&f.context, &f.steps, &mut default_budget()?)?;
            assert_eq!(witness.judgment(), &f.expected);
            assert_eq!(
                f.cas.statement_hypothesis_kind(
                    &f.expected.statement,
                    &mut crate::testing::budgets::graph_budget()?
                )?,
                HypothesisKind::DomainGuard
            );
            let guard = f
                .cas
                .check_proof(&f.context, &f.steps[..1], &mut default_budget()?)?;
            let produced = f
                .cas
                .trig_defined(&function, guard.proof(), &mut default_budget()?)?;
            assert_eq!(produced.judgment(), &f.expected);
            assert_eq!(produced.hypotheses(), guard.hypotheses());
        }
    }
    Ok(())
}

/// The exact-domain premise is a checked case theorem rather than an assumed predicate.
/// ´test:integration:trig-defined-exact-domain-guard´
#[test]
fn trig_defined_exact_domain_guard() -> TestResult {
    use crate::testing::answer::{index, records};
    use crate::testing::cases::Fixture;
    use nrz_cas::request::Task;
    for operator in [Operator::Sine, Operator::Cosine, Operator::Arctangent] {
        let mut f = Fixture::new()?;
        let original = f.term(Operator::Mul, &[f.x.clone(), f.x.clone()])?;
        let request = f.request(
            Task::Coverage {
                domain: f.domain.clone(),
            },
            std::slice::from_ref(&original),
        )?;
        let coverage = f.proved(&request)?;
        let citations = f
            .cas
            .proof_citations(coverage.witness.proof(), &mut default_budget()?)?;
        let root = citations
            .iter()
            .find(|node| &node.proof == coverage.witness.proof())
            .ok_or("coverage root")?;
        let guard = &root.premises.first().ok_or("exact-domain premise")?.proof;
        let domain = citations
            .iter()
            .find(|node| &node.proof == guard)
            .ok_or("domain citation")?;
        assert_eq!(domain.primitive, Primitive::CaseDomain);
        assert_eq!(domain.premises, []);
        let function = f.term(operator, &[original])?;
        let witness = f
            .cas
            .trig_defined(&function, guard, &mut default_budget()?)?;
        assert_eq!(witness.hypotheses(), []);
        assert!(witness.is_discharged());
        let bytes = f.cas.export_witness(&witness, &mut default_budget()?)?;
        let inventory = records(&bytes)?;
        let (_, application) = inventory
            .iter()
            .rev()
            .find(|(tag, _)| *tag == 11)
            .ok_or("consumer")?;
        assert_eq!(index(&bytes, &mut (application.start + 14))?, 1);
        let guard_index = usize::try_from(index(&bytes, &mut (application.start + 18))?)?;
        let (tag, guard_application) = inventory.get(guard_index).ok_or("guard application")?;
        assert_eq!(*tag, 11);
        let statement_index = usize::try_from(index(&bytes, &mut (guard_application.start + 10))?)?;
        let (tag, statement) = inventory
            .get(statement_index)
            .ok_or("exact-domain statement")?;
        assert_eq!(*tag, 10);
        assert_eq!(bytes.get(statement.start + 10), Some(&7));
    }
    Ok(())
}

/// Missing, mistyped and different original guards cannot justify totality.
/// ´test:integration:trig-defined-premise-mutations´
#[test]
fn trig_defined_premise_mutations() -> TestResult {
    for change in 0..3 {
        let (mut f, function) = fixture(Operator::Sine, false, false)?;
        let snapshot = f.cas.canonical_snapshot(&mut default_budget()?)?;
        let mut changed = f.steps.clone();
        match change {
            0 => {
                changed.remove(0);
                changed.last_mut().ok_or("step")?.premises.clear();
            }
            1 => {
                changed.last_mut().ok_or("step")?.premises.push(0);
            }
            _ => {
                changed.first_mut().ok_or("guard")?.conclusion = f.expected.statement.clone();
            }
        }
        assert!(matches!(
            f.cas
                .check_proof(&f.context, &changed, &mut default_budget()?)
                .err()
                .ok_or("mutation accepted")?
                .kind,
            FaultKind::Invalid(_)
        ));
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, snapshot);
        let guard = f
            .cas
            .check_proof(&f.context, &f.steps[..1], &mut default_budget()?)?;
        f.cas
            .trig_defined(&function, guard.proof(), &mut default_budget()?)?;
    }
    let (mut f, function) = fixture(Operator::Neg, true, false)?;
    let guard = f
        .cas
        .check_proof(&f.context, &f.steps[..1], &mut default_budget()?)?;
    let snapshot = f.cas.canonical_snapshot(&mut default_budget()?)?;
    assert_eq!(
        f.cas
            .trig_defined(&function, guard.proof(), &mut default_budget()?)
            .err()
            .ok_or("unsupported operator")?
            .kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, snapshot);
    Ok(())
}

/// A valid original guard cannot fill a different operand or domain slot in the same context.
/// ´test:integration:trig-defined-valid-guard-joins´
#[test]
fn trig_defined_valid_guard_joins() -> TestResult {
    use nrz_cas::domain::{AmbientDomain, RegionDraft, RegionExpr};
    use nrz_cas::proof::Statement;
    use nrz_cas::syntax::{Draft, PredicateKind};
    use nrz_cas::{Rational, Reference};
    for domain_only in [false, true] {
        let (mut f, _) = fixture(Operator::Sine, true, false)?;
        let guard = f
            .cas
            .check_proof(&f.context, &f.steps[..1], &mut default_budget()?)?;
        assert_eq!(
            f.cas.statement_hypothesis_kind(
                &guard.judgment().statement,
                &mut crate::testing::budgets::graph_budget()?
            )?,
            HypothesisKind::DomainGuard
        );
        let mut changed = f.steps.clone();
        let Statement::PredicateFact { region, predicate } =
            &mut changed.last_mut().ok_or("consumer")?.conclusion
        else {
            return Err("predicate".into());
        };
        if domain_only {
            *region = f
                .cas
                .admit(
                    &[Draft::Region(RegionDraft {
                        header: Reference::Existing(f.header.clone()),
                        scope: Vec::new(),
                        ambient: AmbientDomain::Real("x".into()),
                        expression: RegionExpr::Empty,
                    })],
                    &mut default_budget()?,
                )?
                .pop()
                .ok_or("empty region")?;
        } else {
            let one = crate::testing::half_pi::term(
                &mut f.cas,
                &f.header,
                Operator::Rational(Rational::one()),
                &[],
            )?;
            let other =
                crate::testing::half_pi::term(&mut f.cas, &f.header, Operator::Sine, &[one])?;
            *predicate = f
                .cas
                .admit(
                    &[Draft::Condition {
                        header: Reference::Existing(f.header.clone()),
                        scope: Vec::new(),
                        kind: PredicateKind::Defined,
                        arguments: vec![Reference::Existing(other)],
                    }],
                    &mut default_budget()?,
                )?
                .pop()
                .ok_or("other predicate")?;
        }
        assert_eq!(
            changed.last().ok_or("consumer")?.context,
            guard.judgment().context
        );
        let snapshot = f.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.cas
                .check_proof(&f.context, &changed, &mut default_budget()?)
                .err()
                .ok_or("different join")?
                .kind,
            FaultKind::Invalid(InvalidCode::EndpointMismatch)
        );
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, snapshot);
        f.cas
            .check_proof(&f.context, &f.steps, &mut default_budget()?)?;
    }
    Ok(())
}

/// Ordinary coverage cites registered totality and cannot erase an undefined child.
/// ´test:integration:trig-defined-coverage-dispatch´
#[test]
fn trig_defined_coverage_dispatch() -> TestResult {
    use crate::testing::cases::Fixture;
    use nrz_cas::request::{Outcome, Task};
    for operator in [Operator::Sine, Operator::Cosine, Operator::Arctangent] {
        let mut f = Fixture::new()?;
        let u = f.term(Operator::Mul, &[f.x.clone(), f.x.clone()])?;
        let inner = f.term(operator.clone(), &[u])?;
        let function = f.term(operator.clone(), &[inner])?;
        let request = f.request(
            Task::Coverage {
                domain: f.domain.clone(),
            },
            &[function],
        )?;
        let result = f.proved(&request)?;
        assert!(result.witness.is_discharged());
        let bytes = f
            .cas
            .export_witness(&result.witness, &mut default_budget()?)?;
        assert!(
            bytes
                .windows(b"trig.defined".len())
                .any(|word| word == b"trig.defined")
        );
        let undefined = f.term(Operator::Div, &[f.one.clone(), f.zero.clone()])?;
        let hidden = f.term(Operator::Mul, &[f.zero.clone(), undefined])?;
        let function = f.term(operator, &[hidden])?;
        let request = f.request(
            Task::Coverage {
                domain: f.domain.clone(),
            },
            &[function],
        )?;
        assert!(matches!(f.cas.run(&request), Outcome::Refused { .. }));
    }
    Ok(())
}

/// Complex operands fail admission and an admitted equal-spelled call gains no totality authority.
/// ´test:integration:trig-defined-closed-real-boundary´
#[test]
fn trig_defined_closed_real_boundary() -> TestResult {
    use crate::testing::trig::Fixture;
    for operator in [Operator::Sine, Operator::Cosine, Operator::Arctangent] {
        let mut f = Fixture::new()?;
        let snapshot = f.cas.canonical_snapshot(&mut default_budget()?)?;
        assert!(f.term(operator, &[f.z.clone()]).is_err());
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, snapshot);
        let call = f.term(Operator::Call("sine".into()), &[f.x.clone()])?;
        let snapshot = f.cas.canonical_snapshot(&mut default_budget()?)?;
        // The unsupported operator is inspected before the deliberately non-proof handle.
        assert_eq!(
            f.cas
                .trig_defined(&call, &f.x, &mut default_budget()?)
                .err()
                .ok_or("opaque call")?
                .kind,
            FaultKind::Invalid(InvalidCode::EndpointMismatch)
        );
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, snapshot);
    }
    Ok(())
}

/// Unsupported admitted payloads are borrowed before the fixed checkpoint or operator refusal.
/// ´test:integration:trig-defined-borrowed-early-exits´
#[test]
fn trig_defined_borrowed_early_exits() -> TestResult {
    use crate::testing::common::real;
    use crate::testing::half_pi::term;
    use nrz_cas::limits::{Budget, BudgetKind, Limits};
    use nrz_cas::syntax::{AtomicSort, Draft, ExpressionDraft, Field};
    use nrz_cas::{Location, Rational, Reference};
    let (mut f, _) = fixture(Operator::Sine, false, false)?;
    let literal = term(
        &mut f.cas,
        &f.header,
        Operator::Rational(Rational::parse(&"9".repeat(200), &mut default_budget()?)?),
        &[],
    )?;
    let scoped = f
        .cas
        .admit(
            &[Draft::Expression(ExpressionDraft {
                header: Reference::Existing(f.header.clone()),
                space: None,
                operator: Operator::Rational(Rational::zero()),
                sort: real(),
                scope: vec![vec![AtomicSort::Scalar(Field::Real); 8]; 64],
                binding: None,
                operands: Vec::new(),
            })],
            &mut default_budget()?,
        )?
        .pop()
        .ok_or("scoped input")?;
    let snapshot = f.cas.canonical_snapshot(&mut default_budget()?)?;
    for input in [literal, scoped] {
        for (work, staging) in [(293, 403), (294, 403), (294, 402)] {
            let mut budget = Budget::new(Limits {
                work,
                staged_payload_bytes: staging,
                ..Limits::default()
            })?;
            budget.work(37, Location::new("prior totality Work"))?;
            let fault = budget
                .with_retained_payload(19, Location::new("prior totality payload"), |budget| {
                    f.cas.trig_defined(&input, &input, budget)
                })
                .err()
                .ok_or("unsupported input")?;
            if staging == 402 {
                assert!(matches!(
                    fault.kind,
                    FaultKind::Exhausted {
                        budget: BudgetKind::StagedPayloadBytes,
                        ..
                    }
                ));
            } else if work == 293 {
                assert!(matches!(
                    fault.kind,
                    FaultKind::Exhausted {
                        budget: BudgetKind::Work,
                        ..
                    }
                ));
            } else {
                assert_eq!(
                    fault.kind,
                    FaultKind::Invalid(InvalidCode::EndpointMismatch)
                );
            }
            assert_eq!(budget.usage().work, if staging == 402 { 293 } else { work });
            assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, snapshot);
        }
    }
    Ok(())
}
