// SPDX-License-Identifier: AGPL-3.0-only
//! Public original-claim controls for the closed prepared schedule.

use nrz_cas::domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
use nrz_cas::enclosure::prepared_expm1::{Counts, Outcome, Policy, Prepared, Proof, Request};
use nrz_cas::enclosure::{CoordinateBound, EnclosureGoal, EnclosureRequest, Interval};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::syntax::{AtomicSort, Declaration, Environment, Field, Operator, Sort};
use nrz_cas::syntax::{Draft, ExpressionDraft};
use nrz_cas::{Cas, Fault, FaultKind, Handle, InvalidCode, Rational, Reference};
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
#[path = "prepared_expm1/boundaries.rs"]
mod boundaries;
#[path = "prepared_expm1/oracles.rs"]
mod oracles;
#[path = "prepared_expm1/resources.rs"]
mod resources;
fn ledger() -> Result<Budget, Fault> {
    Budget::new(Limits {
        work: 2_000_000_000,
        integer_exponent: 4096,
        arithmetic_steps: 50_000_000,
        name_bytes: 131_072,
        source_bytes: 4_194_304,
        output_bytes: 4_194_304,
        witness_bytes: 4_194_304,
        ..Limits::default()
    })
}
fn number(n: i64, d: u64) -> Result<Rational, Fault> {
    Rational::from_ratio(n, d, &mut ledger()?)
}
fn power(e: i64) -> Result<Rational, Fault> {
    number(2, 1)?.pow(e, &mut ledger()?)
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
        Self::with_names_and_limits(
            names,
            Limits {
                work: 2_000_000_000,
                integer_exponent: 4096,
                arithmetic_steps: 50_000_000,
                ..Limits::default()
            },
        )
    }
    fn with_names_and_limits(names: &[&str], limits: Limits) -> TestResult<Self> {
        let mut environment = Environment::default();
        for name in names {
            environment.declarations.push(Declaration {
                key: (*name).into(),
                display_name: (*name).into(),
                sort: real(),
            });
        }
        let mut cas = Cas::new(limits)?;
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

    fn request(&self, target: Handle) -> EnclosureRequest {
        EnclosureRequest {
            target,
            context: self.context.clone(),
            coordinates: Vec::new(),
            goal: EnclosureGoal::Width,
        }
    }
}
const fn real() -> Sort {
    Sort::Value(AtomicSort::Scalar(Field::Real))
}

fn request(f: &mut Fixture, value: Rational, bits: u32) -> TestResult<Request> {
    let argument = f.term(Operator::Rational(value), &[])?;
    expression(f, argument, bits)
}
fn expression(f: &mut Fixture, argument: Handle, bits: u32) -> TestResult<Request> {
    let e = f.term(Operator::Exponential, &[argument])?;
    let one = f.term(Operator::Rational(Rational::one()), &[])?;
    let target = f.term(Operator::Sub, &[e, one])?;
    Ok(Request {
        original: f.request(target),
        bits,
        environment: 1,
    })
}
fn prepared(f: &mut Fixture, r: &Request) -> TestResult<Prepared> {
    match f
        .cas
        .prepare_expm1(r, Policy::default(), &mut ledger()?, &mut Counts::default())?
    {
        Outcome::Ready(p) => Ok(*p),
        Outcome::TargetNotCertifiable => Err("target".into()),
    }
}
fn invalid(fault: Fault, code: InvalidCode) {
    assert_eq!(fault.kind, FaultKind::Invalid(code));
    drop(fault);
}

