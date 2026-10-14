// SPDX-License-Identifier: AGPL-3.0-only
//! Polynomial smooth multiplication and independently reconstructed signed Leibniz sums.

use crate::budget::{Budget, BudgetKind};
use crate::distribution_model::{DistributionStatement, RegularityKind};
use crate::distribution_syntax::{AT, invalid};
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::Operator;
use crate::number::Rational;
use crate::proof_model::{ApplicationDraft, Certificate, CheckedApplication, Statement};

/// Global polynomial smoothness and its test-product action are separate checked modes.
pub(crate) fn smooth(
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
    match value.as_ref() {
        DistributionStatement::Regularity {
            function,
            space,
            property: RegularityKind::Smooth,
        } => {
            if !premises.is_empty() {
                return Err(invalid());
            }
            crate::distribution_regular::polynomial(cas, function, space, budget)?;
            Ok(())
        }
        DistributionStatement::Pairing {
            pairing,
            value,
            space,
        } => {
            budget.specialized(BudgetKind::PairingSteps, 1, true, AT)?;
            crate::distribution_action::well_formed(cas, pairing, space, premises, budget)?;
            let pair = cas.state.expression(pairing.logical_id())?.clone();
            if pair.operator != Operator::Pair {
                return Err(invalid());
            }
            let [product, test] = pair.operands.as_slice() else {
                return Err(invalid());
            };
            let product = cas.state.expression(*product)?.clone();
            let [function, distribution] = product.operands.as_slice() else {
                return Err(invalid());
            };
            if product.operator != Operator::SmoothMultiply {
                return Err(invalid());
            }
            let header = cas.handle(pair.header);
            let factors = [cas.handle(*function), cas.handle(*test)];
            let test = crate::distribution_terms::node(
                cas,
                &header,
                space,
                Operator::TestMultiply,
                &factors,
                budget,
            )?;
            let distribution = cas.handle(*distribution);
            let action = crate::distribution_terms::node(
                cas,
                &header,
                space,
                Operator::Pair,
                &[distribution, test],
                budget,
            )?;
            crate::distribution_linear::equal_terms(cas, &action, value, &[], budget)
        }
        _ => Err(invalid()),
    }
}

/// The multiplier rule checks polynomial jets, signed binomials and every resulting delta order.
pub(crate) fn delta(
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
    let DistributionStatement::Equality { left, right, space } = value.as_ref() else {
        return Err(invalid());
    };
    crate::distribution_check::membership(cas, left.logical_id(), space, premises, budget)?;
    let expected = expand(cas, left, space, budget)?;
    crate::distribution_linear::equal_terms(cas, &expected, right, &[], budget)
}

/// Reconstruct the Leibniz sum from the multiplier itself, never from a stored jet summary.
pub(crate) fn expand(
    cas: &mut Cas,
    product: &Handle,
    space: &Handle,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let value = cas.state.expression(product.logical_id())?.clone();
    let [function, distribution] = value.operands.as_slice() else {
        return Err(invalid());
    };
    if value.operator != Operator::SmoothMultiply {
        return Err(invalid());
    }
    let delta = cas.state.expression(*distribution)?.clone();
    let Operator::Delta { order } = delta.operator else {
        return Err(invalid());
    };
    budget.specialized(BudgetKind::DeltaOrder, u64::from(order), false, AT)?;
    let location = cas.handle(*delta.operands.first().ok_or_else(invalid)?);
    let point = crate::distribution_check::rational(cas, location.logical_id())?.clone();
    let coefficients =
        crate::distribution_regular::polynomial(cas, &cas.handle(*function), space, budget)?;
    let header = cas.handle(value.header);
    let mut terms = crate::distribution_linear::Terms::new();
    let mut binomial = Rational::one();
    for index in 0..=order {
        budget.specialized(BudgetKind::PairingSteps, 1, true, AT)?;
        let mut coefficient = crate::jet_polynomial::trace(
            &coefficients,
            &point,
            usize::try_from(index).map_err(|_| invalid())?,
            budget,
        )?
        .mul(&binomial, budget)?;
        if index & 1 == 1 {
            coefficient = coefficient.negated(budget)?;
        }
        let term = crate::distribution_terms::node(
            cas,
            &header,
            space,
            Operator::Delta {
                order: order - index,
            },
            std::slice::from_ref(&location),
            budget,
        )?;
        let key = crate::wire_build::syntax_key(cas, term.logical_id(), budget)?;
        crate::distribution_linear::accumulate(&mut terms, key, Some(term), coefficient, budget)?;
        if index < order {
            binomial = binomial
                .mul(
                    &Rational::from_ratio(i64::from(order - index), 1, budget)?,
                    budget,
                )?
                .div(
                    &Rational::from_ratio(i64::from(index + 1), 1, budget)?,
                    budget,
                )?;
        }
    }
    crate::distribution_terms::linear(cas, &header, space, &terms, false, budget)
}
