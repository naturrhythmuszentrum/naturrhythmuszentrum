// SPDX-License-Identifier: AGPL-3.0-only
//! Classical point derivatives require actual ownership and both finite incident germs.

use crate::boundary_check::{premise, rational};
use crate::boundary_model::NonexistenceReason;
use crate::budget::Budget;
use crate::case_syntax::invalid;
use crate::domain_model::Side;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::proof_model::{ApplicationDraft, CheckedApplication, Ownership, Statement};

/// Check continuity first; equality of branch slopes cannot repair a jump in the function.
pub(crate) fn check(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let (function, boundary, coordinate, order) = claim(&draft.conclusion)?;
    let offset = usize::from(order > 1);
    if premises.len() != offset + 3 || order == 0 {
        return Err(invalid());
    }
    let current = crate::case_calculus::current(function, coordinate, order, premises)?;
    let Statement::Ownership {
        function: selected_function,
        boundary: selected_boundary,
        value: Ownership::Present(value),
    } = premise(premises, offset)?
    else {
        return Err(invalid());
    };
    if selected_function != current || selected_boundary != boundary {
        return Err(invalid());
    }
    let below = jet(
        premise(premises, offset + 1)?,
        current,
        boundary,
        Side::Below,
    )?;
    let above = jet(
        premise(premises, offset + 2)?,
        current,
        boundary,
        Side::Above,
    )?;
    if order > 1 {
        let point = crate::case_select::singleton(
            cas,
            draft.context.logical_id(),
            boundary.logical_id(),
            budget,
        )?;
        let point = cas.handle(point);
        for region in [below.0, above.0, &point] {
            crate::case_calculus::prefix_domain(cas, premises, region, budget)?;
        }
    }
    let reason = obstruction(cas, value, below, above)?;
    match (&draft.conclusion, reason) {
        (Statement::DerivativeAt(result), None)
            if rational(cas, &result.value)? == rational(cas, below.2)? =>
        {
            Ok(())
        }
        (Statement::DerivativeDoesNotExist(result), Some(expected))
            if result.reason == expected =>
        {
            Ok(())
        }
        _ => Err(invalid()),
    }
}

/// Continuity is required before equal slopes can establish a classical derivative.
fn obstruction(
    cas: &Cas,
    value: &Handle,
    below: (&Handle, &Handle, &Handle),
    above: (&Handle, &Handle, &Handle),
) -> Result<Option<NonexistenceReason>, Fault> {
    let actual = rational(cas, value)?;
    Ok(if actual != rational(cas, below.1)? {
        Some(NonexistenceReason::Discontinuous(Side::Below))
    } else if actual != rational(cas, above.1)? {
        Some(NonexistenceReason::Discontinuous(Side::Above))
    } else if rational(cas, below.2)? != rational(cas, above.2)? {
        Some(NonexistenceReason::UnequalSlopes)
    } else {
        None
    })
}

/// Borrow the precise requested proposition, including its iterated derivative order.
fn claim(statement: &Statement) -> Result<(&Handle, &Handle, &str, u32), Fault> {
    match statement {
        Statement::DerivativeAt(value) => Ok((
            &value.function,
            &value.boundary,
            &value.coordinate,
            value.order,
        )),
        Statement::DerivativeDoesNotExist(value) => Ok((
            &value.function,
            &value.boundary,
            &value.coordinate,
            value.order,
        )),
        _ => Err(invalid()),
    }
}

/// Require a function-specific, correctly oriented jet containing both value and slope.
fn jet<'a>(
    statement: &'a Statement,
    current: &Handle,
    point: &Handle,
    side: Side,
) -> Result<(&'a Handle, &'a Handle, &'a Handle), Fault> {
    let Statement::Jet {
        function,
        region,
        boundary,
        side: orientation,
        values,
    } = statement
    else {
        return Err(invalid());
    };
    if function != current || boundary != point || *orientation != side {
        return Err(invalid());
    }
    Ok((
        region,
        values.first().ok_or_else(invalid)?,
        values.get(1).ok_or_else(invalid)?,
    ))
}
