// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit exact-export admission binds both numerical coefficients to the original map.

use super::{ConversionError, PiConstant};
use crate::{
    ConversionContract,
    coordinate_numeric::{Coefficient, LoweredMap},
};
use nrz_bindings::{
    Budget, DescriptorConstant, Limits, LimitsOverride, PiFactor,
    readings::algebra::limits::BudgetKind,
};
use nrz_scalar::{ArithmeticContext, LiteralLimits, NumericLiteral, Scalar};
use std::rc::Rc;

/// One immutable pair of coefficients and its shared original pi materialization.
#[derive(Debug)]
pub(crate) struct CertifiedMap<P: Scalar> {
    /// Shared numerical coefficients bound to the original exact map.
    pub(crate) map: Rc<LoweredMap<P>>,
    /// The one original pi and reciprocal numerical allocation.
    pub(crate) pi: PiConstant<P>,
}

/// Resource receipt for a fresh certified map or a compatible cache hit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PiConversionUse {
    /// Work added to the caller's existing ledger.
    work: u64,
    /// Newly retained map/cache bytes, excluding this fixed-size receipt.
    bytes: usize,
    /// Whether the original exact map and numerical region were reused.
    reused: bool,
}

impl PiConversionUse {
    /// Work charged for this lookup and any newly admitted lowering.
    #[must_use]
    pub const fn work(self) -> u64 {
        self.work
    }
    /// Newly retained numerical owner/cache bytes; shared exact maps and pi are separate owners.
    #[must_use]
    pub const fn new_owned_bytes(self) -> usize {
        self.bytes
    }
    /// Whether this call reused the original immutable coefficient materialization.
    #[must_use]
    pub const fn reused(self) -> bool {
        self.reused
    }
}

impl ConversionContract {
    /// Certify pi-dependent coefficients before executing this contract in format `P`.
    ///
    /// Cloned contracts reuse the same coefficient region. Every exact scale and
    /// shift retains its original map/graph identity; both statistical axes and
    /// deterministic channels then consume the same certified pair. Rational-only
    /// maps use ordinary conversion and do not need this admission.
    /// # Errors
    /// Refuses rational-only maps, unsupported constant expressions, mismatched
    /// arithmetic, unproved positive numerical scales and explicit resources.
    /// A refused request changes neither the ledger nor the published cache entries.
    pub fn certify_pi<P: Scalar + 'static>(
        &self,
        pi: &PiConstant<P>,
        exact: &LimitsOverride,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<PiConversionUse, ConversionError> {
        if pi.arithmetic_revision() != context.revision() {
            return Err(ConversionError::Contract);
        }
        let rational_scale = self.map.scale().exact().as_rational().is_some();
        let rational_shift = self.map.shift().exact().as_rational().is_some();
        if rational_scale && rational_shift {
            return Err(ConversionError::Contract);
        }
        let mut cache = self
            .cache
            .try_borrow_mut()
            .map_err(|_| ConversionError::Contract)?;
        let lookup = cache.lookup_work()?;
        if cache.find_pi(&self.map, pi, context).is_some() {
            super::check(
                budget,
                BudgetKind::StagedPayloadBytes,
                size_of::<PiConversionUse>(),
            )?;
            super::check(
                budget,
                BudgetKind::OutputBytes,
                size_of::<PiConversionUse>(),
            )?;
            super::charge(budget, lookup)?;
            return Ok(PiConversionUse {
                work: lookup,
                bytes: 0,
                reused: true,
            });
        }
        let reused_slot = if cache.contains::<P>() {
            super::super::cache::ConstantCache::entry_bytes()
        } else {
            0
        };
        let bytes = owned_bytes::<P>() - reused_slot;
        reserve(pi, exact, bytes, budget)?;
        let remaining = budget.limits().work.saturating_sub(budget.usage().work);
        let available = remaining
            .checked_sub(lookup)
            .ok_or(ConversionError::ResourceLimit)?;
        let (map, work) = lower(self, pi, exact, context, available)?;
        let materialized = Rc::new(CertifiedMap {
            map: Rc::new(map),
            pi: pi.clone(),
        });
        let prepared = cache.stage_pi(&materialized)?;
        let work = work
            .checked_add(lookup)
            .ok_or(ConversionError::ResourceLimit)?;
        super::charge(budget, work)?;
        cache.commit_pi(prepared);
        Ok(PiConversionUse {
            work,
            bytes,
            reused: false,
        })
    }
}

