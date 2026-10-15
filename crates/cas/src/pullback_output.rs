// SPDX-License-Identifier: AGPL-3.0-only
//! Delta-sum construction and independent structural support/coefficient comparison.

use crate::budget::Budget;
use crate::case_syntax::invalid;
use crate::error::Fault;
use crate::graph::{Cas, Handle, size};
use crate::model::Operator;
use crate::number::Rational;
use crate::pullback_geometry::{AT, contains, interval};
use crate::pullback_model::{PullbackCertificate, PullbackValue};

/// Compare the actual output terms with the exact interior roots, without building an expected graph.
pub(crate) fn check(
    cas: &Cas,
    claim: &PullbackValue,
    evidence: &PullbackCertificate,
    budget: &mut Budget,
) -> Result<(), Fault> {
    crate::distribution_check::membership(
        cas,
        cas.checked(&claim.result)?,
        &claim.target_space,
        &[],
        budget,
    )?;
    let target = interval(cas, &claim.target_space, budget)?;
    let mut expected = 0_usize;
    for zero in &evidence.zeros {
        expected += usize::from(contains(&target, &zero.point, budget)?);
    }
    let actual = crate::distribution_linear::terms(cas, cas.checked(&claim.result)?, &[], budget)?;
    if actual.len() != expected {
        return Err(invalid());
    }
    let mut seen = std::collections::BTreeSet::new();
    for (term, coefficient) in actual.values() {
        let term = cas
            .state
            .expression(cas.checked(term.as_ref().ok_or_else(invalid)?)?)?;
        if term.operator != (Operator::Delta { order: 0 }) {
            return Err(invalid());
        }
        let point =
            crate::distribution_check::rational(cas, *term.operands.first().ok_or_else(invalid)?)?;
        budget.work(size(evidence.zeros.len(), AT)?.saturating_mul(128), AT)?;
        let zero = evidence
            .zeros
            .iter()
            .find(|zero| zero.point == *point)
            .ok_or_else(invalid)?;
        if !contains(&target, point, budget)?
            || zero.inverse_jacobian != *coefficient
            || !seen.insert(point.canonical_bytes(budget)?)
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Build the proposed finite delta sum in the supplied increasing root order.
pub(crate) fn build(
    cas: &mut Cas,
    header: &Handle,
    claim: &PullbackValue,
    evidence: &PullbackCertificate,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let target = interval(cas, &claim.target_space, budget)?;
    let mut result = None;
    for zero in &evidence.zeros {
        if !contains(&target, &zero.point, budget)? {
            continue;
        }
        let point = crate::distribution_terms::rational(cas, header, &zero.point, budget)?;
        let delta = crate::distribution_terms::node(
            cas,
            header,
            &claim.target_space,
            Operator::Delta { order: 0 },
            &[point],
            budget,
        )?;
        let term = if zero.inverse_jacobian == Rational::one() {
            delta
        } else {
            let factor =
                crate::distribution_terms::rational(cas, header, &zero.inverse_jacobian, budget)?;
            crate::distribution_terms::node(
                cas,
                header,
                &claim.target_space,
                Operator::DistributionScale,
                &[factor, delta],
                budget,
            )?
        };
        result = Some(match result {
            None => term,
            Some(previous) => crate::distribution_terms::node(
                cas,
                header,
                &claim.target_space,
                Operator::DistributionAdd,
                &[previous, term],
                budget,
            )?,
        });
    }
    result.map_or_else(
        || {
            crate::distribution_terms::node(
                cas,
                header,
                &claim.target_space,
                Operator::DistributionZero,
                &[],
                budget,
            )
        },
        Ok,
    )
}
