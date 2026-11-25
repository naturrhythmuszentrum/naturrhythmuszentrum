// SPDX-License-Identifier: AGPL-3.0-only
//! Parity preserves both residual and qualified original-domain conditions.

use super::{TestResult, default_budget, fixture};

/// Closing an explicit local guard leaves a residual for the parity law to retain.
fn residual_fixture(kind: usize, reordered: bool) -> TestResult<crate::testing::half_pi::Fixture> {
    use nrz_cas::domain::{AssumptionDraft, ContextDraft, HypothesisRef, Layer};
    use nrz_cas::proof::{
        ApplicationDraft, Certificate, ConstructorLaw, GuardedObligation, Primitive, Statement,
    };
    use nrz_cas::{Reference, syntax::Draft};
    let (mut f, _, _) = fixture(kind, true, false, reordered)?;
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
    let Statement::Equality { domain, .. } = &mut consumer.conclusion else {
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
/// ´test:integration:trig-parity-condition-replay´
#[test]
fn trig_parity_condition_replay() -> TestResult {
    use crate::testing::answer::records;
    use nrz_cas::{FaultKind, InvalidCode};
    for kind in 0..3 {
        for residual in [false, true] {
            let prepared = |reordered| -> TestResult<_> {
                if residual {
                    residual_fixture(kind, reordered)
                } else {
                    Ok(fixture(kind, true, false, reordered)?.0)
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
