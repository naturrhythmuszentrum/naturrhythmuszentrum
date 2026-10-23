// SPDX-License-Identifier: AGPL-3.0-only
//! Post-search audit records retain source identity, capabilities, ledgers and the actual checked DAG.

use crate::graph::{Cas, Handle, LogicalId, Vertex};
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

/// Each boundary is constructed independently, including both source and renaming spellings.
/// The Real fixture records its explicit request revision and its actual same-Real initial map.
pub(crate) fn symbolic(source_revision: &str) -> TestResult<String> {
    use crate::model::Field;
    let ordinary = ["d", "c", "b", "a"];
    let renamed = ["delta", "gamma", "beta", "alpha"];
    let mut record = format!("source_revision={source_revision}\n{}", policy_record());
    for (name, keys) in [
        ("blind_symbolic_complex_complete", &ordinary),
        ("blind_symbolic_source_retains_parameters", &ordinary),
        ("blind_symbolic_source_retains_parameters", &renamed),
        ("blind_symbolic_acceptance_audit", &ordinary),
        ("blind_complete_coefficient_renaming", &ordinary),
        ("blind_complete_coefficient_renaming", &renamed),
        ("blind_missing_stratum_mutations", &ordinary),
    ] {
        record.push_str(&boundary(name, keys, &Field::Complex)?);
    }
    record.push_str(&boundary(
        "blind_symbolic_real_complete",
        &ordinary,
        &Field::Real,
    )?);
    Ok(record)
}

/// The runtime baseline is recorded independently of the future symbol-sorts manifest.
fn policy_record() -> String {
    let manifest: Vec<_> = crate::rule::PRIMITIVES
        .iter()
        .map(|rule| {
            let specification = rule.specification();
            (specification.key, specification.version)
        })
        .collect();
    assert_eq!(manifest.len(), 80);
    assert!(
        manifest
            .iter()
            .all(|(key, version)| *version == if *key == "region.decide" { 2 } else { 1 })
    );
    assert_eq!(crate::solving::active_schema_bitmap() & 0x1_5fff, 0x1_5fff);
    let policy = crate::cost::CostPolicy::default();
    assert_eq!((&*policy.key, policy.version), ("logical", 10));
    format!(
        "policy=logical-v10\nschema_manifest={manifest:?}\nactive_bitmap={}\n",
        crate::solving::active_schema_bitmap()
    )
}

/// Read declarations, unknown coordinates and the original form from this fixture's actual graph.
fn declaration_record(
    fixture: &super::source::Fixture,
    request: &crate::request_model::Request,
    expected: &crate::model::Field,
) -> TestResult<String> {
    use crate::model::{AtomicSort, Sort};
    let crate::task::Task::Solve(specification) = &request.task else {
        return Err("boundary is not a solve request".into());
    };
    let problem = crate::solving::elementary::proof::problem(&fixture.cas, &specification.problem)?;
    let environment = &fixture
        .cas
        .state
        .header(problem.header.logical_id())?
        .environment;
    assert_eq!(environment.declarations.len(), 5);
    assert!(
        environment.declarations.iter().all(
            |declaration| declaration.sort == Sort::Value(AtomicSort::Scalar(expected.clone()))
        )
    );
    let unknowns = crate::solving::elementary::proof::coordinates(&fixture.cas, &problem.unknowns)?;
    assert_eq!(unknowns.coordinates.len(), 1);
    assert!(
        unknowns
            .coordinates
            .iter()
            .all(|coordinate| coordinate.field == *expected)
    );
    let parameters =
        crate::solving::elementary::proof::coordinates(&fixture.cas, &problem.coefficients)?;
    assert_eq!(parameters.coordinates.len(), 4);
    assert!(
        parameters
            .coordinates
            .iter()
            .all(|coordinate| coordinate.field == *expected)
    );
    assert_eq!(problem.ambient, *expected);
    assert_eq!(specification.form, crate::solving::SolveForm::Radicals);
    assert_eq!(specification.goal, crate::solving::SolveGoal::Complete);
    Ok(format!(
        "declarations={:?}\nunknowns={:?}\ncoefficient_coordinates={:?}\ndeclaration_field={expected:?}\noriginal_form={:?}\n",
        environment.declarations, unknowns.coordinates, parameters.coordinates, specification.form
    ))
}

