// SPDX-License-Identifier: AGPL-3.0-only
//! Native floor-family proof, complete domain identities and independent replay.

#[path = "lattice_index.rs"]
mod lattice_index;
#[path = "parameters.rs"]
mod parameters;
#[path = "selection.rs"]
mod selection;

use super::{Frame, admit, ledger, real};
use crate::testing::common::TestResult;
use nrz_cas::decision::IntegerFamily;
use nrz_cas::domain::{ContextDraft, Layer, RegionExpr};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::proof::{ApplicationDraft, Certificate, Judgment, Primitive, Statement};
use nrz_cas::syntax::{AtomicSort, Draft, Operator, PredicateKind, Sort};
use nrz_cas::{Cas, FaultKind, Handle, InvalidCode, Location, Rational, Reference};

struct Fixture {
    frame: Frame,
    context: Handle,
    application: ApplicationDraft,
}

fn fixture(bounded: bool) -> TestResult<Fixture> {
    let mut frame = Frame::new()?;
    let (relation, [q, n]) = frame.relation(bounded)?;
    let (function, _) = frame.cases(&relation, n.clone())?;
    let zero = frame.term(
        Operator::Rational(Rational::zero()),
        Sort::Value(AtomicSort::Integer),
        &[],
    )?;
    let mut domain = frame.guard(PredicateKind::NonNegative, std::slice::from_ref(&q))?;
    let upper = if bounded {
        let five = Rational::from_ratio(5, 1, &mut ledger()?)?;
        let bound = frame.term(
            Operator::Rational(five.clone()),
            Sort::Value(AtomicSort::Integer),
            &[],
        )?;
        let constant = frame.term(Operator::Rational(five), real(), &[])?;
        let difference = frame.term(Operator::Sub, real(), &[constant, q.clone()])?;
        let high = frame.guard(PredicateKind::Positive, &[difference])?;
        domain = frame.region(RegionExpr::Meet(vec![
            Reference::Existing(domain),
            Reference::Existing(high),
        ]))?;
        Some(bound)
    } else {
        None
    };
    prepared(
        frame,
        IntegerFamily {
            function,
            coordinate: q,
            index: n,
            domain,
            lower: Some(zero),
            upper,
            count: None,
        },
    )
}

/// Complete universal floor proofs preserve registered citations through independent portable replay.
/// ´test:integration:integer-family-cover´
#[test]
fn integer_family_cover() -> TestResult {
    let mut measured = Vec::new();
    for bounded in [false, true] {
        let mut source = fixture(bounded)?;
        let before = source.frame.cas.counts();
        let mut budget = ledger()?;
        let witness =
            source
                .frame
                .cas
                .check_proof(&source.context, &[source.application], &mut budget)?;
        let bytes = source.frame.cas.export_witness(&witness, &mut ledger()?)?;
        let mut receiver = Cas::new(Limits::default())?;
        let mut replay = ledger()?;
        let imported = receiver.inspect(&bytes, &mut replay)?;
        assert!(imported.is_unconditional());
        assert_eq!(receiver.export_witness(&imported, &mut ledger()?)?, bytes);
        let citations = source
            .frame
            .cas
            .proof_citations(witness.proof(), &mut ledger()?)?;
        assert_eq!(citations.len(), 1);
        let schema = citations[0]
            .auxiliary_schema(&source.frame.cas)?
            .ok_or("missing floor schema")?;
        assert_eq!(schema.name, "decision.integer-floor-family");
        assert_eq!(schema.version, 2);
        measured.push((
            bounded,
            budget.usage().work,
            source.frame.cas.counts().payload_bytes - before.payload_bytes,
            bytes.len(),
            replay.usage().work,
            receiver.counts().payload_bytes,
        ));
    }
    assert_eq!(
        measured,
        [
            (false, 67_141, 1_087, 1_010, 124_524, 11_939),
            (true, 98_253, 1_146, 1_256, 172_358, 14_897),
        ]
    );
    Ok(())
}

fn family_mut(source: &mut Fixture) -> TestResult<&mut IntegerFamily> {
    let Statement::IntegerFamily(family) = &mut source.application.conclusion else {
        return Err("missing family statement".into());
    };
    Ok(family)
}

