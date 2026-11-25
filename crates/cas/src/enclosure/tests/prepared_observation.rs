// SPDX-License-Identifier: AGPL-3.0-only
//! Shared subscriber controls for prepared schedule boundary receipts.

use super::*;
use crate::enclosure::prepared_expm1::{Counts, Outcome, Policy, Request};
use crate::testing::work_trace::{observed, without_observation};
use crate::{FaultKind, InvalidCode};

/// Create the original claim outside the observed numerical preparation boundary.
fn fixture() -> TestResult<(Fixture, Request)> {
    let mut f = Fixture::with_names_and_limits(
        &["x"],
        Limits {
            work: 2_000_000_000,
            integer_exponent: 4096,
            arithmetic_steps: 50_000_000,
            ..Limits::default()
        },
    )?;
    let x = f.rational(-1, 1)?;
    let e = f.term(Operator::Exponential, &[x])?;
    let one = f.rational(1, 1)?;
    let target = f.term(Operator::Sub, &[e, one])?;
    let r = Request {
        original: f.request(target),
        bits: 40,
        environment: 1,
    };
    Ok((f, r))
}

/// Observe enabled and disabled executions without changing their mathematical ledger.
fn run(
    f: &mut Fixture,
    r: &Request,
    work: u64,
) -> TestResult<(Result<Outcome, Fault>, Counts, u64)> {
    let mut budget = Budget::new(Limits {
        work,
        integer_exponent: 4096,
        arithmetic_steps: 50_000_000,
        ..Limits::default()
    })?;
    budget.work(7, crate::Location::new("caller prefix"))?;
    let mut counts = Counts::default();
    let result = f
        .cas
        .prepare_expm1(r, Policy::default(), &mut budget, &mut counts);
    Ok((result, counts, budget.usage().work))
}

/// Shared guards protect default-parallel tests; all completed and refused Work reconciles.
/// # Panics
/// Panics only if observation changes a checked result, counters or Work attribution.
/// ´claim:cas:prepared-expm1-observation´
/// ´test:unit:prepared-expm1-observation´
#[test]
fn prepared_expm1_observation() -> TestResult {
    let (mut plain, r) = fixture()?;
    let (disabled, counts, work) = without_observation(|| run(&mut plain, &r, 2_000_000_000))?;
    let Outcome::Ready(disabled) = disabled? else {
        return Err("target".into());
    };
    let (mut traced, r) = fixture()?;
    let (result, report) = observed(4096, || run(&mut traced, &r, 2_000_000_000));
    let (enabled, enabled_counts, enabled_work) = result?;
    let Outcome::Ready(enabled) = enabled? else {
        return Err("target".into());
    };
    assert_eq!(enabled.statement_key(), disabled.statement_key());
    assert_eq!(enabled_counts, counts);
    assert_eq!(enabled_work, work);
    assert!(report.complete);
    for operation in [
        "nrz.prepare",
        "nrz.lookup",
        "nrz.proposal",
        "nrz.proof",
        "nrz.publish",
    ] {
        // A parent can have zero exclusive Work when all charges belong to children.
        assert!(
            report.receipts.iter().any(
                |((_, _, _, location), receipt)| location == operation && receipt.intervals > 0
            )
        );
    }
    let (warm, warm_report) = observed(4096, || run(&mut traced, &r, 2_000_000_000));
    let (warm, counts, _) = warm?;
    warm?;
    assert_eq!(
        (
            counts.proposals,
            counts.coefficients,
            counts.checks,
            counts.publications
        ),
        (0, 0, 0, 0)
    );
    assert!(warm_report.complete);
    let (mut refused, r) = fixture()?;
    let (result, refusal_report) = observed(4096, || run(&mut refused, &r, work - 1));
    let (result, _, _) = result?;
    assert!(result.is_err());
    assert!(refusal_report.complete);
    assert!(
        refusal_report
            .receipts
            .values()
            .any(|receipt| receipt.refused > 0)
    );
    Ok(())
}
/// The canonical index cannot replace the graph's actual checked-evidence edge.
/// # Panics
/// Panics only if missing graph evidence still authorizes a retained lookup.
/// ´claim:cas:prepared-expm1-citation-control´
/// ´test:unit:prepared-expm1-citation-control´
#[test]
fn prepared_expm1_citation_control() -> TestResult {
    let (mut f, r) = fixture()?;
    run(&mut f, &r, 2_000_000_000)?.0?;
    let edge = f
        .cas
        .state
        .graph
        .edge_indices()
        .find(|edge| {
            matches!(
                f.cas.state.graph.edge_weight(*edge),
                Some(crate::graph::Edge::CheckedEvidence)
            )
        })
        .ok_or("evidence edge")?;
    f.cas.state.graph.remove_edge(edge);
    let (result, _, _) = run(&mut f, &r, 2_000_000_000)?;
    assert_eq!(
        result.expect_err("missing citation").kind,
        crate::FaultKind::Invalid(crate::InvalidCode::Reference)
    );
    Ok(())
}

