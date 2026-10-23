// SPDX-License-Identifier: AGPL-3.0-only
//! Shared immutable declarations prevent contradictory reuse across owners.

use super::{JointError, model::Definition};
use nrz_bindings::{Descriptor, Role};
use nrz_scalar::Scalar;
use std::{cell::RefCell, rc::Rc};

/// Process-local source identity; equality never establishes independence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceId(u64);

/// One declared zero-mean fluctuation with finite second moments.
///
/// Cloning retains identity and the shared joint-definition reservation. Once used
/// in a model, the source cannot be silently rebound to another joint definition.
#[derive(Clone, Debug)]
pub struct Source<P: Scalar> {
    /// Immutable identity.
    id: SourceId,
    /// Immutable declaration revision.
    revision: u64,
    /// Coherent continuous difference schema.
    descriptor: Descriptor,
    /// Shared globally for every clone of this identity, including across heaps.
    pub(crate) definition: Rc<RefCell<Option<Rc<Definition<P>>>>>,
}
impl<P: Scalar> Source<P> {
    /// Declare a zero-mean continuous fluctuation with finite second moments.
    ///
    /// The covariance parameters are supplied by a subsequent joint admission.
    ///
    /// # Errors
    /// Refuses point/count/noncoherent schemas, zero revisions or exhausted identities.
    pub fn new(descriptor: &Descriptor, revision: u64) -> Result<Self, JointError> {
        crate::binding::schema::check_quantity(descriptor)?;
        if descriptor.role() != Role::Difference || revision == 0 {
            return Err(JointError::InvalidSource);
        }
        Ok(Self {
            id: SourceId(crate::heap::identity::identity()?),
            revision,
            descriptor: descriptor.clone(),
            definition: Rc::new(RefCell::new(None)),
        })
    }
    /// Reuse this identity only with its exact immutable declaration.
    ///
    /// # Errors
    /// Any schema or revision change under this identity is a source conflict.
    pub fn redeclare(&self, descriptor: &Descriptor, revision: u64) -> Result<Self, JointError> {
        if descriptor != &self.descriptor || revision != self.revision {
            return Err(JointError::SourceConflict);
        }
        Ok(self.clone())
    }
    /// Immutable source identity.
    #[must_use]
    pub const fn id(&self) -> SourceId {
        self.id
    }
    /// Immutable declaration revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    /// Difference schema for this zero-mean fluctuation.
    #[must_use]
    pub const fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CovarianceBlock, JointLimits, JointModel, SourceFactor};
    /// Conflict refusal must not reserve any earlier unbound source cells.
    /// ´test:unit:source-reservation-is-atomic´
    #[test]
    fn source_reservation_is_atomic() -> Result<(), JointError> {
        let limits = JointLimits::default();
        let fresh = Source::new(&Descriptor::ratio(), 1)?;
        let occupied = Source::new(&Descriptor::ratio(), 1)?;
        JointModel::from_factor(
            std::slice::from_ref(&occupied),
            SourceFactor::new(1, &[1.0_f64], &[1.0], limits)?,
            &[CovarianceBlock::Known],
            limits,
        )?;
        let sources = [fresh.clone(), occupied];
        assert!(matches!(
            JointModel::from_factor(
                &sources,
                SourceFactor::new(2, &[1.0, 1.0], &[1.0], limits)?,
                &vec![CovarianceBlock::Known; 4],
                limits
            ),
            Err(JointError::SourceConflict)
        ));
        assert!(fresh.definition.borrow().is_none());
        Ok(())
    }
}
