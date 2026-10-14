// SPDX-License-Identifier: AGPL-3.0-only
//! Independent finite polynomial jets, projections, and exact oriented jumps.

use crate::boundary_check::{equality, premise, rational};
use crate::boundary_model::BoundaryCertificate;
use crate::budget::{Budget, BudgetKind};
use crate::case_syntax::invalid;
use crate::domain_model::{AmbientDomain, Side};
use crate::error::{Fault, Location};
use crate::graph::{Cas, Handle, size};
use crate::number::Rational;
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, DomainContract, Statement,
};

/// Jet resource accounting location.
const AT: Location = Location::new("polynomial boundary jet");

/// Recompute every derivative trace by coefficient shifting, independently of the producer.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::Jet {
        function,
        region,
        boundary,
        side,
        values,
    } = &draft.conclusion
    else {
        return Err(invalid());
    };
    let Certificate::Boundary(mode) = &draft.certificate else {
        return Err(invalid());
    };
    let BoundaryCertificate::Polynomial {
        polynomial,
        coefficients,
    } = mode.as_ref()
    else {
        return Err(invalid());
    };
    measure(values.len(), budget)?;
    if premises.len() != 2 {
        return Err(invalid());
    }
    equality(premise(premises, 0)?, function, polynomial, region)?;
    let incidence = Statement::Incidence {
        region: region.clone(),
        boundary: boundary.clone(),
        side: *side,
        neighborhood: region.clone(),
    };
    if *premise(premises, 1)? != incidence {
        return Err(invalid());
    }
    let location = cas.state.boundary(cas.checked(boundary)?)?;
    let AmbientDomain::Real(coordinate) = &location.ambient else {
        return Err(invalid());
    };
    crate::boundary_roles::point(cas, function, boundary, coordinate)?;
    crate::boundary_roles::region(cas, function, region, coordinate)?;
    let actual =
        crate::polynomial::univariate(&cas.state, cas.checked(polynomial)?, coordinate, budget)?;
    crate::univariate::validate(coefficients, budget)?;
    if actual != *coefficients {
        return Err(invalid());
    }
    let point = crate::region::boundary_point(&cas.state, boundary.logical_id(), budget)?;
    for (order, value) in values.iter().enumerate() {
        if *rational(cas, value)? != trace(coefficients, &point, order, budget)? {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Bound order and cumulative entries before any coefficient or output allocation.
pub(crate) fn measure(count: usize, budget: &mut Budget) -> Result<(), Fault> {
    let order = count.checked_sub(1).ok_or_else(invalid)?;
    budget.specialized(BudgetKind::JetOrder, size(order, AT)?, false, AT)?;
    budget.specialized(BudgetKind::JetEntries, size(count, AT)?, true, AT)
}

/// Sum falling-factorial coefficients times nonnegative powers, including the constant at zero.
fn trace(
    coefficients: &[Rational],
    point: &Rational,
    order: usize,
    budget: &mut Budget,
) -> Result<Rational, Fault> {
    budget.check(BudgetKind::JetOrder, size(order, AT)?, AT)?;
    let mut result = Rational::zero();
    let mut power = Rational::one();
    for (degree, coefficient) in coefficients.iter().enumerate().skip(order) {
        let mut term = coefficient.mul(&power, budget)?;
        for offset in 0..order {
            let factor = i64::try_from(degree.checked_sub(offset).ok_or_else(invalid)?)
                .map_err(|_| invalid())?;
            term = term.mul(&Rational::from_ratio(factor, 1, budget)?, budget)?;
        }
        result = result.add(&term, budget)?;
        power = power.mul(point, budget)?;
    }
    Ok(result)
}

/// Project exactly one entry without changing the function, point, side or order.
pub(crate) fn projection(
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
) -> Result<(), Fault> {
    let Statement::TraceValue(claim) = &draft.conclusion else {
        return Err(invalid());
    };
    let Statement::Jet {
        function,
        boundary,
        side,
        values,
        ..
    } = premise(premises, 0)?
    else {
        return Err(invalid());
    };
    let index = usize::try_from(claim.order).map_err(|_| invalid())?;
    if premises.len() != 1
        || claim.function != *function
        || claim.boundary != *boundary
        || claim.side != *side
        || values.get(index) != Some(&claim.value)
    {
        return Err(invalid());
    }
    Ok(())
}

/// A jump consumes both matching projections and a checked right-minus-left calculation.
pub(crate) fn jump(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::Jump {
        function,
        boundary,
        order,
        value,
    } = &draft.conclusion
    else {
        return Err(invalid());
    };
    if premises.len() != 3
        || premises.get(2).ok_or_else(invalid)?.rule != crate::rule::Primitive::Exact
    {
        return Err(invalid());
    }
    let mut traces = Vec::new();
    for (index, side) in [Side::Below, Side::Above].iter().enumerate() {
        let Statement::TraceValue(trace) = premise(premises, index)? else {
            return Err(invalid());
        };
        if trace.function != *function
            || trace.boundary != *boundary
            || trace.order != *order
            || trace.side != *side
        {
            return Err(invalid());
        }
        traces.push(&trace.value);
    }
    subtraction(cas, premise(premises, 2)?, &traces, value)?;
    let [left, right] = traces.as_slice() else {
        return Err(invalid());
    };
    if *rational(cas, value)? != rational(cas, right)?.sub(rational(cas, left)?, budget)? {
        return Err(invalid());
    }
    Ok(())
}

/// Check the exact subtraction syntax as well as its independently verified equality.
fn subtraction(
    cas: &Cas,
    statement: &Statement,
    traces: &[&Handle],
    value: &Handle,
) -> Result<(), Fault> {
    let Statement::Equality {
        left,
        right,
        domain_contract: DomainContract::DefinedAndEqual,
        ..
    } = statement
    else {
        return Err(invalid());
    };
    let expression = cas.state.expression(cas.checked(left)?)?;
    let [below, above] = traces else {
        return Err(invalid());
    };
    if *right != *value
        || expression.operator != crate::model::Operator::Sub
        || expression.operands != [above.logical_id(), below.logical_id()]
    {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{measure, trace};
    use crate::budget::{Budget, Limits};
    use crate::number::Rational;

    /// Exact derivatives of x cubed minus twice x plus five at two are hand calculated.
    /// ´test:unit:polynomial-boundary-coefficient-traces´
    #[test]
    fn polynomial_boundary_coefficient_traces() -> Result<(), Box<dyn std::error::Error>> {
        let mut budget = Budget::new(Limits::default())?;
        let mut coefficients = Vec::new();
        for value in [5, -2, 0, 1] {
            coefficients.push(Rational::from_ratio(value, 1, &mut budget)?);
        }
        let point = Rational::from_ratio(2, 1, &mut budget)?;
        for (order, expected) in [9, 10, 12, 6, 0].iter().enumerate() {
            assert_eq!(
                trace(&coefficients, &point, order, &mut budget)?,
                Rational::from_ratio(*expected, 1, &mut budget)?
            );
        }
        assert_eq!(
            trace(&coefficients, &Rational::zero(), 0, &mut budget)?,
            Rational::from_ratio(5, 1, &mut budget)?
        );
        assert_eq!(trace(&[], &point, 3, &mut budget)?, Rational::zero());
        Ok(())
    }

    /// Jet order and cumulative entries are refused before the corresponding allocation.
    /// ´test:unit:boundary-jet-budget-preflight´
    #[test]
    fn boundary_jet_budget_preflight() -> Result<(), Box<dyn std::error::Error>> {
        let mut budget = Budget::new(Limits {
            jet_order: 1,
            jet_entries: 3,
            ..Limits::default()
        })?;
        assert!(trace(&[], &Rational::zero(), 2, &mut budget).is_err());
        assert!(measure(0, &mut budget).is_err());
        assert!(measure(3, &mut budget).is_err());
        measure(2, &mut budget)?;
        assert!(measure(2, &mut budget).is_err());
        Ok(())
    }
}
