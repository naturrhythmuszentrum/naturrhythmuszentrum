// SPDX-License-Identifier: AGPL-3.0-only
//! Tiny independent integer-fraction oracle for the public symbolic test corpus only.

use nrz_cas::{
    Cas, Handle,
    limits::{Budget, Limits, LimitsOverride},
    syntax::{AtomicSort, Declaration as Mathematical, Environment, Field, Operator, Sort},
};

use crate::test_api::{CheckedSensitivity, Context, ExponentPolicy, Registry, SensitivityError};

use crate::testing::declarations::TestResult;

/// Exact test fractions use checked integers; no production rational arithmetic is an oracle.
type Fraction = (i128, i128);

/// Independently evaluate the small replayed scalar graph at integer test coordinates.
pub(crate) fn coefficient(
    cas: &Cas,
    expression: &Handle,
    point: &[(&str, i128)],
) -> TestResult<Fraction> {
    let (operator, _) = cas.expression(expression)?;
    let mut ledger = Budget::new(Limits::default())?;
    let operands = cas.operands(expression, &mut ledger)?;
    let values = operands
        .iter()
        .map(|operand| coefficient(cas, operand, point))
        .collect::<TestResult<Vec<_>>>()?;
    match (operator, values.as_slice()) {
        (Operator::Rational(value), []) => {
            let text = value.exact_string(&mut ledger)?;
            let (numerator, denominator) = text.split_once('/').unwrap_or((&text, "1"));
            Ok((numerator.parse()?, denominator.parse()?))
        }
        (Operator::Free(key), []) => Ok((
            point
                .iter()
                .find(|(name, _)| *name == key)
                .ok_or("missing independent test coordinate")?
                .1,
            1,
        )),
        (Operator::Neg, [value]) => Ok((value.0.checked_neg().ok_or("oracle overflow")?, value.1)),
        (Operator::IntegerPower(exponent), [value]) => power(*value, *exponent),
        (Operator::Mul, [left, right]) => multiply(*left, *right),
        (Operator::Div, [left, right]) if right.0 != 0 => multiply(*left, (right.1, right.0)),
        (Operator::Add, [left, right]) => add(*left, *right),
        (Operator::Sub, [left, right]) => add(
            *left,
            (right.0.checked_neg().ok_or("oracle overflow")?, right.1),
        ),
        _ => Err("unsupported independent oracle node".into()),
    }
}

/// Compare fractions by checked cross multiplication without production normalization.
pub(crate) fn equal_fraction(actual: Fraction, expected: Fraction) -> TestResult {
    assert_ne!(actual.1, 0);
    assert_ne!(expected.1, 0);
    assert_eq!(
        actual.0.checked_mul(expected.1).ok_or("oracle overflow")?,
        expected.0.checked_mul(actual.1).ok_or("oracle overflow")?
    );
    Ok(())
}

/// Small exact multiplication, retaining unreduced fractions.
fn multiply(left: Fraction, right: Fraction) -> TestResult<Fraction> {
    Ok((
        left.0.checked_mul(right.0).ok_or("oracle overflow")?,
        left.1.checked_mul(right.1).ok_or("oracle overflow")?,
    ))
}

/// Small exact addition independent of CAS rational arithmetic.
fn add(left: Fraction, right: Fraction) -> TestResult<Fraction> {
    let first = left.0.checked_mul(right.1).ok_or("oracle overflow")?;
    let second = right.0.checked_mul(left.1).ok_or("oracle overflow")?;
    Ok((
        first.checked_add(second).ok_or("oracle overflow")?,
        left.1.checked_mul(right.1).ok_or("oracle overflow")?,
    ))
}

/// The oracle's powers have small bounded integer exponents.
fn power(value: Fraction, exponent: i64) -> TestResult<Fraction> {
    let magnitude = u32::try_from(exponent.unsigned_abs())?;
    if magnitude > 8 || (exponent < 0 && value.0 == 0) {
        return Err("oracle power outside test domain".into());
    }
    let raised = (
        value.0.checked_pow(magnitude).ok_or("oracle overflow")?,
        value.1.checked_pow(magnitude).ok_or("oracle overflow")?,
    );
    Ok(if exponent < 0 {
        (raised.1, raised.0)
    } else {
        raised
    })
}

/// The shared solve table measures 5,464,087 generation and 3,098,125 construction units.
const GENERATION_WORK: u64 = 16_000_000;

/// Native and scaled-implicit replay peaks reach 4.52 million; each phase retains twofold headroom.
const REPLAY_WORK: u64 = 16_000_000;

/// Explicit request work with all other CAS ceilings unchanged.
pub(crate) fn request_limits() -> LimitsOverride {
    LimitsOverride {
        work: Some(GENERATION_WORK),
        replay_work: Some(REPLAY_WORK),
        ..LimitsOverride::default()
    }
}

/// A sensitivity-specific fixture never changes unrelated units or CAS test defaults.
pub(crate) fn context(keys: &[&str]) -> TestResult<Context> {
    context_with_registry(Registry::new(ExponentPolicy::Rational), keys)
}

