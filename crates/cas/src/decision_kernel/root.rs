// SPDX-License-Identifier: AGPL-3.0-only
//! Reuse an already checked original root instead of admitting its identical reconstruction.

use std::cmp::Ordering;

use super::AT;
use crate::budget::Budget;
use crate::error::Fault;
use crate::exact_constant::ExactConstant;
use crate::model::{Branch, Operator};
use crate::number::Rational;

/// Match the root law against a retained checked original, with exact rational comparisons.
///
/// This shortcut applies only to the final record and a rational radicand. A
/// rational original is checked by squaring; an irrational original must expose
/// the identical strict-real half-power in its already normalized closed graph.
/// Every original operand and every output endpoint relation was checked first.
pub(super) fn matches(
    original: &ExactConstant,
    input: &ExactConstant,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let Some(radicand) = input.as_rational() else {
        return Ok(false);
    };
    budget.work(3, AT)?;
    if let Some(value) = original.as_rational() {
        if value.is_negative() {
            return Ok(false);
        }
        return Ok(value.mul(value, budget)?.compare(radicand, budget)? == Ordering::Equal);
    }
    let Some((cas, root)) = original.graph() else {
        return Ok(false);
    };
    let term = cas.state.expression(cas.checked(root)?)?;
    if term.operator != Operator::GeneralPower(Branch::StrictReal) {
        return Ok(false);
    }
    let [base, exponent] = term.operands.as_slice() else {
        return Ok(false);
    };
    let Operator::Rational(base) = &cas.state.expression(*base)?.operator else {
        return Ok(false);
    };
    let Operator::Rational(exponent) = &cas.state.expression(*exponent)?.operator else {
        return Ok(false);
    };
    let half = Rational::from_ratio(1, 2, budget)?;
    let same_exponent = exponent.compare(&half, budget)? == Ordering::Equal;
    let same_radicand = base.compare(radicand, budget)? == Ordering::Equal;
    Ok(same_exponent && same_radicand)
}
