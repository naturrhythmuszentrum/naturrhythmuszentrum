// SPDX-License-Identifier: AGPL-3.0-only
//! Full displayed products and strict sign facts contradict an exact original zero guard.

use super::{AT, EmptyFactorIdentity, EmptyLeaf, facts, invalid};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle, LogicalId};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::proof_model::{CheckedApplication, DomainContract, Statement};

/// A factor identity has the precise subject and complete displayed product in either orientation.
fn identity(
    cas: &Cas,
    region: &Handle,
    value: &EmptyFactorIdentity,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::Equality {
        left,
        right,
        domain,
        domain_contract: DomainContract::DefinedAndEqual,
    } = &facts::premise(premises, value.identity)?.judgment.statement
    else {
        return Err(invalid());
    };
    let forward = left == &value.subject && right == &value.product;
    let reverse = right == &value.subject && left == &value.product;
    if !(forward || reverse) || !facts::contained(cas, region, domain, budget)? {
        return Err(invalid());
    }
    Ok(())
}

/// Exact syntax reconstructs every ordered product factor; no polynomial search is invoked.
fn product(cas: &Cas, value: &EmptyFactorIdentity, budget: &mut Budget) -> Result<(), Fault> {
    let mut cursor = cas.checked(&value.product)?;
    if !matches!(
        cas.state.expression(cursor)?.sort,
        Sort::Value(AtomicSort::Scalar(
            Field::Rational | Field::Real | Field::Complex
        ))
    ) {
        return Err(invalid());
    }
    for factor in value.factors.iter().rev() {
        budget.work(1, AT)?;
        let expression = cas.state.expression(cursor)?;
        let (Operator::Mul, [left, right]) = (&expression.operator, expression.operands.as_slice())
        else {
            return Err(invalid());
        };
        power(cas, *right, &factor.subject, factor.multiplicity)?;
        cursor = *left;
    }
    if cursor != cas.checked(&value.scalar)? {
        return Err(invalid());
    }
    Ok(())
}

/// A positive literal multiplicity means a full factor power, never a dropped occurrence.
fn power(cas: &Cas, id: LogicalId, subject: &Handle, multiplicity: u32) -> Result<(), Fault> {
    if multiplicity == 0 {
        return Err(invalid());
    }
    let subject = cas.checked(subject)?;
    if multiplicity == 1 && id == subject {
        return Ok(());
    }
    let expression = cas.state.expression(id)?;
    if expression.operator == Operator::IntegerPower(i64::from(multiplicity))
        && expression.operands == [subject]
    {
        Ok(())
    } else {
        Err(invalid())
    }
}

/// Undefined values cannot enter through the defined equality or a true sign/zero predicate.
pub(super) fn check(
    cas: &Cas,
    region: &Handle,
    leaf: &EmptyLeaf,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    match leaf {
        EmptyLeaf::Sign { subject, nonzero } => {
            facts::zero(cas, region, subject, budget)?;
            facts::nonzero(cas, region, subject, nonzero, premises, budget)
        }
        EmptyLeaf::FactorIdentity(value) => {
            facts::zero(cas, region, &value.subject, budget)?;
            identity(cas, region, value, premises, budget)?;
            product(cas, value, budget)?;
            facts::nonzero(
                cas,
                region,
                &value.scalar,
                &value.scalar_nonzero,
                premises,
                budget,
            )?;
            for factor in &value.factors {
                facts::nonzero(
                    cas,
                    region,
                    &factor.subject,
                    &factor.nonzero,
                    premises,
                    budget,
                )?;
            }
            Ok(())
        }
    }
}