/// Distinct unsorted original points include zero, the boundary and a subnormal image.
/// # Panics
/// Panics only if the checked schedule violates this test's asserted contract.
/// ´claim:cas:prepared-expm1-boundaries´
/// ´test:integration:prepared-expm1-boundaries´
#[test]
fn prepared_expm1_boundaries() -> TestResult {
    let mut f = Fixture::new()?;
    for value in [
        power(-40)?.negated(&mut ledger()?)?,
        number(-2, 1)?,
        Rational::zero(),
        number(-1, 1)?,
        power(-1022)?.negated(&mut ledger()?)?,
        power(-10)?.negated(&mut ledger()?)?,
    ] {
        let r = request(&mut f, value, 40)?;
        let p = prepared(&mut f, &r)?;
        assert_eq!(p.proof().stages.len(), (2 * p.proof().degree - 1) as usize);
        let bytes = p.proof().to_bytes(&mut ledger()?)?;
        let proof = Proof::from_bytes(&bytes, &mut ledger()?)?;
        assert_eq!(&proof, p.proof());
        let checked = f.cas.check_expm1(
            &r,
            Policy::default(),
            &proof,
            &mut ledger()?,
            &mut Counts::default(),
        )?;
        assert_eq!(checked.statement_key(), p.statement_key());
    }
    let x = f.term(Operator::Free("x".into()), &[])?;
    let mut r = expression(&mut f, x.clone(), 40)?;
    r.original.coordinates.push(CoordinateBound {
        coordinate: x,
        interval: Interval::new(number(-2, 1)?, Rational::zero(), &mut ledger()?)?,
    });
    prepared(&mut f, &r)?;
    Ok(())
}

