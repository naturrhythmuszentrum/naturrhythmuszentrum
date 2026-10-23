// SPDX-License-Identifier: AGPL-3.0-only
//! Mean-parameter declarations are separate from nominal operating centers.

use super::TargetMap;
use crate::{Handle, Heap, JointError, JointLimits, Value};
use nrz_bindings::{Descriptor, UncertaintyMode, uncertainty};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// Declared mathematical means and certified center-parameter displacements.
///
/// These enclosures describe mean parameters, never the support of random sources.
#[derive(Clone, Debug)]
pub struct MeanParameters<P: Scalar> {
    /// Mean parameter families on both target sides.
    pub(crate) enclosures: [Vec<Enclosure<P>>; 2],
    /// Outward displacement of each represented operating center from its mean family.
    pub(crate) errors: [Vec<P>; 2],
    /// Independently derived difference-radius schemas on the mean parameters.
    pub(crate) descriptors: [Descriptor; 2],
}
impl<P: Scalar> MeanParameters<P> {
    /// Mathematical mean parameter enclosures in target order.
    #[must_use]
    pub const fn enclosures(&self) -> &[Vec<Enclosure<P>>; 2] {
        &self.enclosures
    }
    /// Mean-error descriptor on each side, without a bound on physical fluctuations.
    #[must_use]
    pub const fn descriptors(&self) -> &[Descriptor; 2] {
        &self.descriptors
    }
}
/// Check current centers against declared mean families before retaining parameter errors.
pub(crate) fn prepare<P: Scalar + 'static>(
    heap: &Heap,
    targets: &[TargetMap<P>; 2],
    enclosures: [Vec<Enclosure<P>>; 2],
    context: &ArithmeticContext,
    limits: JointLimits,
) -> Result<MeanParameters<P>, JointError> {
    let total = enclosures[0]
        .len()
        .checked_add(enclosures[1].len())
        .ok_or(JointError::ResourceLimit)?;
    limits.check_work(total)?;
    if total > limits.entries {
        return Err(JointError::ResourceLimit);
    }
    if targets[0].stamp() == targets[1].stamp() && enclosures[0] != enclosures[1] {
        return Err(JointError::SourceConflict);
    }
    let mut errors = [Vec::new(), Vec::new()];
    let mut descriptors = Vec::new();
    for ((target, family), output) in targets.iter().zip(&enclosures).zip(&mut errors) {
        target.target.validate(heap)?;
        let value = heap.value(Handle::<Value, P>::new(heap.scope, target.target.key))?;
        let centers = value.region().export()?;
        if centers.len() != family.len() {
            return Err(JointError::WrongTarget);
        }
        for (&center, &mean) in centers.iter().zip(family) {
            output.push(mean.displacement_from(Finite::new(center)?, context)?.get());
        }
        descriptors.push(
            uncertainty(
                UncertaintyMode::Radius,
                heap.binding_quantity(Handle::<Value, P>::new(heap.scope, target.target.key))?,
                None,
                None,
            )
            .map_err(crate::HeapError::from)?,
        );
    }
    let descriptors = descriptors
        .try_into()
        .map_err(|_| JointError::WrongTarget)?;
    Ok(MeanParameters {
        enclosures,
        errors,
        descriptors,
    })
}