/// Count newly retained coefficient owners, reference headers and cache slot.
const fn owned_bytes<P: Scalar>() -> usize {
    size_of::<LoweredMap<P>>()
        + size_of::<CertifiedMap<P>>()
        + 4 * size_of::<usize>()
        + super::super::cache::ConstantCache::entry_bytes()
}

/// Reserve a named exact-arithmetic allowance rather than pretending it is measured allocator memory.
fn reserve<P: Scalar + 'static>(
    pi: &PiConstant<P>,
    exact: &LimitsOverride,
    bytes: usize,
    budget: &Budget,
) -> Result<(), ConversionError> {
    let limits = Limits::default()
        .resolve(exact)
        .map_err(nrz_bindings::MapError::from)?;
    let exact_bytes = limits
        .staged_payload_bytes
        .checked_add(limits.output_bytes)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(ConversionError::ResourceLimit)?;
    let output = bytes
        .checked_add(size_of::<PiConversionUse>())
        .ok_or(ConversionError::ResourceLimit)?;
    let staged = exact_bytes
        .checked_add(output)
        .and_then(|value| value.checked_add(pi.owned_bytes()))
        .ok_or(ConversionError::ResourceLimit)?;
    super::check(budget, BudgetKind::StagedPayloadBytes, staged)?;
    super::check(budget, BudgetKind::OutputBytes, output)
}

/// Lower both coefficients with one remaining cumulative Work allowance.
fn lower<P: Scalar + 'static>(
    contract: &ConversionContract,
    pi: &PiConstant<P>,
    exact: &LimitsOverride,
    context: &ArithmeticContext,
    available: u64,
) -> Result<(LoweredMap<P>, u64), ConversionError> {
    let scale = coefficient(
        contract.map.scale(),
        pi,
        exact,
        contract.literals,
        context,
        available,
    )?;
    let remaining = available
        .checked_sub(scale.work_used())
        .ok_or(ConversionError::ResourceLimit)?;
    let shift = coefficient(
        contract.map.shift(),
        pi,
        exact,
        contract.literals,
        context,
        remaining,
    )?;
    if scale.enclosure().lower().get() <= P::ZERO {
        return Err(nrz_scalar::ArithmeticFailure::ZeroInDivisor.into());
    }
    let work = scale
        .work_used()
        .checked_add(shift.work_used())
        .ok_or(ConversionError::ResourceLimit)?;
    Ok((
        LoweredMap {
            exact: Rc::clone(&contract.map),
            scale,
            shift,
            revision: context.revision(),
        },
        work,
    ))
}

/// Recognize the exact factor and enclose literal import and the admitted pi operation.
fn coefficient<P: Scalar + 'static>(
    constant: &DescriptorConstant,
    pi: &PiConstant<P>,
    exact: &LimitsOverride,
    literals: LiteralLimits,
    context: &ArithmeticContext,
    available: u64,
) -> Result<Coefficient<P>, ConversionError> {
    let mut policy = exact.clone();
    policy.work = Some(
        policy
            .work
            .unwrap_or_else(|| Limits::default().work)
            .min(available),
    );
    let exported = constant.enclosure_export(&policy)?;
    let remaining = available
        .checked_sub(exported.work_used())
        .ok_or(ConversionError::ResourceLimit)?;
    let limits = LiteralLimits::new(
        literals.max_digits(),
        literals.max_exponent(),
        literals.max_work().min(remaining),
    );
    let decoded = NumericLiteral::new(exported.rational_factor(), limits)?.decode::<P>(context)?;
    let work = exported
        .work_used()
        .checked_add(decoded.work_used())
        .ok_or(ConversionError::ResourceLimit)?;
    if exported.pi_factor() == PiFactor::Rational {
        return Coefficient::enclosed(
            decoded.enclosure(),
            work.checked_add(1)
                .filter(|work| *work <= available)
                .ok_or(ConversionError::ResourceLimit)?,
            context,
        );
    }
    let work = work
        .checked_add(2)
        .filter(|work| *work <= available)
        .ok_or(ConversionError::ResourceLimit)?;
    let factor = match exported.pi_factor() {
        PiFactor::Multiply => pi.pi().enclosure(),
        PiFactor::Divide => pi.reciprocal(),
        PiFactor::Rational => return Err(ConversionError::Contract),
    };
    Coefficient::enclosed(decoded.enclosure().mul(factor, context)?, work, context)
}
