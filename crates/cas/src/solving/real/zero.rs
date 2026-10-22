// SPDX-License-Identifier: AGPL-3.0-only
//! Checked Real zero signs enter the same finite parameter ideal as exact region equalities.

use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::proof_model::{CheckedApplication, Statement};
use crate::solving::coefficients::{polynomial::Polynomial, storage::reserve};
use crate::solving::{RealSignConclusion, SolveStatement};

/// Every additional generator has an earlier checked zero sign in this exact coefficient stratum.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the Real scalar evaluator consumes checked zero predicates"
)]
pub(crate) fn append(
    cas: &Cas,
    domain: &Handle,
    region: &Handle,
    premises: &[CheckedApplication],
    zeros: &mut Vec<Polynomial>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for premise in premises {
        budget.work(1, super::AT)?;
        let Statement::Solve(statement) = &premise.judgment.statement else {
            continue;
        };
        let SolveStatement::RealSign(sign) = statement.as_ref() else {
            continue;
        };
        if sign.region != *region || sign.conclusion != RealSignConclusion::Zero {
            continue;
        }
        if let Some(polynomial) = crate::solving::coefficients::guard_polynomial::read(
            &cas.state,
            domain,
            region.logical_id(),
            sign.subject.logical_id(),
            budget,
        )? && !polynomial.zero(budget)?
        {
            reserve(zeros, 1, budget)?;
            zeros.push(polynomial);
        }
    }
    Ok(())
}

/// Restart full polynomial division after each cancellation; every changed monomial descends in the fixed order.
/// This bounded ideal consequence check is intentionally incomplete and never treats a residual as zero.
#[expect(
    clippy::redundant_pub_crate,
    reason = "same-Real coefficient evaluation consumes the finite ideal reduction"
)]
pub(crate) fn reduce(
    mut remainder: Polynomial,
    relations: &[Polynomial],
    budget: &mut Budget,
) -> Result<bool, Fault> {
    loop {
        if remainder.zero(budget)? {
            return Ok(true);
        }
        let mut changed = false;
        for relation in relations {
            let (_, reduced) =
                crate::solving::coefficients::gcd::division(&remainder, relation, budget)?;
            if !remainder.equals(&reduced, budget)? {
                remainder = reduced;
                changed = true;
                break;
            }
        }
        if !changed {
            return Ok(false);
        }
    }
}
