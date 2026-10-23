// SPDX-License-Identifier: AGPL-3.0-only
//! Only exact ancestor strict guards can supply a leaf sign.

use super::{AT, unavailable};
use crate::budget::Budget;
use crate::domain_model::RegionExpr;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::{Operator, PredicateKind};
use crate::solving::RealSignConclusion as Sign;
use crate::solving::coefficients::storage::{reserve, vector};

/// Follow only ancestor guards and meets, retaining their exact subjects.
pub(super) fn sign(
    cas: &Cas,
    subject: &Handle,
    region: &Handle,
    budget: &mut Budget,
) -> Result<Sign, Fault> {
    let mut pending = vector(1, budget)?;
    pending.push(cas.checked(region)?);
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        match &cas.state.region(id)?.expression {
            RegionExpr::GuardTrue { base, predicate } => {
                let (kind, arguments) =
                    crate::solving::elementary::evidence::predicate(cas, *predicate)?;
                if let (PredicateKind::Positive, [argument]) = (kind, arguments) {
                    if *argument == subject.logical_id() {
                        return Ok(Sign::Positive);
                    }
                    let expression = cas.state.expression(*argument)?;
                    if expression.operator == Operator::Neg
                        && expression.operands == [subject.logical_id()]
                    {
                        return Ok(Sign::Negative);
                    }
                }
                reserve(&mut pending, 1, budget)?;
                pending.push(*base);
            }
            RegionExpr::GuardFalse { base, .. } => {
                reserve(&mut pending, 1, budget)?;
                pending.push(*base);
            }
            RegionExpr::Meet(members) => {
                reserve(&mut pending, members.len(), budget)?;
                pending.extend(members);
            }
            _ => {}
        }
    }
    Err(unavailable())
}
