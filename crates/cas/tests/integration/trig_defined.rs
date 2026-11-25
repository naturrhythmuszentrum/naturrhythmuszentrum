// SPDX-License-Identifier: AGPL-3.0-only
//! Totality witnesses replay against independently constructed original-domain claims.

use crate::testing::common::{TestResult, default_budget};
use crate::testing::trig_defined::fixture;
use nrz_cas::syntax::Operator;

/// Closing an explicit local guard leaves a residual for the totality law to retain.
fn residual_fixture(
    operator: Operator,
    reordered: bool,
) -> TestResult<crate::testing::half_pi::Fixture> {
    use nrz_cas::domain::{AssumptionDraft, ContextDraft, HypothesisRef, Layer};
    use nrz_cas::proof::{
        ApplicationDraft, Certificate, ConstructorLaw, GuardedObligation, Primitive, Statement,
    };
    use nrz_cas::{Reference, syntax::Draft};
    let (mut f, _) = fixture(operator, true, reordered)?;
    let Statement::PredicateFact { predicate, .. } = &f.steps.first().ok_or("guard")?.conclusion
    else {
        return Err("guard predicate".into());
    };
    let predicate = predicate.clone();
    let frame = f
        .cas
        .admit(
            &[Draft::Context(ContextDraft::Local {
                parent: Reference::Existing(f.context.clone()),
                region: Reference::Existing(f.domain.clone()),
                hypotheses: vec![AssumptionDraft {
                    key: "local-original-defined".into(),
                    condition: Reference::Existing(predicate.clone()),
                }],
            })],
            &mut default_budget()?,
        )?
        .pop()
        .ok_or("frame")?;
    let region = f
        .cas
        .admissible_region(&frame, &mut crate::testing::budgets::graph_budget()?)?;
    let fact = Statement::PredicateFact {
        region: region.clone(),
        predicate: predicate.clone(),
    };
    let mut consumer = f.steps.pop().ok_or("consumer")?;
    let Statement::PredicateFact { region: domain, .. } = &mut consumer.conclusion else {
        return Err("conclusion".into());
    };
    *domain = region.clone();
    consumer.premises = vec![2];
    f.expected.statement = consumer.conclusion.clone();
    f.expected.residuals = vec![GuardedObligation {
        home: f.context.clone(),
        scope: Vec::new(),
        layer: Layer::Logic,
        space: None,
        region: region.clone(),
        predicate,
    }];
    f.steps = vec![
        ApplicationDraft {
            rule: Primitive::ContextHypothesis,
            context: frame.clone(),
            conclusion: fact.clone(),
            certificate: Certificate::Hypothesis(HypothesisRef {
                context: frame.clone(),
                slot: 1,
            }),
            premises: Vec::new(),
            substitutions: Vec::new(),
        },
        ApplicationDraft {
            rule: Primitive::RegionConstruct,
            context: f.context.clone(),
            conclusion: Statement::Constructor {
                region,
                law: ConstructorLaw::Membership,
            },
            certificate: Certificate::None,
            premises: Vec::new(),
            substitutions: Vec::new(),
        },
        ApplicationDraft {
            rule: Primitive::ContextClose,
            context: f.context.clone(),
            conclusion: fact,
            certificate: Certificate::Close { frame },
            premises: vec![0, 1],
            substitutions: Vec::new(),
        },
        consumer,
    ];
    Ok(f)
}