/// Each changed obligation fails with its own typed cause and leaves no graph suffix.
/// # Panics
/// Panics only if a mutation is accepted or reports a different obligation.
/// ´claim:cas:prepared-expm1-mutations´
/// ´test:integration:prepared-expm1-mutations´
#[test]
fn prepared_expm1_mutations() -> TestResult {
    let mut f = Fixture::new()?;
    let r = request(&mut f, number(-1, 1)?, 40)?;
    let p = prepared(&mut f, &r)?;
    for (i, code) in [
        (0, InvalidCode::Expm1Coefficient),
        (1, InvalidCode::Expm1Rounding),
        (2, InvalidCode::Expm1Analytic),
        (3, InvalidCode::Expm1Import),
        (4, InvalidCode::Expm1Export),
        (5, InvalidCode::Expm1Rounding),
        (6, InvalidCode::Expm1Claim),
        (7, InvalidCode::UnsupportedVersion),
    ] {
        let mut q = p.proof().clone();
        match i {
            0 => q.coefficients[0].significand += 1,
            1 => {
                q.stages.pop();
            }
            2 => q.relative_tail = Rational::zero(),
            3 => q.import_ratio = Rational::zero(),
            4 => q.restoration_error = Rational::one(),
            5 => q.central_ratio = Rational::zero(),
            6 => q.scope.push(0),
            _ => q.version = 2,
        }
        let before = f.cas.canonical_snapshot(&mut ledger()?)?;
        invalid(
            f.cas
                .check_expm1(
                    &r,
                    Policy::default(),
                    &q,
                    &mut ledger()?,
                    &mut Counts::default(),
                )
                .expect_err("mutated proof"),
            code,
        );
        assert_eq!(f.cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    let q = f.cas.propose_expm1(
        &r,
        Policy::default(),
        p.proof().degree - 1,
        &mut ledger()?,
        &mut Counts::default(),
    )?;
    invalid(
        f.cas
            .check_expm1(
                &r,
                Policy::default(),
                &q,
                &mut ledger()?,
                &mut Counts::default(),
            )
            .expect_err("degree"),
        InvalidCode::Expm1Target,
    );
    Ok(())
}

/// Original claims, environment and mathematical refusals are never resource exhaustion.
/// # Panics
/// Panics only if replay loses its original scope or a refusal changes kind.
/// ´claim:cas:prepared-expm1-original-claim´
/// ´test:integration:prepared-expm1-original-claim´
#[test]
fn prepared_expm1_original_claim() -> TestResult {
    let mut f = Fixture::new()?;
    let r = request(&mut f, number(-1, 1)?, 40)?;
    let p = prepared(&mut f, &r)?;
    let mut receiving = Fixture::new()?;
    receiving.term(Operator::Rational(number(99, 1)?), &[])?;
    let expected = request(&mut receiving, number(-1, 1)?, 40)?;
    receiving.cas.check_expm1(
        &expected,
        Policy::default(),
        p.proof(),
        &mut ledger()?,
        &mut Counts::default(),
    )?;
    let wrong = request(&mut receiving, number(-2, 1)?, 40)?;
    invalid(
        receiving
            .cas
            .check_expm1(
                &wrong,
                Policy::default(),
                p.proof(),
                &mut ledger()?,
                &mut Counts::default(),
            )
            .expect_err("original"),
        InvalidCode::Expm1Claim,
    );
    for (value, code) in [
        (number(1, 1)?, InvalidCode::Expm1OutsideDomain),
        (number(-3, 1)?, InvalidCode::Expm1OutsideDomain),
        (
            power(-1023)?.negated(&mut ledger()?)?,
            InvalidCode::Expm1BelowNormal,
        ),
    ] {
        let r = request(&mut f, value, 40)?;
        invalid(
            f.cas
                .prepare_expm1(
                    &r,
                    Policy::default(),
                    &mut ledger()?,
                    &mut Counts::default(),
                )
                .expect_err("operand"),
            code,
        );
    }
    let mut r = request(&mut f, number(-1, 1)?, 60)?;
    assert!(matches!(
        f.cas.prepare_expm1(
            &r,
            Policy::default(),
            &mut ledger()?,
            &mut Counts::default()
        )?,
        Outcome::TargetNotCertifiable
    ));
    r.environment = 2;
    invalid(
        f.cas
            .prepare_expm1(
                &r,
                Policy::default(),
                &mut ledger()?,
                &mut Counts::default(),
            )
            .expect_err("environment"),
        InvalidCode::Expm1Environment,
    );
    let r = request(&mut f, Rational::zero(), 60)?;
    let p = prepared(&mut f, &r)?;
    let mut proof = p.proof().clone();
    proof.coefficients[0].significand += 1;
    invalid(
        f.cas
            .check_expm1(
                &r,
                Policy::default(),
                &proof,
                &mut ledger()?,
                &mut Counts::default(),
            )
            .expect_err("zero validation"),
        InvalidCode::Expm1Coefficient,
    );
    Ok(())
}

/// Exact Work and wire ceilings pass; one below refuses atomically; reuse builds nothing.
/// # Panics
/// Panics only if a finite resource boundary or construction counter is inconsistent.
/// The whole-route Work boundary is measured from this implementation as a regression pin;
/// it is distinct from the independently specified degree-one entry tariff.
/// ´claim:cas:prepared-expm1-resource-controls´
/// ´test:integration:prepared-expm1-resource-controls´
#[test]
fn prepared_expm1_resource_controls() -> TestResult {
    let mut f = Fixture::new()?;
    let r = request(&mut f, number(-1, 1)?, 40)?;
    let mut budget = ledger()?;
    budget.work(7, nrz_cas::Location::new("caller prefix"))?;
    let result = f
        .cas
        .prepare_expm1(&r, Policy::default(), &mut budget, &mut Counts::default())?;
    let Outcome::Ready(p) = result else {
        return Err("target".into());
    };
    let cold = budget.usage().work;
    for cap in [cold, cold - 1] {
        let mut other = Fixture::new()?;
        let r = request(&mut other, number(-1, 1)?, 40)?;
        let before = other.cas.canonical_snapshot(&mut ledger()?)?;
        let mut budget = Budget::new(Limits {
            work: cap,
            integer_exponent: 4096,
            arithmetic_steps: 50_000_000,
            ..Limits::default()
        })?;
        budget.work(7, nrz_cas::Location::new("caller prefix"))?;
        let result =
            other
                .cas
                .prepare_expm1(&r, Policy::default(), &mut budget, &mut Counts::default());
        if cap == cold {
            assert!(result.is_ok());
        } else {
            assert_eq!(
                result.expect_err("one below").kind,
                FaultKind::Exhausted {
                    reason: nrz_cas::Exhaustion::Limit,
                    budget: BudgetKind::Work,
                    limit: cap,
                    needed: cold,
                }
            );
            let spent = budget.usage().work;
            let repeated =
                other
                    .cas
                    .prepare_expm1(&r, Policy::default(), &mut budget, &mut Counts::default());
            assert!(
                matches!(repeated.expect_err("spent ledger").kind, FaultKind::Exhausted { reason: nrz_cas::Exhaustion::Limit, budget: BudgetKind::Work, limit, .. } if limit == cap)
            );
            assert!(budget.usage().work >= spent);
            assert_eq!(other.cas.canonical_snapshot(&mut ledger()?)?, before);
        }
    }
    let mut counts = Counts::default();
    let mut warm = ledger()?;
    f.cas
        .prepare_expm1(&r, Policy::default(), &mut warm, &mut counts)?;
    assert_eq!(
        (
            counts.proposals,
            counts.coefficients,
            counts.checks,
            counts.publications
        ),
        (0, 0, 0, 0)
    );
    assert!(warm.usage().work > 0 && warm.usage().work < cold);
    let bytes = p.proof().to_bytes(&mut ledger()?)?;
    for cap in [bytes.len() as u64, bytes.len() as u64 - 1] {
        let mut budget = Budget::new(Limits {
            witness_bytes: cap,
            integer_exponent: 4096,
            arithmetic_steps: 50_000_000,
            ..Limits::default()
        })?;
        let decoded = Proof::from_bytes(&bytes, &mut budget);
        if cap == bytes.len() as u64 {
            decoded?;
        } else {
            assert_eq!(
                decoded.expect_err("wire ceiling").kind,
                FaultKind::Exhausted {
                    reason: nrz_cas::Exhaustion::Limit,
                    budget: BudgetKind::WitnessBytes,
                    limit: cap,
                    needed: bytes.len() as u64,
                }
            );
        }
    }
    Ok(())
}
/// Independent finite ceilings retain their own exact-limit and one-below causes.
/// # Panics
/// Panics only if replay crosses a declared finite capacity or changes its refusal cause.
/// ´claim:cas:prepared-expm1-capacity-controls´
/// ´test:integration:prepared-expm1-capacity-controls´
#[test]
fn prepared_expm1_capacity_controls() -> TestResult {
    let mut f = Fixture::new()?;
    let r = request(&mut f, number(-1, 1)?, 40)?;
    let p = prepared(&mut f, &r)?;
    let degree = u64::from(p.proof().degree);
    for (kind, ceiling) in [
        (BudgetKind::PolynomialDegree, degree),
        (BudgetKind::ProofNodes, 3 * degree + 8),
        (BudgetKind::IntegerExponent, 1075),
        (BudgetKind::OutputBytes, f.cas.retained_judgment_bytes()),
    ] {
        for limit in [ceiling, ceiling - 1] {
            let mut receiving = Fixture::new()?;
            let expected = request(&mut receiving, number(-1, 1)?, 40)?;
            let mut limits = ledger()?.limits().clone();
            match kind {
                BudgetKind::PolynomialDegree => limits.polynomial_degree = limit,
                BudgetKind::ProofNodes => limits.proof_nodes = limit,
                BudgetKind::IntegerExponent => limits.integer_exponent = limit,
                BudgetKind::OutputBytes => limits.output_bytes = limit,
                _ => limits.staged_payload_bytes = limit,
            }
            let before = receiving.cas.canonical_snapshot(&mut ledger()?)?;
            let result = receiving.cas.check_expm1(
                &expected,
                Policy::default(),
                p.proof(),
                &mut Budget::new(limits)?,
                &mut Counts::default(),
            );
            if limit == ceiling {
                result?;
            } else {
                let reservation = if kind == BudgetKind::StagedPayloadBytes {
                    320
                } else {
                    0
                };
                assert_eq!(
                    result.expect_err("one below").kind,
                    FaultKind::Exhausted {
                        reason: nrz_cas::Exhaustion::Limit,
                        budget: kind,
                        limit: limit - reservation,
                        needed: ceiling - reservation
                    }
                );
                assert_eq!(receiving.cas.canonical_snapshot(&mut ledger()?)?, before);
            }
        }
    }
    let mut fresh = Fixture::new()?;
    let r = request(&mut fresh, number(-1, 1)?, 40)?;
    let outcome = fresh.cas.prepare_expm1(
        &r,
        Policy {
            attempts: 1,
            ..Policy::default()
        },
        &mut ledger()?,
        &mut Counts::default(),
    )?;
    assert!(matches!(outcome, Outcome::TargetNotCertifiable));
    Ok(())
}
