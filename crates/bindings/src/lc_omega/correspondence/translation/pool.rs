// SPDX-License-Identifier: AGPL-3.0-only
//! First-occurrence rational pool with budgeted copies and exact index equality.

use super::{AT, Result, accounting};
use crate::{AdmissionError, LcInput};
use nrz_cas::{
    Rational,
    limits::{Budget, BudgetKind},
};
use std::cmp::Ordering;

/// Owned deduplicated literals and one pool index per original endpoint.
pub(super) struct Pool {
    /// Only the two recipe constants have known spellings.
    pub(super) values: Vec<Rational>,
    /// Lower before upper for every original row.
    pub(super) endpoints: Vec<usize>,
    /// Predicate nodes counted during the five charged endpoint-index classifications.
    pub(super) predicate_nodes: usize,
}

impl Pool {
    /// Both vector capacities and initialized limb envelopes remain live together.
    pub(super) fn bytes(&self) -> Result<u64> {
        Ok(accounting::vector::<Rational>(self.values.capacity())?
            + accounting::vector::<usize>(self.endpoints.capacity())?
            + self.values.iter().map(accounting::rational).sum::<u64>())
    }
}

/// Capture finite rational endpoints without spelling or cloning an unknown original.
pub(super) fn build(inputs: &[LcInput], budget: &mut Budget) -> Result<Pool> {
    let endpoints = inputs
        .len()
        .checked_mul(2)
        .ok_or(AdmissionError::ResourceLimit)?;
    let mut pool = Pool {
        values: accounting::allocate(
            endpoints
                .checked_add(2)
                .ok_or(AdmissionError::ResourceLimit)?,
            budget,
        )?,
        endpoints: Vec::new(),
        predicate_nodes: 0,
    };
    pool.endpoints = budget.with_retained_payload(pool.bytes()?, AT, |budget| {
        accounting::allocate(endpoints, budget)
    })?;
    for text in ["1", "1/2"] {
        let value = budget.with_retained_payload(pool.bytes()?, AT, |budget| {
            let owned = 130_u64.saturating_sub(size_of::<Rational>() as u64);
            budget.with_retained_payload(owned, AT, |budget| {
                accounting::expression(1, budget)?;
                Ok::<_, AdmissionError>(Rational::parse(text, budget)?)
            })
        })?;
        pool.values.push(value);
    }
    for input in inputs {
        for (position, endpoint) in [input.interval().lower(), input.interval().upper()]
            .into_iter()
            .enumerate()
        {
            let mut index = budget.with_retained_payload(pool.bytes()?, AT, |budget| {
                accounting::original(1, budget)?;
                let value = endpoint
                    .and_then(nrz_cas::ExactConstant::as_rational)
                    .ok_or(AdmissionError::UnsupportedRecipe)?;
                index(&mut pool.values, value, budget)
            })?;
            if position == 1 {
                budget.with_retained_payload(pool.bytes()?, AT, |budget| {
                    accounting::expression(1, budget)
                })?;
                let exact = pool.endpoints.last() == Some(&index);
                pool.predicate_nodes = pool
                    .predicate_nodes
                    .checked_add(if exact { 1 } else { 5 })
                    .ok_or(AdmissionError::ResourceLimit)?;
                if exact {
                    index |= super::EXACT;
                }
            }
            budget.with_retained_payload(pool.bytes()?, AT, |budget| {
                accounting::expression(1, budget)
            })?;
            pool.endpoints.push(index);
        }
    }
    Ok(pool)
}

/// Every lookup compares from zero; an unseen endpoint uses the existing metered text route.
fn index(values: &mut Vec<Rational>, value: &Rational, budget: &mut Budget) -> Result<usize> {
    for (index, existing) in values.iter().enumerate() {
        if value.compare(existing, budget)? == Ordering::Equal {
            return Ok(index);
        }
    }
    let (numerator, denominator) = value.component_bits();
    let text_bound = numerator
        .saturating_add(denominator)
        .saturating_add(3)
        .max(8);
    budget.check(
        BudgetKind::StagedPayloadBytes,
        text_bound + size_of::<String>() as u64,
        AT,
    )?;
    accounting::edges(2, budget)?;
    let text = value.exact_string(budget)?;
    let owned = budget.with_retained_payload(
        text.capacity() as u64 + size_of::<String>() as u64 + accounting::rational(value),
        AT,
        |budget| Ok::<_, AdmissionError>(Rational::parse(&text, budget)?),
    )?;
    drop(text);
    budget.with_retained_payload(accounting::rational(&owned), AT, |budget| {
        accounting::expression(1, budget)
    })?;
    let index = values.len();
    values.push(owned);
    Ok(index)
}
