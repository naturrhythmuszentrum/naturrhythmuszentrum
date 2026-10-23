// SPDX-License-Identifier: AGPL-3.0-only
//! Strict coefficient readers retain divisor domains and cumulative resources.

use super::{TestResult, base, expression, positive, radicand};
use crate::budget::{Budget, BudgetKind, Limits};
use crate::graph::{Cas, Handle};
use crate::model::Operator;
use crate::solving::coefficients::{arithmetic::Engine, proof::fault};

struct Fixture {
    cas: Cas,
    region: Handle,
    domain: Handle,
    quotient: Handle,
}

fn fixture(guarded: bool, cancel: bool) -> TestResult<Fixture> {
    let mut cas = Cas::new(Limits::default())?;
    let mut budget = Budget::new(Limits::default())?;
    let (header, mut region, domain) = base(&mut cas, &mut budget)?;
    let mut factors = Vec::new();
    for key in ["L", "C"] {
        let factor = expression(
            &mut cas,
            &header,
            Operator::Free(key.into()),
            &[],
            &mut budget,
        )?;
        if guarded {
            region = positive(&mut cas, &header, &region, &factor, &mut budget)?;
        }
        factors.push(factor);
    }
    let product = expression(&mut cas, &header, Operator::Mul, &factors, &mut budget)?;
    let one = expression(
        &mut cas,
        &header,
        Operator::Rational(crate::Rational::one()),
        &[],
        &mut budget,
    )?;
    let numerator = if cancel { product.clone() } else { one };
    let quotient = expression(
        &mut cas,
        &header,
        Operator::Div,
        &[numerator, product],
        &mut budget,
    )?;
    Ok(Fixture {
        cas,
        region,
        domain,
        quotient,
    })
}

fn read(value: &Fixture, budget: &mut Budget) -> Result<(), crate::Fault> {
    let engine = Engine {
        state: &value.cas.state,
        domain: &value.domain,
    };
    crate::solving::solve_search::read::expression(
        &value.cas,
        &engine,
        value.quotient.logical_id(),
        "",
        &value.region,
        budget,
    )
    .map(|_| ())
    .map_err(|error| fault(&error))
}

fn run(
    operation: u8,
    limit: u64,
    kind: BudgetKind,
) -> TestResult<(Result<(), crate::Fault>, Budget)> {
    let mut value = fixture(true, false)?;
    let exact = radicand(
        &value.cas,
        &value.domain,
        &mut Budget::new(Limits::default())?,
    )?;
    let mut limits = Limits::default();
    match kind {
        BudgetKind::Work => limits.work = limit,
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = limit,
        BudgetKind::PayloadBytes => limits.payload_bytes = limit,
        _ => return Err("transport boundary resource".into()),
    }
    let mut budget = Budget::new(limits)?;
    let result = match operation {
        0 => read(&value, &mut budget),
        1 => super::guard(&mut value.cas, &value.region, &exact, &mut budget).and_then(|result| {
            result.map(|_| ()).ok_or_else(|| {
                crate::Fault::invalid(
                    crate::InvalidCode::StrictSignUnavailable,
                    crate::Location::new("transport boundary"),
                )
            })
        }),
        _ => super::super::transport::inverse_square(
            &mut value.cas,
            &value.region,
            &exact,
            &mut budget,
        )
        .map(|_| ()),
    };
    Ok((result, budget))
}

/// A removable factor never erases an original undefined divisor.
/// ´test:unit:lc-coefficient-domain-refusals´
#[test]
fn lc_coefficient_domain_refusals() -> TestResult {
    for cancel in [false, true] {
        let unsigned = fixture(false, cancel)?;
        assert!(read(&unsigned, &mut Budget::new(Limits::default())?).is_err());
        let signed = fixture(true, cancel)?;
        read(&signed, &mut Budget::new(Limits::default())?)?;
    }
    Ok(())
}

/// Strict division and derived root guards each have exact Work and scratch boundaries.
/// ´test:unit:lc-coefficient-transport-boundaries´
#[test]
fn lc_coefficient_transport_boundaries() -> TestResult {
    let mut measurements = Vec::new();
    for operation in [0, 1, 2] {
        let (result, budget) = run(operation, Limits::default().work, BudgetKind::Work)?;
        result?;
        let work = budget.usage().work;
        run(operation, work, BudgetKind::Work)?.0?;
        assert!(run(operation, work - 1, BudgetKind::Work)?.0.is_err());
        let mut low = 0;
        let mut high = Limits::default().staged_payload_bytes;
        while high - low > 1 {
            let middle = low + (high - low) / 2;
            if run(operation, middle, BudgetKind::StagedPayloadBytes)?
                .0
                .is_ok()
            {
                high = middle;
            } else {
                low = middle;
            }
        }
        run(operation, high, BudgetKind::StagedPayloadBytes)?.0?;
        assert!(
            run(operation, high - 1, BudgetKind::StagedPayloadBytes)?
                .0
                .is_err()
        );
        measurements.push((operation, work, high));
    }
    assert_eq!(
        measurements,
        vec![(0, 5_026, 3_600), (1, 13_339, 10_510), (2, 10_515, 9_562)]
    );
    Ok(())
}
