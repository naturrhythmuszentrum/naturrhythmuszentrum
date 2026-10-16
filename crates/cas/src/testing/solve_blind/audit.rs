// SPDX-License-Identifier: AGPL-3.0-only
//! Post-search audit records retain source identity, capabilities, ledgers and the actual checked DAG.

use crate::graph::{Cas, LogicalId, Vertex};
use crate::limits::{Budget, Limits};
use crate::proof_model::{ApplicationDraft, VerifiedWitness};
use crate::solving::radicals::route::Audit;
use crate::testing::common::TestResult;
use std::collections::BTreeMap;

/// Reconstruct an independently checked proof's applications without using producer scratch history.
fn visit(
    cas: &Cas,
    proof: LogicalId,
    indices: &mut BTreeMap<LogicalId, u32>,
    steps: &mut Vec<ApplicationDraft>,
) -> TestResult<u32> {
    if let Some(index) = indices.get(&proof) {
        return Ok(*index);
    }
    let Vertex::Proof(application) = cas.state.node(proof)? else {
        return Err("audit proof root".into());
    };
    let Vertex::ScopedApplication(application) = cas.state.node(*application)? else {
        return Err("audit scoped application".into());
    };
    let mut premises = Vec::new();
    for previous in &application.premises {
        premises.push(visit(cas, *previous, indices, steps)?);
    }
    let index = u32::try_from(steps.len())?;
    steps.push(ApplicationDraft {
        rule: application.rule,
        context: application.judgment.context.clone(),
        conclusion: application.judgment.statement.clone(),
        premises,
        substitutions: application.substitutions.clone(),
        certificate: application.certificate.clone(),
    });
    indices.insert(proof, index);
    Ok(index)
}

/// Reconstructed certificates retain exact graph handles; only their untrusted payloads may be mutated.
pub(crate) fn applications(
    cas: &Cas,
    witness: &VerifiedWitness,
) -> TestResult<Vec<ApplicationDraft>> {
    let mut steps = Vec::new();
    visit(
        cas,
        witness.proof.logical_id(),
        &mut BTreeMap::new(),
        &mut steps,
    )?;
    Ok(steps)
}

/// Pending coefficient-map interpretation design restores the full symbolic acceptance audit.
/// The current refusal retains the actual phase and interning evidence without claiming a witness.
pub(crate) fn symbolic(source_revision: &str) -> TestResult<String> {
    let (mut fixture, request) = super::source::symbolic(&["d", "c", "b", "a"])?;
    let limits = Limits::blind_cubic();
    let mut generation = Budget::new(limits.clone())?;
    let mut checker = Budget::new(limits.clone())?;
    let mut audit = Audit::default();
    let before = fixture.cas.interning_snapshot();
    let result = crate::solving::solve_search::produce(
        &mut fixture.cas,
        &request,
        &mut audit,
        &mut generation,
        &mut checker,
    );
    let after = fixture.cas.interning_snapshot();
    let comparisons = after
        .comparisons
        .checked_sub(before.comparisons)
        .ok_or("interning comparison counter regressed")?;
    let compared_bytes = after
        .compared_bytes
        .checked_sub(before.compared_bytes)
        .ok_or("interning byte counter regressed")?;
    let failure = result
        .err()
        .ok_or("symbolic coefficient boundary moved; restore acceptance audit")?;
    let crate::solving::coefficients::model::CoefficientFailure::Fault(fault) = failure else {
        return Err("symbolic boundary changed: coefficient failure is not a fault".into());
    };
    assert_eq!(
        fault.kind,
        crate::FaultKind::Invalid(crate::InvalidCode::UnsupportedFragment)
    );
    assert_eq!(fault.location.operation, "exact coefficients");
    let manifest: Vec<_> = crate::rule::PRIMITIVES
        .iter()
        .map(|rule| {
            let specification = rule.specification();
            (specification.key, specification.version)
        })
        .collect();
    assert_eq!(manifest.len(), 77);
    assert!(manifest.iter().all(|(_, version)| *version == 1));
    assert_eq!(crate::solving::active_schema_bitmap() & 0x1_7fff, 0x1_7fff);
    assert!(generation.usage().coefficient_ops > 0);
    assert!(generation.usage().work <= limits.work);
    assert!(checker.usage().work <= limits.replay_work);
    let policy = crate::cost::CostPolicy::default();
    assert_eq!((&*policy.key, policy.version), ("logical", 9));
    Ok(format!(
        "source_revision={source_revision}\nrefusal={fault:?}\npolicy=logical-v9\nlimits={limits:?}\nschema_manifest={manifest:?}\nactive_bitmap={}\ninterning_interval=produce\ninterning_comparisons={comparisons}\ninterning_compared_bytes={compared_bytes}\nphase_format=((phase,index),region,depth,seed,coefficient_ops,gcd_steps,work)\nphases={:#?}\ncomponents={:#?}\ncoefficient_terms={:#?}\nseeds={:?}\nfinal_generation={:?}\nreplay={:?}\n",
        crate::solving::active_schema_bitmap(),
        audit.phases,
        audit.components,
        audit.coefficient_terms,
        audit.attempts,
        generation.usage(),
        checker.usage()
    ))
}

