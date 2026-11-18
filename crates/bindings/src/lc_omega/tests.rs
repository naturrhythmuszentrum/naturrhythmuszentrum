// SPDX-License-Identifier: AGPL-3.0-only
//! The rational original-declaration subset; no registered correspondence is asserted.

mod owning;

use super::*;
use crate::{
    ExponentPolicy, Kind, LcInput, LcInterval, LcRelation, Limits, ReferenceDefinition, Scale,
};
use nrz_cas::pi_bounds::PiBoundCertificate;

/// Preserve structured failures in independent fixtures.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Original owners remain separate from the borrowed checked view.
struct Fixture {
    /// Complete original rational question.
    question: LcForwardCover,
    /// Ordered source/mapped declarations.
    inputs: Vec<[BindingSymbols; 2]>,
    /// Private requested omega declaration.
    output: BindingSymbols,
    /// Exact original angle interpretation.
    radian: EntryMeaning,
    /// Checked physical multiplication rule.
    registry: Registry,
}

/// Construct original support without reusing an intersected rectangle.
fn input(quantity: LcQuantity, endpoints: [&str; 2], budget: &mut Budget) -> TestResult<LcInput> {
    let lower = Rational::parse(endpoints[0], budget)?;
    let upper = Rational::parse(endpoints[1], budget)?;
    Ok(LcInput::new(
        quantity,
        &quantity.descriptor()?,
        LcInterval::new(Some(lower.into()), Some(upper.into()), budget)?,
        budget,
    )?)
}

/// Named `lc_omega_rational_originals` fixture, with repeated L and zero-admitting Energy.
fn fixture(budget: &mut Budget) -> TestResult<Fixture> {
    let mut originals = Vec::new();
    let mut inputs = Vec::new();
    for (quantity, endpoints) in [
        (LcQuantity::Inductance, ["1", "9"]),
        (LcQuantity::Capacitance, ["1", "1"]),
        (LcQuantity::InitialPhase, ["0", "0"]),
        (LcQuantity::Inductance, ["1", "4"]),
        (LcQuantity::Energy, ["0", "1"]),
    ] {
        originals.push(input(quantity, endpoints, budget)?);
        inputs.push([
            BindingSymbols::declare(&quantity.descriptor()?, [1, 1], [true; 2], budget)?,
            BindingSymbols::declare(&quantity.descriptor()?, [1, 1], [true; 2], budget)?,
        ]);
    }
    let relation = LcRelation::new("lc_omega_rational_originals".into(), 0, originals, budget)?;
    let time = input(LcQuantity::Time(0), ["0", "0"], budget)?;
    let pi = PiBoundCertificate {
        version: 1,
        terms: [2, 1],
        sums: [
            Rational::parse("74/375", budget)?,
            Rational::parse("1/239", budget)?,
        ],
        lower: Rational::parse("31/10", budget)?,
        upper: Rational::parse("22/7", budget)?,
    };
    let question =
        relation.forward_cover(time, [Rational::zero(), Rational::zero()], &pi, budget)?;
    let angle = LcQuantity::InitialPhase.descriptor()?;
    let radian = EntryMeaning::new(
        angle.clone(),
        ReferenceDefinition::unit("original radian", &angle)?,
        None,
        budget,
    )?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_binary(
        BinaryOp::Multiply,
        Kind::rate()?,
        Kind::Angle,
        Kind::angular_frequency()?,
    )?;
    let output = BindingSymbols::declare(
        &LcQuantity::AngularFrequency.descriptor()?,
        [1, 1],
        [true; 2],
        budget,
    )?;
    Ok(Fixture {
        question,
        inputs,
        output,
        radian,
        registry,
    })
}

