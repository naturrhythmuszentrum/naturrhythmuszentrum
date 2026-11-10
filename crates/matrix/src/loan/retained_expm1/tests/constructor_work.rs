// SPDX-License-Identifier: AGPL-3.0-only
//! Public constructor composition uses a separate CAS preparation and explicit lowering visits.

use super::{TestResult, fixture};
use crate::{Heap, HeapLimits, RetainedExpm1Error};
use nrz_bindings::{
    Budget,
    readings::algebra::{
        Cas, Rational,
        enclosure::prepared_expm1::{Counts, Outcome, Policy, Prepared, Request},
    },
};
use nrz_scalar::ArithmeticContext;
use std::cmp::Ordering;

/// The warm graph is built outside the original account, whose limits never change.
fn input(warm: bool) -> TestResult<(Cas, Request)> {
    let (mut cas, request) = fixture::request()?;
    if warm {
        cas.prepare_expm1(
            &request,
            Policy::default(),
            &mut Budget::new(fixture::limits())?,
            &mut Counts::default(),
        )?;
    }
    Ok((cas, request))
}

/// A normalized binary64 power has a 53-bit significand and exponent shifted by 52.
fn power(exponent: i64, negative: bool, budget: &mut Budget) -> TestResult<Rational> {
    let sign = if negative { -1 } else { 1 };
    let significand = Rational::from_ratio(sign * (1_i64 << 52), 1, budget)?;
    let scale = Rational::from_ratio(2, 1, budget)?.pow(exponent - 52, budget)?;
    Ok(significand.mul(&scale, budget)?)
}

/// Enumerate endpoint import, comparisons, error addition and outward-power trials.
/// Only public exact arithmetic supplies its own bigint tariff; no matrix lowering is called.
fn arithmetic(authority: &Prepared) -> TestResult<u64> {
    let mut budget = Budget::new(fixture::limits())?;
    let lower = power(1, true, &mut budget)?;
    let zero = Rational::from_ratio(0, 1, &mut budget)?;
    let scale = Rational::from_ratio(2, 1, &mut budget)?.pow(-1074, &mut budget)?;
    let upper = zero.mul(&scale, &mut budget)?;
    let coordinate = &authority.request().original.coordinates[0];
    assert_eq!(
        lower.compare(coordinate.interval.lower(), &mut budget)?,
        Ordering::Equal
    );
    assert_eq!(
        upper.compare(coordinate.interval.upper(), &mut budget)?,
        Ordering::Equal
    );
    let proof = authority.proof();
    let last = proof.stages.last().ok_or("last rounding stage")?;
    let absolute = last.error.add(&proof.absolute_tail, &mut budget)?;
    assert_eq!(
        absolute.compare(&power(5, false, &mut budget)?, &mut budget)?,
        Ordering::Less
    );
    for exponent in (-1074..=4).rev() {
        if absolute.compare(&power(exponent, false, &mut budget)?, &mut budget)?
            == Ordering::Greater
        {
            return Ok(budget.usage().work);
        }
    }
    Err("fixture outward-power trial did not terminate".into())
}

/// Every constructor preparation count is checked independently of its Work consumption.
fn check_counts(counts: &Counts, warm: bool, degree: u64) {
    let cold = u64::from(!warm);
    assert_eq!(counts.preparations, 1);
    assert_eq!(counts.lookups, 1);
    assert_eq!(counts.proposals, cold * degree);
    assert_eq!(counts.coefficients, cold * degree * (degree + 1) / 2);
    assert_eq!(counts.checks, cold);
    assert_eq!(counts.publications, cold);
    assert_eq!(counts.coordinates, 1);
    assert_eq!(counts.proof_records, 0);
}

/// CAS preparation is an isolated lower-layer oracle; all matrix-owned phase terms are explicit.
/// This checks constructor composition, including the evidence-inventory debit, without using
/// a successful constructor's observed total or the production lowering/inventory helper.
#[test]
fn retained_law_public_constructor_work_boundaries() -> TestResult {
    for warm in [false, true] {
        let (mut exemplar, request) = input(warm)?;
        let mut preparation = Budget::new(fixture::limits())?;
        let mut counts = Counts::default();
        let Outcome::Ready(authority) =
            exemplar.prepare_expm1(&request, Policy::default(), &mut preparation, &mut counts)?
        else {
            return Err("prepared fixture".into());
        };
        let degree = u64::from(authority.proof().degree);
        check_counts(&counts, warm, degree);
        assert_eq!(
            authority.proof().coefficients.len(),
            usize::try_from(degree)?
        );
        assert_eq!(
            authority.proof().stages.len(),
            usize::try_from(2 * degree - 1)?
        );
        // One coefficient visit, two numbers per stage, six bounds and two coordinate endpoints.
        let inventory = degree + 2 * (2 * degree - 1) + 6 + 2;
        let lowering = inventory + arithmetic(&authority)?;
        // Both with_retained_graph calls visit the single warm registration; cold has none.
        let registration = u64::from(warm) + u64::from(warm);
        let shells = 1 + 3 + 2 * 2 + 2;
        let work = preparation.usage().work + lowering + registration + shells;
        for short in [0, 1] {
            constructor_boundary(warm, short, degree, work, &exemplar)?;
        }
    }
    Ok(())
}

/// Spend the original fixed account before the public call and inspect its complete result.
fn constructor_boundary(
    warm: bool,
    short: u64,
    degree: u64,
    work: u64,
    exemplar: &Cas,
) -> TestResult {
    let (cas, request) = input(warm)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = fixture::account()?;
    let limits = account.limits().clone();
    let before = (heap.resource_bytes()?, heap.loans.revision);
    let prior = fixture::leave_work(&mut account, work - short)?;
    assert!(prior > 0);
    let mut counts = Counts::default();
    let injection = crate::loan::fault::Injection::after(usize::MAX);
    let result = heap.prepare_retained_expm1(
        cas,
        &request,
        [-2.0, 0.0],
        Policy::default(),
        &ArithmeticContext::new()?,
        &mut counts,
        &mut account,
    );
    check_counts(&counts, warm, degree);
    if short == 0 {
        let law = result.map_err(|(error, _)| error)?;
        assert_eq!(account.usage().work, prior + work);
        assert_eq!(crate::loan::fault::Injection::attempts(), 3);
        assert_eq!(heap.loans.debits.iter().flatten().count(), 2);
        law.evaluate(fixture::point(0.0), &ArithmeticContext::new()?)?;
    } else {
        let (error, returned) = result.expect_err("complete constructor Work boundary");
        let RetainedExpm1Error::Ownership(error) = error else {
            return Err(format!("{error:?}").into());
        };
        fixture::work_refusal(error, limits.work, prior + work)?;
        assert_eq!(account.usage().work, prior + work - (3 + 2 * 2 + 2));
        returned.expression(&request.original.target)?;
        assert_eq!(returned.counts(), exemplar.counts());
        assert_eq!(returned.retained_judgment_count(), 1);
        assert_eq!(
            returned.retained_judgment_bytes(),
            exemplar.retained_judgment_bytes()
        );
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        assert_eq!(heap.loans.debits.len(), 0);
        assert_eq!(heap.loans.debits.capacity(), 0);
        assert_eq!((heap.resource_bytes()?, heap.loans.revision), before);
    }
    assert_eq!(account.limits(), &limits);
    drop(injection);
    Ok(())
}
