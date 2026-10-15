// SPDX-License-Identifier: AGPL-3.0-only
//! Scoped nonvanishing and finite specialization, separate from injective embeddings.

use super::arithmetic::Engine;
use super::model::{
    CoefficientCertificate, CoefficientFailure, Element, EvaluationScope, GuardedObligation,
    UnitDecision, Value, ZeroDecision,
};
use super::polynomial::Polynomial;
use super::scalar::Scalar;
use super::storage::{AT, begin, domain, invalid, vector};
use crate::budget::{Budget, BudgetKind};
use crate::graph::size;
use crate::proof_model::Statement;
use crate::solving::{
    CoefficientDomainData, CoefficientMapData, MapMode, MapTarget, SolveStatement,
};

impl Engine<'_> {
    /// Validate the exact context/region and finite localization before a regional decision.
    fn scope(
        &self,
        scope: &EvaluationScope<'_>,
        budget: &mut Budget,
    ) -> Result<(), CoefficientFailure> {
        begin(budget)?;
        let context = self.state.context(scope.context.logical_id())?;
        let region = self.state.region(scope.region.logical_id())?;
        if context.header_id != region.header || context.admissible != scope.region.logical_id() {
            return Err(invalid().into());
        }
        budget.solving(
            BudgetKind::DenominatorFactors,
            size(scope.localization.len(), AT)?,
            AT,
        )?;
        for witness in scope.premises {
            budget.work(1, AT)?;
            self.state.node(witness.proof().logical_id())?;
            if witness.judgment().context != *scope.context {
                return Err(invalid().into());
            }
        }
        Ok(())
    }

    /// Only an exact scoped, discharged unit theorem can supply nonvanishing for an imported value.
    fn unit_premise(
        &self,
        element: &Element,
        scope: &EvaluationScope<'_>,
        budget: &mut Budget,
    ) -> Result<Option<CoefficientCertificate>, CoefficientFailure> {
        let Some(origin) = &element.origin else {
            return Ok(None);
        };
        for witness in scope.premises {
            budget.work(1, AT)?;
            if !witness.is_discharged() {
                continue;
            }
            if let Statement::Solve(statement) = &witness.judgment().statement
                && let SolveStatement::Unit(unit) = statement.as_ref()
                && unit.value == *origin
                && unit.domain == *self.domain
                && unit.region == *scope.region
            {
                return Ok(Some(CoefficientCertificate::Premise(
                    witness.proof().clone(),
                )));
            }
        }
        Ok(None)
    }

    /// An unproved formal nonzero carries the actual polynomial and complete evaluation scope.
    fn pending(
        &self,
        polynomial: &Polynomial,
        scope: &EvaluationScope<'_>,
        budget: &mut Budget,
    ) -> Result<ZeroDecision, CoefficientFailure> {
        Ok(ZeroDecision::Undetermined(GuardedObligation {
            domain: self.domain.clone(),
            localization: super::storage::copied(scope.localization, budget)?,
            context: scope.context.clone(),
            region: scope.region.clone(),
            polynomial: polynomial.copy(budget)?,
        }))
    }

    /// A finite localization does not itself prove that its named denominators are nonzero.
    fn localization(
        &self,
        scope: &EvaluationScope<'_>,
        budget: &mut Budget,
    ) -> Result<Option<ZeroDecision>, CoefficientFailure> {
        for handle in scope.localization {
            let value = super::read::sparse(self.state, handle, budget)?;
            if self.polynomial_nonzero(handle, scope, budget)? {
                continue;
            }
            match value.scalar(budget)? {
                Some(constant) if !constant.zero(budget)? => {}
                _ => return Ok(Some(self.pending(&value, scope, budget)?)),
            }
        }
        Ok(None)
    }

    /// Decide regional zero/nonzero only after definedness of the finite localization.
    pub(crate) fn zero_test(
        &self,
        element: &Element,
        scope: &EvaluationScope<'_>,
        budget: &mut Budget,
    ) -> Result<ZeroDecision, CoefficientFailure> {
        begin(budget)?;
        self.scope(scope, budget)?;
        if element.domain != *self.domain {
            return Err(invalid().into());
        }
        if let Some(pending) = self.localization(scope, budget)? {
            return Ok(pending);
        }
        if let Some(certificate) = self.unit_premise(element, scope, budget)? {
            return Ok(ZeroDecision::Nonzero(certificate));
        }
        if let Some(pending) = self.defined_value(element, scope, budget)? {
            return Ok(pending);
        }
        match &element.value {
            Value::Tower(_) => Ok(
                match crate::solving::towers::units::inverse(self, element, scope, budget)? {
                    UnitDecision::Unit { certificate, .. } => ZeroDecision::Nonzero(certificate),
                    UnitDecision::Zero(certificate) => ZeroDecision::Zero(certificate),
                    UnitDecision::Split(certificate) => ZeroDecision::Split(certificate),
                    UnitDecision::Undetermined(obligation) => {
                        ZeroDecision::Undetermined(obligation)
                    }
                },
            ),
            Value::Scalar(value) => Ok(if value.zero(budget)? {
                ZeroDecision::Zero(CoefficientCertificate::Ground)
            } else {
                ZeroDecision::Nonzero(CoefficientCertificate::Ground)
            }),
            Value::Fraction(value) => {
                if value.numerator.zero(budget)? {
                    return Ok(ZeroDecision::Zero(CoefficientCertificate::FormalZero));
                }
                if super::regional::proves(
                    self.state,
                    self.domain,
                    &value.numerator,
                    scope.region,
                    true,
                    budget,
                )? {
                    return Ok(ZeroDecision::Zero(CoefficientCertificate::Regional(
                        Vec::new(),
                    )));
                }
                if self.polynomial_nonzero_value(&value.numerator, scope, budget)? {
                    let mut witnesses = vector(scope.premises.len(), budget)?;
                    for premise in scope.premises {
                        witnesses.push(premise.proof().clone());
                    }
                    return Ok(ZeroDecision::Nonzero(CoefficientCertificate::Regional(
                        witnesses,
                    )));
                }
                self.pending(&value.numerator, scope, budget)
            }
        }
    }

    /// A formal reciprocal is computed only after a regional nonzero decision succeeds.
    pub(crate) fn inverse(
        &self,
        element: &Element,
        scope: &EvaluationScope<'_>,
        budget: &mut Budget,
    ) -> Result<UnitDecision<Element>, CoefficientFailure> {
        begin(budget)?;
        if matches!(element.value, Value::Tower(_)) {
            self.scope(scope, budget)?;
            if let Some(ZeroDecision::Undetermined(obligation)) =
                self.localization(scope, budget)?
            {
                return Ok(UnitDecision::Undetermined(obligation));
            }
            if let Some(ZeroDecision::Undetermined(obligation)) =
                self.defined_value(element, scope, budget)?
            {
                return Ok(UnitDecision::Undetermined(obligation));
            }
            return crate::solving::towers::units::inverse(self, element, scope, budget);
        }
        let certificate = match self.zero_test(element, scope, budget)? {
            ZeroDecision::Split(certificate) => return Ok(UnitDecision::Split(certificate)),
            ZeroDecision::Zero(certificate) => return Ok(UnitDecision::Zero(certificate)),
            ZeroDecision::Undetermined(obligation) => {
                return Ok(UnitDecision::Undetermined(obligation));
            }
            ZeroDecision::Nonzero(certificate) => certificate,
        };
        let inverse = self.element(match &element.value {
            Value::Scalar(value) => {
                Value::Scalar(Scalar::constant(value.ground(), true, budget)?.div(value, budget)?)
            }
            Value::Fraction(value) => Value::Fraction(value.reciprocal(budget)?),
            Value::Tower(_) => return Err(invalid().into()),
        });
        Ok(UnitDecision::Unit {
            inverse,
            certificate,
        })
    }

    /// A finite evaluation obtains denominator inverses only from target-scope unit decisions.
    fn evaluated_inverse(
        &self,
        value: &Element,
        scope: &EvaluationScope<'_>,
        budget: &mut Budget,
    ) -> Result<Element, CoefficientFailure> {
        match self.inverse(value, scope, budget)? {
            UnitDecision::Unit { inverse, .. } => Ok(inverse),
            UnitDecision::Split(_) => Err(CoefficientFailure::Unsupported(
                crate::solving::SolveReason::ComponentChoicePending,
            )),
            UnitDecision::Zero(_) | UnitDecision::Undetermined(_) => {
                Err(CoefficientFailure::Unsupported(
                    crate::solving::SolveReason::SpecializationGuardPending,
                ))
            }
        }
    }

    /// Specialize into a smaller parameter field or another admitted algebra on the finite localization.
    fn specialize(
        &self,
        element: &Element,
        map: &CoefficientMapData<crate::graph::Handle>,
        target: &crate::graph::Handle,
        scope: &EvaluationScope<'_>,
        budget: &mut Budget,
    ) -> Result<Element, CoefficientFailure> {
        let Value::Fraction(value) = &element.value else {
            return Err(invalid().into());
        };
        let (_, _, base) = self.fraction_shape()?;
        let destination = Engine {
            state: self.state,
            domain: target,
        };
        let interpreted = EvaluationScope {
            context: scope.context,
            region: scope.region,
            premises: scope.premises,
            localization: &[],
        };
        let mut images = vector(map.images.len(), budget)?;
        for handle in &map.images {
            let image = destination.read(handle, budget)?;
            if destination
                .defined_value(&image, &interpreted, budget)?
                .is_some()
            {
                return Err(CoefficientFailure::Unsupported(
                    crate::solving::SolveReason::SpecializationGuardPending,
                ));
            }
            images.push(image);
        }
        for denominator in &map.denominators {
            let polynomial = super::read::sparse(self.state, denominator, budget)?;
            let image =
                super::maps::evaluate(self.state, &polynomial, base, target, &images, budget)?;
            destination.evaluated_inverse(&image, &interpreted, budget)?;
        }
        let denominator = super::maps::evaluate(
            self.state,
            &value.denominator,
            base,
            target,
            &images,
            budget,
        )?;
        let inverse = destination.evaluated_inverse(&denominator, &interpreted, budget)?;
        let numerator =
            super::maps::evaluate(self.state, &value.numerator, base, target, &images, budget)?;
        destination.mul(&numerator, &inverse, budget)
    }

    /// Maps retain their finite denominator set and distinguish evaluation from field inclusion.
    pub(crate) fn map(
        &self,
        element: &Element,
        map: &CoefficientMapData<crate::graph::Handle>,
        scope: &EvaluationScope<'_>,
        budget: &mut Budget,
    ) -> Result<Element, CoefficientFailure> {
        begin(budget)?;
        self.scope(scope, budget)?;
        budget.solving(BudgetKind::CoefficientMaps, 1, AT)?;
        budget.solving(
            BudgetKind::DenominatorFactors,
            size(map.denominators.len(), AT)?,
            AT,
        )?;
        if map.source != *self.domain {
            return Err(invalid().into());
        }
        if (&element.domain, &map.region, map.denominators.as_slice())
            != (self.domain, scope.region, scope.localization)
        {
            return Err(invalid().into());
        }
        let MapTarget::CoefficientDomain { domain: target } = &map.target else {
            return Err(CoefficientFailure::Unsupported(
                crate::solving::SolveReason::CoefficientDomainUnavailable,
            ));
        };
        if (map.mode == MapMode::Embedding || (*target == *self.domain && map.images.is_empty()))
            && self.localization(scope, budget)?.is_some()
        {
            return Err(CoefficientFailure::Unsupported(
                crate::solving::SolveReason::SpecializationGuardPending,
            ));
        }
        if *target == *self.domain && map.images.is_empty() {
            return self.copy(element, budget);
        }
        if map.mode == MapMode::Specialization
            && matches!(
                domain(self.state, self.domain)?,
                CoefficientDomainData::Fraction { .. }
            )
        {
            return self.specialize(element, map, target, scope, budget);
        }
        if map.mode == MapMode::Specialization && matches!(element.value, Value::Tower(_)) {
            if self.localization(scope, budget)?.is_some()
                || self.defined_value(element, scope, budget)?.is_some()
            {
                return Err(CoefficientFailure::Unsupported(
                    crate::solving::SolveReason::SpecializationGuardPending,
                ));
            }
            return super::maps::quotient(self.state, element, map, target, budget);
        }
        if map.mode == MapMode::Embedding {
            if map.images.is_empty() {
                return super::maps::canonical(self.state, element, target, budget);
            }
            return super::maps::inclusion(self.state, element, map, target, budget);
        }
        Err(CoefficientFailure::Unsupported(
            crate::solving::SolveReason::CoefficientDomainUnavailable,
        ))
    }
}