impl Fixture {
    /// Proposed view only borrows originals; a refusal cannot replace them.
    fn request(&self) -> LcOmegaOriginalsRequest<'_> {
        LcOmegaOriginalsRequest {
            question: &self.question,
            inputs: &self.inputs,
            positions: [0, 1],
            counts: [1, 1, 1],
            output: &self.output,
            radian: &self.radian,
            registry: &self.registry,
        }
    }
}

/// Ordered repeated positions, question allocation and private output identities are exact.
/// ´test:unit:lc-omega-originals-identity-refusals´
#[test]
fn lc_omega_originals_identity_refusals() -> TestResult {
    owning::identities()?;
    let mut budget = Budget::new(Limits::default())?;
    let f = fixture(&mut budget)?;
    let checked = LcOmegaOriginals::check(f.request(), &mut budget)?;
    checked.check_same(
        &LcOmegaOriginals::check(f.request(), &mut budget)?,
        &mut budget,
    )?;
    let clone = f.question.clone();
    assert!(clone.shares_allocation(&f.question));
    let pi = PiBoundCertificate {
        version: 1,
        terms: [2, 1],
        sums: [
            Rational::parse("74/375", &mut budget)?,
            Rational::parse("1/239", &mut budget)?,
        ],
        lower: Rational::parse("31/10", &mut budget)?,
        upper: Rational::parse("22/7", &mut budget)?,
    };
    let separate_question = f.question.relation().forward_cover(
        f.question.time().clone(),
        f.question.cycles().clone(),
        &pi,
        &mut budget,
    )?;
    assert!(
        separate_question
            .relation()
            .shares_allocation(f.question.relation())
    );
    assert!(!separate_question.shares_allocation(&f.question));
    let mut request = f.request();
    request.question = &separate_question;
    let other = LcOmegaOriginals::check(request, &mut budget)?;
    assert_eq!(
        checked.check_same(&other, &mut budget),
        Err(AdmissionError::ValueMismatch)
    );
    let mut request = f.request();
    request.positions = [3, 1];
    let other = LcOmegaOriginals::check(request, &mut budget)?;
    assert_eq!(
        checked.check_same(&other, &mut budget),
        Err(AdmissionError::ValueMismatch)
    );
    let foreign = fixture(&mut budget)?;
    assert!(!foreign.question.shares_allocation(&f.question));
    let mut request = f.request();
    request.question = &foreign.question;
    let other = LcOmegaOriginals::check(request, &mut budget)?;
    assert_eq!(
        checked.check_same(&other, &mut budget),
        Err(AdmissionError::ValueMismatch)
    );
    let mut swapped = f.inputs.clone();
    swapped.swap(0, 3);
    let mut request = f.request();
    request.inputs = &swapped;
    let other = LcOmegaOriginals::check(request, &mut budget)?;
    assert_eq!(
        checked.check_same(&other, &mut budget),
        Err(AdmissionError::ValueMismatch)
    );
    let mut request = f.request();
    request.output = &foreign.output;
    let other = LcOmegaOriginals::check(request, &mut budget)?;
    assert_eq!(
        checked.check_same(&other, &mut budget),
        Err(AdmissionError::ResultMismatch)
    );
    let text = format!("{checked:?}");
    assert_eq!(
        text,
        "LcOmegaOriginals { positions: [0, 1], counts: [1, 1, 1], .. }"
    );
    Ok(())
}

