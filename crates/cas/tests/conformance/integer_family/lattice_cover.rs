// SPDX-License-Identifier: AGPL-3.0-only
//! Native finite cover, complete affine evidence and independent ordinary replay.

#[path = "lattice_selection.rs"]
mod selection;

use super::{Product, fixture, ledger, matrix, real};
use crate::testing::common::TestResult;
use nrz_cas::lattice::{LatticeBasisOutcome, LatticeBasisQuestion};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::proof::{ApplicationDraft, Certificate, Primitive, Statement, VerifiedWitness};
use nrz_cas::syntax::{Constant, Operator};
use nrz_cas::{Cas, FaultKind, InvalidCode, Rational};

fn product(kind: u32) -> TestResult<Product> {
    let base = if kind == 5 {
        2
    } else if kind < 3 {
        kind
    } else {
        0
    };
    let mut source = fixture(base, 2, false, false)?;
    if kind < 3 {
        return Ok(source);
    }
    let count = if kind == 5 { 1 } else { 2 };
    let (entries, offsets) = if kind == 5 {
        let pi = source
            .frame
            .term(Operator::Constant(Constant::Pi), real(), &[])?;
        let two = source.frame.term(
            Operator::Rational(Rational::from_ratio(2, 1, &mut ledger()?)?),
            real(),
            &[],
        )?;
        (
            vec![
                source
                    .frame
                    .term(Operator::Mul, real(), &[two, pi.clone()])?,
            ],
            vec![pi],
        )
    } else {
        let mut entries = Vec::new();
        for value in if kind == 3 || kind == 6 {
            [1, 1, 1, 2]
        } else {
            [2, 3, 5, 7]
        } {
            entries.push(source.frame.term(
                Operator::Rational(Rational::from_ratio(value, 1, &mut ledger()?)?),
                real(),
                &[],
            )?);
        }
        let mut offsets = Vec::new();
        for value in if kind == 6 { [4, 6] } else { [3, 5] } {
            offsets.push(source.frame.term(
                Operator::Rational(Rational::from_ratio(value, 1, &mut ledger()?)?),
                real(),
                &[],
            )?);
        }
        (entries, offsets)
    };
    let basis = source.frame.term(
        Operator::Matrix {
            rows: count,
            columns: count,
        },
        matrix(count, count),
        &entries,
    )?;
    let offset = source.frame.term(
        Operator::Matrix {
            rows: count,
            columns: 1,
        },
        matrix(count, 1),
        &offsets,
    )?;
    let question = LatticeBasisQuestion {
        context: source.question.basis.context.clone(),
        basis,
        offset,
    };
    let LatticeBasisOutcome::Admitted(basis) = source
        .frame
        .cas
        .admit_lattice_basis(&question, &mut ledger()?)?
    else {
        return Err("cover basis unresolved".into());
    };
    source.basis = basis;
    source.question.basis = question;
    Ok(source)
}

fn prove(source: &mut Product, budget: &mut Budget) -> TestResult<VerifiedWitness> {
    let index = source.frame.cas.admit_lattice_index_domain(
        &source.basis,
        &source.question,
        &source.covers,
        &mut ledger()?,
    )?;
    Ok(source
        .frame
        .cas
        .prove_lattice_cover(&source.basis, &index, budget)?)
}

/// One finite checked schema retains both affine inverse orders and one floor lemma per binder.
/// ´test:integration:lattice-cover-domains´
#[test]
fn lattice_cover_domains() -> TestResult {
    for kind in 0..6 {
        let mut source = product(kind)?;
        let proof = prove(&mut source, &mut ledger()?)?;
        let Statement::LatticeCover(value) = &proof.judgment().statement else {
            return Err("wrong cover statement".into());
        };
        assert_eq!(value.index, source.question);
        assert_eq!(value.forward, *source.basis.forward().expression());
        assert_eq!(value.inverse, *source.basis.inverse().map().expression());
        assert!(proof.is_discharged());
        let citations = source
            .frame
            .cas
            .proof_citations(proof.proof(), &mut ledger()?)?;
        let root = citations
            .iter()
            .find(|citation| &citation.proof == proof.proof())
            .ok_or("missing cover citation")?;
        assert_eq!(root.premises.len(), 3 + source.question.families.len());
        let schema = root
            .auxiliary_schema(&source.frame.cas)?
            .ok_or("missing cover schema")?;
        assert_eq!(schema.name, "decision.lattice-floor-cover");
        assert_eq!(schema.version, 1);
        assert_eq!(
            proof.hypotheses().len(),
            if kind == 2 || kind == 5 { 2 } else { 0 }
        );
        let mut receiver = Cas::new(Limits::default())?;
        let bytes = source.frame.cas.export_witness(&proof, &mut ledger()?)?;
        let inspected = receiver.inspect(&bytes, &mut ledger()?)?;
        assert_eq!(inspected.hypotheses().len(), proof.hypotheses().len());
        assert_eq!(receiver.export_witness(&inspected, &mut ledger()?)?, bytes);
    }
    Ok(())
}