/// Real publication shares its inventory allocation with the index and active rollback journal.
/// # Panics
/// Panics if a production owner copies the key or commit/rollback retains the wrong owners.
/// ´claim:cas:prepared-expm1-production-key´
/// ´test:unit:prepared-expm1-production-key´
#[test]
fn prepared_expm1_production_key() -> TestResult {
    for rollback in [true, false] {
        let name = "x";
        let limits = Limits {
            work: 2_000_000_000,
            integer_exponent: 4096,
            arithmetic_steps: 50_000_000,
            name_bytes: 131_072,
            source_bytes: 4_194_304,
            output_bytes: 4_194_304,
            witness_bytes: 4_194_304,
            ..Limits::default()
        };
        let mut f = Fixture::with_names_and_limits(&[name], limits.clone())?;
        let zero = f.rational(0, 1)?;
        let e = f.term(Operator::Exponential, &[zero])?;
        let one = f.rational(1, 1)?;
        let target = f.term(Operator::Sub, &[e, one])?;
        let request = Request {
            original: f.request(target),
            bits: 40,
            environment: 1,
        };
        let mut budget = Budget::new(limits)?;
        let proof = f.cas.propose_expm1(
            &request,
            Policy::default(),
            1,
            &mut budget,
            &mut Counts::default(),
        )?;
        let before = f.cas.counts();
        let mut stale = None;
        let mut owner = None;
        let result = f.cas.transaction(&mut budget, |cas, budget| {
            cas.check_expm1(
                &request,
                Policy::default(),
                &proof,
                budget,
                &mut Counts::default(),
            )?;
            let (weak, index, journal) = cas.expm1_key_owners()?;
            assert!(index);
            assert!(journal);
            assert_eq!(weak.strong_count(), 3);
            owner = Some(weak);
            stale = Some(schedule(cas)?);
            if rollback {
                Err(Fault::invalid(
                    InvalidCode::Expm1Claim,
                    crate::Location::new("test rollback"),
                ))
            } else {
                Ok(())
            }
        });
        let weak = owner.ok_or("production owner")?;
        if rollback {
            assert_eq!(
                result.expect_err("rollback").kind,
                FaultKind::Invalid(InvalidCode::Expm1Claim)
            );
            assert_eq!(weak.strong_count(), 0);
            assert!(weak.upgrade().is_none());
            assert_eq!(f.cas.counts(), before);
            check_identity(
                &mut f,
                &request,
                &proof,
                &mut budget,
                &stale.ok_or("schedule")?,
            )?;
        } else {
            result?;
            assert_eq!(weak.strong_count(), 2);
            drop(f);
            assert_eq!(weak.strong_count(), 0);
        }
    }
    Ok(())
}

/// This zero-input fixture publishes exactly one anonymous schedule.
fn schedule(cas: &Cas) -> Result<Handle, Fault> {
    let mut ids = cas
        .state
        .indices
        .keys()
        .filter(|id| id.kind == crate::graph::VertexKind::AnonymousLemma);
    let id = ids.next().ok_or_else(|| {
        Fault::invalid(
            InvalidCode::Reference,
            crate::Location::new("schedule identity"),
        )
    })?;
    assert!(ids.next().is_none());
    Ok(cas.handle(*id))
}

/// Rechecking the same prepared proof cannot revive its discarded graph handle.
fn check_identity(
    f: &mut Fixture,
    request: &Request,
    proof: &crate::enclosure::prepared_expm1::Proof,
    budget: &mut Budget,
    stale: &Handle,
) -> TestResult {
    assert_eq!(
        f.cas
            .checked(stale, budget)
            .err()
            .ok_or("discarded schedule survived")?
            .kind,
        FaultKind::Invalid(InvalidCode::Reference)
    );
    f.cas.check_expm1(
        request,
        Policy::default(),
        proof,
        budget,
        &mut Counts::default(),
    )?;
    let replacement = schedule(&f.cas)?;
    assert_ne!(&replacement, stale);
    assert!(replacement.logical_id().serial > stale.logical_id().serial);
    assert_eq!(
        f.cas
            .checked(stale, budget)
            .err()
            .ok_or("schedule reincarnated")?
            .kind,
        FaultKind::Invalid(InvalidCode::Reference)
    );
    let counts = f.cas.counts();
    f.cas.check_expm1(
        request,
        Policy::default(),
        proof,
        budget,
        &mut Counts::default(),
    )?;
    assert_eq!(schedule(&f.cas)?, replacement);
    assert_eq!(f.cas.counts(), counts);
    f.cas.checked(&f.context, budget)?;
    Ok(())
}