fn refusal(source: &mut Fixture) -> TestResult {
    let before = source.frame.cas.counts();
    let snapshot = source.frame.cas.canonical_snapshot(&mut ledger()?)?;
    let mut budget = ledger()?;
    let result = source.frame.cas.check_proof(
        &source.context,
        std::slice::from_ref(&source.application),
        &mut budget,
    );
    assert!(result.is_err());
    assert!(budget.usage().work > 0);
    assert_eq!(source.frame.cas.counts(), before);
    assert_eq!(
        source.frame.cas.canonical_snapshot(&mut ledger()?)?,
        snapshot
    );
    Ok(())
}

/// A projected theorem checks range nonemptiness, exact index identity and Integer endpoint sorts.
/// ´test:integration:integer-family-cover-refusals´
#[test]
fn integer_family_cover_refusals() -> TestResult {
    let mut source = fixture(true)?;
    let original = source.application.clone();
    let zero = family_mut(&mut source)?.lower.clone();
    family_mut(&mut source)?.upper = zero;
    refusal(&mut source)?;
    source.application.clone_from(&original);
    let integer = source.frame.term(
        Operator::Rational(Rational::one()),
        Sort::Value(AtomicSort::Integer),
        &[],
    )?;
    family_mut(&mut source)?.index = integer.clone();
    refusal(&mut source)?;
    source.application.clone_from(&original);
    family_mut(&mut source)?.lower = Some(integer);
    refusal(&mut source)?;
    source.application.clone_from(&original);
    let real_zero = source
        .frame
        .term(Operator::Rational(Rational::zero()), real(), &[])?;
    family_mut(&mut source)?.lower = Some(real_zero);
    refusal(&mut source)?;
    source.application.clone_from(&original);
    source
        .frame
        .cas
        .check_proof(&source.context, &[original], &mut ledger()?)?;
    Ok(())
}

/// A cover of a smaller interval cannot certify a widened projected input domain.
/// ´test:integration:integer-family-cover-domain-hole´
#[test]
fn integer_family_cover_domain_hole() -> TestResult {
    for bounded in [false, true] {
        let mut source = fixture(bounded)?;
        let ambient = source.frame.ambient.clone();
        let context = admit(
            &mut source.frame.cas,
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(source.frame.header.clone()),
                admissible: Reference::Existing(ambient.clone()),
                assumptions: Vec::new(),
            }),
            &mut ledger()?,
        )?;
        family_mut(&mut source)?.domain = ambient;
        source.context.clone_from(&context);
        source.application.context = context;
        refusal(&mut source)?;
    }
    Ok(())
}

fn general_fixture(lower: Option<i64>, upper: Option<i64>) -> TestResult<Fixture> {
    let mut frame = Frame::new()?;
    let q = frame.term(Operator::Free("q".into()), real(), &[])?;
    let n = frame.term(
        Operator::Free("n".into()),
        Sort::Value(AtomicSort::Integer),
        &[],
    )?;
    let embedded = frame.term(Operator::IntegerEmbed, real(), std::slice::from_ref(&n))?;
    let one = frame.term(Operator::Rational(Rational::one()), real(), &[])?;
    let end = frame.term(Operator::Add, real(), &[embedded.clone(), one])?;
    let left = frame.term(Operator::Sub, real(), &[q.clone(), embedded])?;
    let right = frame.term(Operator::Sub, real(), &[end, q.clone()])?;
    let mut relation = vec![
        frame.guard(PredicateKind::NonNegative, &[left])?,
        frame.guard(PredicateKind::Positive, &[right])?,
    ];
    let mut projected = Vec::new();
    let mut endpoints = [None, None];
    for (index, bound) in [lower, upper].into_iter().enumerate() {
        if let Some(bound) = bound {
            let value = Rational::from_ratio(bound, 1, &mut ledger()?)?;
            let value = frame.term(
                Operator::Rational(value),
                Sort::Value(AtomicSort::Integer),
                &[],
            )?;
            relation.push(bound_guard(&mut frame, &n, &value, index == 1, false)?);
            projected.push(bound_guard(&mut frame, &q, &value, index == 1, true)?);
            endpoints[index] = Some(value);
        }
    }
    let relation = frame.region(RegionExpr::Meet(
        relation.into_iter().map(Reference::Existing).collect(),
    ))?;
    let domain = frame.region(RegionExpr::Meet(
        projected.into_iter().map(Reference::Existing).collect(),
    ))?;
    let (function, _) = frame.cases(&relation, n.clone())?;
    let [lower, upper] = endpoints;
    prepared(
        frame,
        IntegerFamily {
            function,
            coordinate: q,
            index: n,
            domain,
            lower,
            upper,
            count: None,
        },
    )
}

