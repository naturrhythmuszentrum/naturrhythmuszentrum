// SPDX-License-Identifier: AGPL-3.0-only
//! Closed original lattice boxes admit only contained native coordinate enclosures.

use super::decision::arithmetic::{enclosure, endpoint};
use crate::{Coverage, FrozenLatticeInputs, Heap, LiveDecisionError};
use nrz_bindings::readings::algebra::{
    Cas, Location, Rational,
    lattice::{CheckedRationalBoxEnclosure, RationalBoxEnclosure},
    limits::BudgetKind,
};
use nrz_bindings::{Budget, DecisionCoordinate, Declaration};
use nrz_scalar::{ArithmeticContext, Enclosure, LiteralLimits, Scalar};

/// This optional receiver owns every additional coordinate check and retained box.
const AT: Location = Location::new("matrix live lattice enclosure");

/// Historical complete native input lease and independently certified original target box.
///
/// The outward boxes are in the question's normalized coordinate frame. The exact
/// certificate is closed rational/Machin evidence; this exports no kernel certificate
/// and confers no selected-tuple authority. Publication must revalidate the input lease.
#[derive(Debug)]
pub struct FrozenLatticeEnclosure<P: Scalar> {
    /// Complete original sources, including basis/offset sources and all companions.
    pub(super) frozen: FrozenLatticeInputs<P>,
    /// CAS-checked original target, context, family and every ordered coordinate origin.
    pub(super) original: CheckedRationalBoxEnclosure,
    /// Complete live normalized boxes, each contained in the corresponding exact interval.
    coordinates: Vec<Enclosure<P>>,
    /// The numerical interpretation used for this historical containment check.
    arithmetic_revision: u32,
    /// Explicit endpoint decoding policy charged before each attempt.
    literals: LiteralLimits,
    /// Retained native snapshot, exact certificate and coordinate metadata allowance.
    pub(super) bytes: u64,
}

impl<P: Scalar> FrozenLatticeEnclosure<P> {
    /// Historical source inventory; current access requires its complete heap lease check.
    #[must_use]
    pub const fn inputs(&self) -> &FrozenLatticeInputs<P> {
        &self.frozen
    }

    /// Independently checked closed original box, without selected-tuple authority.
    #[must_use]
    pub const fn certificate(&self) -> &CheckedRationalBoxEnclosure {
        &self.original
    }

    /// Complete historical normalized vector, retaining declared axis order.
    #[must_use]
    pub fn coordinates(&self) -> &[Enclosure<P>] {
        &self.coordinates
    }

    /// Original numerical arithmetic revision and explicit endpoint-decoding limits.
    #[must_use]
    pub const fn arithmetic_revision(&self) -> u32 {
        self.arithmetic_revision
    }

    /// Explicit endpoint-decoding policy used by the historical numerical checks.
    #[must_use]
    pub const fn literal_limits(&self) -> LiteralLimits {
        self.literals
    }

    /// Complete logical retained payload, excluding already shared exact source graphs.
    #[must_use]
    pub const fn retained_payload_bytes(&self) -> u64 {
        self.bytes
    }
}

impl Heap {
    /// Validate live direct-coordinate boxes inside independently certified original intervals.
    ///
    /// This closed receiver supports original declared constant targets and rational
    /// normalization references/origins. Total radii already include numerical error.
    /// Every exact original origin is replayed before any coordinate result is returned.
    /// # Errors
    /// Refuses unsupported origins/recipes, changed physical identities, stale inputs,
    /// noncontained boxes or exhausted resources, without publishing any vector prefix.
    pub fn enclose_lattice_inputs<P: Scalar + 'static>(
        &self,
        frozen: FrozenLatticeInputs<P>,
        graph: &mut Cas,
        contract: &RationalBoxEnclosure,
        arithmetic: &ArithmeticContext,
        literals: LiteralLimits,
        budget: &mut Budget,
    ) -> Result<FrozenLatticeEnclosure<P>, LiveDecisionError> {
        graph.with_enclosure_attempt(budget, |attempt, budget| {
            self.check_lattice_inputs(&frozen, budget)?;
            let spec = frozen.association().case().specification();
            if contract.target != spec.target || contract.context != *spec.frame.context() {
                return Err(LiveDecisionError::Recipe);
            }
            let original =
                budget.with_retained_payload(frozen.retained_payload_bytes(), AT, |budget| {
                    attempt.check_rational_box_enclosure(contract, budget)
                })?;
            let count = u64::try_from(spec.axes.len()).map_err(|_| LiveDecisionError::Recipe)?;
            let bytes = frozen
                .retained_payload_bytes()
                .saturating_add(original.retained_bytes())
                .saturating_add(count.saturating_mul(128))
                .saturating_add(128);
            budget.check(BudgetKind::OutputBytes, bytes, AT)?;
            let coordinates = budget.with_retained_payload(bytes, AT, |budget| {
                budget.work(count.saturating_mul(128).saturating_add(128), AT)?;
                contain(&frozen, contract, arithmetic, literals, budget)
            })?;
            Ok(FrozenLatticeEnclosure {
                frozen,
                original,
                coordinates,
                arithmetic_revision: arithmetic.revision(),
                literals,
                bytes,
            })
        })
    }
}

