// SPDX-License-Identifier: AGPL-3.0-only
//! Physical counts retain exact data, structural proof or selected-snapshot provenance.

use super::decision_admission::{physical_source, specification};
use super::decision_fixture::{fixture, ledger, signed_fixture};
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
use std::sync::{Arc, Mutex};

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
        Arc::new(Mutex::new(source.frame.cas)),
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
    let before = selected.cas()?.counts();
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
    assert_eq!(selected.cas()?.counts(), before);
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

/// Exact signed storage includes both endpoints and the complete previous unsigned range.
/// ´test:integration:bindings-direct-count-representation´
/// ´claim:bindings:direct-count-representation´ Direct declared and proved counts preserve signed endpoints and the old unsigned range while refusing values outside i128 atomically.
#[test]
fn bindings_direct_count_representation() -> TestResult {
    for (text, domain) in [
        ("-1", CountDomain::SIGNED),
        ("18446744073709551615", CountDomain::NONNEGATIVE),
        ("18446744073709551616", CountDomain::NONNEGATIVE),
        (
            "170141183460469231731687303715884105727",
            CountDomain::SIGNED,
        ),
        (
            "-170141183460469231731687303715884105728",
            CountDomain::SIGNED,
        ),
    ] {
        let expected = text.parse::<i128>()?;
        let owner = declared(text, domain)?;
        let count =
            CountDescriptor::declared(Rc::clone(&owner), "cycles", u128::MAX, &mut ledger()?)?;
        assert_eq!(count.value(), expected);
        let (graph, source, witness) = proved(text)?;
        let before = graph.counts();
        let count = CountDescriptor::proved(
            Rc::clone(&graph),
            source,
            descriptor(domain)?,
            witness,
            u128::MAX,
            &mut ledger()?,
        )?;
        assert_eq!(count.value(), expected);
        assert_eq!(graph.counts(), before);
    }
    for text in [
        "170141183460469231731687303715884105728",
        "-170141183460469231731687303715884105729",
    ] {
        let owner = declared(text, CountDomain::SIGNED)?;
        let Err(fault) = CountDescriptor::declared(owner, "cycles", u128::MAX, &mut ledger()?)
        else {
            panic!("unrepresentable declared count must refuse");
        };
        assert_capacity(&fault)?;
        let (graph, source, witness) = proved(text)?;
        let before = graph.counts();
        let Err(fault) = CountDescriptor::proved(
            Rc::clone(&graph),
            source,
            descriptor(CountDomain::SIGNED)?,
            witness,
            u128::MAX,
            &mut ledger()?,
        ) else {
            panic!("unrepresentable proved count must refuse");
        };
        assert_capacity(&fault)?;
        assert_eq!(graph.counts(), before);
    }
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

/// Prepare a selected exact Integer independently of its finite count representation.
fn selected_exact(text: &str, domain: CountDomain) -> TestResult<SelectedDecision> {
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
    let mut source = signed_fixture(text)?;
    let case = IntegerDecisionCase::declare(
        &source.frame.cas,
        physical,
        specification(&source, domain)?,
        &mut ledger()?,
    )?;
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut ledger()?)?;
    let selected = case.admit(
        Arc::new(Mutex::new(source.frame.cas)),
        witness,
        case.attachment(),
        &mut ledger()?,
    )?;
    Ok(selected)
}

