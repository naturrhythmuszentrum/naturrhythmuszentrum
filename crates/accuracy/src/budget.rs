// SPDX-License-Identifier: AGPL-3.0-only
//! Conservative tolerance evaluation and decisions on available upper bounds.

use crate::{AccuracyFailure, Nonnegative};
use nrz_scalar::{ArithmeticContext, ArithmeticFailure, Enclosure, Finite, Scalar};

/// Absolute and relative tolerances with a strictly positive reference scale.
///
/// The allowed tolerance is `absolute + relative * reference_scale`. Enclosures
/// may represent lowered exact constants; they never imply physical uncertainty.
/// The caller checks compatible difference units, norm and coordinate meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget<P: Scalar> {
    /// Nonnegative absolute difference tolerance.
    absolute: Enclosure<P>,
    /// Nonnegative relative ratio tolerance.
    relative: Enclosure<P>,
    /// Strictly positive characteristic difference scale, independent of origins.
    reference_scale: Enclosure<P>,
}

impl<P: Scalar> Budget<P> {
    /// Construct a budget from exactly represented finite coefficients.
    ///
    /// # Errors
    /// Returns `AccuracyFailure::InvalidBudget` for nonfinite or negative
    /// tolerances, or a nonpositive reference scale, even if the relative part is zero.
    pub fn new(absolute: P, relative: P, reference_scale: P) -> Result<Self, AccuracyFailure> {
        let point = |value| {
            Finite::new(value)
                .map(Enclosure::singleton)
                .map_err(|_| AccuracyFailure::InvalidBudget)
        };
        Self::from_enclosures(point(absolute)?, point(relative)?, point(reference_scale)?)
    }

    /// Admit coefficient enclosures whose entire ranges satisfy the budget domain.
    ///
    /// # Errors
    /// Returns `AccuracyFailure::InvalidBudget` if either tolerance has a negative
    /// lower endpoint or the reference scale is not proved strictly positive.
    pub fn from_enclosures(
        absolute: Enclosure<P>,
        relative: Enclosure<P>,
        reference_scale: Enclosure<P>,
    ) -> Result<Self, AccuracyFailure> {
        if absolute.lower() < Finite::ZERO
            || relative.lower() < Finite::ZERO
            || reference_scale.lower() <= Finite::ZERO
        {
            return Err(AccuracyFailure::InvalidBudget);
        }
        Ok(Self {
            absolute,
            relative,
            reference_scale,
        })
    }

    /// Absolute difference tolerance enclosure.
    #[must_use]
    pub const fn absolute(self) -> Enclosure<P> {
        self.absolute
    }

    /// Relative ratio tolerance enclosure.
    #[must_use]
    pub const fn relative(self) -> Enclosure<P> {
        self.relative
    }

    /// Positive characteristic difference scale enclosure.
    #[must_use]
    pub const fn reference_scale(self) -> Enclosure<P> {
        self.reference_scale
    }

    /// Enclose the allowed tolerance, retaining both comparison directions.
    ///
    /// The lower endpoint is clamped to zero using the proved nonnegativity of
    /// all coefficients. The upper endpoint is never used to accept a budget.
    ///
    /// # Errors
    /// Returns the scalar cause if the tolerance cannot be enclosed finitely.
    pub fn tolerance(self, context: &ArithmeticContext) -> Result<Enclosure<P>, ArithmeticFailure> {
        let relative = self.relative.mul(self.reference_scale, context)?;
        let result = self.absolute.add(relative, context)?;
        let lower = if result.lower() < Finite::ZERO {
            P::ZERO
        } else {
            result.lower().get()
        };
        Enclosure::new(lower, result.upper().get())
    }
}

impl Budget<f32> {
    /// Widen every coefficient endpoint exactly, retaining existing widths.
    #[must_use]
    pub fn widen(self) -> Budget<f64> {
        Budget {
            absolute: self.absolute.widen(),
            relative: self.relative.widen(),
            reference_scale: self.reference_scale.widen(),
        }
    }
}

/// A decision about the available error bound and the requested tolerance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetDecision {
    /// No budget was requested; this is never budget success.
    NotRequested,
    /// The error upper bound is at most the lower allowed tolerance endpoint.
    Met,
    /// The error upper bound exceeds the upper allowed tolerance endpoint.
    /// This rejects the available certificate threshold, not the actual error.
    Exceeded,
    /// The error upper bound lies above the lower and at most the upper endpoint.
    Unresolved,
}

/// Retained numerical inputs and result of a conservative budget comparison.
///
/// This contains no heap certificate or proof of the caller's error-bound claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetAssessment<P: Scalar> {
    /// Available certified upper magnitude, as supplied by the caller.
    upper: Nonnegative<P>,
    /// Allowed tolerance enclosure, absent only when no budget was requested.
    tolerance: Option<Enclosure<P>>,
    /// Conservative comparison result.
    decision: BudgetDecision,
}

impl<P: Scalar> BudgetAssessment<P> {
    /// The upper magnitude compared against the tolerance.
    #[must_use]
    pub const fn upper(self) -> Nonnegative<P> {
        self.upper
    }

    /// Allowed tolerance enclosure, or absence of a request.
    #[must_use]
    pub const fn tolerance(self) -> Option<Enclosure<P>> {
        self.tolerance
    }

    /// The conservative decision, with absence distinct from success.
    #[must_use]
    pub const fn decision(self) -> BudgetDecision {
        self.decision
    }
}

/// Compare an error upper bound with an optional absolute/relative budget.
///
/// Acceptance uses only the lower tolerance endpoint. A proved threshold
/// violation uses the upper endpoint; overlap remains unresolved.
///
/// # Errors
/// Returns the scalar cause if a requested tolerance cannot be enclosed finitely.
pub fn assess_budget<P: Scalar>(
    budget: Option<&Budget<P>>,
    upper: Nonnegative<P>,
    context: &ArithmeticContext,
) -> Result<BudgetAssessment<P>, ArithmeticFailure> {
    let Some(request) = budget else {
        return Ok(BudgetAssessment {
            upper,
            tolerance: None,
            decision: BudgetDecision::NotRequested,
        });
    };
    let tolerance = request.tolerance(context)?;
    let decision = if upper.finite() <= tolerance.lower() {
        BudgetDecision::Met
    } else if upper.finite() > tolerance.upper() {
        BudgetDecision::Exceeded
    } else {
        BudgetDecision::Unresolved
    };
    Ok(BudgetAssessment {
        upper,
        tolerance: Some(tolerance),
        decision,
    })
}