/// Original kinds, exact radians, companion targets and tuple shapes cannot be substituted.
/// ´test:unit:lc-omega-originals-kind-refusals´
#[test]
fn lc_omega_originals_kind_refusals() -> TestResult {
    let mut budget = Budget::new(Limits::default())?;
    let f = fixture(&mut budget)?;
    let mut request = f.request();
    request.positions = [1, 0];
    assert!(matches!(
        LcOmegaOriginals::check(request, &mut budget),
        Err(AdmissionError::ValueMismatch)
    ));
    for (quantity, magnitude) in [(LcQuantity::Rate, "1"), (LcQuantity::InitialPhase, "2")] {
        let descriptor = quantity.descriptor()?;
        let value = Rational::parse(magnitude, &mut budget)?;
        let meaning = EntryMeaning::new(
            descriptor.clone(),
            ReferenceDefinition::new("wrong radian", &descriptor, Scale::Exact(value))?,
            None,
            &mut budget,
        )?;
        let mut request = f.request();
        request.radian = &meaning;
        assert!(matches!(
            LcOmegaOriginals::check(request, &mut budget),
            Err(AdmissionError::ValueMismatch)
        ));
    }
    let original = LcQuantity::Inductance.descriptor()?;
    let foreign_kind = crate::Descriptor::new(
        Kind::Named(
            "equal-dimension foreign L".into(),
            Box::new(original.kind().dimension()?),
        ),
        crate::Measure::coherent(),
        crate::Role::Difference,
    )?;
    let scaled = crate::Descriptor::new(
        original.kind().clone(),
        crate::Measure::new(
            "scaled L",
            Scale::Exact(Rational::parse("2", &mut budget)?),
            Rational::zero(),
        )?,
        crate::Role::Difference,
    )?;
    for descriptor in [foreign_kind, scaled] {
        let mut substituted = f.inputs.clone();
        substituted[0][0] = BindingSymbols::declare(&descriptor, [1, 1], [true; 2], &mut budget)?;
        let mut request = f.request();
        request.inputs = &substituted;
        assert!(matches!(
            LcOmegaOriginals::check(request, &mut budget),
            Err(AdmissionError::ValueMismatch)
        ));
    }
    let missing = BindingSymbols::declare(
        &LcQuantity::AngularFrequency.descriptor()?,
        [1, 1],
        [false; 2],
        &mut budget,
    )?;
    let mut request = f.request();
    request.output = &missing;
    assert!(matches!(
        LcOmegaOriginals::check(request, &mut budget),
        Err(AdmissionError::CompanionMismatch)
    ));
    let mut malformed = f.output.clone();
    malformed.bound = f.inputs[0][0].bound.clone();
    let mut request = f.request();
    request.output = &malformed;
    assert!(matches!(
        LcOmegaOriginals::check(request, &mut budget),
        Err(AdmissionError::CompanionMismatch)
    ));
    let rate = BindingSymbols::declare(
        &LcQuantity::Rate.descriptor()?,
        [1, 1],
        [true; 2],
        &mut budget,
    )?;
    let mut request = f.request();
    request.output = &rate;
    assert!(matches!(
        LcOmegaOriginals::check(request, &mut budget),
        Err(AdmissionError::ValueMismatch)
    ));
    let mut request = f.request();
    request.counts = [1, 2, 1];
    assert!(matches!(
        LcOmegaOriginals::check(request, &mut budget),
        Err(AdmissionError::ValueMismatch)
    ));
    Ok(())
}

