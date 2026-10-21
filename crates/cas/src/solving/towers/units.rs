// SPDX-License-Identifier: AGPL-3.0-only
//! Unit decisions in finite quotients, with explicit component-split evidence.

use super::dense::Dense;
use super::euclid::{Pending, gcd};
use crate::budget::{Budget, BudgetKind};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::model::{
    CoefficientCertificate, CoefficientFailure, Element, EvaluationScope, UnitDecision,
};
use crate::solving::coefficients::storage::{AT, begin, invalid};

/// Preserve the precise earlier-level obstruction when a parent inversion cannot continue.
fn pending(value: Pending) -> Result<UnitDecision<Element>, CoefficientFailure> {
    Ok(match value {
        Pending::ZeroDivisor => {
            return Err(CoefficientFailure::Unsupported(
                crate::solving::SolveReason::MissingUnitProof,
            ));
        }
        Pending::Split(certificate) => UnitDecision::Split(certificate),
        Pending::Guard(obligation) => UnitDecision::Undetermined(obligation),
    })
}

/// Nontrivial gcds split only when their two factors are coprime; repeated support is not reducedness.
fn split(
    engine: &Engine<'_>,
    divisor: Dense,
    modulus: &Dense,
    scope: &EvaluationScope<'_>,
    budget: &mut Budget,
) -> Result<UnitDecision<Element>, CoefficientFailure> {
    let (complement, remainder) =
        match super::euclid::divide(modulus, &divisor, engine.state, scope, budget)? {
            Ok(result) => result,
            Err(obstruction) => return pending(obstruction),
        };
    if !remainder.coefficients.is_empty() {
        return Err(invalid().into());
    }
    let coprime = match gcd(&divisor, &complement, engine.state, scope, budget)? {
        Ok(result) => result,
        Err(obstruction) => return pending(obstruction),
    };
    let parent = Engine {
        state: engine.state,
        domain: &modulus.domain,
    };
    if !coprime
        .gcd
        .equals(&Dense::one(&parent, budget)?, engine.state, budget)?
    {
        return Err(CoefficientFailure::Unsupported(
            crate::solving::SolveReason::ComponentChoicePending,
        ));
    }
    budget.solving(BudgetKind::ComponentSplits, 1, AT)?;
    budget.solving(BudgetKind::SolutionBranches, 2, AT)?;
    let (level, _) = super::arithmetic::level(engine)?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        crate::graph::size(std::mem::size_of::<super::SplitCertificate>(), AT)?,
        AT,
    )?;
    Ok(UnitDecision::Split(Box::new(super::SplitCertificate {
        level: level.clone(),
        left: divisor,
        right: complement,
        first: coprime.first,
        second: coprime.second,
    })))
}

/// Compute a unit, exact zero, or a proper component split without publishing any graph object.
pub(crate) fn inverse(
    engine: &Engine<'_>,
    element: &Element,
    scope: &EvaluationScope<'_>,
    budget: &mut Budget,
) -> Result<UnitDecision<Element>, CoefficientFailure> {
    begin(budget)?;
    let (_, extension) = super::arithmetic::level(engine)?;
    let value = super::arithmetic::dense(engine, element, budget)?;
    if value.coefficients.is_empty() {
        return Ok(UnitDecision::Zero(CoefficientCertificate::FormalZero));
    }
    let modulus = Dense::read(engine.state, &extension.polynomial, budget)?;
    let result = match gcd(&value, &modulus, engine.state, scope, budget)? {
        Ok(result) => result,
        Err(obstruction) => return pending(obstruction),
    };
    let parent = Engine {
        state: engine.state,
        domain: &extension.parent,
    };
    if result
        .gcd
        .equals(&Dense::one(&parent, budget)?, engine.state, budget)?
    {
        let certificate = if matches!(
            engine.state.region(scope.region.logical_id())?.expression,
            crate::domain_model::RegionExpr::Ambient
        ) {
            CoefficientCertificate::FormalIdentity
        } else {
            CoefficientCertificate::Regional(Vec::new())
        };
        return Ok(UnitDecision::Unit {
            inverse: super::arithmetic::reduced(engine, result.first, budget)?,
            certificate,
        });
    }
    if result.gcd.equals(&modulus, engine.state, budget)? {
        return Ok(UnitDecision::Zero(CoefficientCertificate::Regional(
            Vec::new(),
        )));
    }
    split(engine, result.gcd, &modulus, scope, budget)
}
