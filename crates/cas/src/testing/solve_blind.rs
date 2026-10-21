// SPDX-License-Identifier: AGPL-3.0-only
//! The same original source construction is exercised at the private audit boundary.

#[path = "solve_blind_public.rs"]
mod source;
/// Run the original source first; only then decode its retained factor certificate for the oracle.
pub(crate) fn independent_product(coefficients: &[i64]) -> crate::testing::common::TestResult {
    let (mut fixture, request) = source::polynomial(coefficients)?;
    let outcome = fixture.cas.run(&request);
    let crate::request::Outcome::Solved { answer, .. } = outcome else {
        return Err(format!("oracle search outcome: {outcome:?}").into());
    };
    let coverage = answer.coverage.ok_or("oracle missing coverage")?;
    oracle::product(&fixture.cas, &coverage, coefficients)
}

#[path = "solve_blind/oracle.rs"]
mod oracle;

/// Plain checked integer convolution for post-search named factor identities.
pub(crate) fn multiply(
    left: &[i64],
    right: &[i64],
) -> crate::testing::common::TestResult<Vec<i64>> {
    let mut product = vec![0_i64; left.len() + right.len() - 1];
    for (row, coefficient) in left.iter().enumerate() {
        for (column, other) in right.iter().enumerate() {
            product[row + column] = product[row + column]
                .checked_add(
                    coefficient
                        .checked_mul(*other)
                        .ok_or("oracle integer product")?,
                )
                .ok_or("oracle integer sum")?;
        }
    }
    Ok(product)
}

/// Differentiate the original coefficient sequence independently of the coefficient engine.
pub(crate) fn derivative(coefficients: &[i64]) -> crate::testing::common::TestResult<Vec<i64>> {
    coefficients
        .iter()
        .enumerate()
        .skip(1)
        .map(|(power, value)| {
            value
                .checked_mul(i64::try_from(power)?)
                .ok_or_else(|| "oracle integer derivative".into())
        })
        .collect()
}

pub(crate) use source::{polynomial, regional_unit, symbolic};

#[path = "solve_blind/trace.rs"]
mod trace;
pub(crate) use trace::{prefixes, symbolic_prefixes};

#[path = "solve_blind/audit.rs"]
mod audit;
pub(crate) use audit::symbolic as symbolic_audit;

pub(crate) use audit::retained_replay_boundary;

#[path = "solve_blind/mutations.rs"]
mod mutations;
pub(crate) use mutations::{factors as false_factors, strata as false_strata};

pub(crate) use trace::transported_choices;

#[path = "solve_blind/rename.rs"]
mod rename;
pub(crate) use rename::complete as renamed_complete_set;

#[path = "solve_blind/boundaries.rs"]
mod boundaries;
pub(crate) use boundaries::measured as measured_boundaries;

#[path = "solve_blind/unlinked.rs"]
mod unlinked;
pub(crate) use unlinked::refuses as unlinked_choices;

pub(crate) use audit::formal_consumer;

pub(crate) use audit::real_choice_consumer;
