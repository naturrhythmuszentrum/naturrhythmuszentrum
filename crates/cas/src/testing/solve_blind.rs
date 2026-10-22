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

pub(crate) use source::{
    Fixture, gaussian_polynomial, nonzero_consequence, polynomial, real_polynomial, regional_unit,
    symbolic, symbolic_in_field,
};

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

/// Search a nonreal source before independently multiplying the retained factors.
pub(crate) fn independent_gaussian_product(
    coefficients: &[(i64, i64)],
) -> crate::testing::common::TestResult {
    let (mut fixture, request) = gaussian_polynomial(coefficients)?;
    let outcome = fixture.cas.run(&request);
    let crate::request::Outcome::Solved { answer, .. } = outcome else {
        return Err(format!("Gaussian oracle search: {outcome:?}").into());
    };
    oracle::gaussian_product(
        &fixture.cas,
        &answer.coverage.ok_or("Gaussian coverage")?,
        coefficients,
    )
}

#[path = "solve_blind/acceptance.rs"]
mod acceptance;
pub(crate) use acceptance::source as retained_source;

/// Search the original symbolic problem before any independent specialization data is inspected.
pub(crate) fn independent_symbolic_product() -> crate::testing::common::TestResult {
    let keys = ["d", "c", "b", "a"];
    let (fixture, _, answer) = acceptance::solved(&keys)?;
    let coverage = answer.coverage.as_ref().ok_or("symbolic oracle coverage")?;
    let inventory = oracle::samples(&fixture.cas, coverage, &keys)?;
    assert_eq!(inventory.len(), 11);
    acceptance::fresh(&fixture.cas, &answer, &keys)
}

#[path = "solve_blind/inclusion.rs"]
mod inclusion;
pub(crate) use inclusion::check as inclusion_candidates;

#[path = "solve_blind/real.rs"]
mod real;
pub(crate) use real::prefixes as real_prefixes;

#[path = "solve_blind/real_acceptance.rs"]
mod real_acceptance;
pub(crate) use real_acceptance::check as real_acceptance;

#[path = "solve_blind/real_mutations.rs"]
mod real_mutations;
pub(crate) use real_mutations::strata as real_stratum_mutations;

#[path = "solve_blind/real_budget.rs"]
mod real_budget;
pub(crate) use real_budget::measured as real_measured_boundaries;

#[path = "solve_blind/real_replay.rs"]
mod real_replay;
pub(crate) use real_replay::refusals as real_expected_refusals;

// Independent family samples share only the test-side rational quotient arithmetic.
pub(crate) use oracle::{family_cubic, family_quadratic};

pub(crate) use audit::applications as family_applications;

#[path = "solve_blind/answer.rs"]
mod answer;
pub(crate) use answer::symbolic as complete_answer_symbolic;
