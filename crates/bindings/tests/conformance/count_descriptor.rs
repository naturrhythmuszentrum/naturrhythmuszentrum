// SPDX-License-Identifier: AGPL-3.0-only
//! Physical counts retain exact data, structural proof or selected-snapshot provenance.

use super::decision_admission::{physical_source, specification};
use super::decision_fixture::{fixture, ledger};
use crate::testing::declarations::TestResult;
use nrz_bindings::{
    CheckedDeclarations, Context, CountDescriptor, CountDomain, CountEvidence, DecisionExpression,
    Declaration, Descriptor, Evidence, ExponentPolicy, IntegerDecisionCase, Kind, Measure,
    Registry, Role, SelectedDecision,
};
use nrz_cas::{
    Cas, Handle, Rational,
    proof::{ApplicationDraft, Certificate, DomainContract, Primitive, Statement, VerifiedWitness},
};
use std::rc::Rc;

/// Reuse the nominal physically checked complete-period decision.
fn selected() -> TestResult<SelectedDecision> {
    let physical = DecisionExpression::declared(
        physical_source("LC nominal reference", false)?,
        "quotient",
        &mut ledger()?,
    )?;
    let mut source = fixture(false, "3", "10/Pi")?;
    let case = IntegerDecisionCase::declare(
        &source.frame.cas,
        physical,
        specification(&source, CountDomain::NONNEGATIVE)?,
        &mut ledger()?,
    )?;
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut ledger()?)?;
    Ok(case.admit(
        Rc::new(source.frame.cas),
        witness,
        case.attachment(),
        &mut ledger()?,
    )?)
}

/// A physical count names the thing counted and its exact mathematical domain.
fn descriptor(domain: CountDomain) -> TestResult<Descriptor> {
    Ok(Descriptor::new(
        Kind::Count("complete_lc_cycles".into(), domain),
        Measure::coherent(),
        Role::Difference,
    )?)
}

/// An exact data declaration requires neither floor cases nor numerical evidence.
fn declared(value: &str, domain: CountDomain) -> TestResult<Rc<CheckedDeclarations>> {
    let mut owner = Context::with_real_symbols(
        Registry::new(ExponentPolicy::Rational),
        nrz_bindings::Limits::default(),
        &["cycles"],
    )?;
    owner.declare(
        "cycles".into(),
        Declaration::Constant {
            descriptor: descriptor(domain)?,
            value: Rational::parse(value, &mut ledger()?)?,
            evidence: Evidence {
                source: "exact tally".into(),
                assurance: "declared exact data".into(),
            },
        },
    )?;
    Ok(Rc::new(owner.freeze()?))
}

/// Only reflexivity is checked; the existing fixture supplies syntax, never a decision proof.
fn proved(value: &str) -> TestResult<(Rc<Cas>, Handle, VerifiedWitness)> {
    let mut source = fixture(false, value, value)?;
    let Statement::IntegerSelection(selection) =
        &source.steps.last().ok_or("missing syntax")?.conclusion
    else {
        return Err("missing Integer syntax".into());
    };
    let value = selection.value.clone();
    let application = ApplicationDraft {
        rule: Primitive::Reflexivity,
        context: source.context.clone(),
        conclusion: Statement::Equality {
            left: value.clone(),
            right: value.clone(),
            domain: selection.family.domain.clone(),
            domain_contract: DomainContract::DefinedAndEqual,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    };
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &[application], &mut ledger()?)?;
    assert!(witness.is_unconditional());
    Ok((Rc::new(source.frame.cas), value, witness))
}