/// Original kinds, exact radians, companion targets and tuple shapes cannot be substituted.
/// ´test:unit:lc-omega-correspondence-kind-refusals´
#[test]
fn lc_omega_correspondence_kind_refusals() -> TestResult {
    let mut budget = Budget::new(Limits::default())?;
    let f = fixture(&mut budget)?;
    let mut request = f.request();
    request.positions = [1, 0];
    assert!(matches!(
        owning::admit(request, &mut budget),
        Err(AdmissionError::ValueMismatch)
    ));
    for (quantity, magnitude) in [(LcQuantity::Rate, "1"), (LcQuantity::InitialPhase, "2")] {
        let descriptor = quantity.descriptor()?;
        let value = Rational::parse(magnitude, &mut budget)?;
        let meaning = EntryMeaning::new(
            descriptor.clone(),
            ReferenceDefinition::new("wrong radian", &descriptor, Scale::Exact(value))?,
            None,
            &mut budget,
        )?;
        let mut request = f.request();
        request.radian = &meaning;
        assert!(matches!(
            owning::admit(request, &mut budget),
            Err(AdmissionError::ValueMismatch)
        ));
    }
    let original = LcQuantity::Inductance.descriptor()?;
    let foreign_kind = crate::Descriptor::new(
        Kind::Named(
            "equal-dimension foreign L".into(),
            Box::new(original.kind().dimension()?),
        ),
        crate::Measure::coherent(),
        crate::Role::Difference,
    )?;
    let scaled = crate::Descriptor::new(
        original.kind().clone(),
        crate::Measure::new(
            "scaled L",
            Scale::Exact(Rational::parse("2", &mut budget)?),
            Rational::zero(),
        )?,
        crate::Role::Difference,
    )?;
    for descriptor in [foreign_kind, scaled] {
        let mut substituted = f.inputs.clone();
        substituted[0][0] = BindingSymbols::declare(&descriptor, [1, 1], [true; 2], &mut budget)?;
        let mut request = f.request();
        request.inputs = &substituted;
        assert!(matches!(
            owning::admit(request, &mut budget),
            Err(AdmissionError::ValueMismatch)
        ));
    }
    let missing = BindingSymbols::declare(
        &LcQuantity::AngularFrequency.descriptor()?,
        [1, 1],
        [false; 2],
        &mut budget,
    )?;
    let mut request = f.request();
    request.output = &missing;
    assert!(matches!(
        owning::admit(request, &mut budget),
        Err(AdmissionError::CompanionMismatch)
    ));
    let mut malformed = f.output.clone();
    malformed.bound = f.inputs[0][0].bound.clone();
    let mut request = f.request();
    request.output = &malformed;
    assert!(matches!(
        owning::admit(request, &mut budget),
        Err(AdmissionError::CompanionMismatch)
    ));
    let rate = BindingSymbols::declare(
        &LcQuantity::Rate.descriptor()?,
        [1, 1],
        [true; 2],
        &mut budget,
    )?;
    let mut request = f.request();
    request.output = &rate;
    assert!(matches!(
        owning::admit(request, &mut budget),
        Err(AdmissionError::ValueMismatch)
    ));
    let mut request = f.request();
    request.counts = [1, 2, 1];
    assert!(matches!(
        owning::admit(request, &mut budget),
        Err(AdmissionError::ValueMismatch)
    ));
    Ok(())
}

/// Early and late physical refusal preserves every borrowed owner and spent Work.
/// ´test:unit:lc-omega-originals-failure-ownership´
#[test]
fn lc_omega_originals_failure_ownership() -> TestResult {
    owning::failures()?;
    let mut budget = Budget::new(Limits::default())?;
    let f = fixture(&mut budget)?;
    let question = f.question.clone();
    let output = f.output.clone();
    let baseline = budget.usage().work;
    let mut request = f.request();
    request.counts = [1, 1, 0];
    assert!(LcOmegaOriginals::check(request, &mut budget).is_err());
    assert_eq!(budget.usage().work, baseline + 1);
    let wrong_registry = Registry::new(ExponentPolicy::Rational);
    let mut request = f.request();
    request.registry = &wrong_registry;
    assert!(matches!(
        LcOmegaOriginals::check(request, &mut budget),
        Err(AdmissionError::Units(_))
    ));
    assert_eq!(budget.usage().work, baseline + 874);
    assert!(f.question.shares_allocation(&question));
    assert_eq!(f.output.value(), output.value());
    assert_eq!(f.output.bound(), output.bound());
    assert_eq!(f.output.error(), output.error());
    LcOmegaOriginals::check(f.request(), &mut budget)?;
    Ok(())
}

