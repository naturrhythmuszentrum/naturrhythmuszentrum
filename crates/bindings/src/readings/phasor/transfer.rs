// SPDX-License-Identifier: AGPL-3.0-only
//! Uniform positive component metrics retain the exact block-to-Complex residual factor.

use super::PhasorSolveDefinition;
use crate::{
    Budget, Role,
    readings::{CoordinateNormalization, ReadingError, support},
};
use nrz_cas::{ExactConstant, limits::BudgetKind};

/// Norm of a residual after division by its declared voltage reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhasorResidualNorm {
    /// Euclidean norm of the ordered real/imaginary residual pair.
    ComponentEuclidean,
    /// Frobenius norm of the full reconstructed residual block.
    BlockFrobenius,
}

/// Exact transport of a phasor residual to the named current component metric.
///
/// For `Z=i*X`, `R(Z)^T*R(Z)=X^2*I`. Thus the induced two-norm is
/// `abs(X)*I_ref/V_ref`, whereas its Frobenius norm has an additional `sqrt(2)`.
/// A block residual also has `sqrt(2)` times the component norm. The squared
/// residual-to-current factor is `(V_ref/I_ref)^2/(weight*X^2)` with weight one
/// or two. This closed identity is conditional on nonzero reactance; numerical
/// execution must supply its separated enclosure and outward square root.
#[derive(Clone, Debug)]
pub struct PhasorResidualTransfer {
    /// Original operation and checked complete block correspondence.
    definition: PhasorSolveDefinition,
    /// Ordered voltage then current metrics, retaining their exact reference identities.
    frames: [CoordinateNormalization; 2],
    /// Explicit residual convention; the output always uses the current component norm.
    norm: PhasorResidualNorm,
    /// Exact positive reference quotient, independent of runtime numerical inputs.
    ratio: ExactConstant,
    /// Conservative retained metadata and proof allowance.
    bytes: u64,
}

impl PhasorResidualTransfer {
    /// Check uniform difference metrics and retain the exact closed residual transport.
    /// # Errors
    /// Refuses changed kinds/order, unequal component references, contexts, point roles and limits.
    pub fn new(
        definition: PhasorSolveDefinition,
        frames: [CoordinateNormalization; 2],
        norm: PhasorResidualNorm,
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        for (frame, reading) in frames
            .iter()
            .zip([definition.voltage(), definition.output()])
        {
            budget.work(1, support::AT)?;
            if frame.context() != reading.reading().family().context() || frame.entries().len() != 2
            {
                return Err(ReadingError::Shape);
            }
            frame.entries()[0].check(&frame.entries()[1])?;
            for (entry, target) in frame.entries().iter().zip(reading.reading().entries()) {
                budget.work(1, support::AT)?;
                entry.descriptor().compatible_kind(target.descriptor())?;
                if entry.descriptor().role() != Role::Difference {
                    return Err(ReadingError::Role);
                }
            }
        }
        let ratio = frames[0].entries()[0]
            .magnitude()
            .div(frames[1].entries()[0].magnitude(), budget)?;
        let bytes =
            frames
                .iter()
                .try_fold(definition.retained_payload_bytes(), |size, frame| {
                    size.checked_add(frame.retained_payload_bytes())
                        .ok_or(ReadingError::Shape)
                })?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, support::AT)?;
        budget.check(BudgetKind::OutputBytes, bytes, support::AT)?;
        Ok(Self {
            definition,
            frames,
            norm,
            ratio,
            bytes,
        })
    }
    /// Original checked LC solve and whole-family block law.
    #[must_use]
    pub const fn definition(&self) -> &PhasorSolveDefinition {
        &self.definition
    }
    /// Ordered voltage and current metrics, with explicit immutable references.
    #[must_use]
    pub const fn frames(&self) -> &[CoordinateNormalization; 2] {
        &self.frames
    }
    /// Named residual norm; changing it changes the transport factor.
    #[must_use]
    pub const fn norm(&self) -> PhasorResidualNorm {
        self.norm
    }
    /// Exact voltage-reference over current-reference magnitude.
    #[must_use]
    pub const fn reference_ratio(&self) -> &ExactConstant {
        &self.ratio
    }
    /// Squared block/component residual norm ratio.
    #[must_use]
    pub const fn weight(&self) -> u8 {
        match self.norm {
            PhasorResidualNorm::ComponentEuclidean => 1,
            PhasorResidualNorm::BlockFrobenius => 2,
        }
    }
    /// Conservative retained exact metadata allowance.
    #[must_use]
    pub const fn retained_payload_bytes(&self) -> u64 {
        self.bytes
    }
}