/// A decided count can remain an exact mathematical Integer beyond signed machine representation.
/// ´test:integration:bindings-decided-count-representation´
/// ´claim:bindings:decided-count-representation´ Selected counts preserve negative and large exact Integers and refuse i128 overflow without altering their snapshots.
#[test]
fn bindings_decided_count_representation() -> TestResult {
    for text in ["-1", "18446744073709551616"] {
        let selected = selected_exact(text, CountDomain::SIGNED)?;
        let before = selected.cas()?.counts();
        assert_eq!(
            selected.count(u128::MAX, &mut ledger()?)?.value(),
            text.parse::<i128>()?
        );
        if text == "18446744073709551616" {
            let Err(fault) = selected.count(u128::from(u64::MAX), &mut ledger()?) else {
                panic!("selected value must still obey the old machine capacity");
            };
            assert_capacity(&fault)?;
            assert_eq!(selected.value(), &Rational::parse(text, &mut ledger()?)?);
        }
        assert_eq!(selected.cas()?.counts(), before);
    }
    let text = "170141183460469231731687303715884105728";
    let selected = selected_exact(text, CountDomain::SIGNED)?;
    let before = selected.cas()?.counts();
    let Err(fault) = selected.count(u128::MAX, &mut ledger()?) else {
        panic!("unrepresentable selected count must refuse");
    };
    assert_capacity(&fault)?;
    assert_eq!(selected.value(), &Rational::parse(text, &mut ledger()?)?);
    assert_eq!(selected.cas()?.counts(), before);
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

/// Mathematical sign and range admission precedes signed executable capacity.
/// ´test:integration:bindings-signed-count-capacity´
/// ´claim:bindings:signed-count-capacity´ Signed constructors enforce magnitude capacity including i128 minimum while invalid mathematical domains refuse before representation.
#[test]
fn bindings_signed_count_capacity() -> TestResult {
    let text = "-170141183460469231731687303715884105728";
    let owner = declared(text, CountDomain::SIGNED)?;
    for capacity in [1_u128 << 127, (1_u128 << 127) - 1] {
        let result =
            CountDescriptor::declared(Rc::clone(&owner), "cycles", capacity, &mut ledger()?);
        if capacity == 1_u128 << 127 {
            assert_eq!(result?.value(), i128::MIN);
        } else {
            let Err(fault) = result else {
                panic!("signed magnitude must fit capacity");
            };
            assert_capacity(&fault)?;
        }
    }
    let (graph, source, witness) = proved(text)?;
    let before = graph.counts();
    let Err(nrz_bindings::DecisionError::Units(fault)) = CountDescriptor::proved(
        Rc::clone(&graph),
        source,
        descriptor(CountDomain::NONNEGATIVE)?,
        witness,
        0,
        &mut ledger()?,
    ) else {
        panic!("mathematical sign must refuse before executable capacity");
    };
    assert_eq!(fault, nrz_bindings::UnitError::CountValue);
    assert_eq!(graph.counts(), before);
    let selected = selected_exact("-1", CountDomain::SIGNED)?;
    assert_eq!(selected.count(1, &mut ledger()?)?.value(), -1);
    let Err(fault) = selected.count(0, &mut ledger()?) else {
        panic!("selected magnitude must fit capacity");
    };
    assert_capacity(&fault)?;
    Ok(())
}

/// Every signed source route has fresh literal Work and retained-storage boundary twins.
/// ´test:integration:bindings-signed-count-work-boundaries´
/// ´claim:bindings:signed-count-work-boundaries´ Declared, proved and selected negative counts retain their evidence at measured Work and storage boundaries while one-below refusals preserve original owners and prior spending.
#[test]
fn bindings_signed_count_work_boundaries() -> TestResult {
    for (route, needed, bytes) in [(0, 97, 2_074), (1, 230, 7_444), (2, 89, 2_068)] {
        for boundary in 0..3 {
            for accepted in [true, false] {
                let owner = declared("-1", CountDomain::SIGNED)?;
                let (graph, source, witness) = proved("-1")?;
                let selected = selected_exact("-1", CountDomain::SIGNED)?;
                let before = graph.counts();
                let selected_before = selected.cas()?.counts();
                let mut limits = nrz_bindings::Limits::default();
                let expected = match boundary {
                    0 => {
                        limits.work = needed + 17 - u64::from(!accepted);
                        nrz_cas::limits::BudgetKind::Work
                    }
                    1 => {
                        limits.output_bytes = bytes - u64::from(!accepted);
                        nrz_cas::limits::BudgetKind::OutputBytes
                    }
                    _ => {
                        limits.staged_payload_bytes = bytes - u64::from(!accepted);
                        nrz_cas::limits::BudgetKind::StagedPayloadBytes
                    }
                };
                let mut budget = nrz_bindings::Budget::new(limits)?;
                budget.work(17, nrz_cas::Location::new("prior signed count work"))?;
                let result = match route {
                    0 => CountDescriptor::declared(owner, "cycles", 1, &mut budget),
                    1 => CountDescriptor::proved(
                        Rc::clone(&graph),
                        source,
                        descriptor(CountDomain::SIGNED)?,
                        witness,
                        1,
                        &mut budget,
                    ),
                    _ => selected.count(1, &mut budget),
                };
                if accepted {
                    let count = result?;
                    assert_eq!(count.value(), -1);
                    assert_eq!(count.retained_payload_bytes(), bytes);
                    assert_eq!(budget.usage().work, needed + 17);
                } else {
                    let Err(nrz_bindings::DecisionError::Algebra(fault)) = result else {
                        panic!("one-below signed count policy must refuse");
                    };
                    assert!(
                        matches!(fault.kind, nrz_cas::FaultKind::Exhausted { budget, .. } if budget == expected)
                    );
                }
                assert!(budget.usage().work >= 17);
                assert_eq!(graph.counts(), before);
                assert_eq!(selected.cas()?.counts(), selected_before);
            }
        }
    }
    Ok(())
}

/// The nonnegative family proves its cover but excludes a negative selected index.
/// ´test:integration:bindings-signed-count-family-domain´
/// ´claim:bindings:signed-count-family-domain´ A negative selection fails the original zero lower bound after its family cover passes and succeeds with an unconstrained signed family.
#[test]
fn bindings_signed_count_family_domain() -> TestResult {
    let mut source = fixture(false, "-1", "-1")?;
    let end = source.steps.len() - 1;
    source
        .frame
        .cas
        .check_proof(&source.context, &source.steps[end - 1..end], &mut ledger()?)?;
    let before = source.frame.cas.counts();
    let Err(fault) = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut ledger()?)
    else {
        panic!("negative selection must fail the family's zero lower bound");
    };
    assert_eq!(
        fault.kind,
        nrz_cas::FaultKind::Invalid(nrz_cas::InvalidCode::DecisionIntegerRange)
    );
    assert_eq!(
        fault.location,
        nrz_cas::Location::new("integer floor family")
    );
    assert_eq!(source.frame.cas.counts(), before);
    assert_eq!(
        selected_exact("-1", CountDomain::SIGNED)?
            .count(1, &mut ledger()?)?
            .value(),
        -1
    );
    Ok(())
}
