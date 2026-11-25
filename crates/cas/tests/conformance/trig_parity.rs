// SPDX-License-Identifier: AGPL-3.0-only
//! Parity reconstructs both sides from one original operand and one exact-domain proof.

use crate::testing::common::{TestResult, default_budget};
use crate::testing::half_pi::term;
use crate::testing::trig_parity::{fixture, operator};
use nrz_cas::proof::{HypothesisKind, Statement};
use nrz_cas::syntax::Operator;
use nrz_cas::{FaultKind, InvalidCode};

/// Original simple, composite and totality-derived guards preserve the same conditions.
/// ´test:integration:trig-parity-original-operands´
#[test]
fn trig_parity_original_operands() -> TestResult {
    for kind in 0..3 {
        for composite in [false, true] {
            for nested in [false, true] {
                let (mut f, function, _) = fixture(kind, composite, nested, false)?;
                let guard = f.cas.check_proof(
                    &f.context,
                    &f.steps[..f.steps.len() - 1],
                    &mut default_budget()?,
                )?;
                assert_eq!(
                    f.cas.statement_hypothesis_kind(
                        &guard.judgment().statement,
                        &mut crate::testing::budgets::graph_budget()?
                    )?,
                    HypothesisKind::DomainGuard
                );
                let checked = f
                    .cas
                    .check_proof(&f.context, &f.steps, &mut default_budget()?)?;
                assert_eq!(checked.judgment(), &f.expected);
                assert_eq!(checked.hypotheses(), guard.hypotheses());
                let produced =
                    f.cas
                        .trig_parity(&function, guard.proof(), &mut default_budget()?)?;
                assert_eq!(produced.judgment(), &f.expected);
                assert_eq!(produced.hypotheses(), guard.hypotheses());
                if nested {
                    let bytes = f.cas.export_witness(&produced, &mut default_budget()?)?;
                    assert!(
                        bytes
                            .windows(b"trig.defined".len())
                            .any(|word| word == b"trig.defined")
                    );
                }
            }
        }
    }
    Ok(())
}

/// A valid guard cannot join different operands, equal-spelled declarations or another domain.
/// ´test:integration:trig-parity-repeated-operand-mutations´
#[test]
fn trig_parity_repeated_operand_mutations() -> TestResult {
    use nrz_cas::Reference;
    use nrz_cas::domain::{AmbientDomain, RegionDraft, RegionExpr};
    use nrz_cas::syntax::Draft;
    for kind in 0..3 {
        for change in 0..4 {
            let (mut f, _, _) = fixture(kind, false, false, false)?;
            f.cas
                .check_proof(&f.context, &f.steps[..1], &mut default_budget()?)?;
            // x and y have the same displayed name u in one declaration table.
            let y = term(&mut f.cas, &f.header, Operator::Free("y".into()), &[])?;
            let negative = term(
                &mut f.cas,
                &f.header,
                Operator::Neg,
                std::slice::from_ref(&y),
            )?;
            let other_left = term(&mut f.cas, &f.header, operator(kind), &[negative])?;
            let other_function = term(&mut f.cas, &f.header, operator(kind), &[y])?;
            let other_right = if kind == 1 {
                other_function
            } else {
                term(&mut f.cas, &f.header, Operator::Neg, &[other_function])?
            };
            let mut steps = f.steps.clone();
            let Statement::Equality {
                left,
                right,
                domain,
                ..
            } = &mut steps.last_mut().ok_or("parity")?.conclusion
            else {
                return Err("equality".into());
            };
            match change {
                0 => *left = other_left,
                1 => *right = other_right,
                2 => {
                    *left = other_left;
                    *right = other_right;
                }
                _ => {
                    *domain = f
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
                        .ok_or("other domain")?;
                }
            }
            let snapshot = f.cas.canonical_snapshot(&mut default_budget()?)?;
            assert_eq!(
                f.cas
                    .check_proof(&f.context, &steps, &mut default_budget()?)
                    .err()
                    .ok_or("different operand")?
                    .kind,
                FaultKind::Invalid(InvalidCode::EndpointMismatch)
            );
            assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, snapshot);
            f.cas
                .check_proof(&f.context, &f.steps, &mut default_budget()?)?;
        }
    }
    Ok(())
}