/// Measure the actual checker path, then exercise the public service at that exact replay cap.
pub(crate) fn retained_replay_boundary() -> TestResult {
    let (mut fixture, request) = super::source::polynomial(&[8, 0, 0, 1])?;
    let limits = Limits::blind_cubic();
    let mut checker = Budget::new(limits.clone())?;
    let baseline = crate::solving::solve_search::produce(
        &mut fixture.cas,
        &request,
        &mut Audit::default(),
        &mut Budget::new(limits.clone())?,
        &mut checker,
    )?;
    assert!(baseline.fault.is_none());
    assert_eq!(baseline.answer.unresolved_regions, []);
    let required = checker.usage().work;
    for cap in [
        required,
        required.checked_sub(1).ok_or("empty replay ledger")?,
    ] {
        let (mut fixture, mut request) = super::source::polynomial(&[8, 0, 0, 1])?;
        request.limits.replay_work = Some(cap);
        let outcome = fixture.cas.run(&request);
        if cap == required {
            assert!(
                matches!(outcome, crate::request::Outcome::Solved { .. }),
                "{outcome:?}"
            );
        } else {
            let crate::request::Outcome::Exhausted {
                solve_best: Some(best),
                fault,
                replay_usage,
                ..
            } = outcome
            else {
                return Err(format!("missing retained coverage: {outcome:?}").into());
            };
            assert!(
                matches!(fault.kind, crate::error::FaultKind::Exhausted { budget: crate::limits::BudgetKind::Work, limit, needed, .. } if limit == cap && needed > cap)
            );
            assert!(replay_usage.work <= cap);
            assert_eq!(best.form, crate::solving::SolveForm::AnyExact);
            assert!(best.facts.is_empty());
            let coverage = best.coverage.ok_or("lost checked coverage")?;
            assert!(coverage.is_discharged());
            let bytes = fixture
                .cas
                .export_witness(&coverage, &mut Budget::new(limits.clone())?)?;
            let mut fresh = Cas::new(limits.clone())?;
            let witness = fresh.inspect(&bytes, &mut Budget::new(limits.clone())?)?;
            assert!(witness.is_discharged());
            assert_eq!(
                fresh.export_witness(&witness, &mut Budget::new(limits.clone())?)?,
                bytes
            );
        }
    }
    Ok(())
}