/// Each exact count route preserves its original evidence and measured owner increment.
/// ´test:integration:bindings-count-descriptor-routes´
#[test]
fn bindings_count_descriptor_routes() -> TestResult {
    let selected = selected()?;
    let mut decided_work = ledger()?;
    let count = selected.count(3, &mut decided_work)?;
    assert_eq!(count.value(), 3);
    assert!(matches!(count.evidence(), CountEvidence::Decided(_)));
    let owner = declared("3", CountDomain::NONNEGATIVE)?;
    let mut declared_work = ledger()?;
    let count = CountDescriptor::declared(owner, "cycles", 3, &mut declared_work)?;
    assert_eq!(count.value(), 3);
    assert!(matches!(count.evidence(), CountEvidence::Declared { .. }));
    let (graph, value, witness) = proved("3")?;
    let mut proved_work = ledger()?;
    let count = CountDescriptor::proved(
        graph,
        value,
        descriptor(CountDomain::NONNEGATIVE)?,
        witness,
        3,
        &mut proved_work,
    )?;
    assert_eq!(count.value(), 3);
    assert!(matches!(count.evidence(), CountEvidence::Proved { .. }));
    assert_eq!(decided_work.usage().work, 59);
    assert_eq!(declared_work.usage().work, 384);
    assert_eq!(proved_work.usage().work, 517);
    Ok(())
}

/// Executable capacity refusal is distinct and leaves the exact mathematical snapshot usable.
/// ´test:integration:bindings-count-capacity-refusal´
#[test]
fn bindings_count_capacity_refusal() -> TestResult {
    let selected = selected()?;
    let before = selected.cas().counts();
    let fault = selected
        .count(2, &mut ledger()?)
        .err()
        .ok_or("count exceeded capacity")?;
    assert_capacity(&fault)?;
    assert_eq!(
        selected.value(),
        &Rational::from_ratio(3, 1, &mut ledger()?)?
    );
    assert_eq!(selected.count(3, &mut ledger()?)?.value(), 3);
    assert_eq!(selected.cas().counts(), before);
    let owner = declared("3", CountDomain::NONNEGATIVE)?;
    assert_capacity(
        &CountDescriptor::declared(Rc::clone(&owner), "cycles", 2, &mut ledger()?)
            .err()
            .ok_or("data exceeded capacity")?,
    )?;
    assert_eq!(
        CountDescriptor::declared(owner, "cycles", 3, &mut ledger()?)?.value(),
        3
    );
    let (graph, value, witness) = proved("3")?;
    let before = graph.counts();
    assert_capacity(
        &CountDescriptor::proved(
            Rc::clone(&graph),
            value,
            descriptor(CountDomain::NONNEGATIVE)?,
            witness,
            2,
            &mut ledger()?,
        )
        .err()
        .ok_or("proof exceeded capacity")?,
    )?;
    assert_eq!(graph.counts(), before);
    Ok(())
}

/// Every route preserves the typed mathematical-versus-executable capacity distinction.
fn assert_capacity(error: &nrz_bindings::DecisionError) -> TestResult {
    let nrz_bindings::DecisionError::Algebra(fault) = error else {
        return Err("not an algebra capacity refusal".into());
    };
    assert_eq!(
        fault.kind,
        nrz_cas::FaultKind::Invalid(nrz_cas::InvalidCode::DecisionMaterializationCapacity)
    );
    Ok(())
}

/// Signed and large exact data remain mathematically valid without acquiring a u64 representation.
/// ´test:integration:bindings-direct-count-representation´
#[test]
fn bindings_direct_count_representation() -> TestResult {
    for (value, domain) in [
        ("-1", CountDomain::SIGNED),
        ("18446744073709551616", CountDomain::NONNEGATIVE),
    ] {
        let owner = declared(value, domain)?;
        assert_capacity(
            &CountDescriptor::declared(Rc::clone(&owner), "cycles", u64::MAX, &mut ledger()?)
                .err()
                .ok_or("unrepresentable data accepted")?,
        )?;
        assert!(owner.checked("cycles").is_ok());
        let (graph, source, witness) = proved(value)?;
        let before = graph.counts();
        assert_capacity(
            &CountDescriptor::proved(
                Rc::clone(&graph),
                source,
                descriptor(domain)?,
                witness,
                u64::MAX,
                &mut ledger()?,
            )
            .err()
            .ok_or("unrepresentable proof accepted")?,
        )?;
        assert_eq!(graph.counts(), before);
    }
    let owner = declared("18446744073709551615", CountDomain::NONNEGATIVE)?;
    assert_eq!(
        CountDescriptor::declared(owner, "cycles", u64::MAX, &mut ledger()?)?.value(),
        u64::MAX
    );
    Ok(())
}

