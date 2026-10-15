// SPDX-License-Identifier: AGPL-3.0-only
//! Cross-value lineage preparation without a covariance propagation kernel.

use super::{CompanionRef, StatisticalView};
use crate::{Heap, JointError, JointLimits, JointModel, ValueStamp};
use nrz_scalar::Scalar;

/// Validated source-selection sensitivities for a later `J_y U_s J_zᵀ` contraction.
///
/// Each output row selects exactly one common source coordinate. Repeated rows
/// select the same fluctuation. This declaration supplies no nonlinear Jacobian.
#[derive(Debug)]
pub struct CrossLineage<'h, P: Scalar> {
    /// Current evidence for the first retained value.
    left: StatisticalView<'h, P>,
    /// Current evidence for the second retained value.
    right: StatisticalView<'h, P>,
}
impl<P: Scalar> CrossLineage<'_, P> {
    /// Common declared covariance model, checked on both retained values.
    #[must_use]
    pub const fn model(&self) -> &JointModel<P> {
        self.left.model()
    }
    /// Ordered physical target versions for the prospective cross block.
    #[must_use]
    pub const fn targets(&self) -> [ValueStamp; 2] {
        [self.left.targets()[0], self.right.targets()[0]]
    }
    /// Source-selection rows of the first value's exact declared linear relation.
    #[must_use]
    pub fn left_sources(&self) -> &[usize] {
        &self.left.source_maps()[0]
    }
    /// Source-selection rows of the second value's exact declared linear relation.
    #[must_use]
    pub fn right_sources(&self) -> &[usize] {
        &self.right.source_maps()[0]
    }
}
impl Heap {
    /// Validate two retained values' shared source lineage and every requested cross block.
    ///
    /// The first target side of each companion identifies the respective output.
    /// This prepares the relation only; no covariance propagation kernel is executed.
    ///
    /// # Errors
    /// Refuses stale/foreign evidence, unrelated models, unknown blocks and work exhaustion.
    pub fn cross_lineage<P: Scalar + 'static>(
        &self,
        left: CompanionRef<P>,
        right: CompanionRef<P>,
        limits: JointLimits,
    ) -> Result<CrossLineage<'_, P>, JointError> {
        let left = self.statistical(left)?;
        let right = self.statistical(right)?;
        if left.model().id() != right.model().id() {
            return Err(JointError::MissingCrossBlock);
        }
        left.model()
            .require_blocks(&left.source_maps()[0], &right.source_maps()[0], limits)?;
        Ok(CrossLineage { left, right })
    }
}