/// Expected-judgment replay preserves parameter scope and refuses reordered binders or changed original bases.
/// ´test:integration:lattice-cover-replay´
#[test]
fn lattice_cover_replay() -> TestResult {
    let mut source = product(3)?;
    let proof = prove(&mut source, &mut ledger()?)?;
    let bytes = source.frame.cas.export_witness(&proof, &mut ledger()?)?;
    let mut receiver = product(3)?;
    let expected = prove(&mut receiver, &mut ledger()?)?.judgment().clone();
    receiver
        .frame
        .cas
        .replay(&bytes, &expected, &mut ledger()?)?;
    let mut reordered = expected.clone();
    let Statement::LatticeCover(value) = &mut reordered.statement else {
        return Err("wrong statement".into());
    };
    value.index.families.swap(0, 1);
    let before = receiver.frame.cas.counts();
    let fault = receiver
        .frame
        .cas
        .replay(&bytes, &reordered, &mut ledger()?)
        .err()
        .ok_or("changed binder order accepted")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(receiver.frame.cas.counts(), before);
    for kind in [4, 6] {
        let mut changed = product(kind)?;
        let expected = prove(&mut changed, &mut ledger()?)?.judgment().clone();
        let before = changed.frame.cas.counts();
        let fault = changed
            .frame
            .cas
            .replay(&bytes, &expected, &mut ledger()?)
            .err()
            .ok_or("changed basis or offset accepted")?;
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::EndpointMismatch)
        );
        assert_eq!(changed.frame.cas.counts(), before);
    }
    let mut parameters = product(2)?;
    let proof = prove(&mut parameters, &mut ledger()?)?;
    let bytes = parameters
        .frame
        .cas
        .export_witness(&proof, &mut ledger()?)?;
    let mut receiver = product(2)?;
    let expected = prove(&mut receiver, &mut ledger()?)?.judgment().clone();
    let replayed = receiver
        .frame
        .cas
        .replay(&bytes, &expected, &mut ledger()?)?;
    assert_eq!(replayed.hypotheses().len(), 2);
    let mut changed = fixture(2, 3, false, false)?;
    let expected = prove(&mut changed, &mut ledger()?)?.judgment().clone();
    let before = changed.frame.cas.counts();
    let fault = changed
        .frame
        .cas
        .replay(&bytes, &expected, &mut ledger()?)
        .err()
        .ok_or("changed binding context accepted")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(changed.frame.cas.counts(), before);
    Ok(())
}

/// A typed cover statement without its complete checked premise inventory has no authority.
/// ´test:integration:lattice-cover-missing-premises´
#[test]
fn lattice_cover_missing_premises() -> TestResult {
    let mut source = product(0)?;
    let proof = prove(&mut source, &mut ledger()?)?;
    let draft = ApplicationDraft {
        rule: Primitive::RegionDecide,
        context: proof.judgment().context.clone(),
        conclusion: proof.judgment().statement.clone(),
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    };
    let before = source.frame.cas.counts();
    let fault = source
        .frame
        .cas
        .check_proof(&draft.context.clone(), &[draft], &mut ledger()?)
        .err()
        .ok_or("unproved cover admitted")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::DecisionIntegerRange)
    );
    assert_eq!(source.frame.cas.counts(), before);
    Ok(())
}

/// Kind, composition Work, witness bytes, added graph bytes and fresh inspection Work.
const COSTS: [(u32, u64, usize, u64, u64); 6] = [
    (0, 123_728, 2_332, 2_251, 453_616), // Identity basis, unrestricted Z².
    (1, 195_228, 3_664, 2_487, 746_989), // Identity basis, signed Integer box.
    (2, 154_294, 3_005, 2_135, 553_490), // Identity basis, parameter/count intersection.
    (3, 150_179, 2_910, 2_251, 691_884), // Translated determinant-one basis.
    (4, 157_503, 3_110, 2_251, 717_998), // Translated negative-determinant skew basis.
    (5, 182_930, 3_577, 2_266, 734_367), // Exact 2Pi phase, Pi offset, parameter range.
];