/// Missing, excessive and forged original guards never acquire parity authority.
/// ´test:integration:trig-parity-guard-mutations´
#[test]
fn trig_parity_guard_mutations() -> TestResult {
    for kind in 0..3 {
        for change in 0..3 {
            let (mut f, _, _) = fixture(kind, true, false, false)?;
            let mut steps = f.steps.clone();
            match change {
                0 => {
                    steps.remove(0);
                    steps.last_mut().ok_or("parity")?.premises.clear();
                }
                1 => steps.last_mut().ok_or("parity")?.premises.push(0),
                _ => steps.first_mut().ok_or("guard")?.conclusion = f.expected.statement.clone(),
            }
            let snapshot = f.cas.canonical_snapshot(&mut default_budget()?)?;
            assert!(matches!(
                f.cas
                    .check_proof(&f.context, &steps, &mut default_budget()?)
                    .err()
                    .ok_or("malformed guard")?
                    .kind,
                FaultKind::Invalid(_)
            ));
            assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, snapshot);
            f.cas
                .check_proof(&f.context, &f.steps, &mut default_budget()?)?;
        }
    }
    Ok(())
}

/// Unsupported admitted payloads are borrowed before the fixed checkpoint or operator refusal.
/// ´test:integration:trig-parity-borrowed-early-exits´
#[test]
fn trig_parity_borrowed_early_exits() -> TestResult {
    use crate::testing::common::real;
    use crate::testing::half_pi::term;
    use nrz_cas::limits::{Budget, BudgetKind, Limits};
    use nrz_cas::syntax::{AtomicSort, Draft, ExpressionDraft, Field};
    use nrz_cas::{Location, Rational, Reference};
    let (mut f, _, _) = fixture(0, false, false, false)?;
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
            budget.work(37, Location::new("prior parity Work"))?;
            let fault = budget
                .with_retained_payload(19, Location::new("prior parity payload"), |budget| {
                    f.cas.trig_parity(&input, &input, budget)
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

/// A valid proof in another context cannot supply the original ordered guard.
/// ´test:integration:trig-parity-context-join´
#[test]
fn trig_parity_context_join() -> TestResult {
    use nrz_cas::Reference;
    use nrz_cas::domain::ContextDraft;
    use nrz_cas::syntax::Draft;
    for kind in 0..3 {
        let (mut f, _, _) = fixture(kind, true, false, false)?;
        f.cas
            .check_proof(&f.context, &f.steps[..1], &mut default_budget()?)?;
        let other = f
            .cas
            .admit(
                &[Draft::Context(ContextDraft::Root {
                    header: Reference::Existing(f.header.clone()),
                    admissible: Reference::Existing(f.domain.clone()),
                    assumptions: Vec::new(),
                })],
                &mut default_budget()?,
            )?
            .pop()
            .ok_or("other context")?;
        let mut changed = f.steps.clone();
        changed.last_mut().ok_or("parity")?.context = other.clone();
        let snapshot = f.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.cas
                .check_proof(&other, &changed, &mut default_budget()?)
                .err()
                .ok_or("context join")?
                .kind,
            FaultKind::Invalid(InvalidCode::EndpointMismatch)
        );
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, snapshot);
        f.cas
            .check_proof(&f.context, &f.steps, &mut default_budget()?)?;
    }
    Ok(())
}

/// Equal printed names in a different declaration frame cannot replace either endpoint.
/// ´test:integration:trig-parity-declaration-frame´
#[test]
fn trig_parity_declaration_frame() -> TestResult {
    use crate::testing::common::real;
    use nrz_cas::domain::HeaderDraft;
    use nrz_cas::syntax::{Declaration, Draft, Environment};
    for kind in 0..3 {
        let (mut f, _, _) = fixture(kind, false, false, false)?;
        let header = f
            .cas
            .admit(
                &[Draft::Header(HeaderDraft::Core(Environment {
                    declarations: vec![Declaration {
                        key: "x".into(),
                        display_name: "u".into(),
                        sort: real(),
                    }],
                    ..Environment::default()
                }))],
                &mut default_budget()?,
            )?
            .pop()
            .ok_or("other header")?;
        assert_ne!(header, f.header);
        let u = term(&mut f.cas, &header, Operator::Free("x".into()), &[])?;
        let negated = term(&mut f.cas, &header, Operator::Neg, &[u])?;
        let other = term(&mut f.cas, &header, operator(kind), &[negated])?;
        let mut changed = f.steps.clone();
        let Statement::Equality { left, .. } = &mut changed.last_mut().ok_or("parity")?.conclusion
        else {
            return Err("equality".into());
        };
        *left = other;
        let snapshot = f.cas.canonical_snapshot(&mut default_budget()?)?;
        assert!(matches!(
            f.cas
                .check_proof(&f.context, &changed, &mut default_budget()?)
                .err()
                .ok_or("declaration frame")?
                .kind,
            FaultKind::Invalid(_)
        ));
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, snapshot);
        f.cas
            .check_proof(&f.context, &f.steps, &mut default_budget()?)?;
    }
    Ok(())
}