/// The structural path checks exact original source, proof authority, mathematical domain and kind.
/// ´test:integration:bindings-direct-count-refusals´
#[test]
fn bindings_direct_count_refusals() -> TestResult {
    let (graph, value, witness) = proved("3")?;
    let before = graph.counts();
    assert!(matches!(
        CountDescriptor::proved(
            Rc::clone(&graph),
            value.clone(),
            Descriptor::ratio(),
            witness.clone(),
            3,
            &mut ledger()?
        ),
        Err(nrz_bindings::DecisionError::CountKind)
    ));
    assert!(matches!(
        CountDescriptor::proved(
            Rc::clone(&graph),
            value.clone(),
            descriptor(CountDomain::new(Some(0), Some(2))?)?,
            witness.clone(),
            3,
            &mut ledger()?
        ),
        Err(nrz_bindings::DecisionError::Units(
            nrz_bindings::UnitError::CountValue
        ))
    ));
    assert!(matches!(
        CountDescriptor::proved(
            Rc::clone(&graph),
            witness.judgment().context.clone(),
            descriptor(CountDomain::NONNEGATIVE)?,
            witness.clone(),
            3,
            &mut ledger()?
        ),
        Err(nrz_bindings::DecisionError::Case)
    ));
    let foreign = Rc::new(Cas::new(nrz_bindings::Limits::default())?);
    assert!(
        CountDescriptor::proved(
            foreign,
            value,
            descriptor(CountDomain::NONNEGATIVE)?,
            witness,
            3,
            &mut ledger()?
        )
        .is_err()
    );
    assert_eq!(graph.counts(), before);
    let mut selected = fixture(false, "3", "10/Pi")?;
    let witness =
        selected
            .frame
            .cas
            .check_proof(&selected.context, &selected.steps, &mut ledger()?)?;
    let Statement::IntegerSelection(selection) = &witness.judgment().statement else {
        return Err("missing selection".into());
    };
    let value = selection.value.clone();
    assert!(matches!(
        CountDescriptor::proved(
            Rc::new(selected.frame.cas),
            value,
            descriptor(CountDomain::NONNEGATIVE)?,
            witness,
            3,
            &mut ledger()?
        ),
        Err(nrz_bindings::DecisionError::Case)
    ));
    Ok(())
}

/// Each new route has a literal N/N−1 caller-Work boundary including prior spending.
/// ´test:integration:bindings-count-work-boundaries´
#[test]
fn bindings_count_work_boundaries() -> TestResult {
    let selected = selected()?;
    let owner = declared("3", CountDomain::NONNEGATIVE)?;
    let (graph, value, witness) = proved("3")?;
    let before = graph.counts();
    for (route, work, accepted) in [
        (0, 76, true),
        (0, 75, false),
        (1, 401, true),
        (1, 400, false),
        (2, 534, true),
        (2, 533, false),
    ] {
        let mut budget = nrz_bindings::Budget::new(nrz_bindings::Limits {
            work,
            ..nrz_bindings::Limits::default()
        })?;
        budget.work(17, nrz_cas::Location::new("prior count construction"))?;
        let result = match route {
            0 => selected.count(3, &mut budget),
            1 => CountDescriptor::declared(Rc::clone(&owner), "cycles", 3, &mut budget),
            _ => CountDescriptor::proved(
                Rc::clone(&graph),
                value.clone(),
                descriptor(CountDomain::NONNEGATIVE)?,
                witness.clone(),
                3,
                &mut budget,
            ),
        };
        if accepted {
            assert_eq!(result?.value(), 3);
            assert_eq!(budget.usage().work, work);
        } else {
            let error = result.err().ok_or("one-below Work accepted")?;
            let nrz_bindings::DecisionError::Algebra(fault) = error else {
                return Err("wrong Work refusal".into());
            };
            assert!(matches!(
                fault.kind,
                nrz_cas::FaultKind::Exhausted {
                    budget: nrz_cas::limits::BudgetKind::Work,
                    ..
                }
            ));
        }
        assert!(budget.usage().work >= 17);
        assert_eq!(graph.counts(), before);
    }
    Ok(())
}

