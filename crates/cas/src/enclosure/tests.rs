// SPDX-License-Identifier: AGPL-3.0-only
//! Full original-scope controls exercise producer, replay, retention and stopping separately.

use super::*;

mod clamps;
mod derivatives;
mod exponentials;
mod powers;
mod sigmoids;
mod tangents;
use crate::budget::{Budget, Limits};
use crate::domain_model::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
use crate::graph::{Draft, ExpressionDraft};
use crate::model::{AtomicSort, Declaration, Environment, Field, Operator, Sort};
use crate::{Cas, Fault, Handle, Rational, Reference};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// The previous dependency-widened quotient is retained only as a comparison oracle.
fn old_saturating_quotient(
    certificate: &EnclosureCertificate,
    negative: bool,
    tanh: bool,
) -> TestResult<Interval> {
    let mut budget = ledger()?;
    let proof = &certificate.exponentials[0].lower;
    let source = certificate.records[0].lower();
    let argument = if source.is_negative() {
        source.add(&Rational::zero(), &mut budget)?
    } else {
        source.negated(&mut budget)?
    };
    let argument = if tanh {
        argument.mul(&number(2, 1)?, &mut budget)?
    } else {
        argument
    };
    let e = exp::check_endpoint(&argument, proof, certificate.grid, &mut budget, &mut |_| {
        false
    })?;
    let one = Interval::singleton(&Rational::one(), &mut budget)?;
    let denominator = one.add(&e, certificate.grid, &mut budget)?;
    let numerator = if tanh {
        one.sub(&e, certificate.grid, &mut budget)?
    } else if negative {
        e
    } else {
        one
    };
    let result = numerator.mul(
        &denominator.reciprocal(certificate.grid, &mut budget)?,
        certificate.grid,
        &mut budget,
    )?;
    let result = if tanh && negative {
        Interval::new(
            result.upper().negated(&mut budget)?,
            result.lower().negated(&mut budget)?,
            &mut budget,
        )?
    } else {
        result
    };
    Ok(result.intersection(&interval(if tanh { -1 } else { 0 }, 1, 1)?, &mut budget)?)
}

fn ledger() -> Result<Budget, Fault> {
    Budget::new(Limits::default())
}
fn policy() -> RefinementPolicy {
    RefinementPolicy {
        version: 1,
        precision: 4,
        grid_ceiling: 64,
        rounds: 2,
        reduction_ceiling: 16,
        term_ceiling: 64,
        checking_work: 50_000,
    }
}
fn number(n: i64, d: u64) -> Result<Rational, Fault> {
    Rational::from_ratio(n, d, &mut ledger()?)
}
fn interval(a: i64, b: i64, d: u64) -> Result<Interval, Fault> {
    Interval::new(number(a, d)?, number(b, d)?, &mut ledger()?)
}

struct Fixture {
    cas: Cas,
    header: Handle,
    context: Handle,
}
impl Fixture {
    fn new() -> TestResult<Self> {
        Self::with_names(&["x"])
    }
    fn with_names(names: &[&str]) -> TestResult<Self> {
        let mut environment = Environment::default();
        for name in names {
            environment.declarations.push(Declaration {
                key: (*name).into(),
                display_name: (*name).into(),
                sort: real(),
            });
        }
        let mut cas = Cas::new(Limits::default())?;
        let drafts = [
            Draft::Header(HeaderDraft::Core(environment)),
            Draft::Region(RegionDraft {
                header: Reference::Local(0),
                scope: Vec::new(),
                ambient: AmbientDomain::Assignments,
                expression: RegionExpr::Ambient,
            }),
            Draft::Context(ContextDraft::Root {
                header: Reference::Local(0),
                admissible: Reference::Local(1),
                assumptions: Vec::new(),
            }),
        ];
        let [header, _, context]: [Handle; 3] = cas
            .admit(&drafts, &mut ledger()?)?
            .try_into()
            .map_err(|_| "fixture")?;
        Ok(Self {
            cas,
            header,
            context,
        })
    }
    fn term(&mut self, operator: Operator, operands: &[Handle]) -> TestResult<Handle> {
        let draft = Draft::Expression(ExpressionDraft {
            space: None,
            operator,
            sort: real(),
            header: Reference::Existing(self.header.clone()),
            scope: Vec::new(),
            binding: None,
            operands: operands.iter().cloned().map(Reference::Existing).collect(),
        });
        self.cas
            .admit(&[draft], &mut ledger()?)?
            .into_iter()
            .next()
            .ok_or_else(|| "term".into())
    }
    fn rational(&mut self, n: i64, d: u64) -> TestResult<Handle> {
        self.term(Operator::Rational(number(n, d)?), &[])
    }
    fn request(&self, target: Handle) -> EnclosureRequest {
        EnclosureRequest {
            target,
            context: self.context.clone(),
            coordinates: Vec::new(),
            goal: EnclosureGoal::Width,
        }
    }
}
fn real() -> Sort {
    Sort::Value(AtomicSort::Scalar(Field::Real))
}