/// Match every original target to its designated native input before coordinate arithmetic.
fn contain<P: Scalar>(
    frozen: &FrozenLatticeInputs<P>,
    contract: &RationalBoxEnclosure,
    arithmetic: &ArithmeticContext,
    literals: LiteralLimits,
    budget: &mut Budget,
) -> Result<Vec<Enclosure<P>>, LiveDecisionError> {
    let spec = frozen.association().case().specification();
    let complete = spec.axes.len() == contract.coordinates.len()
        && spec.axes.len() == spec.frame.entries().len();
    let declared = frozen
        .operands()
        .iter()
        .all(|operand| operand.input().coverage == Coverage::DeclaredBounds);
    if !complete || !declared {
        return Err(LiveDecisionError::Recipe);
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(spec.axes.len())
        .map_err(|_| LiveDecisionError::Recipe)?;
    for ((axis, meaning), exact) in spec
        .axes
        .iter()
        .zip(spec.frame.entries())
        .zip(&contract.coordinates)
    {
        let slot = original_slot(frozen, &axis.target, budget)?;
        let value = enclosure(frozen.operands()[slot].coordinates(), arithmetic)?;
        let scale = meaning
            .magnitude()
            .as_rational()
            .ok_or(LiveDecisionError::Recipe)?;
        let shift = meaning
            .shift()
            .as_rational()
            .ok_or(LiveDecisionError::Recipe)?;
        let mut normalized = value;
        if !shift.is_zero() {
            normalized = normalized.sub(
                endpoint(shift, arithmetic, literals, budget, AT)?,
                arithmetic,
            )?;
        }
        if scale != &Rational::one() {
            normalized = normalized.div(
                endpoint(scale, arithmetic, literals, budget, AT)?,
                arithmetic,
            )?;
        }
        let lower = endpoint::<P>(&exact.lower, arithmetic, literals, budget, AT)?;
        let upper = endpoint::<P>(&exact.upper, arithmetic, literals, budget, AT)?;
        budget.work(32, AT)?;
        if normalized.lower() < lower.upper() || normalized.upper() > upper.lower() {
            return Err(LiveDecisionError::Enclosure);
        }
        result.push(normalized);
    }
    Ok(result)
}

/// Original syntax handles carry their source-store identity; names or equal values do not.
fn original_slot<P: Scalar>(
    frozen: &FrozenLatticeInputs<P>,
    target: &DecisionCoordinate,
    budget: &mut Budget,
) -> Result<usize, LiveDecisionError> {
    if !matches!(
        target.owner().declarations().get(target.name()),
        Some(Declaration::Constant { .. })
    ) {
        return Err(LiveDecisionError::Recipe);
    }
    let original = target
        .checked()
        .map_err(|cause| LiveDecisionError::Admission(Box::new(cause)))?;
    for (slot, operand) in frozen.association().operands().iter().enumerate() {
        budget.work(1, AT)?;
        let candidate = operand
            .original()
            .map_err(|cause| LiveDecisionError::Admission(Box::new(cause)))?;
        if candidate.original().syntax == original.original().syntax
            && operand.name() == target.name()
        {
            return Ok(slot);
        }
    }
    Err(LiveDecisionError::Recipe)
}