/// Permanent store ceilings and request allowances are independently explicit.
pub(crate) fn context_with_registry(registry: Registry, keys: &[&str]) -> TestResult<Context> {
    let environment = Environment {
        declarations: keys
            .iter()
            .map(|key| Mathematical {
                key: (*key).into(),
                display_name: (*key).into(),
                sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            })
            .collect(),
        ..Environment::default()
    };
    let ceilings = Limits {
        work: GENERATION_WORK,
        replay_work: REPLAY_WORK,
        ..Limits::default()
    };
    Ok(Context::with_cas_limits(
        registry,
        Limits::default(),
        environment,
        ceilings,
    )?)
}

/// Rebuild the same recipe at its actual replay peak and one unit below it.
pub(crate) fn replay_boundary(
    lower: impl Fn(&LimitsOverride) -> TestResult<CheckedSensitivity>,
) -> TestResult {
    let baseline = lower(&request_limits())?.jacobian().work();
    assert!(
        baseline.generation <= GENERATION_WORK / 2,
        "generation headroom: {baseline:?}"
    );
    assert!(
        baseline.replay <= REPLAY_WORK / 2,
        "replay headroom: {baseline:?}"
    );
    assert!(
        baseline.construction <= GENERATION_WORK / 2,
        "construction headroom: {baseline:?}"
    );
    let mut limits = request_limits();
    limits.replay_work = Some(baseline.replay);
    let exact = lower(&limits)?;
    assert_eq!(exact.jacobian().work(), baseline);
    let below = baseline
        .replay
        .checked_sub(1)
        .ok_or("empty replay ledger")?;
    limits.replay_work = Some(below);
    let error = lower(&limits)
        .err()
        .ok_or("one-below replay request succeeded")?;
    let Some(SensitivityError::Evidence(outcome)) = error.downcast_ref::<SensitivityError>() else {
        return Err(error);
    };
    let nrz_cas::request::Outcome::Exhausted {
        fault,
        replay_usage,
        ..
    } = outcome.as_ref()
    else {
        return Err("one-below request did not exhaust".into());
    };
    assert!(
        matches!(fault.kind, nrz_cas::FaultKind::Exhausted { budget: nrz_cas::limits::BudgetKind::Work, limit, .. } if limit == below)
    );
    assert!(replay_usage.work <= below);
    Ok(())
}

/// Generation includes optional response bytes, so locate the actual complete-proof boundary.
pub(crate) fn generation_boundary(
    lower: impl Fn(&LimitsOverride) -> TestResult<CheckedSensitivity>,
) -> TestResult {
    let baseline = lower(&request_limits())?.jacobian().work();
    assert!(
        baseline.generation <= GENERATION_WORK / 2,
        "generation headroom: {baseline:?}"
    );
    assert!(
        baseline.replay <= REPLAY_WORK / 2,
        "replay headroom: {baseline:?}"
    );
    assert!(
        baseline.construction <= GENERATION_WORK / 2,
        "construction headroom: {baseline:?}"
    );
    assert!(baseline.generation > baseline.replay && baseline.generation > baseline.construction);
    let threshold = generation_limit(&lower, baseline.construction, baseline.generation)?;
    let mut limits = request_limits();
    limits.work = Some(threshold);
    let exact = lower(&limits)?;
    assert_eq!(exact.jacobian().work().requests, baseline.requests);
    assert!(exact.jacobian().work().generation <= threshold);
    let below = threshold.checked_sub(1).ok_or("empty generation ledger")?;
    limits.work = Some(below);
    generation_exhausted(
        lower(&limits)
            .err()
            .ok_or("one-below generation request succeeded")?,
        below,
    )
}

/// Fresh deterministic schedules find the success/refusal boundary in at most 64 bounded trials.
fn generation_limit(
    lower: &impl Fn(&LimitsOverride) -> TestResult<CheckedSensitivity>,
    mut floor: u64,
    mut ceiling: u64,
) -> TestResult<u64> {
    for _ in 0..u64::BITS {
        if floor == ceiling {
            return Ok(floor);
        }
        let middle = floor + (ceiling - floor) / 2;
        let mut limits = request_limits();
        limits.work = Some(middle);
        match lower(&limits) {
            Ok(_) => ceiling = middle,
            Err(error) => {
                generation_exhausted(error, middle)?;
                floor = middle.checked_add(1).ok_or("boundary overflow")?;
            }
        }
    }
    Err("generation boundary did not converge".into())
}

/// Only a generation-work refusal, not a semantic or replay failure, brackets this boundary.
fn generation_exhausted(error: Box<dyn std::error::Error>, ceiling: u64) -> TestResult {
    let Some(SensitivityError::Evidence(outcome)) = error.downcast_ref::<SensitivityError>() else {
        return Err(error);
    };
    let nrz_cas::request::Outcome::Exhausted { fault, usage, .. } = outcome.as_ref() else {
        return Err("generation request did not exhaust".into());
    };
    assert!(
        matches!(fault.kind, nrz_cas::FaultKind::Exhausted { budget: nrz_cas::limits::BudgetKind::Work, limit, .. } if limit == ceiling)
    );
    assert!(usage.work <= ceiling);
    Ok(())
}