/// Unchanged valid bytes retain the original hypothesis or residual on both receiving orders.
/// ´test:integration:trig-defined-condition-replay´
#[test]
fn trig_defined_condition_replay() -> TestResult {
    use crate::testing::answer::records;
    use nrz_cas::{FaultKind, InvalidCode};
    for operator in [Operator::Sine, Operator::Cosine, Operator::Arctangent] {
        for residual in [false, true] {
            let prepared = |reordered| -> TestResult<_> {
                if residual {
                    residual_fixture(operator.clone(), reordered)
                } else {
                    Ok(fixture(operator.clone(), true, reordered)?.0)
                }
            };
            let mut source = prepared(false)?;
            let proof =
                source
                    .cas
                    .check_proof(&source.context, &source.steps, &mut default_budget()?)?;
            assert_eq!(proof.judgment(), &source.expected);
            assert_eq!(proof.hypotheses().len(), usize::from(!residual));
            let bytes = source.cas.export_witness(&proof, &mut default_budget()?)?;
            let inventory = records(&bytes)?;
            let (_, root) = inventory.iter().find(|(tag, _)| *tag == 12).ok_or("root")?;
            let (length, count, start, end) = if residual {
                (27_u32, 15, 19, 23)
            } else {
                (35, 19, 23, 35)
            };
            assert_eq!(root.len(), usize::try_from(length)?);
            assert_eq!(
                bytes.get(root.start - 4..root.start),
                Some(length.to_le_bytes().as_slice())
            );
            assert_eq!(
                bytes.get(root.start + count..root.start + count + 4),
                Some(1_u32.to_le_bytes().as_slice())
            );
            let mut changed = bytes.clone();
            changed
                .get_mut(root.start + count..root.start + count + 4)
                .ok_or("count")?
                .copy_from_slice(&0_u32.to_le_bytes());
            changed.drain(root.start + start..root.start + end);
            changed
                .get_mut(root.start - 4..root.start)
                .ok_or("length")?
                .copy_from_slice(&(length - u32::try_from(end - start)?).to_le_bytes());
            assert_eq!(records(&changed)?.len(), inventory.len());
            for reordered in [false, true] {
                let mut receiver = prepared(reordered)?;
                let snapshot = receiver.cas.canonical_snapshot(&mut default_budget()?)?;
                let fault = receiver
                    .cas
                    .replay(&changed, &receiver.expected, &mut default_budget()?)
                    .err()
                    .ok_or("dropped condition")?;
                assert_eq!(
                    fault.kind,
                    FaultKind::Invalid(InvalidCode::ConditionMismatch)
                );
                assert_eq!(
                    receiver.cas.canonical_snapshot(&mut default_budget()?)?,
                    snapshot
                );
                let checked =
                    receiver
                        .cas
                        .replay(&bytes, &receiver.expected, &mut default_budget()?)?;
                assert_eq!(
                    receiver
                        .cas
                        .export_witness(&checked, &mut default_budget()?)?,
                    bytes
                );
                if residual {
                    let mut stripped = receiver.expected.clone();
                    stripped.residuals.clear();
                    let snapshot = receiver.cas.canonical_snapshot(&mut default_budget()?)?;
                    assert!(
                        receiver
                            .cas
                            .replay(&bytes, &stripped, &mut default_budget()?)
                            .is_err()
                    );
                    assert_eq!(
                        receiver.cas.canonical_snapshot(&mut default_budget()?)?,
                        snapshot
                    );
                }
            }
        }
    }
    Ok(())
}

/// Receiving stores build claims independently in either allocation order.
/// ´test:integration:trig-defined-fresh-replay´
#[test]
fn trig_defined_fresh_replay() -> TestResult {
    for operator in [Operator::Sine, Operator::Cosine, Operator::Arctangent] {
        let (mut source, _) = fixture(operator.clone(), true, false)?;
        let witness =
            source
                .cas
                .check_proof(&source.context, &source.steps, &mut default_budget()?)?;
        let bytes = source
            .cas
            .export_witness(&witness, &mut default_budget()?)?;
        for reordered in [false, true] {
            let (mut f, _) = fixture(operator.clone(), true, reordered)?;
            let one = crate::testing::half_pi::term(
                &mut f.cas,
                &f.header,
                Operator::Rational(nrz_cas::Rational::one()),
                &[],
            )?;
            let other =
                crate::testing::half_pi::term(&mut f.cas, &f.header, operator.clone(), &[one])?;
            let predicate = f
                .cas
                .admit(
                    &[nrz_cas::syntax::Draft::Condition {
                        header: nrz_cas::Reference::Existing(f.header.clone()),
                        scope: Vec::new(),
                        kind: nrz_cas::syntax::PredicateKind::Defined,
                        arguments: vec![nrz_cas::Reference::Existing(other)],
                    }],
                    &mut default_budget()?,
                )?
                .pop()
                .ok_or("different endpoint")?;
            let mut changed = f.expected.clone();
            let nrz_cas::proof::Statement::PredicateFact {
                predicate: endpoint,
                ..
            } = &mut changed.statement
            else {
                return Err("predicate claim".into());
            };
            *endpoint = predicate;
            let snapshot = f.cas.canonical_snapshot(&mut default_budget()?)?;
            assert!(
                f.cas
                    .replay(&bytes, &changed, &mut default_budget()?)
                    .is_err()
            );
            assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, snapshot);
            let imported = f.cas.replay(&bytes, &f.expected, &mut default_budget()?)?;
            assert_eq!(
                f.cas.export_witness(&imported, &mut default_budget()?)?,
                bytes
            );
            let (mut different, _) = fixture(operator.clone(), false, reordered)?;
            let snapshot = different.cas.canonical_snapshot(&mut default_budget()?)?;
            assert!(
                different
                    .cas
                    .replay(&bytes, &different.expected, &mut default_budget()?)
                    .is_err()
            );
            assert_eq!(
                different.cas.canonical_snapshot(&mut default_budget()?)?,
                snapshot
            );
        }
    }
    Ok(())
}