fn bound_guard(
    frame: &mut Frame,
    subject: &Handle,
    endpoint: &Handle,
    upper: bool,
    projected: bool,
) -> TestResult<Handle> {
    let endpoint = if projected {
        frame.term(
            Operator::IntegerEmbed,
            real(),
            std::slice::from_ref(endpoint),
        )?
    } else {
        endpoint.clone()
    };
    let sort = if projected {
        real()
    } else {
        Sort::Value(AtomicSort::Integer)
    };
    let operands = if upper {
        [endpoint, subject.clone()]
    } else {
        [subject.clone(), endpoint]
    };
    let difference = frame.term(Operator::Sub, sort, &operands)?;
    let kind = if upper {
        PredicateKind::Positive
    } else {
        PredicateKind::NonNegative
    };
    frame.guard(kind, &[difference])
}

/// Exact missing bounds admit all integers or a one-sided family, including negative indices.
/// ´test:integration:integer-family-cover-unbounded´
#[test]
fn integer_family_cover_unbounded() -> TestResult {
    for (lower, upper) in [
        (None, None),
        (None, Some(-2)),
        (Some(-4), None),
        (Some(-4), Some(-2)),
    ] {
        let mut source = general_fixture(lower, upper)?;
        let witness =
            source
                .frame
                .cas
                .check_proof(&source.context, &[source.application], &mut ledger()?)?;
        assert!(witness.is_unconditional());
        let bytes = source.frame.cas.export_witness(&witness, &mut ledger()?)?;
        let mut receiver = Cas::new(Limits::default())?;
        let imported = receiver.inspect(&bytes, &mut ledger()?)?;
        assert_eq!(receiver.export_witness(&imported, &mut ledger()?)?, bytes);
    }
    Ok(())
}

/// Closing the upper cell edge creates two indices at an integer and refuses uniqueness.
/// ´test:integration:integer-family-cover-closed-upper´
#[test]
fn integer_family_cover_closed_upper() -> TestResult {
    let mut source = fixture(false)?;
    let q = family_mut(&mut source)?.coordinate.clone();
    let n = family_mut(&mut source)?.index.clone();
    let frame = &mut source.frame;
    let embedded = frame.term(Operator::IntegerEmbed, real(), std::slice::from_ref(&n))?;
    let one = frame.term(Operator::Rational(Rational::one()), real(), &[])?;
    let end = frame.term(Operator::Add, real(), &[embedded.clone(), one])?;
    let low = frame.term(Operator::Sub, real(), &[q.clone(), embedded])?;
    let high = frame.term(Operator::Sub, real(), &[end, q])?;
    let lower = frame.guard(PredicateKind::NonNegative, &[low])?;
    let upper = frame.guard(PredicateKind::NonNegative, &[high])?;
    let range = frame.guard(PredicateKind::NonNegative, std::slice::from_ref(&n))?;
    let relation = frame.region(RegionExpr::Meet(vec![
        Reference::Existing(lower),
        Reference::Existing(upper),
        Reference::Existing(range),
    ]))?;
    let (function, _) = frame.cases(&relation, n)?;
    family_mut(&mut source)?.function = function;
    refusal(&mut source)
}

fn resource_limits(kind: BudgetKind, amount: u64) -> TestResult<Limits> {
    let mut limits = Limits::default();
    match kind {
        BudgetKind::Work => limits.work = amount,
        BudgetKind::ReplayWork => limits.replay_work = amount,
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = amount,
        BudgetKind::OutputBytes => limits.output_bytes = amount,
        BudgetKind::PayloadBytes => limits.payload_bytes = amount,
        _ => return Err("unsupported floor resource".into()),
    }
    Ok(limits)
}

