// SPDX-License-Identifier: AGPL-3.0-only
//! Supplied typed steps and ordinary application records own their complete premise inventories.

use super::Decoded;
use crate::budget::{Budget, BudgetKind, Limits};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle};
use crate::proof_model::{ApplicationDraft, Certificate, Judgment, VerifiedWitness};
use crate::rule::Primitive;
use crate::testing::common::TestResult;
use crate::testing::native::fresh_budget as default_budget;
use crate::testing::native::{Fixture as Base, admit, predicate, term};
use crate::wire_build::Entity;
use crate::wire_read::Reader;

/// Identical prior usage and retained caller storage surround every attempt.
const AT: Location = Location::new("supplied premise control");

/// Admit exact literals in the fixture's original scalar header.
fn number(base: &mut Base, text: &str) -> TestResult<Handle> {
    let value = crate::Rational::parse(text, &mut default_budget()?)?;
    term(
        &mut base.cas,
        &base.header,
        crate::model::Operator::Rational(value),
        &[],
    )
}

/// Reconstruct ordinary definedness independently of any later consuming step.
fn defined(base: &mut Base) -> TestResult<ApplicationDraft> {
    let predicate = predicate(
        &mut base.cas,
        &base.header,
        crate::model::PredicateKind::Defined,
        std::slice::from_ref(&base.one),
    )?;
    let mut step = ApplicationDraft {
        context: base.context.clone(),
        rule: Primitive::CaseDomain,
        conclusion: crate::proof_model::Statement::PredicateFact {
            region: base.domain.clone(),
            predicate,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    };
    let (certificate, conditions) =
        crate::case_check::reconstruct(&mut base.cas, &step, &[], &mut default_budget()?)?;
    assert_eq!(conditions.requirements, []);
    assert_eq!(conditions.residuals, []);
    assert_eq!(conditions.hypotheses, []);
    step.certificate = Certificate::Cases(Box::new(certificate));
    Ok(step)
}

/// Hand integral of the constant series one, with ordinary Defined(1) in slot zero.
fn integral(reordered: bool) -> TestResult<(Base, Vec<ApplicationDraft>)> {
    use crate::graph::{Draft, Reference};
    use crate::series::{SeriesCertificate, SeriesIntegralRow, SeriesObject, SeriesStatement};
    let mut base = Base::new()?;
    for text in if reordered { ["3", "2"] } else { ["2", "3"] } {
        number(&mut base, text)?;
    }
    let field = admit(
        &mut base.cas,
        Draft::Solve(crate::solving::SolveDraft::CoefficientDomain(
            crate::solving::CoefficientDomainData::Rational,
        )),
    )?;
    let mut objects = Vec::new();
    for order in [1, 2] {
        objects.push(admit(
            &mut base.cas,
            Draft::SeriesObject(SeriesObject {
                field: Reference::Existing(field.clone()),
                centre: Reference::Existing(base.zero.clone()),
                variable: Reference::Existing(base.x.clone()),
                context: Reference::Existing(base.context.clone()),
                scope: Vec::new(),
                order,
                coefficients: vec![Reference::Existing(base.one.clone()); usize::try_from(order)?],
            }),
        )?);
    }
    let steps = vec![
        defined(&mut base)?,
        ApplicationDraft {
            rule: Primitive::SeriesIntegrate,
            context: base.context.clone(),
            conclusion: crate::proof_model::Statement::Series(Box::new(
                SeriesStatement::Integrate {
                    input: objects[0].clone(),
                    constant: base.one.clone(),
                    output: objects[1].clone(),
                },
            )),
            premises: vec![0],
            substitutions: Vec::new(),
            certificate: Certificate::Series(Box::new(SeriesCertificate::Integrate {
                form: 0,
                rows: vec![SeriesIntegralRow {
                    index: 0,
                    integer: crate::Rational::one(),
                    inverse: crate::Rational::one(),
                    input: crate::Rational::one(),
                    result: crate::Rational::one(),
                }],
            })),
        },
    ];
    Ok((base, steps))
}

/// A record retains the same checked earlier dependencies as a complete replay.
struct Record {
    /// Bytes after the common application prefix.
    bytes: Vec<u8>,
    /// Earlier independently checked objects and propositions.
    records: Vec<Decoded>,
    /// The one primitive dispatched by this isolated record.
    rule: Primitive,
}

/// Patch symbolic dependencies without changing the record's certificate or slots.
fn record(cas: &Cas, witness: &VerifiedWitness) -> TestResult<Record> {
    let mut budget = crate::testing::common::default_budget()?;
    let mut value = crate::wire_records::describe(
        cas,
        &Entity::Object(witness.proof().logical_id()),
        &mut default_budget()?,
    )?;
    assert_eq!(value.tag, 11);
    let mut records = Vec::new();
    for (offset, entity) in value.references {
        let decoded = match entity {
            Entity::Object(id)
                if matches!(
                    cas.state.node(id, &mut budget)?,
                    crate::graph::Vertex::Proof(_)
                ) =>
            {
                Decoded::Application(cas.handle(id))
            }
            Entity::Object(id) => Decoded::Object(cas.handle(id)),
            Entity::Statement(value) => Decoded::Statement(*value),
            _ => return Err("ordinary application dependency".into()),
        };
        value.payload[offset..offset + 4]
            .copy_from_slice(&u32::try_from(records.len())?.to_le_bytes());
        records.push(decoded);
    }
    Ok(Record {
        bytes: value.payload,
        records,
        rule: value.rule.ok_or("application primitive")?,
    })
}

/// Call the ordinary decoder with its actual earlier records and shared ledger.
fn decode(cas: &mut Cas, record: &Record, budget: &mut Budget) -> Result<Handle, Fault> {
    let mut reader = Reader::new(&record.bytes, budget)?;
    let Decoded::Application(proof) = super::application(
        cas,
        crate::domain_model::Layer::Logic,
        None,
        &mut reader,
        &record.records,
        &[record.rule],
    )?
    else {
        return Err(Fault::invalid(InvalidCode::Rule, AT));
    };
    reader.finished()?;
    Ok(proof)
}

/// Every graph weight, edge, counter and derived index is compared after refusal.
fn snapshot(cas: &Cas) -> TestResult<(Vec<u8>, String)> {
    Ok((
        cas.canonical_snapshot(&mut default_budget()?)?,
        format!("{:?}", cas.state),
    ))
}

/// Repeated backward slots cannot reach either premise sizing or premise copying.
/// ´test:crate:supplied-premise-surplus-slots´
#[test]
fn supplied_premise_surplus_slots() -> TestResult {
    for reordered in [false, true] {
        let (mut base, mut steps) = integral(reordered)?;
        steps.last_mut().ok_or("integral")?.premises = vec![0, 0];
        let before = snapshot(&base.cas)?;
        let mut budget = default_budget()?;
        let limits = budget.limits().clone();
        budget.work(37, AT)?;
        let copies = crate::proof_view::copied_applications();
        let fault = budget
            .with_retained_payload(113, AT, |budget| {
                base.cas.check_proof(&base.context, &steps, budget)
            })
            .expect_err("surplus integral slot");
        assert_eq!(crate::proof_view::copied_applications(), copies);
        assert_eq!(
            fault,
            Fault::invalid(InvalidCode::Rule, Location::new("typed proof admission"))
        );
        assert_eq!(budget.usage().work, 32_539);
        assert_eq!(budget.limits(), &limits);
        assert_eq!(snapshot(&base.cas)?, before);
        let ordinary = base
            .cas
            .check_proof(&base.context, &steps[..1], &mut default_budget()?)?;
        let root = steps.last().ok_or("integral")?;
        let record = Record {
            bytes: [0_u32, 0, 2, 1, 1]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect(),
            records: vec![
                Decoded::Statement(Judgment {
                    context: root.context.clone(),
                    layer: crate::domain_model::Layer::Logic,
                    space: None,
                    statement: root.conclusion.clone(),
                    residuals: Vec::new(),
                }),
                Decoded::Application(ordinary.proof().clone()),
            ],
            rule: root.rule,
        };
        let before = snapshot(&base.cas)?;
        let mut budget = default_budget()?;
        budget.work(37, AT)?;
        let copies = crate::proof_view::copied_applications();
        let fault = budget
            .with_retained_payload(113, AT, |budget| {
                base.cas
                    .transaction(budget, |cas, budget| decode(cas, &record, budget))
            })
            .expect_err("surplus ordinary record slot");
        assert_eq!(crate::proof_view::copied_applications(), copies);
        assert_eq!(
            fault,
            Fault::invalid(InvalidCode::Rule, Location::new("typed witness import"))
        );
        assert_eq!(budget.usage().work, 1329);
        assert_eq!(budget.limits(), &limits);
        assert_eq!(snapshot(&base.cas)?, before);
    }
    Ok(())
}

/// A large independently reconstructed case-domain certificate is valid ordinary evidence.
fn large(
    reordered: bool,
    count: usize,
) -> TestResult<(Base, ApplicationDraft, Handle, VerifiedWitness)> {
    use crate::domain_model::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
    use crate::graph::{Draft, Reference};
    use crate::model::{Operator, PredicateKind};
    let mut base = Base::new()?;
    for text in if reordered { ["3", "2"] } else { ["2", "3"] } {
        number(&mut base, text)?;
    }
    let mut members = Vec::new();
    for index in 0..8 {
        let value = number(&mut base, &format!("{}{index}", "1234567890".repeat(24)))?;
        let difference = term(
            &mut base.cas,
            &base.header,
            Operator::Sub,
            &[base.x.clone(), value],
        )?;
        let predicate = predicate(
            &mut base.cas,
            &base.header,
            PredicateKind::Positive,
            &[difference],
        )?;
        members.push(Reference::Existing(admit(
            &mut base.cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(base.header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Real("x".into()),
                expression: RegionExpr::GuardTrue {
                    base: Reference::Existing(base.domain.clone()),
                    predicate: Reference::Existing(predicate),
                },
            }),
        )?));
    }
    base.domain = base
        .cas
        .admit(
            &[Draft::Region(RegionDraft {
                header: Reference::Existing(base.header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Real("x".into()),
                expression: RegionExpr::Union(members),
            })],
            &mut default_budget()?,
        )?
        .pop()
        .ok_or("union")?;
    base.context = base
        .cas
        .admit(
            &[Draft::Context(ContextDraft::Root {
                header: Reference::Existing(base.header.clone()),
                admissible: Reference::Existing(base.domain.clone()),
                assumptions: Vec::new(),
            })],
            &mut default_budget()?,
        )?
        .pop()
        .ok_or("context")?;
    let original = defined(&mut base)?;
    let proof = base.cas.check_proof(
        &base.context,
        std::slice::from_ref(&original),
        &mut default_budget()?,
    )?;
    let mut step = original.clone();
    step.premises = vec![0; count];
    let witness = base.cas.check_proof(
        &base.context,
        &[original, step.clone()],
        &mut default_budget()?,
    )?;
    Ok((base, step, proof.proof().clone(), witness))
}

/// Identical route checkpoints keep allowance arithmetic independent of allocation order.
fn attempt(
    cas: &Cas,
    step: &ApplicationDraft,
    proof: &Handle,
    record: &Record,
    decoded: bool,
    allowance: u64,
) -> TestResult<(Result<Handle, Fault>, u64)> {
    let mut candidate = cas.full_copy_for_test();
    let before = snapshot(&candidate)?;
    let limits = Limits {
        staged_payload_bytes: allowance,
        ..default_budget()?.limits().clone()
    };
    let mut budget = Budget::new(limits.clone())?;
    budget.work(37, AT)?;
    let result = budget.with_retained_payload(113, AT, |budget| {
        candidate.transaction(budget, |cas, budget| {
            if decoded {
                decode(cas, record, budget)
            } else {
                crate::scoped_check::check_supplied(cas, step, std::slice::from_ref(proof), budget)
            }
        })
    });
    assert_eq!(budget.limits(), &limits);
    assert_eq!(snapshot(&candidate)?, before);
    Ok((result, budget.usage().work))
}

/// A valid single premise coexists with all checking and publication scratch.
/// ´test:crate:supplied-premise-single-payload´
#[test]
fn supplied_premise_single_payload() -> TestResult {
    probe(1)
}

/// Repeated references require separate owned payloads under both admission routes.
/// ´test:crate:supplied-premise-repeated-payload´
#[test]
fn supplied_premise_repeated_payload() -> TestResult {
    probe(2)
}

/// Literal N and N-minus-one controls preserve caller storage and the complete arena.
fn probe(count: usize) -> TestResult {
    let mut canonical = None;
    for reordered in [false, true] {
        let (base, step, proof, witness) = large(reordered, count)?;
        let bytes = base.cas.export_witness(&witness, &mut default_budget()?)?;
        if let Some(expected) = &canonical {
            assert_eq!(&bytes, expected);
        } else {
            canonical = Some(bytes.clone());
        }
        let record = record(&base.cas, &witness)?;
        let payload = crate::proof_view::application_bytes(
            &base.cas,
            proof.logical_id(),
            &mut default_budget()?,
        )?;
        assert_eq!(payload, 4_408);
        let mut checkpoint_budget = default_budget()?;
        let outer = checkpoint_budget.limits().staged_payload_bytes;
        let checkpoint = base
            .cas
            .full_copy_for_test()
            .transaction(&mut checkpoint_budget, |_, budget| {
                Ok(outer - budget.limits().staged_payload_bytes)
            })?;
        assert_eq!(checkpoint, 576);
        for decoded in [false, true] {
            if count == 2 {
                let aggregate = payload * 2;
                let allowance = 113 + checkpoint + aggregate - 1;
                let copies = crate::proof_view::copied_applications();
                let (failure, spent) =
                    attempt(&base.cas, &step, &proof, &record, decoded, allowance)?;
                assert_eq!(crate::proof_view::copied_applications(), copies);
                let location = if decoded {
                    "typed witness import"
                } else {
                    "typed proof admission"
                };
                assert_eq!(
                    failure.expect_err("both owned occurrences"),
                    Fault::exhausted(
                        BudgetKind::StagedPayloadBytes,
                        aggregate - 1,
                        aggregate,
                        Location::new(location),
                    )
                );
                assert_eq!(allowance, 9_504);
                assert_eq!(spent, if decoded { 91_639 } else { 36_273 });
            }
            let (allowance, successful_work, refused_work) = match (count, decoded) {
                (1, false) => (170_099, 1_704_230, 1_102_159),
                (1, true) => (170_099, 1_759_080, 1_157_009),
                (2, false) => (174_507, 1_740_229, 1_138_140),
                (2, true) => (174_507, 1_795_595, 1_193_506),
                _ => return Err("unsupported premise count".into()),
            };
            let (success, work) = attempt(&base.cas, &step, &proof, &record, decoded, allowance)?;
            assert_eq!(success?, *witness.proof());
            assert_eq!(work, successful_work);
            assert_eq!(
                base.cas.export_witness(&witness, &mut default_budget()?)?,
                bytes
            );
            let (failure, spent) =
                attempt(&base.cas, &step, &proof, &record, decoded, allowance - 1)?;
            assert_eq!(
                failure.expect_err("N minus one"),
                Fault::exhausted(
                    BudgetKind::StagedPayloadBytes,
                    159_545,
                    159_546,
                    Location::new("univariate arithmetic"),
                )
            );
            assert_eq!(spent, refused_work);
        }
    }
    Ok(())
}

/// Variable shapes stop at `ProofNodes` before looking up or owning surplus slots.
/// ´test:crate:supplied-premise-variable-bound´
#[test]
fn supplied_premise_variable_bound() -> TestResult {
    for reordered in [false, true] {
        let (mut base, mut step, proof, witness) = large(reordered, 2)?;
        let mut record = record(&base.cas, &witness)?;
        let limits = Limits {
            proof_nodes: 2,
            ..default_budget()?.limits().clone()
        };
        let before = snapshot(&base.cas)?;
        for decoded in [false, true] {
            let mut budget = Budget::new(limits.clone())?;
            let actual = if decoded {
                decode(&mut base.cas, &record, &mut budget)?
            } else {
                crate::scoped_check::check_supplied(
                    &mut base.cas,
                    &step,
                    std::slice::from_ref(&proof),
                    &mut budget,
                )?
            };
            assert_eq!(actual, *witness.proof());
            assert_eq!(budget.limits(), &limits);
            assert_eq!(snapshot(&base.cas)?, before);
        }
        step.premises.push(0);
        record.bytes[8..12].copy_from_slice(&3_u32.to_le_bytes());
        for decoded in [false, true] {
            let mut budget = Budget::new(limits.clone())?;
            budget.work(37, AT)?;
            let copies = crate::proof_view::copied_applications();
            let fault = if decoded {
                decode(&mut base.cas, &record, &mut budget)
            } else {
                crate::scoped_check::check_supplied(
                    &mut base.cas,
                    &step,
                    std::slice::from_ref(&proof),
                    &mut budget,
                )
            }
            .expect_err("declared variable-slot maximum");
            assert_eq!(crate::proof_view::copied_applications(), copies);
            let location = if decoded {
                "witness decoding"
            } else {
                "typed proof admission"
            };
            assert_eq!(
                fault,
                Fault::exhausted(BudgetKind::ProofNodes, 2, 3, Location::new(location))
            );
            assert_eq!(budget.usage().work, if decoded { 49 } else { 37 });
            assert_eq!(budget.limits(), &limits);
            assert_eq!(snapshot(&base.cas)?, before);
        }
    }
    Ok(())
}
