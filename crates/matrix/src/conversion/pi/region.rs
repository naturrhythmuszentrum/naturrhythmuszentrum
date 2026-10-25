// SPDX-License-Identifier: AGPL-3.0-only
//! One immutable numerical pi region, with its reciprocal evaluated once.

use super::{ConversionError, PiConstantEvidence};
use nrz_bindings::{Budget, readings::algebra::limits::BudgetKind};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, PiBudget, PiEnclosure, PiLimits, Scalar};
use std::rc::Rc;

/// Shared fixed numerical storage with its rational theorem and arithmetic revision.
#[derive(Debug)]
pub(super) struct Region<P: Scalar> {
    /// Complete checked scalar pi enclosure.
    pub(super) pi: PiEnclosure<P>,
    /// One outward reciprocal, reused by all inverse consumers.
    pub(super) reciprocal: Enclosure<P>,
    /// Admitted scalar arithmetic revision.
    pub(super) arithmetic: u32,
}

/// Immutable pi and reciprocal regions shared by compatible checked map consumers.
///
/// This source-free constant has no physical uncertainty or writable heap handle.
/// A certified map binds its original exact graph nodes to this numerical owner.
/// Cloning shares the same region; it never recomputes pi or its reciprocal.
#[derive(Clone, Debug)]
pub struct PiConstant<P: Scalar>(pub(super) Rc<Region<P>>);

impl<P: Scalar + 'static> PiConstant<P> {
    /// Enclose pi and its reciprocal under explicit scalar and cumulative owner limits.
    ///
    /// The scalar result and one outward reciprocal cost their measured Work.
    /// Logical staging reserves the scalar policy plus the new fixed owner; output
    /// counts that owner and its reference-count header. No exact map is copied.
    /// # Errors
    /// Refuses invalid tolerance, resolution, arithmetic or resource exhaustion.
    /// Every failure leaves the supplied cumulative ledger unchanged.
    pub fn new(
        tolerance: Finite<P>,
        limits: PiLimits,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<Self, ConversionError> {
        let owned = size_of::<Region<P>>() + 2 * size_of::<usize>();
        let staged = limits
            .staged_bytes
            .checked_add(owned)
            .ok_or(ConversionError::ResourceLimit)?;
        super::check(budget, BudgetKind::StagedPayloadBytes, staged)?;
        super::check(budget, BudgetKind::OutputBytes, owned)?;
        let remaining = budget.limits().work.saturating_sub(budget.usage().work);
        let work = remaining
            .checked_sub(1)
            .ok_or(ConversionError::ResourceLimit)?;
        let mut scalar = PiBudget::new(PiLimits {
            work: limits.work.min(work),
            ..limits
        });
        let pi = PiEnclosure::new(tolerance, context, &mut scalar)?;
        let reciprocal = Enclosure::singleton(Finite::ONE).div(pi.enclosure(), context)?;
        let result = Self(Rc::new(Region {
            pi,
            reciprocal,
            arithmetic: context.revision(),
        }));
        super::charge(budget, pi.work_used() + 1)?;
        Ok(result)
    }

    /// The original pi enclosure and its inspectable rational evidence.
    #[must_use]
    pub fn pi(&self) -> PiEnclosure<P> {
        self.0.pi
    }

    /// The reciprocal enclosure, including the one outward division error.
    #[must_use]
    pub fn reciprocal(&self) -> Enclosure<P> {
        self.0.reciprocal
    }

    /// Scalar arithmetic revision required by every consumer.
    #[must_use]
    pub fn arithmetic_revision(&self) -> u32 {
        self.0.arithmetic
    }

    /// Logical immutable owner and reference-count header, excluding allocator overhead.
    #[must_use]
    pub fn owned_bytes(&self) -> usize {
        std::mem::size_of_val(self.0.as_ref()) + 2 * size_of::<usize>()
    }

    /// Whether two consumers share the one original numerical materialization.
    #[must_use]
    pub fn same_region(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    /// Retain the original region as format-erased, read-only operation evidence.
    #[must_use]
    pub fn evidence(&self) -> PiConstantEvidence {
        PiConstantEvidence::new(Rc::clone(&self.0))
    }
}