/// Reconstruct the producer's initial map from the actual ordered fraction frame, before search.
fn initial_map(fixture: &mut super::source::Fixture) -> TestResult<String> {
    use crate::solving::{CoefficientDomainData, MapMode, MapTarget, SolveView};
    let CoefficientDomainData::Fraction { base, coordinates } =
        crate::solving::coefficients::storage::domain(&fixture.cas.state, &fixture.algebra)?
    else {
        return Err("boundary coefficient domain is not a fraction field".into());
    };
    assert!(matches!(
        crate::solving::coefficients::storage::domain(&fixture.cas.state, base)?,
        CoefficientDomainData::Rational
    ));
    let frame = crate::solving::elementary::proof::coordinates(&fixture.cas, coordinates)?;
    let keys: Vec<_> = frame
        .coordinates
        .iter()
        .map(|coordinate| coordinate.key.clone())
        .collect();
    let mut images = Vec::new();
    for key in &keys {
        images.push(fixture.free(key)?);
    }
    let construction = crate::solving::radicals::construction::Construction {
        header: fixture.header.clone(),
        context: fixture.context.clone(),
        region: fixture.region.clone(),
        steps: Vec::new(),
    };
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let map = if fixture.field == crate::model::Field::Real {
        fixture
            .cas
            .admit(
                &[crate::graph::Draft::Solve(
                    crate::solving::SolveDraft::CoefficientMap(
                        crate::solving::CoefficientMapData {
                            source: crate::Reference::Existing(fixture.algebra.clone()),
                            target: MapTarget::ScalarAmbient {
                                header: crate::Reference::Existing(fixture.header.clone()),
                                field: fixture.field.clone(),
                            },
                            mode: MapMode::Specialization,
                            region: crate::Reference::Existing(fixture.region.clone()),
                            images: images
                                .iter()
                                .cloned()
                                .map(crate::Reference::Existing)
                                .collect(),
                            denominators: Vec::new(),
                        },
                    ),
                )],
                &mut budget,
            )?
            .into_iter()
            .next()
            .ok_or("Real initial map")?
    } else {
        construction.mapping(&mut fixture.cas, &fixture.algebra, &images, &mut budget)?
    };
    let SolveView::CoefficientMap(mapping) = fixture.cas.solve_view(&map)? else {
        return Err("initial map missing".into());
    };
    crate::solving::coefficients::homomorphism::check(
        &fixture.cas,
        mapping,
        &[],
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(mapping.source, fixture.algebra);
    assert_eq!(mapping.mode, MapMode::Specialization);
    assert_eq!(mapping.region, fixture.region);
    assert_eq!(mapping.images, images);
    assert_eq!(mapping.denominators, [] as [Handle; 0]);
    assert_eq!(
        mapping.target,
        MapTarget::ScalarAmbient {
            header: fixture.header.clone(),
            field: fixture.field.clone()
        }
    );
    Ok(format!(
        "initial_map={mapping:?}\ninitial_map_source=Q({})\ninitial_map_images={keys:?}\ninitial_map_target=ScalarAmbient({:?})\ninitial_map_check=exact-Free accepted\n",
        keys.join(","),
        fixture.field
    ))
}

/// Rebuilding a fixture never infers its field, map or form from another boundary's refusal.
fn boundary(name: &str, keys: &[&str], field: &crate::model::Field) -> TestResult<String> {
    let (mut fixture, request) = super::source::symbolic_in_field(keys, field)?;
    let declarations = declaration_record(&fixture, &request, field)?;
    let mapping = initial_map(&mut fixture)?;
    let measurement = if *field == crate::model::Field::Complex {
        complex_acceptance(&mut fixture, &request, keys)?
    } else {
        let mut request = request;
        let crate::task::Task::Solve(value) = &mut request.task else {
            return Err("Real audit request".into());
        };
        assert_eq!(value.form, crate::solving::SolveForm::Radicals);
        value.form = crate::solving::SolveForm::AnyExact;
        super::real_acceptance::measurement(&mut fixture, &request)?
    };
    Ok(format!(
        "boundary={name}\nkeys={keys:?}\n{declarations}{mapping}{measurement}"
    ))
}

/// Record real generation, reconstruction, independent samples and fresh expected-claim replay.
fn complex_acceptance(
    fixture: &mut super::source::Fixture,
    request: &crate::request_model::Request,
    keys: &[&str],
) -> TestResult<String> {
    let limits = Limits::blind_cubic();
    let mut generation = Budget::new(limits.clone())?;
    let mut checker = Budget::new(limits.clone())?;
    let mut audit = Audit::default();
    let before = fixture.cas.interning_snapshot();
    let result = crate::solving::solve_search::produce(
        &mut fixture.cas,
        request,
        &mut audit,
        &mut generation,
        &mut checker,
    )?;
    let after = fixture.cas.interning_snapshot();
    assert!(result.fault.is_none());
    assert_eq!(result.answer.form, crate::solving::SolveForm::Radicals);
    assert_eq!(result.answer.unresolved_regions, [] as [Handle; 0]);
    let coverage = result.answer.coverage.as_ref().ok_or("audit coverage")?;
    assert!(coverage.is_discharged());
    let steps = applications(&fixture.cas, coverage)?;
    reached(&steps)?;
    let inventory = super::oracle::samples(&fixture.cas, coverage, keys)?;
    super::acceptance::fresh(&fixture.cas, &result.answer, keys)?;
    assert!(generation.usage().work <= limits.work);
    assert!(checker.usage().work > 0);
    let comparisons = after
        .comparisons
        .checked_sub(before.comparisons)
        .ok_or("comparison counter")?;
    let compared_bytes = after
        .compared_bytes
        .checked_sub(before.compared_bytes)
        .ok_or("byte counter")?;
    Ok(format!(
        "result=Solved(Radicals)\nchecked_applications={}\nindependent_strata={inventory:?}\nfresh_coverage=true\nfresh_presentation=true\nlimits={limits:?}\ninterning_interval=produce\ninterning_comparisons={comparisons}\ninterning_compared_bytes={compared_bytes}\nphase_format=((phase,index),region,depth,seed,coefficient_ops,gcd_steps,work)\nphases={:#?}\ncomponents={:#?}\ncoefficient_terms={:#?}\nseeds={:?}\nfinal_generation={:?}\nreplay={:?}\n",
        steps.len(),
        audit.phases,
        audit.components,
        audit.coefficient_terms,
        audit.attempts,
        generation.usage(),
        checker.usage()
    ))
}

/// Inventory comes from sealed applications, independently of observer events and producer scratch.
fn reached(steps: &[ApplicationDraft]) -> TestResult {
    use crate::proof_model::Certificate;
    use crate::rule::Primitive;
    use crate::solving::{SolveCertificate, SolveSchema};
    for schema in [
        SolveSchema::PermutationOrbitCoverage,
        SolveSchema::CoefficientIdentity,
        SolveSchema::RootAdjunction,
        SolveSchema::ProductCompleteness,
        SolveSchema::TotalPredicateSplit,
    ] {
        assert!(
            steps
                .iter()
                .any(|step| step.rule == Primitive::Solve(schema))
        );
    }
    assert!(
        steps
            .iter()
            .any(|step| step.rule == Primitive::ContextClose)
    );
    assert!(steps.iter().any(|step| matches!(&step.certificate, Certificate::Solve(certificate)
        if matches!(certificate.as_ref(), SolveCertificate::CoefficientIdentity(value)
            if value.combinations.iter().any(|row| row.hypothesis.as_ref().is_some_and(|hypothesis| hypothesis.vieta.is_some()))))));
    formal_steps(steps)
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