fn resource_attempt(
    bounded: bool,
    replay: bool,
    kind: BudgetKind,
    amount: u64,
    prior: u64,
) -> TestResult<bool> {
    let mut source = fixture(bounded)?;
    let bytes = if replay {
        let witness = source.frame.cas.check_proof(
            &source.context,
            std::slice::from_ref(&source.application),
            &mut ledger()?,
        )?;
        Some(source.frame.cas.export_witness(&witness, &mut ledger()?)?)
    } else {
        None
    };
    let mut receiver = Cas::new(Limits::default())?;
    let cas = if replay {
        &mut receiver
    } else {
        &mut source.frame.cas
    };
    let before = cas.counts();
    let limits = resource_limits(kind, amount)?;
    let mut budget = Budget::new(limits.clone())?;
    budget.work(prior, Location::new("prior floor proof work"))?;
    let result = if let Some(bytes) = bytes {
        cas.inspect(&bytes, &mut budget)
    } else {
        cas.check_proof(&source.context, &[source.application], &mut budget)
    };
    assert!(budget.usage().work >= prior);
    assert_eq!(budget.limits(), &limits);
    match result {
        Ok(_) => Ok(true),
        Err(fault) => {
            assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind));
            assert_eq!(cas.counts(), before);
            Ok(false)
        }
    }
}

/// Literal proof/import resource limits are atomic and keep caller Work cumulative.
/// ´test:integration:integer-family-resource-boundaries´
#[test]
fn integer_family_resource_boundaries() -> TestResult {
    for (bounded, replay, work, staged, payload) in [
        (false, false, 67_141, 79_744, 11_939),
        (false, true, 124_524, 79_040, 11_939),
        (true, false, 98_253, 87_616, 14_897),
        (true, true, 172_358, 86_912, 14_897),
    ] {
        for (kind, amount) in [
            (BudgetKind::Work, work),
            (BudgetKind::StagedPayloadBytes, staged),
            (BudgetKind::PayloadBytes, payload),
        ] {
            assert!(resource_attempt(bounded, replay, kind, amount, 0)?);
            assert!(!resource_attempt(bounded, replay, kind, amount - 1, 0)?);
        }
        assert!(resource_attempt(
            bounded,
            replay,
            BudgetKind::OutputBytes,
            0,
            0
        )?);
        assert!(!resource_attempt(
            bounded,
            replay,
            BudgetKind::Work,
            work,
            1
        )?);
        assert!(resource_attempt(
            bounded,
            replay,
            BudgetKind::Work,
            work + 1,
            1
        )?);
        if replay {
            assert!(resource_attempt(
                bounded,
                true,
                BudgetKind::ReplayWork,
                work,
                0
            )?);
            assert!(!resource_attempt(
                bounded,
                true,
                BudgetKind::ReplayWork,
                work - 1,
                0
            )?);
        }
    }
    Ok(())
}

fn prepared(mut frame: Frame, family: IntegerFamily) -> TestResult<Fixture> {
    let context = admit(
        &mut frame.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(frame.header.clone()),
            admissible: Reference::Existing(family.domain.clone()),
            assumptions: Vec::new(),
        }),
        &mut ledger()?,
    )?;
    let application = ApplicationDraft {
        rule: Primitive::RegionDecide,
        context: context.clone(),
        conclusion: Statement::IntegerFamily(Box::new(family)),
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    };
    Ok(Fixture {
        frame,
        context,
        application,
    })
}