/// Measured finite composition and fresh replay costs name every fixture and preserve all old pins.
/// ´test:integration:lattice-cover-costs´
#[test]
fn lattice_cover_costs() -> TestResult {
    let mut costs = Vec::new();
    for (kind, ..) in COSTS {
        let mut source = product(kind)?;
        let mut budget = ledger()?;
        let before = source.frame.cas.counts().payload_bytes;
        let proof = prove(&mut source, &mut budget)?;
        let graph = source.frame.cas.counts().payload_bytes - before;
        let bytes = source.frame.cas.export_witness(&proof, &mut ledger()?)?;
        let mut receiver = Cas::new(Limits::default())?;
        let mut replay = ledger()?;
        receiver.inspect(&bytes, &mut replay)?;
        costs.push((
            kind,
            budget.usage().work,
            bytes.len(),
            graph,
            replay.usage().work,
        ));
    }
    assert_eq!(costs, COSTS);
    Ok(())
}

fn bounded(kind: BudgetKind, amount: u64) -> TestResult<Budget> {
    let mut limits = Limits::default();
    match kind {
        BudgetKind::Work => limits.work = amount,
        BudgetKind::WitnessBytes => limits.witness_bytes = amount,
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = amount,
        _ => return Err("unsupported cover budget fixture".into()),
    }
    Ok(Budget::new(limits)?)
}

fn attempt(
    kind: u32,
    owner: BudgetKind,
    amount: u64,
    prior: u64,
    retained: u64,
) -> TestResult<bool> {
    let mut source = product(kind)?;
    let index = source.frame.cas.admit_lattice_index_domain(
        &source.basis,
        &source.question,
        &source.covers,
        &mut ledger()?,
    )?;
    let before = source.frame.cas.counts();
    let snapshot = source.frame.cas.canonical_snapshot(&mut ledger()?)?;
    let mut budget = bounded(owner, amount)?;
    let limits = budget.limits().clone();
    budget.work(prior, nrz_cas::Location::new("earlier cover work"))?;
    let result = budget.with_retained_payload(
        retained,
        nrz_cas::Location::new("retained cover caller data"),
        |budget| {
            source
                .frame
                .cas
                .prove_lattice_cover(&source.basis, &index, budget)
        },
    );
    assert!(budget.usage().work >= prior);
    assert_eq!(budget.limits(), &limits);
    match result {
        Ok(_) => Ok(true),
        Err(fault) => {
            assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == owner));
            assert_eq!(source.frame.cas.counts(), before);
            assert_eq!(
                source.frame.cas.canonical_snapshot(&mut ledger()?)?,
                snapshot
            );
            Ok(false)
        }
    }
}

fn replay_attempt(bytes: &[u8], amount: u64, prior: u64, succeeds: bool) -> TestResult {
    let mut cas = Cas::new(Limits::default())?;
    let before = cas.counts();
    let snapshot = cas.canonical_snapshot(&mut ledger()?)?;
    let mut budget = bounded(BudgetKind::Work, amount)?;
    let limits = budget.limits().clone();
    budget.work(prior, nrz_cas::Location::new("earlier cover replay work"))?;
    let result = cas.inspect(bytes, &mut budget);
    assert_eq!(result.is_ok(), succeeds);
    assert!(budget.usage().work >= prior);
    assert_eq!(budget.limits(), &limits);
    if !succeeds {
        let fault = result.err().ok_or("missing replay refusal")?;
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            }
        ));
        assert_eq!(cas.counts(), before);
        assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, snapshot);
    }
    Ok(())
}

/// Every measured composition and fresh replay has literal N/N−1 twins and cumulative prior usage.
/// ´test:integration:lattice-cover-work´
#[test]
fn lattice_cover_work() -> TestResult {
    for (kind, needed, _, _, replay) in COSTS {
        assert!(attempt(kind, BudgetKind::Work, needed, 0, 0)?);
        assert!(!attempt(kind, BudgetKind::Work, needed - 1, 0, 0)?);
        let mut source = product(kind)?;
        let proof = prove(&mut source, &mut ledger()?)?;
        let bytes = source.frame.cas.export_witness(&proof, &mut ledger()?)?;
        replay_attempt(&bytes, replay, 0, true)?;
        replay_attempt(&bytes, replay - 1, 0, false)?;
        if kind == 2 {
            assert!(!attempt(kind, BudgetKind::Work, needed, 37, 0)?);
            assert!(attempt(kind, BudgetKind::Work, needed + 37, 37, 0)?);
            replay_attempt(&bytes, replay, 37, false)?;
            replay_attempt(&bytes, replay + 37, 37, true)?;
        }
    }
    Ok(())
}

/// Literal witness and simultaneous staged-byte boundaries retain caller payload and atomic rollback.
/// ´test:integration:lattice-cover-capacities´
#[test]
fn lattice_cover_capacities() -> TestResult {
    for (kind, needed) in [
        (BudgetKind::WitnessBytes, 3_005),
        (BudgetKind::StagedPayloadBytes, 91_770),
    ] {
        assert!(attempt(2, kind, needed, 0, 0)?);
        assert!(!attempt(2, kind, needed - 1, 0, 0)?);
    }
    assert!(attempt(2, BudgetKind::StagedPayloadBytes, 92_027, 37, 257)?);
    assert!(!attempt(
        2,
        BudgetKind::StagedPayloadBytes,
        92_026,
        37,
        257
    )?);
    Ok(())
}

