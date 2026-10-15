// SPDX-License-Identifier: AGPL-3.0-only
//! Borrowed bound vectors and explicitly named norm summaries.

use crate::{AccuracyFailure, Nonnegative};
use nrz_scalar::{ArithmeticContext, Scalar};

/// A norm on all supplied logical entries, in caller-selected coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormKind {
    /// Sum of entry magnitudes; not an induced matrix one-norm.
    One,
    /// Square root of the sum of squared vector entries.
    Euclidean,
    /// Square root of the sum of squared logical matrix entries.
    Frobenius,
    /// Maximum entry magnitude; not an induced matrix infinity-norm.
    Infinity,
}

/// A norm upper bound with its entry count and interpretation retained.
///
/// This is a numerical summary, without physical coordinates or heap authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormSummary<P: Scalar> {
    /// Norm used to summarize the supplied entries.
    kind: NormKind,
    /// Number of supplied logical entries, including repetitions.
    entry_count: usize,
    /// Outward upper bound on the specified norm.
    upper: Nonnegative<P>,
}

impl<P: Scalar> NormSummary<P> {
    /// The norm evaluated over the supplied entries.
    #[must_use]
    pub const fn kind(self) -> NormKind {
        self.kind
    }

    /// Number of logical entries included in this summary.
    #[must_use]
    pub const fn entry_count(self) -> usize {
        self.entry_count
    }

    /// Finite nonnegative upper bound on the specified norm.
    #[must_use]
    pub const fn upper(self) -> Nonnegative<P> {
        self.upper
    }
}

/// An immutable borrowed plain slice whose entries are finite and nonnegative.
///
/// No buffer is allocated or rewritten. Signed zero may remain in the borrowed
/// slice; returned magnitudes use canonical zero. The caller supplies every
/// logical multiplicity, including duplicated entries of structured matrices.
#[derive(Debug, Clone, Copy)]
pub struct BoundVectorRef<'a, P: Scalar> {
    /// Checked entries, held immutably for the entire reference lifetime.
    values: &'a [P],
}

impl<'a, P: Scalar> BoundVectorRef<'a, P> {
    /// Validate every entry before lending a bound vector.
    ///
    /// # Errors
    /// Returns `AccuracyFailure::InvalidBound` for any negative or nonfinite entry.
    pub fn new(values: &'a [P]) -> Result<Self, AccuracyFailure> {
        for &value in values {
            Nonnegative::new(value)?;
        }
        Ok(Self { values })
    }

    /// The original plain data, borrowed without copying or changing zero bits.
    #[must_use]
    pub const fn as_slice(self) -> &'a [P] {
        self.values
    }

    /// Bound the sum with an increasing-index left fold starting at zero.
    ///
    /// # Errors
    /// Returns the scalar cause for an unbounded intermediate or result.
    pub fn sum(self, context: &ArithmeticContext) -> Result<Nonnegative<P>, AccuracyFailure> {
        Nonnegative::new(nrz_scalar::sum(self.values, context)?.upper().get())
    }

    /// Bound a sum of pairwise products in increasing index order.
    ///
    /// Multiplication is enclosed before addition, with no implicit fusion.
    ///
    /// # Errors
    /// Refuses unequal lengths or a scalar operation without finite endpoints.
    pub fn dot(
        self,
        rhs: Self,
        context: &ArithmeticContext,
    ) -> Result<Nonnegative<P>, AccuracyFailure> {
        if self.values.len() != rhs.values.len() {
            return Err(AccuracyFailure::LengthMismatch);
        }
        Nonnegative::new(
            nrz_scalar::dot(self.values, rhs.values, context)?
                .upper()
                .get(),
        )
    }

    /// Summarize a named norm of all supplied logical entries.
    ///
    /// Euclidean and Frobenius norms use scalar's enclosed power-of-two scaling
    /// to avoid direct squared-entry overflow. Every empty norm is zero.
    ///
    /// # Errors
    /// Returns the scalar cause for an unbounded intermediate or result.
    pub fn norm(
        self,
        kind: NormKind,
        context: &ArithmeticContext,
    ) -> Result<NormSummary<P>, AccuracyFailure> {
        let upper = match kind {
            NormKind::One => self.sum(context)?,
            NormKind::Euclidean | NormKind::Frobenius => {
                Nonnegative::new(nrz_scalar::norm_upper(self.values, context)?.get())?
            }
            NormKind::Infinity => self.maximum()?,
        };
        Ok(NormSummary {
            kind,
            entry_count: self.values.len(),
            upper,
        })
    }

    /// Select the exact maximum with zero as the empty result.
    fn maximum(self) -> Result<Nonnegative<P>, AccuracyFailure> {
        let mut largest = P::ZERO;
        for &value in self.values {
            if value > largest {
                largest = value;
            }
        }
        Nonnegative::new(largest)
    }
}
