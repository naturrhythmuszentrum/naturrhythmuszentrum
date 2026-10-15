// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit bounded refinement and optional precision promotion.

use crate::{AccuracyFailure, Budget, Limits, Recovery};
use nrz_scalar::Scalar;

/// Permission for a fresh execution in a wider scalar format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Promotion {
    /// Keep the requested precision.
    Never,
    /// Permit one fresh binary64 execution after a recoverable binary32 stop.
    F32ToF64,
}

/// Finite refinement policy; progress is measured only by certified upper radii.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryPolicy {
    /// Maximum reserved refinements after an initial execution, per precision.
    max_refinements: u32,
    /// Consecutive attempts without a new strictly smaller best certified radius.
    stagnation_window: u32,
    /// Whether a fresh binary64 execution may follow a binary32 stop.
    promotion: Promotion,
}

impl RecoveryPolicy {
    /// Set the per-precision refinement limit and positive stagnation window.
    ///
    /// Zero refinements and no promotion are valid. A window of one stops at the
    /// first attempt that fails to beat the best previous certified radius.
    ///
    /// # Errors
    /// Returns `AccuracyFailure::InvalidPolicy` for a zero stagnation window.
    pub const fn new(
        max_refinements: u32,
        stagnation_window: u32,
        promotion: Promotion,
    ) -> Result<Self, AccuracyFailure> {
        if stagnation_window == 0 {
            return Err(AccuracyFailure::InvalidPolicy);
        }
        Ok(Self {
            max_refinements,
            stagnation_window,
            promotion,
        })
    }

    /// Maximum refinements following the initial execution at each precision.
    #[must_use]
    pub const fn max_refinements(self) -> u32 {
        self.max_refinements
    }

    /// Number of consecutive nonimproving attempts that stops refinement.
    #[must_use]
    pub const fn stagnation_window(self) -> u32 {
        self.stagnation_window
    }

    /// Explicit precision-retry permission.
    #[must_use]
    pub const fn promotion(self) -> Promotion {
        self.promotion
    }

    /// Start a run with a fixed numerical budget and cumulative resource limits.
    #[must_use]
    pub const fn start<P: Scalar>(self, budget: Option<Budget<P>>, limits: Limits) -> Recovery<P> {
        Recovery::new(self, budget, limits.start())
    }
}