/// Complete witnesses retain pinned premise slots, predicate statements and empty certificates.
/// ´test:integration:trig-defined-wire-mutations´
#[test]
fn trig_defined_wire_mutations() -> TestResult {
    use crate::testing::answer::{index, records};
    for operator in [Operator::Sine, Operator::Cosine, Operator::Arctangent] {
        let (mut source, _) = fixture(operator.clone(), true, false)?;
        let witness =
            source
                .cas
                .check_proof(&source.context, &source.steps, &mut default_budget()?)?;
        let bytes = source
            .cas
            .export_witness(&witness, &mut default_budget()?)?;
        let records = records(&bytes)?;
        let (_, application) = records
            .iter()
            .rev()
            .find(|(tag, _)| *tag == 11)
            .ok_or("application")?;
        assert_eq!(application.len(), 27);
        assert_eq!(
            bytes.get(application.start + 14..application.start + 18),
            Some(1_u32.to_le_bytes().as_slice())
        );
        let conclusion = index(&bytes, &mut (application.start + 10))?;
        let (tag, statement) = records
            .get(usize::try_from(conclusion)?)
            .ok_or("statement")?;
        assert_eq!(*tag, 10);
        assert_eq!(statement.len(), 19);
        assert_eq!(bytes.get(statement.start + 10), Some(&1));
        assert_eq!(bytes.get(application.end - 1), Some(&0));
        let mut changes = Vec::new();
        let key = b"trig.defined";
        let offsets: Vec<_> = bytes
            .windows(key.len())
            .enumerate()
            .filter_map(|(at, value)| (value == key).then_some(at))
            .collect();
        let [at] = offsets.as_slice() else {
            return Err("unique totality schema".into());
        };
        assert_eq!(
            bytes.get(at - 4..*at),
            Some(u32::try_from(key.len())?.to_le_bytes().as_slice())
        );
        let version = at + key.len();
        assert_eq!(
            bytes.get(version..version + 4),
            Some(1_u32.to_le_bytes().as_slice())
        );
        let mut wrong = bytes.clone();
        wrong
            .get_mut(version..version + 4)
            .ok_or("version")?
            .copy_from_slice(&2_u32.to_le_bytes());
        changes.push(wrong);
        let mut wrong = bytes.clone();
        wrong[statement.start + 10] = 32;
        changes.push(wrong);
        let mut wrong = bytes.clone();
        wrong[application.end - 1] = 20;
        changes.push(wrong);
        let mut wrong = bytes.clone();
        wrong
            .get_mut(application.start + 18..application.start + 22)
            .ok_or("premise")?
            .copy_from_slice(&conclusion.to_le_bytes());
        changes.push(wrong);
        for reordered in [false, true] {
            let (mut f, _) = fixture(operator.clone(), true, reordered)?;
            for changed in &changes {
                let snapshot = f.cas.canonical_snapshot(&mut default_budget()?)?;
                assert!(
                    f.cas
                        .replay(changed, &f.expected, &mut default_budget()?)
                        .is_err()
                );
                assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, snapshot);
                f.cas.replay(&bytes, &f.expected, &mut default_budget()?)?;
            }
        }
    }
    Ok(())
}