fn drafts(source: &Product, conclusion: Statement) -> TestResult<Vec<ApplicationDraft>> {
    let mut steps = Vec::new();
    for (witness, rule) in [
        (
            source.basis.inverse().nonsingularity(),
            Primitive::RationalIdentity,
        ),
        (source.basis.inverse().witness(), Primitive::Jets),
        (source.basis.left_inverse(), Primitive::Jets),
    ]
    .into_iter()
    .chain(
        source
            .covers
            .iter()
            .map(|proof| (proof, Primitive::RegionDecide)),
    ) {
        steps.push(ApplicationDraft {
            rule,
            context: witness.judgment().context.clone(),
            conclusion: witness.judgment().statement.clone(),
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::None,
        });
    }
    steps.push(ApplicationDraft {
        rule: Primitive::RegionDecide,
        context: source.question.basis.context.clone(),
        conclusion,
        premises: (0..steps.len())
            .map(u32::try_from)
            .collect::<Result<Vec<_>, _>>()?,
        substitutions: Vec::new(),
        certificate: Certificate::None,
    });
    Ok(steps)
}

/// The public schema independently rejects changed affine roles, premise orders and range products.
/// ´test:integration:lattice-cover-forgeries´
#[test]
fn lattice_cover_forgeries() -> TestResult {
    let mut source = product(3)?;
    let proof = prove(&mut source, &mut ledger()?)?;
    let steps = drafts(&source, proof.judgment().statement.clone())?;
    source
        .frame
        .cas
        .check_proof(&source.question.basis.context, &steps, &mut ledger()?)?;
    // Changed premise inventories leave an earlier step unreachable, so generic
    // complete-root preflight catches them before the lattice schema runs.
    for (change, expected) in [
        (0, InvalidCode::DecisionIntegerRange),
        (1, InvalidCode::DecisionIntegerRange),
        (2, InvalidCode::DecisionIntegerRange),
        (3, InvalidCode::Rule),
        (4, InvalidCode::Rule),
        (5, InvalidCode::DecisionIntegerRange),
        (6, InvalidCode::DecisionIntegerRange),
        (7, InvalidCode::Rule),
        (8, InvalidCode::Rule),
    ] {
        let mut changed = steps.clone();
        let last = changed.last_mut().ok_or("missing cover")?;
        let Statement::LatticeCover(value) = &mut last.conclusion else {
            return Err("wrong statement".into());
        };
        match change {
            0 => value.inverse.clone_from(&value.forward),
            1 => value.forward.clone_from(&value.inverse),
            2 => last.premises.swap(1, 2),
            3 => last.premises[4] = 3,
            4 => {
                last.premises.pop();
            }
            5 => value
                .index
                .coordinate_domain
                .clone_from(&source.frame.ambient),
            6 => value
                .determinant
                .clone_from(&source.question.families[0].coordinate),
            7 => last.premises[0] = 1,
            _ => {
                value.index.families[1] = value.index.families[0].clone();
                last.premises[4] = 3;
            }
        }
        let before = source.frame.cas.counts();
        let snapshot = source.frame.cas.canonical_snapshot(&mut ledger()?)?;
        let fault = source
            .frame
            .cas
            .check_proof(&source.question.basis.context, &changed, &mut ledger()?)
            .err()
            .ok_or("forged cover admitted")?;
        assert_eq!(fault.kind, FaultKind::Invalid(expected), "forgery {change}");
        assert_eq!(source.frame.cas.counts(), before);
        assert_eq!(
            source.frame.cas.canonical_snapshot(&mut ledger()?)?,
            snapshot
        );
    }
    Ok(())
}

/// Independently valid scalar and inverse proofs do not admit a coordinate-dependent basis.
/// ´test:integration:lattice-cover-coupled-basis´
#[test]
fn lattice_cover_coupled_basis() -> TestResult {
    let mut source = fixture(1, 2, false, true)?;
    let value = nrz_cas::lattice::LatticeCover {
        index: source.question.clone(),
        forward: source.basis.forward().expression().clone(),
        inverse: source.basis.inverse().map().expression().clone(),
        determinant: source.basis.inverse().determinant().clone(),
    };
    let steps = drafts(&source, Statement::LatticeCover(Box::new(value)))?;
    let before = source.frame.cas.counts();
    let fault = source
        .frame
        .cas
        .check_proof(&source.question.basis.context, &steps, &mut ledger()?)
        .err()
        .ok_or("coupled cover admitted")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::DecisionIntegerRange)
    );
    assert_eq!(source.frame.cas.counts(), before);
    Ok(())
}
