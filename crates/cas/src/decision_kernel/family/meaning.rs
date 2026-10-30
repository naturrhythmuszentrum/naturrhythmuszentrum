// SPDX-License-Identifier: AGPL-3.0-only
//! Original syntax, including every divisor and zero operand, must match the conditional trace.

use super::super::model::{ScalarKernelCertificate, ScalarKernelLaw};
use crate::budget::{Budget, BudgetKind};
use crate::graph::{Cas, LogicalId, size};
use crate::model::{Branch, Operator};
use crate::{ExactConstant, Fault, InvalidCode, Location, Rational};

/// A correct interval for a different symbolic recipe cannot certify the original.
const AT: Location = Location::new("scalar kernel family original");

/// The supported source-to-result correspondence is an exact ordered recipe.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::EndpointMismatch, AT)
}

/// Walk the original and the backward-only certificate together under a metered scratch ceiling.
pub(super) fn check(
    cas: &Cas,
    certificate: &ScalarKernelCertificate,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let last = certificate
        .records
        .len()
        .checked_sub(1)
        .ok_or_else(invalid)?;
    budget.work(32, AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, 32, AT)?;
    let mut pending = vec![(cas.checked(&certificate.expression)?, last)];
    while let Some((original, index)) = pending.pop() {
        budget.work(3, AT)?;
        let bytes = size(pending.len().saturating_add(3), AT)?.saturating_mul(32);
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        pending.try_reserve(2).map_err(|_| Fault::allocation(AT))?;
        budget.with_retained_payload(bytes, AT, |budget| {
            step(cas, certificate, original, index, &mut pending, budget)
        })?;
    }
    Ok(())
}

/// Literal leaves use the existing constant checker; open leaves require their exact source handle.
fn step(
    cas: &Cas,
    certificate: &ScalarKernelCertificate,
    original: LogicalId,
    index: usize,
    pending: &mut Vec<(LogicalId, usize)>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let record = certificate.records.get(index).ok_or_else(invalid)?;
    let expression = cas.state.expression(original)?;
    let child = |slot: u32| -> Result<usize, Fault> {
        let slot = usize::try_from(slot).map_err(|_| invalid())?;
        if slot >= index {
            return Err(invalid());
        }
        Ok(slot)
    };
    match &record.law {
        ScalarKernelLaw::Family(input) => {
            if cas.checked(&input.family.source)? != original {
                return Err(invalid());
            }
        }
        ScalarKernelLaw::Rational(value) => constant(cas, original, &value.clone().into(), budget)?,
        ScalarKernelLaw::Input(value) => constant(
            cas,
            original,
            &super::super::input::exact(value, budget)?,
            budget,
        )?,
        ScalarKernelLaw::Sum { left, right } => {
            let [a, b] = expression.operands.as_slice() else {
                return Err(invalid());
            };
            if expression.operator != Operator::Add {
                return Err(invalid());
            }
            pending.push((*a, child(*left)?));
            pending.push((*b, child(*right)?));
        }
        ScalarKernelLaw::Product { left, right } => {
            let [a, b] = expression.operands.as_slice() else {
                return Err(invalid());
            };
            let right = child(*right)?;
            let right = if expression.operator == Operator::Div {
                let ScalarKernelLaw::Reciprocal { input } = certificate.records[right].law else {
                    return Err(invalid());
                };
                let input = usize::try_from(input).map_err(|_| invalid())?;
                if input >= right {
                    return Err(invalid());
                }
                input
            } else if expression.operator == Operator::Mul {
                right
            } else {
                return Err(invalid());
            };
            pending.push((*a, child(*left)?));
            pending.push((*b, right));
        }
        ScalarKernelLaw::Reciprocal { input } => {
            let [numerator, denominator] = expression.operands.as_slice() else {
                return Err(invalid());
            };
            if expression.operator != Operator::Div {
                return Err(invalid());
            }
            constant(cas, *numerator, &ExactConstant::one(), budget)?;
            pending.push((*denominator, child(*input)?));
        }
        ScalarKernelLaw::SquareRoot { input } => {
            let [radicand, exponent] = expression.operands.as_slice() else {
                return Err(invalid());
            };
            if expression.operator != Operator::GeneralPower(Branch::StrictReal) {
                return Err(invalid());
            }
            let half = Rational::from_ratio(1, 2, budget)?;
            constant(cas, *exponent, &half.into(), budget)?;
            pending.push((*radicand, child(*input)?));
        }
    }
    Ok(())
}

/// Reconstruct every original closed operand before comparing exact meanings.
fn constant(
    cas: &Cas,
    original: LogicalId,
    expected: &ExactConstant,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let actual = ExactConstant::from_expression(cas, &cas.handle(original), budget)?;
    budget.work(
        actual
            .retained_payload_bytes()?
            .max(expected.retained_payload_bytes()?),
        AT,
    )?;
    if &actual == expected {
        Ok(())
    } else {
        Err(invalid())
    }
}
