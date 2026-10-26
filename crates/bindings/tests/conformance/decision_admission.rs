// SPDX-License-Identifier: AGPL-3.0-only
//! Closed physical target admission retains the complete declaration and exact CAS selection.

use super::decision_fixture::{SelectionFixture, fixture, ledger};
use crate::testing::declarations::TestResult;
use nrz_bindings::{
    BaseDimension, CheckedDeclarations, Context, CountDomain, DecisionExpression, Declaration,
    Descriptor, Dimension, Evidence, ExponentPolicy, IntegerDecisionCase, IntegerDecisionSpec,
    Kind, Measure, Observable, Registry, Role, Scale,
};
use nrz_cas::{
    ExactConstant, Rational,
    proof::{Certificate, Statement},
    syntax::Constant,
};
use std::rc::Rc;

/// A nominal period and duration carry physical time units before their ratio is checked.
pub(super) fn physical_source(
    source: &str,
    doubled_unit: bool,
) -> TestResult<Rc<CheckedDeclarations>> {
    let mut budget = ledger()?;
    let duration = Kind::Named(
        "duration".into(),
        Box::new(Dimension::base(BaseDimension::Time)),
    );
    let second = Descriptor::new(duration.clone(), Measure::coherent(), Role::Difference)?;
    let pi = ExactConstant::named(Constant::Pi, &mut budget)?;
    let divisor = Rational::from_ratio(if doubled_unit { 500 } else { 1_000 }, 1, &mut budget)?;
    let period = Descriptor::new(
        duration,
        Measure::new(
            "period coordinate",
            Scale::Expression(pi.div(&divisor.into(), &mut budget)?),
            Rational::zero(),
        )?,
        Role::Difference,
    )?;
    let mut owner = Context::with_real_symbols(
        Registry::new(ExponentPolicy::Rational),
        nrz_bindings::Limits::default(),
        &["duration", "period", "quotient"],
    )?;
    for (name, descriptor, value) in [
        (
            "duration",
            second,
            Rational::from_ratio(1, 100, &mut budget)?,
        ),
        (
            "period",
            period,
            Rational::from_ratio(1, if doubled_unit { 2 } else { 1 }, &mut budget)?,
        ),
    ] {
        owner.declare(
            name.into(),
            Declaration::Constant {
                descriptor,
                value,
                evidence: Evidence {
                    source: source.into(),
                    assurance: "declared exact nominal reference".into(),
                },
            },
        )?;
    }
    let expression = owner.parse("nominal complete-period ratio", "duration/period")?;
    owner.declare(
        "quotient".into(),
        Declaration::Derived(Observable {
            descriptor: Descriptor::ratio(),
            expression,
            evidence: Evidence {
                source: source.into(),
                assurance: "exact time ratio".into(),
            },
        }),
    )?;
    Ok(Rc::new(owner.freeze()?))
}

/// Extract the predeclared mathematical question without treating the proposed certificate as proof.
pub(super) fn specification(
    source: &SelectionFixture,
    domain: CountDomain,
) -> TestResult<IntegerDecisionSpec> {
    let proposal = source.steps.last().ok_or("missing proposal")?;
    let Statement::IntegerSelection(selection) = &proposal.conclusion else {
        return Err("missing selection proposition".into());
    };
    let Certificate::IntegerSelection(enclosure) = &proposal.certificate else {
        return Err("missing target proposal".into());
    };
    Ok(IntegerDecisionSpec {
        context: source.context.clone(),
        family: selection.family.clone(),
        target: enclosure.target.clone(),
        parameters: Vec::new(),
        count: Descriptor::new(
            Kind::Count("complete_lc_cycles".into(), domain),
            Measure::coherent(),
            Role::Difference,
        )?,
    })
}

/// Exact source and case preparation are reused while each selected snapshot pays its own checks.
/// ´test:integration:bindings-decision-admission´
#[test]
fn bindings_decision_admission() -> TestResult {
    let owner = physical_source("LC nominal reference", false)?;
    let mut source = fixture(false, "3", "10/Pi")?;
    let mut budget = ledger()?;
    let physical = DecisionExpression::declared(owner, "quotient", &mut budget)?;
    let physical_work = budget.usage().work;
    let case = IntegerDecisionCase::declare(
        &source.frame.cas,
        physical,
        specification(&source, CountDomain::NONNEGATIVE)?,
        &mut budget,
    )?;
    let case_work = budget.usage().work - physical_work;
    let attachment = case.attachment();
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut budget)?;
    let proof_end = budget.usage().work;
    let repeated = witness.clone();
    let graph = Rc::new(source.frame.cas);
    let before = graph.counts();
    let selected = case.admit(Rc::clone(&graph), witness, attachment, &mut budget)?;
    let admission_work = budget.usage().work - proof_end;
    assert_eq!(
        selected.value(),
        &Rational::from_ratio(3, 1, &mut ledger()?)?
    );
    assert_eq!(selected.witness().hypotheses().len(), 1);
    assert_eq!(
        (physical_work, case_work, admission_work),
        (52_584, 32_295, 496)
    );
    assert_eq!(proof_end - physical_work - case_work, 166_952);
    assert_eq!(budget.usage().work, 252_327);
    assert_eq!(selected.retained_payload_bytes(), 13_842);
    assert_eq!(selected.usage()[1].work - selected.usage()[0].work, 496);
    let mut reused = ledger()?;
    case.admit(Rc::clone(&graph), repeated, case.attachment(), &mut reused)?;
    assert_eq!(reused.usage().work, 496);
    assert_eq!(graph.counts(), before);
    Ok(())
}

