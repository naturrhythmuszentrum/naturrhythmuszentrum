// SPDX-License-Identifier: AGPL-3.0-only
//! Standalone limit and series claims retain their approach and remainder convention.
//!
//! # Limits and side series
//!
//! `Task::Limit(Box<GermAt>)` uses checked polynomial side jets and trace projections on nonempty punctured neighborhoods. A two-sided request requires equal limits; sign therefore has limits −1 and 1 and a refused two-sided result. `Task::Series(Box<SeriesAt>)` retains centre, side, order and formal/analytic mode. Its exact remainder witnesses f(a+h) = P(h) + h^(n+1)R(h); ramp on the right at zero has coefficients `[0, 1, 0]` at order two and zero remainder.
//!
//! Formal side claims concern a punctured domain. Ordinary two-sided Taylor claims require matching coefficients and the actual centre value. Analytic mode additionally requires both full polynomial germs, including their remainders, to agree across that centre. This checks polynomial analyticity and supplies no general convergence theorem. `limit.case-restrict` and `series.case-restrict` have semantic version 1 and weights 2 and 4. All trace and remainder dependencies survive canonical expected replay, with the existing jet budgets and partition cache.

use crate::domain_model::Side;
use crate::graph::Handle;
use crate::number::Rational;

/// A punctured approach to an exact real boundary location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GermAt {
    /// Exact declared real coordinate.
    pub coordinate: String,
    /// Boundary object retaining the exact centre.
    pub point: Handle,
    /// One side, or both sides when absent.
    pub side: Option<Side>,
}

/// A finite polynomial expansion with an explicit mathematical mode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesAt {
    /// Centre, coordinate and approach.
    pub at: GermAt,
    /// Largest retained power of the increment, including order zero.
    pub order: u32,
    /// Require a common analytic polynomial germ on both sides including its centre.
    pub analytic: bool,
}

/// The relation established from the checked side germs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GermMode {
    /// Finite limit on the stated punctured approach.
    Limit,
    /// Polynomial congruence modulo the first omitted power, with an exact remainder.
    Series {
        /// Largest included power of the increment.
        order: u32,
        /// The common full polynomial germ is analytic at the owned centre.
        analytic: bool,
    },
}

/// A function-specific finite limit or side expansion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GermValue {
    /// Complete function syntax.
    pub function: Handle,
    /// Exact centre and punctured approach.
    pub at: GermAt,
    /// Limit, formal congruence or analytic polynomial germ.
    pub mode: GermMode,
    /// One limit value, or exactly order+1 ascending coefficients in h = x-centre.
    pub coefficients: Vec<Rational>,
}

/// Exact polynomial remainders for each checked incident side in Below/Above order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GermCertificate {
    /// R(h) in f(centre+h) = P(h) + h^(order+1) R(h), one per required side.
    pub remainders: Vec<Vec<Rational>>,
}
