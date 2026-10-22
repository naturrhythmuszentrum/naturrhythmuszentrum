// SPDX-License-Identifier: AGPL-3.0-only
//! Exact finite-endpoint and infinity signs reconstructed from earlier sign theorems.

use super::sign::{mask, negate};
use super::{AT, invalid};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::Handle;
use crate::proof_model::Statement;
use crate::solving::coefficients::storage;
use crate::solving::elementary::{evaluate::Evaluation, proof::premise, value::Value};
use crate::solving::{CoefficientMapData, RealEndpoint, SolveStatement};

/// A cursor consumes every required endpoint/infinity entry, rejecting both omission and surplus.
pub(super) struct Signs<'a> {
    /// Actual same-Real evaluation of the root relation's coefficient map.
    pub(super) evaluation: Evaluation<'a>,
    /// The relation's independently checked scalar map.
    pub(super) map: &'a CoefficientMapData<Handle>,
    /// Ordered earlier-premise slots, never producer sign bytes.
    pub(super) slots: &'a [u32],
    /// Next mandatory slot.
    pub(super) cursor: usize,
}

impl Signs<'_> {
    /// Exact arithmetic compares the supplied sign's subject with the reconstructed target value.
    pub(super) fn next(&mut self, expected: &Value, budget: &mut Budget) -> Result<u8, Fault> {
        budget.solving(BudgetKind::IsolationSteps, 1, AT)?;
        let slot = *self.slots.get(self.cursor).ok_or_else(invalid)?;
        self.cursor = self.cursor.checked_add(1).ok_or_else(invalid)?;
        let Statement::Solve(statement) =
            &premise(self.evaluation.premises, slot)?.judgment.statement
        else {
            return Err(invalid());
        };
        let SolveStatement::RealSign(claim) = statement.as_ref() else {
            return Err(invalid());
        };
        if claim.region != *self.evaluation.region {
            return Err(invalid());
        }
        let actual = self
            .evaluation
            .expression(claim.subject.logical_id(), true, budget)?;
        self.evaluation.zero(
            &actual.sub(expected, &self.evaluation.engine, budget)?,
            budget,
        )?;
        Ok(mask(claim.conclusion))
    }

    /// Each leading sign is reconstructed twice, retaining correlation between both infinities.
    pub(super) fn infinities(
        &mut self,
        chain: &[Handle],
        budget: &mut Budget,
    ) -> Result<Vec<(u32, u32)>, Fault> {
        let first = self.leading(chain, budget)?;
        let second = self.leading(chain, budget)?;
        let mut alternatives = storage::vector(1, budget)?;
        alternatives.push(Vec::new());
        for (left, right) in first.iter().zip(&second) {
            let possible = left & right;
            if possible & 2 != 0 || possible == 0 {
                return Err(invalid());
            }
            let mut next = storage::vector(0, budget)?;
            for signs in alternatives {
                for sign in [1, 4] {
                    if possible & sign == 0 {
                        continue;
                    }
                    let mut extended = storage::copied(&signs, budget)?;
                    storage::reserve(&mut extended, 1, budget)?;
                    extended.push(sign);
                    storage::reserve(&mut next, 1, budget)?;
                    next.push(extended);
                }
            }
            alternatives = next;
        }
        let mut result = storage::vector(alternatives.len(), budget)?;
        for signs in alternatives {
            let positive = variations(&signs, budget)?;
            let mut negative = storage::copied(&signs, budget)?;
            for (sign, polynomial) in negative.iter_mut().zip(chain) {
                let stored = storage::polynomial(&self.evaluation.cas.state, polynomial)?;
                if !stored
                    .terms
                    .first()
                    .ok_or_else(invalid)?
                    .exponents
                    .first()
                    .ok_or_else(invalid)?
                    .is_multiple_of(2)
                {
                    *sign = negate(*sign);
                }
            }
            result.push((variations(&negative, budget)?, positive));
        }
        Ok(result)
    }

    /// A nonzero leading sign may be weak only when every retained possibility verifies the same counts.
    fn leading(&mut self, chain: &[Handle], budget: &mut Budget) -> Result<Vec<u8>, Fault> {
        let mut signs = storage::vector(chain.len(), budget)?;
        for handle in chain {
            let polynomial = storage::polynomial(&self.evaluation.cas.state, handle)?;
            let term = polynomial.terms.first().ok_or_else(invalid)?;
            let value = self
                .evaluation
                .mapped(&term.coefficient, self.map, budget)?;
            let sign = self.next(&value, budget)?;
            if !matches!(sign, 1 | 4 | 5) {
                return Err(invalid());
            }
            signs.push(sign);
        }
        Ok(signs)
    }

    /// Evaluate the kth derivative directly by falling factorials in the original polynomial support.
    fn derivative_value(
        &self,
        polynomial: &Handle,
        endpoint: &Value,
        order: u32,
        budget: &mut Budget,
    ) -> Result<Value, Fault> {
        let polynomial = storage::polynomial(&self.evaluation.cas.state, polynomial)?;
        let mut result = Value::integer(0, &self.evaluation.engine, budget)?;
        for term in &polynomial.terms {
            let exponent = *term.exponents.first().ok_or_else(invalid)?;
            if exponent < order {
                continue;
            }
            let mut value = self
                .evaluation
                .mapped(&term.coefficient, self.map, budget)?;
            for decrement in 0..order {
                let factor = Value::integer(
                    i64::from(exponent - decrement),
                    &self.evaluation.engine,
                    budget,
                )?;
                value = value.mul(&factor, &self.evaluation.engine, budget)?;
            }
            let power = endpoint.pow(exponent - order, &self.evaluation.engine, budget)?;
            result = result.add(
                &value.mul(&power, &self.evaluation.engine, budget)?,
                &self.evaluation.engine,
                budget,
            )?;
        }
        Ok(result)
    }

    /// Vanishing endpoint values use the first nonzero derivative for the exact one-sided sign.
    fn side(
        &mut self,
        polynomial: &Handle,
        endpoint: &Value,
        budget: &mut Budget,
    ) -> Result<(u8, u8, bool), Fault> {
        let stored = storage::polynomial(&self.evaluation.cas.state, polynomial)?;
        let degree = *stored
            .terms
            .first()
            .ok_or_else(invalid)?
            .exponents
            .first()
            .ok_or_else(invalid)?;
        for order in 0..=degree {
            let value = self.derivative_value(polynomial, endpoint, order, budget)?;
            let sign = self.next(&value, budget)?;
            if sign == 2 {
                continue;
            }
            if !matches!(sign, 1 | 4) {
                return Err(invalid());
            }
            let left = if order.is_multiple_of(2) {
                sign
            } else {
                negate(sign)
            };
            return Ok((left, sign, order > 0));
        }
        Err(invalid())
    }

    /// Both one-sided variation counts and root status are reconstructed for a finite endpoint.
    pub(super) fn endpoint(
        &mut self,
        chain: &[Handle],
        endpoint: &RealEndpoint<Handle>,
        infinities: (u32, u32),
        budget: &mut Budget,
    ) -> Result<(u32, u32, bool), Fault> {
        let handle = match endpoint {
            RealEndpoint::NegativeInfinity => return Ok((infinities.0, infinities.0, false)),
            RealEndpoint::PositiveInfinity => return Ok((infinities.1, infinities.1, false)),
            RealEndpoint::RealExpression(handle) => handle,
        };
        let endpoint = self
            .evaluation
            .expression(handle.logical_id(), true, budget)?;
        let mut below = storage::vector(chain.len(), budget)?;
        let mut above = storage::vector(chain.len(), budget)?;
        let mut root = false;
        for (index, polynomial) in chain.iter().enumerate() {
            let (left, right, zero) = self.side(polynomial, &endpoint, budget)?;
            below.push(left);
            above.push(right);
            if index == 0 {
                root = zero;
            }
        }
        Ok((
            variations(&below, budget)?,
            variations(&above, budget)?,
            root,
        ))
    }

    /// Finite endpoint order is an independently proved strict sign of right minus left.
    pub(super) fn ordered(
        &mut self,
        left: &RealEndpoint<Handle>,
        right: &RealEndpoint<Handle>,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        if let (RealEndpoint::RealExpression(left), RealEndpoint::RealExpression(right)) =
            (left, right)
        {
            if left == right {
                return Ok(());
            }
            let difference = self
                .evaluation
                .expression(right.logical_id(), true, budget)?
                .sub(
                    &self
                        .evaluation
                        .expression(left.logical_id(), true, budget)?,
                    &self.evaluation.engine,
                    budget,
                )?;
            if self.next(&difference, budget)? != 4 {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

/// Sturm variation deletes zero entries; all remaining entries must be exact strict signs.
pub(super) fn variations(signs: &[u8], budget: &mut Budget) -> Result<u32, Fault> {
    let mut previous = None;
    let mut result = 0_u32;
    for sign in signs {
        budget.work(1, AT)?;
        if *sign == 2 {
            continue;
        }
        if !matches!(sign, 1 | 4) {
            return Err(invalid());
        }
        if previous.is_some_and(|value| value != *sign) {
            result = result.checked_add(1).ok_or_else(invalid)?;
        }
        previous = Some(*sign);
    }
    Ok(result)
}