impl Engine<'_> {
    /// Match only discharged evidence in the exact scope already validated by the caller.
    fn polynomial_nonzero(
        &self,
        handle: &crate::graph::Handle,
        scope: &EvaluationScope<'_>,
        budget: &mut Budget,
    ) -> Result<bool, CoefficientFailure> {
        self.polynomial_nonzero_value(
            &super::read::sparse(self.state, handle, budget)?,
            scope,
            budget,
        )
    }

    /// Scratch arithmetic retains the same regional evidence after copying a canonical polynomial.
    fn polynomial_nonzero_value(
        &self,
        polynomial: &Polynomial,
        scope: &EvaluationScope<'_>,
        budget: &mut Budget,
    ) -> Result<bool, CoefficientFailure> {
        if let Some(constant) = polynomial.scalar(budget)? {
            return Ok(!constant.zero(budget)?);
        }
        if super::regional::proves(
            self.state,
            self.domain,
            polynomial,
            scope.region,
            false,
            budget,
        )? {
            return Ok(true);
        }
        for witness in scope.premises {
            if witness.is_discharged()
                && super::nonvanishing::proves_polynomial(
                    self.state,
                    self.domain,
                    polynomial,
                    scope.region,
                    &witness.judgment().statement,
                    budget,
                )?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl Engine<'_> {
    /// Check actual retained denominators before reporting zero or a unit in a quotient.
    fn defined_value(
        &self,
        element: &Element,
        scope: &EvaluationScope<'_>,
        budget: &mut Budget,
    ) -> Result<Option<ZeroDecision>, CoefficientFailure> {
        begin(budget)?;
        if element.domain != *self.domain {
            return Err(invalid().into());
        }
        match &element.value {
            Value::Scalar(_) => Ok(None),
            Value::Fraction(value) => {
                if self.polynomial_nonzero_value(&value.denominator, scope, budget)? {
                    Ok(None)
                } else {
                    Ok(Some(self.pending(&value.denominator, scope, budget)?))
                }
            }
            Value::Tower(coefficients) => {
                let (_, extension) = crate::solving::towers::arithmetic::level(self)?;
                let parent = Self {
                    state: self.state,
                    domain: &extension.parent,
                };
                for coefficient in coefficients {
                    if let Some(pending) = parent.defined_value(coefficient, scope, budget)? {
                        return Ok(Some(pending));
                    }
                }
                let modulus = super::storage::polynomial(self.state, &extension.polynomial)?;
                for term in &modulus.terms {
                    let coefficient = parent.read(&term.coefficient, budget)?;
                    if let Some(pending) = parent.defined_value(&coefficient, scope, budget)? {
                        return Ok(Some(pending));
                    }
                }
                Ok(None)
            }
        }
    }
}