/// Equal exact nominal values do not allow an attachment to migrate to another physical declaration.
/// ´test:integration:bindings-decision-attachment-refusals´
#[test]
fn bindings_decision_attachment_refusals() -> TestResult {
    let owner = physical_source("first source", false)?;
    let mut source = fixture(false, "3", "10/Pi")?;
    let physical = DecisionExpression::declared(owner, "quotient", &mut ledger()?)?;
    let specification = specification(&source, CountDomain::NONNEGATIVE)?;
    let case = IntegerDecisionCase::declare(
        &source.frame.cas,
        physical.clone(),
        specification.clone(),
        &mut ledger()?,
    )?;
    let attachment = case.attachment();
    let mut alternatives = Vec::new();
    case.validate_attachment(&attachment, &mut ledger()?)?;
    for (name, doubled_unit) in [("other source", false), ("first source", true)] {
        let changed = DecisionExpression::declared(
            physical_source(name, doubled_unit)?,
            "quotient",
            &mut ledger()?,
        )?;
        assert_eq!(changed.exact(), physical.exact());
        let other = IntegerDecisionCase::declare(
            &source.frame.cas,
            changed,
            specification.clone(),
            &mut ledger()?,
        )?;
        alternatives.push(other);
    }
    let mut revision = specification.clone();
    revision.family.lower = None;
    let revised = IntegerDecisionCase::declare(
        &source.frame.cas,
        physical.clone(),
        revision,
        &mut ledger()?,
    )?;
    alternatives.push(revised);
    let mut parameters = specification;
    parameters.parameters.push(
        parameters
            .family
            .lower
            .clone()
            .ok_or("missing zero parameter")?,
    );
    let parameter_case =
        IntegerDecisionCase::declare(&source.frame.cas, physical, parameters, &mut ledger()?)?;
    alternatives.push(parameter_case);
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut ledger()?)?;
    let graph = Rc::new(source.frame.cas);
    let before = graph.counts();
    for other in alternatives {
        assert!(matches!(
            other.admit(
                Rc::clone(&graph),
                witness.clone(),
                attachment.clone(),
                &mut ledger()?
            ),
            Err(nrz_bindings::DecisionError::Attachment)
        ));
        assert_eq!(graph.counts(), before);
    }
    Ok(())
}

/// A source-free rational surrogate cannot replace the original physically checked quotient.
/// ´test:integration:bindings-decision-original-target-refusal´
#[test]
fn bindings_decision_original_target_refusal() -> TestResult {
    let physical = DecisionExpression::declared(
        physical_source("LC reference", false)?,
        "quotient",
        &mut ledger()?,
    )?;
    let source = fixture(false, "3", "51/16")?;
    let before = source.frame.cas.counts();
    let result = IntegerDecisionCase::declare(
        &source.frame.cas,
        physical,
        specification(&source, CountDomain::NONNEGATIVE)?,
        &mut ledger()?,
    );
    assert!(matches!(result, Err(nrz_bindings::DecisionError::Target)));
    assert_eq!(source.frame.cas.counts(), before);
    Ok(())
}

/// A certified mathematical result still checks the declared physical count domain and family.
/// ´test:integration:bindings-decision-domain-refusals´
#[test]
fn bindings_decision_domain_refusals() -> TestResult {
    let physical = DecisionExpression::declared(
        physical_source("LC reference", false)?,
        "quotient",
        &mut ledger()?,
    )?;
    let mut source = fixture(false, "3", "10/Pi")?;
    let narrow = IntegerDecisionCase::declare(
        &source.frame.cas,
        physical.clone(),
        specification(&source, CountDomain::new(Some(0), Some(2))?)?,
        &mut ledger()?,
    )?;
    let mut changed = specification(&source, CountDomain::NONNEGATIVE)?;
    changed.family.lower = None;
    let changed =
        IntegerDecisionCase::declare(&source.frame.cas, physical, changed, &mut ledger()?)?;
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut ledger()?)?;
    let graph = Rc::new(source.frame.cas);
    let before = graph.counts();
    assert!(matches!(
        narrow.admit(
            Rc::clone(&graph),
            witness.clone(),
            narrow.attachment(),
            &mut ledger()?
        ),
        Err(nrz_bindings::DecisionError::Units(
            nrz_bindings::UnitError::CountValue
        ))
    ));
    assert!(matches!(
        changed.admit(
            Rc::clone(&graph),
            witness,
            changed.attachment(),
            &mut ledger()?
        ),
        Err(nrz_bindings::DecisionError::Case)
    ));
    assert_eq!(graph.counts(), before);
    Ok(())
}