/// The returned coverage actually consumes the closed formal derivation in its original context.
pub(crate) fn formal_consumer() -> TestResult {
    let (mut fixture, request) = super::source::polynomial(&[-4, -15, 0, 1])?;
    let outcome = fixture.cas.run(&request);
    let crate::request::Outcome::Solved { answer, .. } = outcome else {
        return Err(format!("formal blind consumer: {outcome:?}").into());
    };
    let coverage = answer.coverage.ok_or("formal consumer coverage")?;
    assert!(coverage.is_discharged());
    let steps = applications(&fixture.cas, &coverage)?;
    formal_steps(&steps)?;
    let bytes = fixture
        .cas
        .export_witness(&coverage, &mut Budget::new(Limits::blind_cubic())?)?;
    let mut fresh = Cas::new(Limits::blind_cubic())?;
    let replayed = fresh.inspect(&bytes, &mut Budget::new(Limits::blind_cubic())?)?;
    assert!(replayed.is_discharged());
    formal_steps(&applications(&fresh, &replayed)?)?;
    assert_eq!(
        fresh.export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}

/// A scoped expansion, complete anchored discharge and ordinary radical consumer remain reachable.
fn formal_steps(steps: &[ApplicationDraft]) -> TestResult {
    use crate::proof_model::Certificate;
    use crate::rule::Primitive;
    use crate::solving::{SolveCertificate, SolveSchema};
    let mut consumed = 0;
    for (index, step) in steps.iter().enumerate() {
        if step.rule != Primitive::ContextClose {
            continue;
        }
        let [source] = step.premises.as_slice() else {
            continue;
        };
        let source = steps
            .get(usize::try_from(*source)?)
            .ok_or("closed source")?;
        let Certificate::Solve(certificate) = &source.certificate else {
            continue;
        };
        let SolveCertificate::CoefficientIdentity(identity) = certificate.as_ref() else {
            continue;
        };
        assert_ne!(identity.combinations, []);
        assert!(identity.combinations.iter().all(|row| {
            row.hypothesis
                .as_ref()
                .is_some_and(|hypothesis| hypothesis.vieta.is_some())
        }));
        let expanded = source
            .premises
            .iter()
            .filter_map(|premise| steps.get(usize::try_from(*premise).ok()?))
            .any(|earlier| {
                let Certificate::Solve(certificate) = &earlier.certificate else {
                    return false;
                };
                let SolveCertificate::CoefficientIdentity(value) = certificate.as_ref() else {
                    return false;
                };
                value.left != value.right
                    && value.combinations.iter().any(|row| {
                        row.hypothesis
                            .as_ref()
                            .is_some_and(|hypothesis| hypothesis.vieta.is_none())
                    })
            });
        let visible = source
            .premises
            .iter()
            .filter_map(|premise| steps.get(usize::try_from(*premise).ok()?))
            .any(|earlier| {
                let Certificate::Solve(certificate) = &earlier.certificate else {
                    return false;
                };
                let SolveCertificate::CoefficientIdentity(value) = certificate.as_ref() else {
                    return false;
                };
                value.combinations.iter().any(|row| {
                    row.hypothesis
                        .as_ref()
                        .is_some_and(|hypothesis| hypothesis.vieta.is_none())
                })
            });
        assert!(
            visible,
            "missing visible expansion, including constant identities"
        );
        let index = u32::try_from(index)?;
        consumed += usize::from(
            expanded
                && steps.iter().any(|consumer| {
                    consumer.context == step.context
                        && matches!(
                            consumer.rule,
                            Primitive::Solve(
                                SolveSchema::RootAdjunction | SolveSchema::ProductCompleteness
                            )
                        )
                        && consumer.premises.contains(&index)
                }),
        );
    }
    assert_ne!(consumed, 0, "missing ordinary blind radical consumer");
    Ok(())
}

/// The blind ground-binomial route supplies an actual named real witness before complex refinement.
pub(crate) fn real_choice_consumer() -> TestResult {
    let (mut fixture, request) = super::source::polynomial(&[8, 0, 0, 1])?;
    let crate::request::Outcome::Solved { answer, .. } = fixture.cas.run(&request) else {
        return Err("real-choice cubic did not solve".into());
    };
    let coverage = answer.coverage.ok_or("real-choice coverage")?;
    assert!(coverage.is_discharged());
    let steps = applications(&fixture.cas, &coverage)?;
    assert!(steps.iter().any(|step| matches!(&step.certificate, crate::proof_model::Certificate::Solve(certificate)
        if matches!(certificate.as_ref(), crate::solving::SolveCertificate::RootAdjunction(value) if value.mode == crate::solving::AdjunctionMode::RealOddRoot))));
    assert!(steps.iter().any(|step| matches!(&step.certificate, crate::proof_model::Certificate::Solve(certificate)
        if matches!(certificate.as_ref(), crate::solving::SolveCertificate::RootSelectionRefinement(value) if value.mode == crate::solving::RefinementMode::Existence))));
    let bytes = fixture
        .cas
        .export_witness(&coverage, &mut Budget::new(Limits::blind_cubic())?)?;
    let mut fresh = Cas::new(Limits::blind_cubic())?;
    let replayed = fresh.inspect(&bytes, &mut Budget::new(Limits::blind_cubic())?)?;
    assert!(replayed.is_discharged());
    assert_eq!(
        fresh.export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
        bytes
    );
    Ok(())
}
