// SPDX-License-Identifier: AGPL-3.0-only
//! One graph-bound coordinate coefficient owner shares pi across every compatible live consumer.

use super::{CoordinateContract, CoordinateError, ReadingBinding, lowering::Lowered};
use crate::{ConstantCoefficient, HeapError, PiConstant};
use nrz_bindings::{
    Budget, ExactEnclosureExport, Limits, LimitsOverride,
    readings::{
        CoordinateImage,
        algebra::{Cas, Location, limits::BudgetKind},
    },
};
use nrz_scalar::{ArithmeticContext, Scalar};
use std::rc::Rc;

/// Certified normalization and restoration bound to their original immutable graph nodes.
///
/// Clones reuse both coefficient arrays and one pi region across all compatible
/// selected inputs. Numerical values and their revisions remain consumer-specific.
#[derive(Clone, Debug)]
pub struct PiCoordinateContract<P: Scalar>(Rc<Certified<P>>);

/// Exact graph and map identities are retained beside the two numerical materializations.
#[derive(Debug)]
struct Certified<P: Scalar> {
    /// Original exact coordinate law and independent reference identities.
    original: CoordinateContract,
    /// The one immutable graph authority that authenticated both image keys.
    graph: Rc<Cas>,
    /// Normalization and restoration, in that order.
    images: [Rc<Lowered<P>>; 2],
    /// The shared pi and reciprocal regions.
    pi: PiConstant<P>,
    /// Newly owned fixed metadata and both coefficient buffers.
    bytes: u64,
}

impl<P: Scalar + 'static> PiCoordinateContract<P> {
    /// Authenticate both exact nodes and lower their coefficients before any input float is read.
    ///
    /// Exact arithmetic uses the explicit child policy; only complete successful
    /// Work is committed to the caller. Existing exact declarations and pi are shared.
    /// # Errors
    /// Refuses foreign graph authority, rational-only contracts, unsupported constants,
    /// mismatched arithmetic, nonpositive enclosed scales and exhausted resources atomically.
    pub fn new(
        graph: &Rc<Cas>,
        original: &CoordinateContract,
        pi: &PiConstant<P>,
        exact: &LimitsOverride,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<Self, CoordinateError> {
        let changed = pi.arithmetic_revision() != context.revision();
        let rational = original
            .normalization()
            .scales()
            .iter()
            .chain(original.normalization().offsets())
            .all(|value| value.as_rational().is_some());
        if changed || rational {
            return Err(CoordinateError::Identity);
        }
        let bytes = owned_bytes::<P>(original.normalization().scales().len())?;
        let mut local = reserve(original, pi, exact, bytes, budget)?;
        for image in [original.normalization(), original.restoration()] {
            graph.validate_reuse_key(image.reuse_key(), &mut local)?;
        }
        let authentication = local.usage().work;
        let mut limits = local.limits().clone();
        limits.work = limits.work.saturating_sub(authentication);
        drop(local);
        let mut local = Budget::new(limits)?;
        let images = [
            Rc::new(lower(
                original.normalization(),
                original,
                pi,
                exact,
                context,
                &mut local,
            )?),
            Rc::new(lower(
                original.restoration(),
                original,
                pi,
                exact,
                context,
                &mut local,
            )?),
        ];
        let result = Self(Rc::new(Certified {
            original: original.clone(),
            graph: Rc::clone(graph),
            images,
            pi: pi.clone(),
            bytes,
        }));
        let work = authentication
            .checked_add(local.usage().work)
            .ok_or(HeapError::ExtentOverflow)?;
        budget.work(work, AT)?;
        Ok(result)
    }
    /// Original typed exact coordinate definition and both graph nodes.
    #[must_use]
    pub fn contract(&self) -> &CoordinateContract {
        &self.0.original
    }
    /// Original shared pi and reciprocal region.
    #[must_use]
    pub fn constant(&self) -> &PiConstant<P> {
        &self.0.pi
    }
    /// New coefficient and metadata ownership; original exact graphs and pi remain separate.
    #[must_use]
    pub fn owned_bytes(&self) -> u64 {
        self.0.bytes
    }
    /// Whether consumers share both materialized coefficient arrays and exact graph authority.
    #[must_use]
    pub fn same_region(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
    /// Reject graph substitution before either caller usage or numerical input access.
    pub(super) fn check(
        &self,
        input: &ReadingBinding<P>,
        context: &ArithmeticContext,
    ) -> Result<(), CoordinateError> {
        let same_graph = Rc::ptr_eq(&self.0.graph, &input.group.graph);
        let same_revision = self.0.pi.arithmetic_revision() == context.revision();
        if same_graph && same_revision {
            Ok(())
        } else {
            Err(CoordinateError::Identity)
        }
    }
    /// Select the already materialized original node without repeating exact recognition.
    pub(super) fn lowered(&self, restore: bool) -> Rc<Lowered<P>> {
        Rc::clone(&self.0.images[usize::from(restore)])
    }
}

/// Every coordinate owner charge has an identifiable ledger location.
const AT: Location = Location::new("certified coordinate coefficients");

/// Fixed shared owners and both exact-capacity coefficient arrays precede allocation.
fn owned_bytes<P: Scalar>(count: usize) -> Result<u64, CoordinateError> {
    let arrays = count
        .checked_mul(4 * size_of::<ConstantCoefficient<P>>())
        .ok_or(HeapError::ExtentOverflow)?;
    let metadata = size_of::<Certified<P>>() + 2 * size_of::<Lowered<P>>() + 6 * size_of::<usize>();
    u64::try_from(
        arrays
            .checked_add(metadata)
            .ok_or(HeapError::ExtentOverflow)?,
    )
    .map_err(|_| HeapError::ExtentOverflow.into())
}

/// The parent reserves named child exact allowances, retained definitions and numerical owners.
fn reserve<P: Scalar + 'static>(
    original: &CoordinateContract,
    pi: &PiConstant<P>,
    exact: &LimitsOverride,
    bytes: u64,
    budget: &Budget,
) -> Result<Budget, CoordinateError> {
    let mut limits = Limits::default().resolve(exact)?;
    let staged = limits
        .staged_payload_bytes
        .checked_add(limits.output_bytes)
        .and_then(|value| value.checked_add(original.0.bytes))
        .and_then(|value| value.checked_add(u64::try_from(pi.owned_bytes()).ok()?))
        .and_then(|value| value.checked_add(bytes))
        .ok_or(HeapError::ExtentOverflow)?;
    budget.check(BudgetKind::StagedPayloadBytes, staged, AT)?;
    budget.check(BudgetKind::OutputBytes, bytes, AT)?;
    limits.work = limits
        .work
        .min(budget.limits().work.saturating_sub(budget.usage().work));
    Ok(Budget::new(limits)?)
}