fn product_fixture() -> TestResult<(Fixture, EnclosureRequest)> {
    let mut f = Fixture::new()?;
    let a = f.rational(-2, 3)?;
    let b = f.rational(7, 5)?;
    let target = f.term(Operator::Mul, &[a, b])?;
    let request = f.request(target);
    Ok((f, request))
}

/// Every threshold outcome is separate from range precision and width-only success.
/// ´test:unit:enclosure-honest-outcomes´
/// ´claim:cas:enclosure-honest-outcomes´
#[test]
fn enclosure_honest_outcomes() -> TestResult {
    let mut f = Fixture::new()?;
    let x = f.term(Operator::Free("x".into()), &[])?;
    let mut request = f.request(x.clone());
    request.coordinates.push(CoordinateBound {
        coordinate: x,
        interval: interval(-1, 1, 1)?,
    });
    for (goal, outcome) in [
        (
            EnclosureGoal::AtMost(number(2, 1)?),
            EnclosureOutcome::Proved,
        ),
        (
            EnclosureGoal::LessThan(number(-1, 1)?),
            EnclosureOutcome::Refuted,
        ),
        (
            EnclosureGoal::Equal(number(0, 1)?),
            EnclosureOutcome::Undecided(UndecidedReason::Straddle),
        ),
        (
            EnclosureGoal::Width,
            EnclosureOutcome::Undecided(UndecidedReason::WidthLimit),
        ),
    ] {
        request.goal = goal;
        let result = f
            .cas
            .enclose(&request, policy(), &mut ledger()?, &mut |_| false)?;
        assert_eq!(result.outcome, outcome);
        assert!(!result.precision_met);
        assert_eq!(
            result.checked.ok_or("checked")?.interval(),
            &interval(-1, 1, 1)?
        );
    }
    let exact = f.rational(1, 2)?;
    request = f.request(exact);
    request.goal = EnclosureGoal::Equal(number(1, 2)?);
    assert_eq!(
        f.cas
            .enclose(&request, policy(), &mut ledger()?, &mut |_| false)?
            .outcome,
        EnclosureOutcome::Proved
    );
    Ok(())
}

/// A receiving store replays the complete original key and independent endpoint relations.
/// ´test:unit:enclosure-fresh-store-replay´
/// ´claim:cas:enclosure-fresh-store-replay´
#[test]
fn enclosure_fresh_store_replay() -> TestResult {
    let (mut f, request) = product_fixture()?;
    let report = f
        .cas
        .enclose(&request, policy(), &mut ledger()?, &mut |_| false)?;
    assert_eq!(report.outcome, EnclosureOutcome::Bound);
    let checked = report.checked.ok_or("bound")?;
    let bytes = checked.certificate().to_bytes(&mut ledger()?)?;
    let certificate = EnclosureCertificate::from_bytes(&bytes, &mut ledger()?)?;
    let mut receiving = Fixture::new()?;
    receiving.rational(99, 1)?;
    let a = receiving.rational(-2, 3)?;
    let b = receiving.rational(7, 5)?;
    let target = receiving.term(Operator::Mul, &[a, b])?;
    let expected = receiving.request(target);
    let replay = receiving.cas.check_enclosure(
        &expected,
        policy(),
        &certificate,
        &mut ledger()?,
        &mut |_| false,
    )?;
    assert_eq!(replay.outcome, report.outcome);
    assert_eq!(
        replay.checked.ok_or("replay")?.interval(),
        checked.interval()
    );
    assert_eq!(receiving.cas.retained_judgment_count(), 1);
    assert_eq!(
        checked.formal_schema(),
        EnclosureLaw::RationalInclusion.formal_schema()
    );
    assert_eq!(
        checked.premise_kinds().last(),
        Some(&crate::proof::HypothesisKind::Enclosure)
    );
    Ok(())
}

