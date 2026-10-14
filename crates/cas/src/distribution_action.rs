// SPDX-License-Identifier: AGPL-3.0-only
//! Delta action on ordinary tests and explicitly supported polynomial germs.

use crate::budget::{Budget, BudgetKind};
use crate::distribution_model::DistributionStatement;
use crate::distribution_syntax::{AT, invalid};
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::Operator;
use crate::proof_model::{ApplicationDraft, Certificate, CheckedApplication, Statement};

/// Reconstruct every pairing premise before evaluating any test or germ.
pub(crate) fn well_formed(
    cas: &Cas,
    pairing: &Handle,
    space: &Handle,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let value = cas.state.expression(pairing.logical_id())?.clone();
    let [distribution, function] = value.operands.as_slice() else {
        return Err(invalid());
    };
    crate::distribution_check::membership(cas, *distribution, space, premises, budget)?;
    match value.operator {
        Operator::Pair => crate::distribution_check::test(cas, *function, space, budget),
        Operator::PairNearSupport => {
            crate::distribution_check::measure_test(cas, *function, budget)?;
            crate::distribution_support::action_premise(*distribution, *function, space, premises)
        }
        _ => Err(invalid()),
    }
}

/// Delta evaluation checks its sign, exact order, space and interior location independently.
pub(crate) fn check(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    if draft.certificate != Certificate::None {
        return Err(invalid());
    }
    let Statement::Distribution(value) = &draft.conclusion else {
        return Err(invalid());
    };
    let DistributionStatement::Pairing {
        pairing,
        value,
        space,
    } = value.as_ref()
    else {
        return Err(invalid());
    };
    budget.specialized(BudgetKind::PairingSteps, 1, true, AT)?;
    well_formed(cas, pairing, space, premises, budget)?;
    let expected = delta_value(cas, pairing, space, budget)?;
    crate::distribution_linear::equal_terms(cas, &expected, value, &[], budget)
}

/// Evaluate by the derivative definition; a polynomial is used only in the supported mode.
pub(crate) fn delta_value(
    cas: &mut Cas,
    pairing: &Handle,
    space: &Handle,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let pair = cas.state.expression(pairing.logical_id())?.clone();
    let [distribution, function] = pair.operands.as_slice() else {
        return Err(invalid());
    };
    let delta = cas.state.expression(*distribution)?.clone();
    let Operator::Delta { order } = delta.operator else {
        return Err(invalid());
    };
    budget.specialized(BudgetKind::DeltaOrder, u64::from(order), false, AT)?;
    let location = cas.handle(*delta.operands.first().ok_or_else(invalid)?);
    let header = cas.handle(pair.header);
    if pair.operator == Operator::PairNearSupport {
        let coefficients =
            crate::distribution_regular::polynomial(cas, &cas.handle(*function), space, budget)?;
        let point = crate::distribution_check::rational(cas, location.logical_id())?;
        let mut value = crate::jet_polynomial::trace(
            &coefficients,
            point,
            usize::try_from(order).map_err(|_| invalid())?,
            budget,
        )?;
        if order & 1 == 1 {
            value = value.negated(budget)?;
        }
        return crate::distribution_terms::rational(cas, &header, &value, budget);
    }
    let mut test = cas.handle(*function);
    if order > 0 {
        test = crate::distribution_terms::node(
            cas,
            &header,
            space,
            Operator::TestDerivative { order },
            &[test],
            budget,
        )?;
    }
    let evaluation = crate::distribution_terms::node(
        cas,
        &header,
        space,
        Operator::TestApply,
        &[test, location],
        budget,
    )?;
    if order & 1 == 0 {
        Ok(evaluation)
    } else {
        crate::distribution_terms::node(cas, &header, space, Operator::Neg, &[evaluation], budget)
    }
}

/// Expand action over a structural rational map without evaluating a regular integral.
pub(crate) fn linear_value(
    cas: &mut Cas,
    pairing: &Handle,
    space: &Handle,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let pair = cas.state.expression(pairing.logical_id())?.clone();
    let [distribution, function] = pair.operands.as_slice() else {
        return Err(invalid());
    };
    let terms = crate::distribution_linear::terms(cas, *distribution, &[], budget)?;
    let header = cas.handle(pair.header);
    let function = cas.handle(*function);
    let mut output = crate::distribution_linear::Terms::new();
    for (term, coefficient) in terms.values() {
        let term = term.as_ref().ok_or_else(invalid)?;
        let action = crate::distribution_terms::node(
            cas,
            &header,
            space,
            pair.operator.clone(),
            &[term.clone(), function.clone()],
            budget,
        )?;
        let key = crate::wire_build::syntax_key(cas, action.logical_id(), budget)?;
        crate::distribution_linear::accumulate(
            &mut output,
            key,
            Some(action),
            coefficient.clone(),
            budget,
        )?;
    }
    crate::distribution_terms::linear(cas, &header, space, &output, true, budget)
}
