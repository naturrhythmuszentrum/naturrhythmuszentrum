// SPDX-License-Identifier: AGPL-3.0-only
//! A declared immutable case binds mathematical targets to their complete physical source.

use super::{DecisionError, DecisionExpression, storage};
use crate::{Descriptor, Kind};
use nrz_cas::{
    Cas, ExactConstant, Handle,
    decision::IntegerFamily,
    limits::Budget,
    syntax::{AtomicSort, Operator, Sort},
};
use std::rc::Rc;

/// Original case declaration; a candidate Integer is deliberately absent.
#[derive(Clone, Debug)]
pub struct IntegerDecisionSpec {
    /// Exact source context retained by the selection proof.
    pub context: Handle,
    /// Complete native floor family, including option and mathematical count constraints.
    pub family: IntegerFamily,
    /// Original closed target, independently matched to the physical expression.
    pub target: Handle,
    /// Ordered exact Integer parameter instance, retained even when values coincide.
    pub parameters: Vec<Handle>,
    /// Predeclared physical counted kind and its mathematical domain.
    pub count: Descriptor,
}

/// Physical declaration and exact original question, shared without mutable state.
#[derive(Debug)]
pub(crate) struct CaseData {
    /// Frozen physical expression and every source/map dependency.
    pub(crate) source: DecisionExpression,
    /// Complete declared mathematical and physical interface.
    pub(crate) specification: IntegerDecisionSpec,
    /// Conservative bounded comparison allowance for the complete family value.
    pub(crate) comparison: u64,
}

/// One immutable declared case; cloning retains the original declaration identity.
#[derive(Clone, Debug)]
pub struct IntegerDecisionCase(pub(crate) Rc<CaseData>);

/// An immutable attachment to exactly one source, map, case and parameter declaration.
///
/// This authenticates no live heap version. A different declaration must establish
/// and retain its own attachment before using a certified mathematical result.
#[derive(Clone, Debug)]
pub struct DecisionAttachment(pub(crate) Rc<CaseData>);

impl IntegerDecisionCase {
    /// Declare the exact physical question before admitting any selected outcome.
    ///
    /// The closed constant checker proves exact correspondence between the original
    /// CAS target and the frozen physical expression's coherent coordinates. Case
    /// cover and option admissibility remain mandatory at certified witness admission.
    /// # Errors
    /// Refuses changed exact targets, non-count outputs, malformed references or parameters,
    /// open target expressions and exhausted cumulative caller resources.
    pub fn declare(
        cas: &Cas,
        source: DecisionExpression,
        specification: IntegerDecisionSpec,
        budget: &mut Budget,
    ) -> Result<Self, DecisionError> {
        if source.exact().is_none() {
            return Err(DecisionError::Source);
        }
        Self::checked(cas, source, specification, budget)
    }

    /// Declare an original quotient under an explicit physical input-family correspondence.
    /// # Errors
    /// Refuses closed substitutes, changed source identities, boxes, assumptions, targets and limits.
    pub fn declare_family(
        cas: &Cas,
        source: DecisionExpression,
        specification: IntegerDecisionSpec,
        budget: &mut Budget,
    ) -> Result<Self, DecisionError> {
        if source.input_families().is_empty() {
            return Err(DecisionError::Source);
        }
        Self::checked(cas, source, specification, budget)
    }

    /// Both source forms share the original case and parameter admission owner.
    fn checked(
        cas: &Cas,
        source: DecisionExpression,
        specification: IntegerDecisionSpec,
        budget: &mut Budget,
    ) -> Result<Self, DecisionError> {
        budget.work(8, storage::AT)?;
        let Kind::Count(name, _) = specification.count.kind() else {
            return Err(DecisionError::CountKind);
        };
        if name.trim().is_empty() {
            return Err(DecisionError::CountKind);
        }
        let count =
            u64::try_from(specification.parameters.len()).map_err(|_| DecisionError::Capacity)?;
        let bytes = count
            .checked_mul(64)
            .and_then(|value| value.checked_add(512))
            .and_then(|value| value.checked_add(u64::try_from(name.len()).ok()?))
            .ok_or(DecisionError::Capacity)?;
        storage::retained(bytes, budget)?;
        budget.work(bytes, storage::AT)?;
        let expected_domain = cas.admissible_region(&specification.context)?;
        if expected_domain != specification.family.domain {
            return Err(DecisionError::Case);
        }
        let header = cas.context_header(&specification.context)?;
        for node in [
            &specification.family.function,
            &specification.family.coordinate,
            &specification.family.index,
            &specification.target,
        ] {
            let syntax = cas.describe_syntax(node, budget)?;
            if syntax.header != header || !syntax.scope.is_empty() || syntax.binding.is_some() {
                return Err(DecisionError::Case);
            }
        }
        for parameter in &specification.parameters {
            let syntax = cas.describe_syntax(parameter, budget)?;
            let (operator, sort) = cas.expression(parameter)?;
            let integer = matches!(operator, Operator::Rational(number) if number.is_integer());
            if syntax.header != header
                || !syntax.scope.is_empty()
                || syntax.binding.is_some()
                || sort != &Sort::Value(AtomicSort::Integer)
                || !integer
            {
                return Err(DecisionError::Parameter);
            }
        }
        if let Some(expected) = source.exact() {
            let exact = ExactConstant::from_expression(cas, &specification.target, budget)?;
            budget.work(
                storage::constant_bytes(&exact).max(storage::constant_bytes(expected)),
                storage::AT,
            )?;
            if &exact != expected {
                return Err(DecisionError::Target);
            }
        } else {
            super::family::correspondence(cas, &source, &specification, budget)?;
        }
        let mut comparison = 128_u64;
        if let Some(domain) = &specification.family.count {
            for value in [&domain.lower, &domain.upper].into_iter().flatten() {
                comparison = comparison
                    .checked_add(storage::rational_bytes(value))
                    .ok_or(DecisionError::Capacity)?;
            }
        }
        storage::retained(
            bytes
                .checked_add(comparison)
                .ok_or(DecisionError::Capacity)?,
            budget,
        )?;
        Ok(Self(Rc::new(CaseData {
            source,
            specification,
            comparison,
        })))
    }

    /// Original physical expression, including the frozen source and unit interpretation.
    #[must_use]
    pub fn source(&self) -> &DecisionExpression {
        &self.0.source
    }
    /// Complete original target, case and parameter tuple.
    #[must_use]
    pub fn specification(&self) -> &IntegerDecisionSpec {
        &self.0.specification
    }
    /// Capture the already declared immutable identity without copying its graph or proof.
    #[must_use]
    pub fn attachment(&self) -> DecisionAttachment {
        DecisionAttachment(Rc::clone(&self.0))
    }

    /// Match the entire immutable declaration, including dependencies with equal nominal values.
    /// # Errors
    /// Refuses attachments from independently declared sources, maps, cases or parameters.
    pub fn validate_attachment(
        &self,
        attachment: &DecisionAttachment,
        budget: &mut Budget,
    ) -> Result<(), DecisionError> {
        budget.work(1, storage::AT)?;
        if Rc::ptr_eq(&self.0, &attachment.0) {
            Ok(())
        } else {
            Err(DecisionError::Attachment)
        }
    }
}