/// Bad versions, joins, records and wire framing refuse without graph publication.
/// ´test:unit:enclosure-certificate-mutations´
/// ´claim:cas:enclosure-certificate-mutations´
#[test]
fn enclosure_certificate_mutations() -> TestResult {
    let (mut f, request) = product_fixture()?;
    let checked = f
        .cas
        .enclose(&request, policy(), &mut ledger()?, &mut |_| false)?
        .checked
        .ok_or("bound")?;
    for mutation in 0..9 {
        let (mut receiving, expected) = product_fixture()?;
        let before = receiving.cas.canonical_snapshot(&mut ledger()?)?;
        let mut certificate = checked.certificate().clone();
        match mutation {
            0 => certificate.version = 2,
            1 => certificate.scope.push(0),
            2 => {
                certificate.records.pop();
            }
            3 => certificate.records.push(interval(0, 1, 1)?),
            4 => certificate.records[0] = interval(0, 0, 1)?,
            5 => certificate.presentation += 1,
            6 => certificate.records.swap(0, 1),
            7 => certificate.grid += 1,
            _ => {
                let last = certificate.records.last_mut().ok_or("record")?;
                *last = interval(0, 0, 1)?;
            }
        }
        assert!(
            receiving
                .cas
                .check_enclosure(
                    &expected,
                    policy(),
                    &certificate,
                    &mut ledger()?,
                    &mut |_| false
                )
                .is_err()
        );
        assert_eq!(receiving.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    let bytes = checked.certificate().to_bytes(&mut ledger()?)?;
    for end in [0, 7, bytes.len() - 1] {
        assert!(EnclosureCertificate::from_bytes(&bytes[..end], &mut ledger()?).is_err());
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert!(EnclosureCertificate::from_bytes(&trailing, &mut ledger()?).is_err());
    Ok(())
}

/// Unsupported syntax and impossible schedules refuse before Work, including hidden operands.
/// ´test:unit:enclosure-zero-work-preflight´
/// ´claim:cas:enclosure-zero-work-preflight´
#[test]
fn enclosure_zero_work_preflight() -> TestResult {
    let (mut f, request) = product_fixture()?;
    for changed in [
        RefinementPolicy {
            version: 2,
            ..policy()
        },
        RefinementPolicy {
            precision: u32::MAX,
            ..policy()
        },
        RefinementPolicy {
            rounds: 0,
            ..policy()
        },
        RefinementPolicy {
            grid_ceiling: 11,
            ..policy()
        },
    ] {
        let mut budget = ledger()?;
        assert_eq!(
            f.cas
                .enclose(&request, changed, &mut budget, &mut |_| false)?
                .outcome,
            EnclosureOutcome::Unsupported
        );
        assert_eq!(budget.usage().work, 0);
    }
    let pi = f.term(Operator::Constant(crate::model::Constant::Pi), &[])?;
    let zero = f.rational(0, 1)?;
    let target = f.term(Operator::Mul, &[zero, pi])?;
    let mut budget = ledger()?;
    assert_eq!(
        f.cas
            .enclose(&f.request(target), policy(), &mut budget, &mut |_| false)?
            .outcome,
        EnclosureOutcome::Unsupported
    );
    assert_eq!(budget.usage().work, 0);
    Ok(())
}

/// Hidden undefined operands, foreign contexts and duplicate coordinates retain their refusals.
/// ´test:unit:enclosure-original-domain-refusals´
/// ´claim:cas:enclosure-original-domain-refusals´
#[test]
fn enclosure_original_domain_refusals() -> TestResult {
    let mut f = Fixture::new()?;
    let zero = f.rational(0, 1)?;
    let one = f.rational(1, 1)?;
    let division = f.term(Operator::Div, &[one, zero.clone()])?;
    let hidden = f.term(Operator::Mul, &[zero, division])?;
    let before = f.cas.canonical_snapshot(&mut ledger()?)?;
    assert!(
        f.cas
            .enclose(&f.request(hidden), policy(), &mut ledger()?, &mut |_| false)
            .is_err()
    );
    assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    let x = f.term(Operator::Free("x".into()), &[])?;
    let mut request = f.request(x.clone());
    let coordinate = CoordinateBound {
        coordinate: x,
        interval: interval(-1, 1, 1)?,
    };
    request.coordinates = vec![coordinate.clone(), coordinate];
    assert!(
        f.cas
            .enclose(&request, policy(), &mut ledger()?, &mut |_| false)
            .is_err()
    );
    request.coordinates.pop();
    request.context = Fixture::new()?.context;
    assert!(
        f.cas
            .enclose(&request, policy(), &mut ledger()?, &mut |_| false)
            .is_err()
    );
    Ok(())
}

/// Each charged boundary interrupts atomically and leaves spent Work visible.
/// ´test:unit:enclosure-interruption-rollback´
/// ´claim:cas:enclosure-interruption-rollback´
#[test]
fn enclosure_interruption_rollback() -> TestResult {
    for stop in 1..=8 {
        let (mut f, request) = product_fixture()?;
        let before = f.cas.canonical_snapshot(&mut ledger()?)?;
        let mut calls = 0;
        let mut budget = ledger()?;
        let report = f.cas.enclose(&request, policy(), &mut budget, &mut |_| {
            calls += 1;
            calls == stop
        })?;
        assert_eq!(report.outcome, EnclosureOutcome::Interrupted);
        assert!(report.checked.is_none());
        assert!(report.work > 0);
        assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    Ok(())
}

/// Warm requests reuse checked graph nodes and can finish within an allowance cold work exhausts.
/// ´test:unit:enclosure-cold-warm-work´
/// ´claim:cas:enclosure-cold-warm-work´
#[test]
fn enclosure_cold_warm_work() -> TestResult {
    let (mut f, request) = product_fixture()?;
    let p = RefinementPolicy {
        checking_work: 0,
        ..policy()
    };
    let cold = f.cas.enclose(&request, p, &mut ledger()?, &mut |_| false)?;
    let snapshot = f.cas.canonical_snapshot(&mut ledger()?)?;
    let warm = f.cas.enclose(&request, p, &mut ledger()?, &mut |_| false)?;
    assert_eq!(cold.outcome, EnclosureOutcome::Bound);
    assert_eq!(warm.outcome, cold.outcome);
    assert!(warm.work < cold.work);
    let mut allowance = Budget::new(Limits {
        work: warm.work,
        ..Limits::default()
    })?;
    assert_eq!(
        f.cas
            .enclose(&request, p, &mut allowance, &mut |_| false)?
            .outcome,
        EnclosureOutcome::Bound
    );
    let (mut other, expected) = product_fixture()?;
    let mut allowance = Budget::new(Limits {
        work: warm.work,
        ..Limits::default()
    })?;
    assert_eq!(
        other
            .cas
            .enclose(&expected, p, &mut allowance, &mut |_| false)?
            .outcome,
        EnclosureOutcome::Exhausted
    );
    assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, snapshot);
    assert_eq!((cold.work, warm.work), (78_336, 20_259));
    let mut short = Budget::new(Limits {
        work: 20_258,
        ..Limits::default()
    })?;
    assert_eq!(
        f.cas
            .enclose(&request, p, &mut short, &mut |_| false)?
            .outcome,
        EnclosureOutcome::Exhausted
    );
    let (mut fresh, expected) = product_fixture()?;
    let mut exact = Budget::new(Limits {
        work: 78_336,
        ..Limits::default()
    })?;
    assert_eq!(
        fresh
            .cas
            .enclose(&expected, p, &mut exact, &mut |_| false)?
            .outcome,
        EnclosureOutcome::Bound
    );
    let (mut fresh, expected) = product_fixture()?;
    let mut short = Budget::new(Limits {
        work: 78_335,
        ..Limits::default()
    })?;
    assert_eq!(
        fresh
            .cas
            .enclose(&expected, p, &mut short, &mut |_| false)?
            .outcome,
        EnclosureOutcome::Exhausted
    );

    Ok(())
}

/// Enclosing rollback preserves earlier receipts and invalidates reused source serials.
/// ´test:unit:enclosure-enclosing-rollback´
/// ´claim:cas:enclosure-enclosing-rollback´
#[test]
fn enclosure_enclosing_rollback() -> TestResult {
    let (mut f, request) = product_fixture()?;
    f.cas
        .enclose(&request, policy(), &mut ledger()?, &mut |_| false)?;
    let snapshot = f.cas.canonical_snapshot(&mut ledger()?)?;
    let count = f.cas.retained_judgment_count();
    let mut altered = request.clone();
    altered.goal = EnclosureGoal::AtMost(number(5, 1)?);
    let mut budget = ledger()?;
    let result = f.cas.transaction(&mut budget, |cas, budget| {
        let report = cas.enclose(&altered, policy(), budget, &mut |_| false)?;
        assert_eq!(report.outcome, EnclosureOutcome::Proved);
        Err::<(), _>(Fault::invalid(crate::InvalidCode::Reference, AT))
    });
    assert!(result.is_err());
    assert!(budget.usage().work > 0);
    assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, snapshot);
    assert_eq!(f.cas.retained_judgment_count(), count);
    let refreshed = f
        .cas
        .enclose(&request, policy(), &mut ledger()?, &mut |_| false)?;
    assert_eq!(refreshed.outcome, EnclosureOutcome::Bound);
    assert!(refreshed.work > 20_259);
    assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, snapshot);
    Ok(())
}

/// A missing graph evidence edge cannot be replaced by matching key bytes.
/// ´test:unit:enclosure-evidence-edge-refusal´
/// ´claim:cas:enclosure-evidence-edge-refusal´
#[test]
fn enclosure_evidence_edge_refusal() -> TestResult {
    use petgraph::visit::EdgeRef;
    let (mut f, request) = product_fixture()?;
    f.cas
        .enclose(&request, policy(), &mut ledger()?, &mut |_| false)?;
    let edge = f
        .cas
        .state
        .graph
        .edge_references()
        .find(|e| matches!(e.weight(), crate::graph::Edge::CheckedEvidence))
        .ok_or("edge")?
        .id();
    f.cas.state.graph.remove_edge(edge);
    assert!(
        f.cas
            .enclose(&request, policy(), &mut ledger()?, &mut |_| false)
            .is_err()
    );
    Ok(())
}

/// Stopping after a completed wide round retains its bound and never upgrades its verdict.
/// ´test:unit:enclosure-completed-round-survives-stopping´
/// ´claim:cas:enclosure-completed-round-survives-stopping´
#[test]
fn enclosure_completed_round_survives_stopping() -> TestResult {
    fn case() -> TestResult<(Fixture, EnclosureRequest)> {
        let mut f = Fixture::new()?;
        let x = f.term(Operator::Free("x".into()), &[])?;
        let mut request = f.request(x.clone());
        request.coordinates.push(CoordinateBound {
            coordinate: x,
            interval: interval(-1, 1, 1)?,
        });
        Ok((f, request))
    }
    let (mut f, request) = case()?;
    let p = RefinementPolicy {
        rounds: 1,
        checking_work: 0,
        ..policy()
    };
    let first = f.cas.enclose(&request, p, &mut ledger()?, &mut |_| false)?;
    let (mut other, expected) = case()?;
    let mut limited = Budget::new(Limits {
        work: first.work + 1,
        ..Limits::default()
    })?;
    let result = other.cas.enclose(
        &expected,
        RefinementPolicy { rounds: 2, ..p },
        &mut limited,
        &mut |_| false,
    )?;
    assert_eq!(
        result.outcome,
        EnclosureOutcome::Undecided(UndecidedReason::WorkExhausted)
    );
    assert_eq!(result.completed_rounds, 1);
    assert_eq!(
        result.checked.ok_or("prior bound")?.interval(),
        &interval(-1, 1, 1)?
    );
    assert_eq!(other.cas.retained_judgment_count(), 1);
    let (mut other, expected) = case()?;
    let mut calls = 0;
    let result = other.cas.enclose(
        &expected,
        RefinementPolicy { rounds: 2, ..p },
        &mut ledger()?,
        &mut |_| {
            calls += 1;
            calls == 5
        },
    )?;
    assert_eq!(result.outcome, EnclosureOutcome::Interrupted);
    assert_eq!(result.completed_rounds, 1);
    assert!(result.checked.is_some());
    Ok(())
}

/// Foreign stores cannot inherit retained authority from matching copied graph bytes.
/// ´test:unit:enclosure-owner-revalidation´
/// ´claim:cas:enclosure-owner-revalidation´
#[test]
fn enclosure_owner_revalidation() -> TestResult {
    let (mut f, request) = product_fixture()?;
    let p = RefinementPolicy {
        checking_work: 0,
        ..policy()
    };
    f.cas.enclose(&request, p, &mut ledger()?, &mut |_| false)?;
    let mut copy = f.cas.full_copy_for_test();
    let checked = copy.enclose(&request, p, &mut ledger()?, &mut |_| false)?;
    assert_eq!(checked.outcome, EnclosureOutcome::Bound);
    assert!(checked.work > 20_259);
    assert_eq!(copy.retained_judgment_count(), 1);
    Ok(())
}

/// Generation, independent replay and publication each have their own measured Work boundary.
/// ´test:unit:enclosure-phase-work-pins´
/// ´claim:cas:enclosure-phase-work-pins´
#[test]
fn enclosure_phase_work_pins() -> TestResult {
    let (f, request) = product_fixture()?;
    let scope = scope::key(&f.cas, &request, &mut ledger()?)?;
    let mut generation = ledger()?;
    let certificate = producer::generate(
        &f.cas,
        &request,
        scope,
        (12, 6),
        &mut generation,
        &mut |_| false,
    )?;
    let mut replay = ledger()?;
    let checked = checker::check(&f.cas, &request, &certificate, &mut replay, &mut |_| false)?;
    let (mut receiving, expected) = product_fixture()?;
    let local = checker::check(
        &receiving.cas,
        &expected,
        &certificate,
        &mut ledger()?,
        &mut |_| false,
    )?;
    let prefix = service::prefix(&certificate.scope, 12, 6, &mut ledger()?)?;
    let key = service::retention_key(&prefix, &certificate, &mut ledger()?)?;
    let mut publication = ledger()?;
    receiving
        .cas
        .retain_enclosure(key, local, &mut publication)?;
    assert_eq!(
        (
            generation.usage().work,
            replay.usage().work,
            publication.usage().work
        ),
        (16_169, 37_516, 2_961)
    );
    for (limit, passes) in [(16_169, true), (16_168, false)] {
        let mut bounded = Budget::new(Limits {
            work: limit + 7,
            ..Limits::default()
        })?;
        bounded.work(7, AT)?;
        let source = scope::key(&f.cas, &request, &mut ledger()?)?;
        let outcome =
            producer::generate(&f.cas, &request, source, (12, 6), &mut bounded, &mut |_| {
                false
            });
        assert_eq!(outcome.is_ok(), passes);
    }
    for (limit, passes) in [(37_516, true), (37_515, false)] {
        let mut bounded = Budget::new(Limits {
            work: limit + 7,
            ..Limits::default()
        })?;
        bounded.work(7, AT)?;
        let outcome = checker::check(&f.cas, &request, &certificate, &mut bounded, &mut |_| false);
        assert_eq!(outcome.is_ok(), passes);
    }
    for (limit, passes) in [(2_961, true), (2_960, false)] {
        let (mut target, expected) = product_fixture()?;
        let value = checker::check(
            &target.cas,
            &expected,
            &certificate,
            &mut ledger()?,
            &mut |_| false,
        )?;
        let prefix = service::prefix(&certificate.scope, 12, 6, &mut ledger()?)?;
        let key = service::retention_key(&prefix, &certificate, &mut ledger()?)?;
        let before = target.cas.canonical_snapshot(&mut ledger()?)?;
        let mut bounded = Budget::new(Limits {
            work: limit + 7,
            ..Limits::default()
        })?;
        bounded.work(7, AT)?;
        assert_eq!(
            target
                .cas
                .retain_enclosure(key, value, &mut bounded)
                .is_ok(),
            passes
        );
        if !passes {
            assert_eq!(target.cas.canonical_snapshot(&mut ledger()?)?, before);
        }
    }
    assert_eq!(
        checked.interval(),
        &certificate
            .records
            .last()
            .ok_or("record")?
            .coarsen(6, &mut ledger()?)?
    );
    Ok(())
}

/// The same portable bytes cannot answer a different target, context, goal or box.
/// ´test:unit:enclosure-request-joins´
/// ´claim:cas:enclosure-request-joins´
#[test]
fn enclosure_request_joins() -> TestResult {
    let mut f = Fixture::new()?;
    let x = f.term(Operator::Free("x".into()), &[])?;
    let mut request = f.request(x.clone());
    request.coordinates.push(CoordinateBound {
        coordinate: x,
        interval: interval(-1, 1, 1)?,
    });
    let certificate = f
        .cas
        .enclose(&request, policy(), &mut ledger()?, &mut |_| false)?
        .checked
        .ok_or("bound")?
        .certificate()
        .clone();
    for kind in 0..3 {
        let mut wrong = request.clone();
        match kind {
            0 => wrong.goal = EnclosureGoal::AtMost(number(2, 1)?),
            1 => wrong.coordinates[0].interval = interval(-2, 2, 1)?,
            _ => wrong.target = f.rational(0, 1)?,
        }
        let before = f.cas.canonical_snapshot(&mut ledger()?)?;
        assert!(
            f.cas
                .check_enclosure(&wrong, policy(), &certificate, &mut ledger()?, &mut |_| {
                    false
                })
                .is_err()
        );
        assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    Ok(())
}

/// Independent replay covers each original rational operator, including signed division.
/// ´test:unit:enclosure-operator-replay´
/// ´claim:cas:enclosure-operator-replay´
#[test]
fn enclosure_operator_replay() -> TestResult {
    for operator in [
        Operator::Add,
        Operator::Sub,
        Operator::Mul,
        Operator::Div,
        Operator::Neg,
    ] {
        let mut fixture = Fixture::new()?;
        let left = fixture.rational(-2, 3)?;
        let right = fixture.rational(-7, 5)?;
        let operands = if operator == Operator::Neg {
            vec![left]
        } else {
            vec![left, right]
        };
        let target = fixture.term(operator.clone(), &operands)?;
        let request = fixture.request(target);
        let checked = fixture
            .cas
            .enclose(&request, policy(), &mut ledger()?, &mut |_| false)?
            .checked
            .ok_or("bound")?;
        let replay = checker::check(
            &fixture.cas,
            &request,
            checked.certificate(),
            &mut ledger()?,
            &mut |_| false,
        )?;
        let (numerator, denominator) = match operator {
            Operator::Add => (-31, 15),
            Operator::Sub => (11, 15),
            Operator::Mul => (14, 15),
            Operator::Div => (10, 21),
            _ => (2, 3),
        };
        let lo = replay
            .interval()
            .lower()
            .small_ratio(&mut ledger()?)?
            .ok_or("lo")?;
        let hi = replay
            .interval()
            .upper()
            .small_ratio(&mut ledger()?)?
            .ok_or("hi")?;
        assert!(i128::from(lo.0) * denominator <= numerator * i128::from(lo.1));
        assert!(i128::from(hi.0) * denominator >= numerator * i128::from(hi.1));
    }
    Ok(())
}

/// Independent resource ceilings refuse without publishing a partial receipt.
/// ´test:unit:enclosure-resource-ceilings´
/// ´claim:cas:enclosure-resource-ceilings´
#[test]
fn enclosure_resource_ceilings() -> TestResult {
    for limits in [
        Limits {
            component_bits: 12,
            ..Limits::default()
        },
        Limits {
            intermediate_bits: 12,
            component_bits: 12,
            ..Limits::default()
        },
        Limits {
            staged_payload_bytes: 1,
            ..Limits::default()
        },
        Limits {
            output_bytes: 1,
            ..Limits::default()
        },
        Limits {
            witness_bytes: 1,
            ..Limits::default()
        },
        Limits {
            proof_nodes: 2,
            ..Limits::default()
        },
        Limits {
            integer_exponent: 11,
            ..Limits::default()
        },
    ] {
        let (mut f, request) = product_fixture()?;
        let snapshot = f.cas.canonical_snapshot(&mut ledger()?)?;
        let mut budget = Budget::new(limits)?;
        let result = f.cas.enclose(
            &request,
            RefinementPolicy {
                checking_work: 0,
                ..policy()
            },
            &mut budget,
            &mut |_| false,
        )?;
        assert_eq!(result.outcome, EnclosureOutcome::Exhausted);
        assert!(result.checked.is_none());
        assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, snapshot);
    }
    Ok(())
}

/// Witness and retained-output byte ceilings have separate literal refusal boundaries.
/// ´test:unit:enclosure-byte-pins´
/// ´claim:cas:enclosure-byte-pins´
#[test]
fn enclosure_byte_pins() -> TestResult {
    let (mut f, request) = product_fixture()?;
    let certificate = f
        .cas
        .enclose(&request, policy(), &mut ledger()?, &mut |_| false)?
        .checked
        .ok_or("bound")?
        .certificate()
        .clone();
    let encoded = certificate.to_bytes(&mut ledger()?)?;
    assert_eq!(encoded.len(), 617);
    for (bytes, passes) in [(617, true), (616, false)] {
        let mut budget = Budget::new(Limits {
            witness_bytes: bytes,
            ..Limits::default()
        })?;
        assert_eq!(certificate.to_bytes(&mut budget).is_ok(), passes);
        let mut budget = Budget::new(Limits {
            witness_bytes: bytes,
            ..Limits::default()
        })?;
        assert_eq!(
            EnclosureCertificate::from_bytes(&encoded, &mut budget).is_ok(),
            passes
        );
    }
    for (bytes, passes) in [(2961, true), (2960, false)] {
        for output in [true, false] {
            let (mut receiving, expected) = product_fixture()?;
            let checked = checker::check(
                &receiving.cas,
                &expected,
                &certificate,
                &mut ledger()?,
                &mut |_| false,
            )?;
            let prefix = service::prefix(&certificate.scope, 12, 6, &mut ledger()?)?;
            let key = service::retention_key(&prefix, &certificate, &mut ledger()?)?;
            let limits = if output {
                Limits {
                    output_bytes: bytes,
                    ..Limits::default()
                }
            } else {
                Limits {
                    staged_payload_bytes: bytes,
                    ..Limits::default()
                }
            };
            let before = receiving.cas.canonical_snapshot(&mut ledger()?)?;
            assert_eq!(
                receiving
                    .cas
                    .retain_enclosure(key, checked, &mut Budget::new(limits)?)
                    .is_ok(),
                passes
            );
            if !passes {
                assert_eq!(receiving.cas.canonical_snapshot(&mut ledger()?)?, before);
            }
        }
    }
    Ok(())
}

/// A retained proved predicate can finish within the allowance that exhausts a cold request.
/// ´test:unit:enclosure-warm-predicate-budget´
/// ´claim:cas:enclosure-warm-predicate-budget´
#[test]
fn enclosure_warm_predicate_budget() -> TestResult {
    let (mut f, request) = product_fixture()?;
    let p = RefinementPolicy {
        checking_work: 0,
        ..policy()
    };
    let mut predicate = request;
    predicate.goal = EnclosureGoal::AtMost(number(0, 1)?);
    assert_eq!(
        f.cas
            .enclose(&predicate, p, &mut ledger()?, &mut |_| false)?
            .outcome,
        EnclosureOutcome::Proved
    );
    let warm = f
        .cas
        .enclose(&predicate, p, &mut ledger()?, &mut |_| false)?;
    let mut bounded = Budget::new(Limits {
        work: warm.work,
        ..Limits::default()
    })?;
    assert_eq!(
        f.cas
            .enclose(&predicate, p, &mut bounded, &mut |_| false)?
            .outcome,
        EnclosureOutcome::Proved
    );
    let (mut other, mut original) = product_fixture()?;
    original.goal = predicate.goal;
    let mut bounded = Budget::new(Limits {
        work: warm.work,
        ..Limits::default()
    })?;
    assert_eq!(
        other
            .cas
            .enclose(&original, p, &mut bounded, &mut |_| false)?
            .outcome,
        EnclosureOutcome::Exhausted
    );

    Ok(())
}

/// A staged suffix cannot publish one receipt before another fails shared preflight.
/// ´test:unit:enclosure-staged-publication´
/// ´claim:cas:enclosure-staged-publication´
#[test]
fn enclosure_staged_publication() -> TestResult {
    let (mut fixture, request) = product_fixture()?;
    let scope = scope::key(&fixture.cas, &request, &mut ledger()?)?;
    let certificate = producer::generate(
        &fixture.cas,
        &request,
        scope,
        (12, 6),
        &mut ledger()?,
        &mut |_| false,
    )?;
    let checked = checker::check(
        &fixture.cas,
        &request,
        &certificate,
        &mut ledger()?,
        &mut |_| false,
    )?;
    let prefix = service::prefix(&certificate.scope, 12, 6, &mut ledger()?)?;
    let key = service::retention_key(&prefix, &certificate, &mut ledger()?)?;
    let before = fixture.cas.canonical_snapshot(&mut ledger()?)?;
    let first = fixture.cas.stage_enclosure(key, checked, &mut ledger()?)?;
    assert_eq!(fixture.cas.canonical_snapshot(&mut ledger()?)?, before);
    let second_certificate = producer::generate(
        &fixture.cas,
        &request,
        certificate.scope.clone(),
        (20, 6),
        &mut ledger()?,
        &mut |_| false,
    )?;
    let second = checker::check(
        &fixture.cas,
        &request,
        &second_certificate,
        &mut ledger()?,
        &mut |_| false,
    )?;
    let second_prefix = service::prefix(&certificate.scope, 20, 6, &mut ledger()?)?;
    let second_key = service::retention_key(&second_prefix, &second_certificate, &mut ledger()?)?;
    let second = fixture
        .cas
        .stage_enclosure(second_key, second, &mut ledger()?)?;
    let mut pending = vec![first, second];
    let one = Budget::new(Limits {
        vertices: fixture.cas.counts().vertices as u64 + 1,
        ..Limits::default()
    })?;
    assert!(
        fixture
            .cas
            .publish_staged_judgments(&mut pending, &one)
            .is_err()
    );
    assert_eq!(pending.len(), 2);
    assert_eq!(fixture.cas.canonical_snapshot(&mut ledger()?)?, before);
    fixture
        .cas
        .publish_staged_judgments(&mut pending, &ledger()?)?;
    assert!(pending.is_empty());
    assert_eq!(fixture.cas.retained_judgment_count(), 2);
    assert!(
        fixture
            .cas
            .retained_enclosure(&prefix, &mut ledger()?)?
            .is_some()
    );
    assert!(
        fixture
            .cas
            .retained_enclosure(&second_prefix, &mut ledger()?)?
            .is_some()
    );
    Ok(())
}

mod logarithms;
mod roots;