/// One wrapper header changes while the operand, guard and admitted common context stay fixed.
/// ´test:integration:trig-parity-isolated-wrapper-frame´
#[test]
fn trig_parity_isolated_wrapper_frame() -> TestResult {
    use nrz_cas::Reference;
    use nrz_cas::domain::{AssumptionDraft, ContextDraft, HypothesisRef};
    use nrz_cas::proof::{ApplicationDraft, Certificate, DomainContract, Primitive};
    use nrz_cas::syntax::Draft;
    for kind in 0..3 {
        for right_side in [false, true] {
            let (mut f, function, original) = fixture(kind, false, false, false)?;
            let extended = extended_header(&mut f.cas, &f.header)?;
            let Statement::PredicateFact { predicate, .. } = &f.steps[0].conclusion else {
                return Err("guard".into());
            };
            let context = f
                .cas
                .admit(
                    &[Draft::Context(ContextDraft::Root {
                        header: nrz_cas::Reference::Existing(extended.clone()),
                        admissible: Reference::Existing(f.domain.clone()),
                        assumptions: vec![AssumptionDraft {
                            key: "original-defined".into(),
                            condition: Reference::Existing(predicate.clone()),
                        }],
                    })],
                    &mut default_budget()?,
                )?
                .pop()
                .ok_or("common context")?;
            for step in &mut f.steps {
                step.context = context.clone();
            }
            f.steps[0].certificate = Certificate::Hypothesis(HypothesisRef {
                context: context.clone(),
                slot: 0,
            });
            let accepted = f
                .cas
                .check_proof(&context, &f.steps, &mut default_budget()?)?;
            assert_eq!(accepted.hypotheses().len(), 1);
            let changed_wrapper = if right_side {
                if kind == 1 {
                    term(&mut f.cas, &extended, operator(kind), &[original])?
                } else {
                    term(&mut f.cas, &extended, Operator::Neg, &[function])?
                }
            } else {
                let negative = term(&mut f.cas, &f.header, Operator::Neg, &[original])?;
                term(&mut f.cas, &extended, operator(kind), &[negative])?
            };
            // A checked scalar endpoint in this same context rules out generic context rejection.
            f.cas.check_proof(
                &context,
                &[ApplicationDraft {
                    context: context.clone(),
                    rule: Primitive::Reflexivity,
                    conclusion: Statement::Equality {
                        left: changed_wrapper.clone(),
                        right: changed_wrapper.clone(),
                        domain: f.domain.clone(),
                        domain_contract: DomainContract::DefinedAndEqual,
                    },
                    premises: Vec::new(),
                    substitutions: Vec::new(),
                    certificate: Certificate::None,
                }],
                &mut default_budget()?,
            )?;
            let mut changed = f.steps.clone();
            let Statement::Equality { left, right, .. } = &mut changed[1].conclusion else {
                return Err("parity".into());
            };
            if right_side {
                *right = changed_wrapper;
            } else {
                *left = changed_wrapper;
            }
            let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
            assert_eq!(
                f.cas
                    .check_proof(&context, &changed, &mut default_budget()?)
                    .err()
                    .ok_or("wrapper frame accepted")?
                    .kind,
                FaultKind::Invalid(InvalidCode::EndpointMismatch)
            );
            assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
            f.cas
                .check_proof(&context, &f.steps, &mut default_budget()?)?;
        }
    }
    Ok(())
}