/// A decided count can remain an exact mathematical Integer beyond machine representation.
/// ´test:integration:bindings-decided-count-representation´
#[test]
fn bindings_decided_count_representation() -> TestResult {
    let text = "18446744073709551616";
    let mut owner = Context::with_real_symbols(
        Registry::new(ExponentPolicy::Rational),
        nrz_bindings::Limits::default(),
        &["quotient"],
    )?;
    owner.declare(
        "quotient".into(),
        Declaration::Constant {
            descriptor: Descriptor::ratio(),
            value: Rational::parse(text, &mut ledger()?)?,
            evidence: Evidence::default(),
        },
    )?;
    let physical =
        DecisionExpression::declared(Rc::new(owner.freeze()?), "quotient", &mut ledger()?)?;
    let mut source = fixture(false, text, text)?;
    let case = IntegerDecisionCase::declare(
        &source.frame.cas,
        physical,
        specification(&source, CountDomain::NONNEGATIVE)?,
        &mut ledger()?,
    )?;
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut ledger()?)?;
    let selected = case.admit(
        Rc::new(source.frame.cas),
        witness,
        case.attachment(),
        &mut ledger()?,
    )?;
    let before = selected.cas().counts();
    assert_capacity(
        &selected
            .count(u64::MAX, &mut ledger()?)
            .err()
            .ok_or("unrepresentable decision accepted")?,
    )?;
    assert_eq!(selected.value(), &Rational::parse(text, &mut ledger()?)?);
    assert_eq!(selected.cas().counts(), before);
    Ok(())
}

/// Each count route prepays its complete conservative ownership envelope before publication.
/// ´test:integration:bindings-count-storage-boundaries´
#[test]
fn bindings_count_storage_boundaries() -> TestResult {
    let selected = selected()?;
    let owner = declared("3", CountDomain::NONNEGATIVE)?;
    let (graph, value, witness) = proved("3")?;
    let before = graph.counts();
    for (route, bytes, accepted) in [
        (0, 2_068, true),
        (0, 2_067, false),
        (1, 2_074, true),
        (1, 2_073, false),
        (2, 7_444, true),
        (2, 7_443, false),
    ] {
        for staged in [false, true] {
            let mut limits = nrz_bindings::Limits::default();
            let expected = if staged {
                limits.staged_payload_bytes = bytes;
                nrz_cas::limits::BudgetKind::StagedPayloadBytes
            } else {
                limits.output_bytes = bytes;
                nrz_cas::limits::BudgetKind::OutputBytes
            };
            let mut budget = nrz_bindings::Budget::new(limits)?;
            budget.work(17, nrz_cas::Location::new("prior retained count work"))?;
            let result = match route {
                0 => selected.count(3, &mut budget),
                1 => CountDescriptor::declared(Rc::clone(&owner), "cycles", 3, &mut budget),
                _ => CountDescriptor::proved(
                    Rc::clone(&graph),
                    value.clone(),
                    descriptor(CountDomain::NONNEGATIVE)?,
                    witness.clone(),
                    3,
                    &mut budget,
                ),
            };
            if accepted {
                assert_eq!(result?.retained_payload_bytes(), bytes);
            } else {
                let nrz_bindings::DecisionError::Algebra(fault) =
                    result.err().ok_or("one-below count storage accepted")?
                else {
                    return Err("wrong count storage refusal".into());
                };
                assert!(matches!(
                    fault.kind,
                    nrz_cas::FaultKind::Exhausted { budget, .. } if budget == expected
                ));
            }
            assert!(budget.usage().work >= 17);
            assert_eq!(graph.counts(), before);
        }
    }
    Ok(())
}