/// Each independently reusable preparation has its own literal caller-Work boundary.
/// ´test:integration:bindings-decision-preparation-boundaries´
#[test]
fn bindings_decision_preparation_boundaries() -> TestResult {
    let owner = physical_source("LC nominal reference", false)?;
    let before = owner.cas()?.counts();
    for (work, accepted) in [(52_584, true), (52_583, false)] {
        let mut budget = nrz_bindings::Budget::new(nrz_bindings::Limits {
            work,
            ..nrz_bindings::Limits::default()
        })?;
        let result = DecisionExpression::declared(Rc::clone(&owner), "quotient", &mut budget);
        assert_eq!(result.is_ok(), accepted);
        assert_eq!(owner.cas()?.counts(), before);
    }
    let physical = DecisionExpression::declared(owner, "quotient", &mut ledger()?)?;
    let source = fixture(false, "3", "10/Pi")?;
    let before = source.frame.cas.counts();
    for (work, accepted) in [(32_295, true), (32_294, false)] {
        let mut budget = nrz_bindings::Budget::new(nrz_bindings::Limits {
            work,
            ..nrz_bindings::Limits::default()
        })?;
        let result = IntegerDecisionCase::declare(
            &source.frame.cas,
            physical.clone(),
            specification(&source, CountDomain::NONNEGATIVE)?,
            &mut budget,
        );
        assert_eq!(result.is_ok(), accepted);
        assert_eq!(source.frame.cas.counts(), before);
    }
    Ok(())
}

/// Snapshot admission retains prior Work and refuses one below its exact Work or ownership bound.
/// ´test:integration:bindings-decision-snapshot-boundaries´
#[test]
fn bindings_decision_snapshot_boundaries() -> TestResult {
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
    let graph = Rc::new(source.frame.cas);
    let before = graph.counts();
    for (work, output_bytes, accepted) in [
        (513, 13_842, true),
        (512, 13_842, false),
        (513, 13_841, false),
    ] {
        let mut budget = nrz_bindings::Budget::new(nrz_bindings::Limits {
            work,
            output_bytes,
            ..nrz_bindings::Limits::default()
        })?;
        budget.work(17, nrz_cas::Location::new("prior physical decision work"))?;
        let result = case.admit(
            Rc::clone(&graph),
            witness.clone(),
            case.attachment(),
            &mut budget,
        );
        assert_eq!(result.is_ok(), accepted);
        if accepted {
            assert_eq!(budget.usage().work, 513);
        }
        assert!(budget.usage().work >= 17);
        assert_eq!(graph.counts(), before);
    }
    Ok(())
}

/// The complete new path uses a single cumulative ledger after existing declarations are prepared.
/// ´test:integration:bindings-decision-cumulative-boundary´
#[test]
fn bindings_decision_cumulative_boundary() -> TestResult {
    let owner = physical_source("LC nominal reference", false)?;
    for (work, accepted) in [(252_344, true), (252_343, false)] {
        let mut source = fixture(false, "3", "10/Pi")?;
        let mut budget = nrz_bindings::Budget::new(nrz_bindings::Limits {
            work,
            ..nrz_bindings::Limits::default()
        })?;
        budget.work(17, nrz_cas::Location::new("prior complete decision work"))?;
        let physical = DecisionExpression::declared(Rc::clone(&owner), "quotient", &mut budget)?;
        let case = IntegerDecisionCase::declare(
            &source.frame.cas,
            physical,
            specification(&source, CountDomain::NONNEGATIVE)?,
            &mut budget,
        )?;
        let witness = source
            .frame
            .cas
            .check_proof(&source.context, &source.steps, &mut budget)?;
        let graph = Rc::new(source.frame.cas);
        let before = graph.counts();
        let result = case.admit(Rc::clone(&graph), witness, case.attachment(), &mut budget);
        assert_eq!(result.is_ok(), accepted);
        if accepted {
            assert_eq!(budget.usage().work, 252_344);
        }
        assert_eq!(graph.counts(), before);
    }
    Ok(())
}