fn extended_header(
    cas: &mut nrz_cas::Cas,
    header: &nrz_cas::Handle,
) -> TestResult<nrz_cas::Handle> {
    use nrz_cas::domain::HeaderDraft;
    use nrz_cas::syntax::{Declaration, Draft};
    Ok(cas
        .admit(
            &[Draft::Header(HeaderDraft::Extend {
                parent: nrz_cas::Reference::Existing(header.clone()),
                declarations: vec![Declaration {
                    key: "unused".into(),
                    display_name: "unused".into(),
                    sort: crate::testing::common::real(),
                }],
                spaces: Vec::new(),
            })],
            &mut default_budget()?,
        )?
        .pop()
        .ok_or("extended header")?)
}

/// Case-domain evidence proves original definedness without any assumed predicate fact.
/// ´test:integration:trig-parity-discharged-domain-guard´
#[test]
fn trig_parity_discharged_domain_guard() -> TestResult {
    use crate::testing::cases::Fixture;
    use nrz_cas::request::Task;
    for kind in 0..3 {
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
            .ok_or("coverage")?;
        let guard = &root.premises.first().ok_or("exact domain")?.proof;
        let function = f.term(operator(kind), &[original])?;
        let witness = f
            .cas
            .trig_parity(&function, guard, &mut default_budget()?)?;
        assert_eq!(witness.hypotheses(), []);
        assert!(witness.is_discharged());
    }
    Ok(())
}

/// Opaque calls and direct or hidden undefined operands acquire no direct parity authority.
/// ´test:integration:trig-parity-opaque-undefined-operands´
#[test]
fn trig_parity_opaque_undefined_operands() -> TestResult {
    use crate::testing::trig::Fixture;
    for kind in 0..3 {
        let mut f = Fixture::new()?;
        let call = f.term(
            Operator::Call(crate::testing::trig::operators()[kind].1.into()),
            &[f.x.clone()],
        )?;
        // The opaque outer operator has no authority even before premise inspection.
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.cas
                .trig_parity(&call, &f.x, &mut default_budget()?)
                .err()
                .ok_or("opaque parity")?
                .kind,
            FaultKind::Invalid(InvalidCode::EndpointMismatch)
        );
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
        for hidden in [false, true] {
            let (mut f, _, _) = fixture(kind, false, false, false)?;
            let one = term(
                &mut f.cas,
                &f.header,
                Operator::Rational(nrz_cas::Rational::one()),
                &[],
            )?;
            let zero = term(
                &mut f.cas,
                &f.header,
                Operator::Rational(nrz_cas::Rational::zero()),
                &[],
            )?;
            let quotient = term(&mut f.cas, &f.header, Operator::Div, &[one, zero.clone()])?;
            let original = if hidden {
                term(&mut f.cas, &f.header, Operator::Mul, &[zero, quotient])?
            } else {
                quotient
            };
            let function = term(&mut f.cas, &f.header, operator(kind), &[original])?;
            let guard = f
                .cas
                .check_proof(&f.context, &f.steps[..1], &mut default_budget()?)?;
            // This valid proof defines x, not the undefined original candidate.
            // The direct producer must reject that join rather than normalize away the divisor.
            let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
            assert_eq!(
                f.cas
                    .trig_parity(&function, guard.proof(), &mut default_budget()?)
                    .err()
                    .ok_or("undefined parity")?
                    .kind,
                FaultKind::Invalid(InvalidCode::EndpointMismatch)
            );
            assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
        }
    }
    Ok(())
}