/// Acceptance compares the independently built complete family judgment, including optional bounds.
/// ´test:integration:integer-family-cover-expected´
#[test]
fn integer_family_cover_expected() -> TestResult {
    let mut source = fixture(true)?;
    let witness =
        source
            .frame
            .cas
            .check_proof(&source.context, &[source.application], &mut ledger()?)?;
    let bytes = source.frame.cas.export_witness(&witness, &mut ledger()?)?;
    let mut receiver = fixture(true)?;
    let mut expected = Judgment {
        context: receiver.context,
        layer: Layer::Logic,
        space: None,
        statement: receiver.application.conclusion,
        residuals: Vec::new(),
    };
    let replayed = receiver
        .frame
        .cas
        .replay(&bytes, &expected, &mut ledger()?)?;
    assert_eq!(replayed.judgment(), &expected);
    let Statement::IntegerFamily(family) = &mut expected.statement else {
        return Err("missing expected family".into());
    };
    family.lower = None;
    let before = receiver.frame.cas.counts();
    let fault = receiver
        .frame
        .cas
        .replay(&bytes, &expected, &mut ledger()?)
        .err()
        .ok_or("changed projected range accepted")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(receiver.frame.cas.counts(), before);
    Ok(())
}

/// Scalar floor proof and fresh replay ownership are the unchanged lattice-coordinate baseline.
/// ´test:integration:lattice-scalar-baseline´
#[test]
fn lattice_scalar_baseline() -> TestResult {
    let mut measured = Vec::new();
    for range in [0, 1, 2] {
        let mut source = if range == 0 {
            general_fixture(None, None)?
        } else {
            fixture(range == 2)?
        };
        let before = source.frame.cas.counts();
        let mut budget = ledger()?;
        let witness =
            source
                .frame
                .cas
                .check_proof(&source.context, &[source.application], &mut budget)?;
        let bytes = source.frame.cas.export_witness(&witness, &mut ledger()?)?;
        let mut receiver = Cas::new(Limits::default())?;
        let mut replay = ledger()?;
        receiver.inspect(&bytes, &mut replay)?;
        measured.push((
            range,
            budget.usage().work,
            source.frame.cas.counts().payload_bytes - before.payload_bytes,
            bytes.len(),
            replay.usage().work,
            receiver.counts().payload_bytes,
        ));
    }
    assert_eq!(
        measured,
        [
            (0, 50_969, 1_028, 868, 98_096, 10_039),
            (1, 67_141, 1_087, 1_010, 124_524, 11_939),
            (2, 98_253, 1_146, 1_256, 172_358, 14_897),
        ]
    );
    Ok(())
}

/// Unrestricted scalar proof and replay use literal Work boundaries with cumulative prior usage.
/// ´test:integration:lattice-scalar-baseline-work´
#[test]
fn lattice_scalar_baseline_work() -> TestResult {
    for (replay, needed) in [(false, 50_969), (true, 98_096)] {
        for (allowance, prior, succeeds) in [
            (needed, 0, true),
            (needed - 1, 0, false),
            (needed, 37, false),
            (needed + 37, 37, true),
        ] {
            let mut source = general_fixture(None, None)?;
            let bytes = if replay {
                let witness = source.frame.cas.check_proof(
                    &source.context,
                    std::slice::from_ref(&source.application),
                    &mut ledger()?,
                )?;
                Some(source.frame.cas.export_witness(&witness, &mut ledger()?)?)
            } else {
                None
            };
            let mut receiver = Cas::new(Limits::default())?;
            let cas = if replay {
                &mut receiver
            } else {
                &mut source.frame.cas
            };
            let before = cas.counts();
            let limits = Limits {
                work: allowance,
                ..Limits::default()
            };
            let mut budget = Budget::new(limits.clone())?;
            budget.work(prior, Location::new("prior lattice baseline work"))?;
            let result = if let Some(bytes) = bytes {
                cas.inspect(&bytes, &mut budget)
            } else {
                cas.check_proof(&source.context, &[source.application], &mut budget)
            };
            assert_eq!(result.is_ok(), succeeds);
            assert!(budget.usage().work >= prior);
            assert_eq!(budget.limits(), &limits);
            if succeeds {
                assert_eq!(budget.usage().work, needed + prior);
            } else {
                let fault = result.err().ok_or("missing Work refusal")?;
                assert!(matches!(
                    fault.kind,
                    FaultKind::Exhausted {
                        budget: BudgetKind::Work,
                        ..
                    }
                ));
                assert_eq!(cas.counts(), before);
            }
        }
    }
    Ok(())
}
