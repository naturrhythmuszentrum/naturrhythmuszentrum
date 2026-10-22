// SPDX-License-Identifier: AGPL-3.0-only
//! Equal distribution derivatives on a connected open interval imply a constant difference.
//!
//! # Primitive uniqueness
//!
//! `Task::PrimitiveUnique { space }` checks equal first derivatives on a connected open interval. `DistributionStatement::PrimitiveUnique` asserts that the two distributions differ by some constant distribution; it does not select a constant or an initial condition. Step and step plus seven both differentiate to `δ_0`. The three primitive keys are `dist.affine-pullback`, `dist.simple-zero-pullback` and `dist.primitive-unique`, at semantic version 1 and logical weights 12, 12 and 8. The complete default policy is version 10 with 80 registered schemas: the existing 59 executable checkers and 21 registered solving schemas with separate availability.

use crate::budget::Budget;
use crate::case_syntax::invalid;
use crate::distribution_model::DistributionStatement;
use crate::distribution_producer::Runner;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::proof_model::{ApplicationDraft, Certificate, CheckedApplication, Statement};
use crate::rule::Primitive;

/// Check the connected-space theorem and the exact pair of equal first derivatives.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::Distribution(value) = &draft.conclusion else {
        return Err(invalid());
    };
    let DistributionStatement::PrimitiveUnique { left, right, space } = value.as_ref() else {
        return Err(invalid());
    };
    if draft.certificate != Certificate::None || !(2..=3).contains(&premises.len()) {
        return Err(invalid());
    }
    let domain = cas.state.space(cas.checked(space)?)?.domain;
    crate::region::check_open(&crate::region_cache::partition(cas, &[domain], budget)?)?;
    let first = derivative(left, space, premises.first().ok_or_else(invalid)?)?;
    let second = derivative(right, space, premises.get(1).ok_or_else(invalid)?)?;
    if premises.len() == 2 {
        if first != second {
            return Err(invalid());
        }
    } else {
        let expected = Statement::Distribution(Box::new(DistributionStatement::Equality {
            left: first.clone(),
            right: second.clone(),
            space: space.clone(),
        }));
        if premises.get(2).ok_or_else(invalid)?.judgment.statement != expected {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Derivative relations retain their original function, exact space and positive order.
fn derivative<'a>(
    input: &Handle,
    space: &Handle,
    premise: &'a CheckedApplication,
) -> Result<&'a Handle, Fault> {
    let Statement::Distribution(value) = &premise.judgment.statement else {
        return Err(invalid());
    };
    let DistributionStatement::Derivative {
        distribution,
        derivative,
        space: actual,
        order: 1,
    } = value.as_ref()
    else {
        return Err(invalid());
    };
    if distribution != input || actual != space {
        return Err(invalid());
    }
    Ok(derivative)
}

impl Runner<'_> {
    /// Produce both derivatives, then use the independent uniqueness schema; no constant is guessed.
    pub(crate) fn primitive_unique(
        &mut self,
        left: &Handle,
        right: &Handle,
    ) -> Result<(Handle, Handle), Fault> {
        self.ensure(left)?;
        self.ensure(right)?;
        let (first, left_proof) = self.differentiate(left, 1)?;
        let (second, right_proof) = self.differentiate(right, 1)?;
        let mut premises = vec![left_proof, right_proof];
        if first != second {
            let supplied = self.premises.clone();
            premises.push(self.push(
                Primitive::DistLinear,
                DistributionStatement::Equality {
                    left: first,
                    right: second,
                    space: self.space.clone(),
                },
                &supplied,
                Certificate::None,
            )?);
        }
        let proof = self.push(
            Primitive::DistPrimitiveUnique,
            DistributionStatement::PrimitiveUnique {
                left: left.clone(),
                right: right.clone(),
                space: self.space.clone(),
            },
            &premises,
            Certificate::None,
        )?;
        Ok((right.clone(), proof))
    }
}
