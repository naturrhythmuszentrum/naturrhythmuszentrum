// SPDX-License-Identifier: AGPL-3.0-only
//! Limit restriction and side-series schemas consume independently checked jets and traces.

use crate::boundary_check::{premise, rational};
use crate::boundary_model::BoundaryCertificate;
use crate::budget::{Budget, BudgetKind};
use crate::case_syntax::invalid;
use crate::domain_model::Side;
use crate::error::{Fault, InvalidCode, Location};
use crate::germ_model::{GermMode, GermValue};
use crate::graph::{Cas, size};
use crate::number::Rational;
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, Ownership, ScopedConditions, Statement,
};
use crate::rule::Primitive;

/// Location shared by analytic disagreement and resource evidence.
pub(crate) const AT: Location = Location::new("case germ restriction");

/// Required sides are explicit; analytic mode always requires the full neighborhood.
pub(crate) fn sides(value: &GermValue) -> Vec<Side> {
    if matches!(value.mode, GermMode::Series { analytic: true, .. }) || value.at.side.is_none() {
        vec![Side::Below, Side::Above]
    } else {
        value.at.side.into_iter().collect()
    }
}

/// Validate roles and coefficient counts before copying the statement.
pub(crate) fn validate(cas: &Cas, value: &GermValue, budget: &mut Budget) -> Result<(), Fault> {
    crate::boundary_roles::point(cas, &value.function, &value.at.point, &value.at.coordinate)?;
    let order = match value.mode {
        GermMode::Limit => 0,
        GermMode::Series { order, .. } => order,
    };
    budget.specialized(BudgetKind::JetOrder, u64::from(order), false, AT)?;
    let expected = u64::from(order).checked_add(1).ok_or_else(invalid)?;
    if size(value.coefficients.len(), AT)? != expected {
        return Err(invalid());
    }
    budget.check(BudgetKind::JetEntries, expected, AT)?;
    for number in &value.coefficients {
        number.check(budget, AT)?;
    }
    Ok(())
}

/// Check only exact rule/mode combinations and preserve every consumed condition.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let Statement::Germ(value) = &draft.conclusion else {
        return Err(invalid());
    };
    validate(cas, value, budget)?;
    if !draft.substitutions.is_empty() {
        return Err(invalid());
    }
    match (draft.rule, value.mode) {
        (Primitive::LimitCaseRestrict, GermMode::Limit) => {
            limit(cas, value, &draft.certificate, premises)?;
        }
        (Primitive::SeriesCaseRestrict, GermMode::Series { .. }) => {
            series(cas, value, &draft.certificate, premises, budget)?;
        }
        _ => return Err(invalid()),
    }
    let mut result = ScopedConditions::default();
    for previous in premises {
        if previous.judgment.context != draft.context {
            return Err(invalid());
        }
        crate::statement::merge(cas, &mut result, &previous.conditions, budget)?;
    }
    Ok(result)
}

/// Trace projections already retain nonempty punctured incidence and polynomial limit evidence.
fn limit(
    cas: &Cas,
    value: &GermValue,
    certificate: &Certificate,
    premises: &[CheckedApplication],
) -> Result<(), Fault> {
    let approaches = sides(value);
    if *certificate != Certificate::None || premises.len() != approaches.len() {
        return Err(invalid());
    }
    for (index, side) in approaches.iter().enumerate() {
        let Statement::TraceValue(trace) = premise(premises, index)? else {
            return Err(invalid());
        };
        if trace.function != value.function
            || trace.boundary != value.at.point
            || trace.side != *side
            || trace.order != 0
            || value.coefficients.first() != Some(rational(cas, &trace.value)?)
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Matching jets alone are supplemented with an exact polynomial remainder on each side.
fn series(
    cas: &Cas,
    value: &GermValue,
    certificate: &Certificate,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Certificate::Germ(evidence) = certificate else {
        return Err(invalid());
    };
    let approaches = sides(value);
    let owned = approaches.len() == 2;
    if premises.len() != approaches.len() + usize::from(owned)
        || evidence.remainders.len() != approaches.len()
    {
        return Err(invalid());
    }
    let centre = crate::region::boundary_point(&cas.state, cas.checked(&value.at.point)?, budget)?;
    for (index, side) in approaches.iter().enumerate() {
        let previous = premises.get(index).ok_or_else(invalid)?;
        jet(cas, value, *side, previous, budget)?;
        let Certificate::Boundary(mode) = &previous.certificate else {
            return Err(invalid());
        };
        let BoundaryCertificate::Polynomial { coefficients, .. } = mode.as_ref() else {
            return Err(invalid());
        };
        crate::germ_polynomial::remainder(
            coefficients,
            &centre,
            &value.coefficients,
            evidence.remainders.get(index).ok_or_else(invalid)?,
            budget,
        )?;
    }
    if matches!(value.mode, GermMode::Series { analytic: true, .. })
        && evidence.remainders.first() != evidence.remainders.get(1)
    {
        return Err(invalid());
    }
    if owned {
        ownership(cas, value, premise(premises, approaches.len())?)?;
    }
    Ok(())
}

/// Exact factorial conversion ties each requested coefficient to the function-specific jet.
fn jet(
    cas: &Cas,
    value: &GermValue,
    approach: Side,
    previous: &CheckedApplication,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::Jet {
        function,
        boundary,
        side,
        values,
        ..
    } = &previous.judgment.statement
    else {
        return Err(invalid());
    };
    if *function != value.function
        || *boundary != value.at.point
        || *side != approach
        || values.len() != value.coefficients.len()
    {
        return Err(invalid());
    }
    let mut factorial = Rational::one();
    for (index, (actual, proposed)) in values.iter().zip(&value.coefficients).enumerate() {
        if index > 0 {
            factorial = factorial.mul(
                &Rational::from_ratio(i64::try_from(index).map_err(|_| invalid())?, 1, budget)?,
                budget,
            )?;
        }
        if *rational(cas, actual)? != proposed.mul(&factorial, budget)? {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Ordinary Taylor and analytic claims include the actual centre, unlike punctured side claims.
fn ownership(cas: &Cas, value: &GermValue, previous: &Statement) -> Result<(), Fault> {
    let Statement::Ownership {
        function,
        boundary,
        value: Ownership::Present(actual),
    } = previous
    else {
        return Err(invalid());
    };
    if *function != value.function
        || *boundary != value.at.point
        || value.coefficients.first() != Some(rational(cas, actual)?)
    {
        return Err(invalid());
    }
    Ok(())
}

/// A disagreement is a named refusal of this requested gluing, never a successful common limit.
pub(crate) const fn disagreement() -> Fault {
    Fault::invalid(InvalidCode::AnalyticDisagreement, AT)
}
