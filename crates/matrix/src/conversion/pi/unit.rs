// SPDX-License-Identifier: AGPL-3.0-only
//! Typed exact scalar definitions retain their certified numerical coefficient and shared pi region.

use super::{ConversionError, PiConstant};
use crate::{ExactCoefficient, coordinate_numeric::ConstantCoefficient};
use nrz_bindings::{
    Budget, ExactEnclosureExport, Limits, LimitsOverride, readings::algebra::limits::BudgetKind,
};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, LiteralLimits, Scalar};
use std::rc::Rc;

/// One certified typed coefficient; cloning shares all exact and numerical evidence.
#[derive(Clone, Debug)]
pub struct CertifiedCoefficient<P: Scalar>(Rc<Owner<P>>);

/// Exact definitions remain separately shared; this owner contains only finite numerical evidence.
#[derive(Debug)]
struct Owner<P: Scalar> {
    /// Original typed source-free exact definition.
    exact: Rc<ExactCoefficient>,
    /// Certified factor, candidate and complete numerical displacement.
    value: ConstantCoefficient<P>,
    /// Original immutable pi and reciprocal materialization.
    pi: PiConstant<P>,
}

impl<P: Scalar + 'static> CertifiedCoefficient<P> {
    /// Lower one typed exact pi coefficient under an explicit cumulative caller policy.
    ///
    /// Exact definitions and pi remain shared owners. This admission performs no
    /// structural expansion and turns no constant error into statistical uncertainty.
    /// # Errors
    /// Refuses rational-only coefficients, unsupported constants, mismatched arithmetic,
    /// overflowing numerical bounds and exhausted Work or byte policies atomically.
    pub fn new(
        definition: &Rc<ExactCoefficient>,
        pi: &PiConstant<P>,
        exact: &LimitsOverride,
        literals: LiteralLimits,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<Self, ConversionError> {
        let rational = definition.exact_map().scale.as_rational().is_some();
        let changed_arithmetic = pi.arithmetic_revision() != context.revision();
        if rational || changed_arithmetic {
            return Err(ConversionError::Contract);
        }
        reserve::<P>(pi, exact, budget)?;
        let available = budget.limits().work.saturating_sub(budget.usage().work);
        let mut policy = exact.clone();
        policy.work = Some(
            policy
                .work
                .unwrap_or_else(|| Limits::default().work)
                .min(available),
        );
        let exported = ExactEnclosureExport::new(&definition.exact_map().scale, &policy)?;
        let value = super::coefficient::decode(
            (
                exported.rational_factor(),
                exported.pi_factor(),
                exported.work_used(),
            ),
            pi,
            literals,
            context,
            available,
        )?;
        let result = Self(Rc::new(Owner {
            exact: Rc::clone(definition),
            value,
            pi: pi.clone(),
        }));
        super::charge(budget, value.work_used())?;
        Ok(result)
    }
    /// Original typed exact definition, with its original irrational graph node.
    #[must_use]
    pub fn definition(&self) -> &ExactCoefficient {
        &self.0.exact
    }
    /// The original shared pi materialization.
    #[must_use]
    pub fn constant(&self) -> &PiConstant<P> {
        &self.0.pi
    }
    /// Finite candidate coefficient.
    #[must_use]
    pub fn center(&self) -> Finite<P> {
        self.0.value.center()
    }
    /// Complete enclosure of the exact coefficient.
    #[must_use]
    pub fn enclosure(&self) -> Enclosure<P> {
        self.0.value.enclosure()
    }
    /// Outward displacement from the candidate to every enclosed exact value.
    #[must_use]
    pub fn displacement(&self) -> Finite<P> {
        self.0.value.displacement()
    }
    /// Original admission Work; cloning performs no recognition or numerical evaluation.
    #[must_use]
    pub fn work_used(&self) -> u64 {
        self.0.value.work_used()
    }
    /// New numerical owner and reference header; exact definitions and pi are separate owners.
    #[must_use]
    pub fn owned_bytes(&self) -> usize {
        std::mem::size_of_val(self.0.as_ref()) + 2 * size_of::<usize>()
    }
    /// Whether two consumers reuse the same certified coefficient allocation.
    #[must_use]
    pub fn same_region(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// Count the actual fixed numerical owner without copying its exact graph or descriptor.
const fn owner_bytes<P: Scalar>() -> usize {
    size_of::<Owner<P>>() + 2 * size_of::<usize>()
}

/// Exact-arithmetic allowances cover temporary recognition; retained definitions remain separate.
fn reserve<P: Scalar + 'static>(
    pi: &PiConstant<P>,
    exact: &LimitsOverride,
    budget: &Budget,
) -> Result<(), ConversionError> {
    let limits = Limits::default()
        .resolve(exact)
        .map_err(nrz_bindings::MapError::from)?;
    let staged = limits
        .staged_payload_bytes
        .checked_add(limits.output_bytes)
        .and_then(|value| usize::try_from(value).ok())
        .and_then(|value| value.checked_add(pi.owned_bytes()))
        .and_then(|value| value.checked_add(owner_bytes::<P>()))
        .ok_or(ConversionError::ResourceLimit)?;
    super::check(budget, BudgetKind::StagedPayloadBytes, staged)?;
    super::check(budget, BudgetKind::OutputBytes, owner_bytes::<P>())
}
