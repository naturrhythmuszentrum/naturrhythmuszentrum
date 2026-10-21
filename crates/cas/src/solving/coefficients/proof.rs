// SPDX-License-Identifier: AGPL-3.0-only
//! Scoped premises and independently checked definedness for coefficient replay.

use super::arithmetic::Engine;
use super::model::CoefficientFailure;
use super::storage::{AT, domain, invalid, polynomial};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::proof_model::{ApplicationDraft, CheckedApplication, ScopedConditions};
use crate::solving::{
    CoefficientDomainData, CoefficientMapData, CoefficientValueData, MapTarget, SolveDraft,
};

/// Resource failures keep their exact counter and location across checker boundaries.
pub(crate) const fn fault(error: &CoefficientFailure) -> Fault {
    match error {
        CoefficientFailure::Fault(fault) => *fault,
        CoefficientFailure::Unsupported(_) => invalid(),
    }
}

/// Preserve every scoped requirement and used hypothesis from the supplied earlier applications.
pub(crate) fn conditions(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    if !draft.substitutions.is_empty() {
        return Err(invalid());
    }
    let mut result = ScopedConditions::default();
    for premise in premises {
        budget.work(1, AT)?;
        if premise.judgment.context != draft.context {
            return Err(invalid());
        }
        crate::statement::merge(cas, &mut result, &premise.conditions, budget)?;
    }
    Ok(result)
}

/// A map's coefficient region must be exactly the claimed region in the full context.
pub(crate) fn mapping<'a>(
    cas: &'a Cas,
    handle: &Handle,
    context: &Handle,
    region: &Handle,
    budget: &mut Budget,
) -> Result<&'a CoefficientMapData<Handle>, Fault> {
    budget.work(1, AT)?;
    let SolveDraft::CoefficientMap(map) =
        crate::solving::graph::object(&cas.state, cas.checked(handle)?)?
    else {
        return Err(invalid());
    };
    let frame = cas.state.context(cas.checked(context)?)?;
    let coefficient_region = cas.state.region(cas.checked(region)?)?;
    if map.region != *region
        || frame.header_id != coefficient_region.header
        || frame.admissible != region.logical_id()
    {
        return Err(invalid());
    }
    Ok(map)
}

/// Admit only canonical ground-domain maps here; other maps require their specific relation checks.
pub(crate) fn ground_map(
    cas: &Cas,
    map: &CoefficientMapData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(1, AT)?;
    let source = domain(&cas.state, &map.source)?;
    match &map.target {
        MapTarget::CoefficientDomain { domain: target }
            if *target == map.source && map.images.is_empty() =>
        {
            Ok(())
        }
        MapTarget::CoefficientDomain { domain: target }
            if map.images.is_empty()
                && matches!(source, CoefficientDomainData::Rational)
                && matches!(domain(&cas.state, target)?, CoefficientDomainData::Gaussian) =>
        {
            Ok(())
        }
        MapTarget::ScalarAmbient { field, .. } if map.images.is_empty() => {
            if matches!(
                (source, field),
                (
                    CoefficientDomainData::Rational,
                    crate::model::Field::Rational
                        | crate::model::Field::Real
                        | crate::model::Field::Complex
                ) | (
                    CoefficientDomainData::Gaussian,
                    crate::model::Field::Complex
                )
            ) {
                Ok(())
            } else {
                Err(invalid())
            }
        }
        _ => Err(invalid()),
    }
}

/// Every actual denominator must be defined and nonzero on the identical coefficient region.
pub(crate) fn denominator(
    cas: &Cas,
    handle: &Handle,
    source: &Handle,
    region: &Handle,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let polynomial = super::read::sparse(&cas.state, handle, budget)?;
    if super::nonzero::proves(
        &cas.state,
        source,
        &polynomial,
        region,
        premises.iter().map(|premise| &premise.judgment.statement),
        budget,
    )? {
        return Ok(());
    }
    Err(invalid())
}

/// Walk only the referenced value closure, retaining denominators at every parent level.
pub(crate) fn defined(
    cas: &Cas,
    handle: &Handle,
    region: &Handle,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut pending = super::storage::vector(1, budget)?;
    pending.push(handle);
    while let Some(current) = pending.pop() {
        budget.work(1, AT)?;
        match crate::solving::graph::object(&cas.state, cas.checked(current)?)? {
            SolveDraft::Polynomial(_) => {
                let stored = polynomial(&cas.state, current)?;
                super::storage::reserve(&mut pending, stored.terms.len(), budget)?;
                pending.extend(stored.terms.iter().map(|term| &term.coefficient));
            }
            SolveDraft::CoefficientValue(stored) => match &stored.value {
                CoefficientValueData::Fraction {
                    denominator: divisor,
                    ..
                } => denominator(cas, divisor, &stored.domain, region, premises, budget)?,
                CoefficientValueData::Tower { coefficients } => {
                    let engine = Engine {
                        state: &cas.state,
                        domain: &stored.domain,
                    };
                    let (_, extension) = crate::solving::towers::arithmetic::level(&engine)
                        .map_err(|error| fault(&error))?;
                    super::storage::reserve(&mut pending, 1, budget)?;
                    pending.push(&extension.polynomial);
                    super::storage::reserve(&mut pending, coefficients.len(), budget)?;
                    pending.extend(coefficients);
                }
                CoefficientValueData::Rational { .. } | CoefficientValueData::Gaussian { .. } => {}
            },
            _ => return Err(invalid()),
        }
    }
    Ok(())
}

/// Identity source maps are formal homomorphisms; specialization with images needs evaluation.
pub(crate) fn identity_map(
    cas: &Cas,
    map: &CoefficientMapData<Handle>,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    super::homomorphism::check(cas, map, premises, budget)
}

/// Endpoint definedness is checked before an identity uses an exact regional zero coefficient.
pub(crate) fn zero(
    interpreter: &super::normalize::Interpretation<'_>,
    value: &super::model::Element,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    interpreter
        .engine
        .zero_on(value, interpreter.region, budget)
        .map_err(|error| fault(&error))
}

impl Engine<'_> {
    /// Recompute a zero identity on the exact region after endpoint definedness has been checked.
    pub(crate) fn zero_on(
        &self,
        value: &super::model::Element,
        region: &Handle,
        budget: &mut Budget,
    ) -> Result<bool, CoefficientFailure> {
        if self.formal_zero(value, budget)? {
            return Ok(true);
        }
        match &value.value {
            super::model::Value::Fraction(fraction) => Ok(super::regional::proves(
                self.state,
                self.domain,
                &fraction.numerator,
                region,
                true,
                budget,
            )?),
            super::model::Value::Tower(coefficients) => {
                let (_, extension) = crate::solving::towers::arithmetic::level(self)?;
                let parent = Self {
                    state: self.state,
                    domain: &extension.parent,
                };
                for coefficient in coefficients {
                    if !parent.zero_on(coefficient, region, budget)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            super::model::Value::Scalar(_) => Ok(false),
        }
    }

    /// Regional equality retains the same formal subtraction and requires defined endpoints.
    pub(crate) fn equal_on(
        &self,
        left: &super::model::Element,
        right: &super::model::Element,
        region: &Handle,
        budget: &mut Budget,
    ) -> Result<bool, CoefficientFailure> {
        super::storage::begin(budget)?;
        let difference = self.sub(left, right, budget)?;
        self.zero_on(&difference, region, budget)
    }
}
