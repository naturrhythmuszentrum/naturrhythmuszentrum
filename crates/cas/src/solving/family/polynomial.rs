// SPDX-License-Identifier: AGPL-3.0-only
//! Untrusted mixed polynomial discovery retains the original native case support.

use super::{AT, body, invalid, region};
use crate::budget::Budget;
use crate::graph::{Cas, Handle, LogicalId};
use crate::solving::coefficients::{arithmetic::Engine, model::CoefficientFailure};
use crate::solving::towers::dense::Dense;

/// Each discovered coefficient is read through the original base/fibre argument identity.
pub(crate) fn read(
    cas: &Cas,
    engine: &Engine<'_>,
    root: LogicalId,
    unknown: &str,
    domain: &Handle,
    budget: &mut Budget,
) -> Result<Dense, CoefficientFailure> {
    if let Some(value) = body(cas, root)? {
        if region::family(cas, domain.logical_id(), budget)? != value.family.logical_id()
            || !region::subset(cas, domain.logical_id(), value.region.logical_id(), budget)?
        {
            return Err(invalid().into());
        }
        let mut operands =
            crate::solving::coefficients::storage::vector(value.arguments.len(), budget)?;
        for argument in &value.arguments {
            budget.work(1, AT)?;
            operands.push(crate::solving::solve_search::read::expression(
                cas,
                engine,
                argument.value.logical_id(),
                unknown,
                domain,
                budget,
            )?);
        }
        return crate::solving::solve_search::read::operation(
            engine,
            &value.operator,
            &operands,
            unknown,
            domain,
            budget,
        );
    }
    let expression = cas.state.expression(root)?;
    let [id] = expression.arms.as_slice() else {
        return Err(invalid().into());
    };
    let arm = cas.state.arm(*id)?;
    if !region::subset(cas, domain.logical_id(), arm.region, budget)? {
        return Err(invalid().into());
    }
    read(cas, engine, arm.value, unknown, domain, budget)
}