/// Literal event inventory: 44 physical visits and 26 one-limb arithmetic tariffs.
/// The view retains ten machine words, with no new graph or owning edge.
/// ´test:unit:lc-omega-originals-resource-boundaries´
#[test]
fn lc_omega_originals_resource_boundaries() -> TestResult {
    owning::boundaries()?;
    let mut preparation = Budget::new(Limits::default())?;
    let f = fixture(&mut preparation)?;
    // Preflight, branch visit, selected positions, ordered rows, symbols, radian,
    // registry relation and the three requested output channels are disjoint.
    let visits = 1 + 1 + 2 + 5 + 5 * 2 * 3 + 1 + 1 + 3;
    // Zero-minus-zero costs 13 tariffs, plus one costs 12, component read costs one.
    let arithmetic = (13 + 12 + 1) * 32;
    assert_eq!(visits + arithmetic, 876);
    assert_eq!(1 + 5 * 2 * 3 + 3, 34);
    assert_eq!(std::mem::size_of::<LcOmegaOriginals<'_>>(), 80);
    for ceiling in [883, 882] {
        let mut budget = Budget::new(Limits {
            work: ceiling,
            ..Limits::default()
        })?;
        budget.work(7, AT)?;
        let result = LcOmegaOriginals::check(f.request(), &mut budget);
        if ceiling == 883 {
            result?;
            assert_eq!(budget.usage().work, 883);
        } else {
            exhausted(result, BudgetKind::Work, 882, 883)?;
            assert_eq!(budget.usage().work, 880);
        }
    }
    let checked = LcOmegaOriginals::check(f.request(), &mut preparation)?;
    for ceiling in [41, 40] {
        let mut budget = Budget::new(Limits {
            work: ceiling,
            ..Limits::default()
        })?;
        budget.work(7, AT)?;
        let result = checked.check_same(&checked, &mut budget);
        if ceiling == 41 {
            result?;
            assert_eq!(budget.usage().work, 41);
        } else {
            exhausted(result, BudgetKind::Work, 40, 41)?;
            assert_eq!(budget.usage().work, 39);
        }
    }
    byte_boundaries(&f)
}

/// Preserve exact CAS fault kind, ceiling and needed amount through bindings.
fn exhausted<T>(
    result: Result<T, AdmissionError>,
    kind: BudgetKind,
    limit: u64,
    needed: u64,
) -> TestResult {
    let Err(AdmissionError::Algebra(error)) = result else {
        return Err("expected preserved CAS exhaustion".into());
    };
    assert!(matches!(error.kind, nrz_cas::FaultKind::Exhausted {
        budget, limit: actual_limit, needed: actual_needed, ..
    } if budget == kind && actual_limit == limit && actual_needed == needed));
    Ok(())
}

/// Caller residence remains reserved during admission and is restored on either outcome.
fn byte_boundaries(f: &Fixture) -> TestResult {
    for ceiling in [80, 79] {
        let mut budget = Budget::new(Limits {
            output_bytes: ceiling,
            ..Limits::default()
        })?;
        budget.work(7, AT)?;
        let result = LcOmegaOriginals::check(f.request(), &mut budget);
        if ceiling == 80 {
            result?;
            assert_eq!(budget.usage().work, 883);
        } else {
            exhausted(result, BudgetKind::OutputBytes, 79, 80)?;
            assert_eq!(budget.usage().work, 8);
        }
    }
    for ceiling in [117, 116] {
        let mut budget = Budget::new(Limits {
            staged_payload_bytes: ceiling,
            ..Limits::default()
        })?;
        budget.work(7, AT)?;
        let result = budget.with_retained_payload(37, AT, |budget| {
            LcOmegaOriginals::check(f.request(), budget)
        });
        if ceiling == 117 {
            result?;
            assert_eq!(budget.usage().work, 883);
        } else {
            exhausted(result, BudgetKind::StagedPayloadBytes, 79, 80)?;
            assert_eq!(budget.usage().work, 8);
        }
        assert_eq!(budget.limits().staged_payload_bytes, ceiling);
        budget.check(BudgetKind::StagedPayloadBytes, ceiling, AT)?;
    }
    Ok(())
}