/// Both exact diagonal arrays preserve their original image and one shared numerical pi owner.
fn lower<P: Scalar + 'static>(
    image: &CoordinateImage,
    original: &CoordinateContract,
    pi: &PiConstant<P>,
    exact: &LimitsOverride,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<Lowered<P>, CoordinateError> {
    let count = image.scales().len();
    let mut coefficients = crate::binding::buffer(count)?;
    for (scale, offset) in image.scales().iter().zip(image.offsets()) {
        let pair = [
            coefficient(scale, original, pi, exact, context, budget)?,
            coefficient(offset, original, pi, exact, context, budget)?,
        ];
        if pair[0].enclosure().lower().get() <= P::ZERO {
            return Err(nrz_scalar::ArithmeticFailure::ZeroInDivisor.into());
        }
        coefficients.push(pair);
    }
    Ok(Lowered {
        image: image.clone(),
        coefficients,
        pi: Some(pi.clone()),
    })
}

/// Exact recognition and numerical enclosure share the remaining cumulative child Work.
fn coefficient<P: Scalar + 'static>(
    value: &nrz_bindings::ExactConstant,
    original: &CoordinateContract,
    pi: &PiConstant<P>,
    exact: &LimitsOverride,
    context: &ArithmeticContext,
    budget: &mut Budget,
) -> Result<ConstantCoefficient<P>, CoordinateError> {
    let available = budget.limits().work.saturating_sub(budget.usage().work);
    let mut policy = exact.clone();
    policy.work = Some(available);
    let exported =
        ExactEnclosureExport::new(value, &policy).map_err(crate::ConversionError::from)?;
    let result = crate::conversion::enclose_pi_factor(
        (
            exported.rational_factor(),
            exported.pi_factor(),
            exported.work_used(),
        ),
        pi,
        original.0.literals,
        context,
        available,
    )?;
    budget.work(result.work_used(), AT)?;
    Ok(result)
}
