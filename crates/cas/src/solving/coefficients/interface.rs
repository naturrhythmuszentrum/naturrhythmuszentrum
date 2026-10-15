// SPDX-License-Identifier: AGPL-3.0-only
//! A sealed interface: no operator trait can bypass the caller's budget.

use super::arithmetic::Engine;
use super::model::{CoefficientFailure, Element, EvaluationScope, UnitDecision, ZeroDecision};
use super::storage::{begin, domain, invalid};
use crate::budget::Budget;
use crate::graph::{Handle, State};
use crate::solving::{CoefficientDomainData, CoefficientMapData};

/// External implementations cannot promise unchecked field arithmetic.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the private sealing trait is shared with its parent module"
)]
mod sealed {
    /// Only this leaf's checked domain views implement the interface.
    pub(crate) trait Sealed {}
}

/// One fallible arithmetic capability shared by every polynomial algorithm.
pub(crate) trait ExactCoefficients: sealed::Sealed {
    /// Owned, canonical scratch value; never a sealed theorem.
    type Element;
    /// Exact graph-owned domain.
    fn domain(&self) -> Handle;
    /// Metered additive identity.
    fn zero(&self, budget: &mut Budget) -> Result<Self::Element, CoefficientFailure>;
    /// Metered multiplicative identity.
    fn one(&self, budget: &mut Budget) -> Result<Self::Element, CoefficientFailure>;
    /// Metered canonical copy.
    fn copy(
        &self,
        value: &Self::Element,
        budget: &mut Budget,
    ) -> Result<Self::Element, CoefficientFailure>;
    /// Metered sign change.
    fn neg(
        &self,
        value: &Self::Element,
        budget: &mut Budget,
    ) -> Result<Self::Element, CoefficientFailure>;
    /// Metered exact sum.
    fn add(
        &self,
        left: &Self::Element,
        right: &Self::Element,
        budget: &mut Budget,
    ) -> Result<Self::Element, CoefficientFailure>;
    /// Metered exact difference.
    fn sub(
        &self,
        left: &Self::Element,
        right: &Self::Element,
        budget: &mut Budget,
    ) -> Result<Self::Element, CoefficientFailure>;
    /// Metered exact product.
    fn mul(
        &self,
        left: &Self::Element,
        right: &Self::Element,
        budget: &mut Budget,
    ) -> Result<Self::Element, CoefficientFailure>;
    /// Metered nonnegative formal power.
    fn pow(
        &self,
        value: &Self::Element,
        exponent: u32,
        budget: &mut Budget,
    ) -> Result<Self::Element, CoefficientFailure>;
    /// Metered canonical reduction.
    fn reduce(
        &self,
        value: &Self::Element,
        budget: &mut Budget,
    ) -> Result<Self::Element, CoefficientFailure>;
    /// Regional zero decision, distinct from a formal nonzero test.
    fn zero_test(
        &self,
        value: &Self::Element,
        scope: &EvaluationScope<'_>,
        budget: &mut Budget,
    ) -> Result<ZeroDecision, CoefficientFailure>;
    /// Inversion requires regional definedness and a unit decision.
    fn inverse(
        &self,
        value: &Self::Element,
        scope: &EvaluationScope<'_>,
        budget: &mut Budget,
    ) -> Result<UnitDecision<Self::Element>, CoefficientFailure>;
    /// Explicit map with finite localization and full evaluation scope.
    fn map(
        &self,
        value: &Self::Element,
        map: &CoefficientMapData<Handle>,
        scope: &EvaluationScope<'_>,
        budget: &mut Budget,
    ) -> Result<Self::Element, CoefficientFailure>;
}

/// Forward the sealed interface without duplicating any arithmetic implementation.
macro_rules! coefficient_domain {
    ($name:ident, $pattern:pat, $description:literal) => {
        #[doc = $description]
        #[derive(Debug)]
        pub(crate) struct $name<'a>(Engine<'a>);

        impl<'a> $name<'a> {
            /// Validate the exact domain before exposing its arithmetic capability.
            pub(crate) fn new(
                state: &'a State,
                handle: &'a Handle,
                budget: &mut Budget,
            ) -> Result<Self, CoefficientFailure> {
                begin(budget)?;
                if !matches!(domain(state, handle)?, $pattern) {
                    return Err(invalid().into());
                }
                Ok(Self(Engine {
                    state,
                    domain: handle,
                }))
            }

            /// Import an admitted graph value through the same budgeted arithmetic boundary.
            pub(crate) fn read(
                &self,
                handle: &Handle,
                budget: &mut Budget,
            ) -> Result<Element, CoefficientFailure> {
                self.0.read(handle, budget)
            }
        }

        impl sealed::Sealed for $name<'_> {}

        impl ExactCoefficients for $name<'_> {
            type Element = Element;
            fn domain(&self) -> Handle {
                self.0.domain.clone()
            }
            fn zero(&self, budget: &mut Budget) -> Result<Element, CoefficientFailure> {
                self.0.constant(false, budget)
            }
            fn one(&self, budget: &mut Budget) -> Result<Element, CoefficientFailure> {
                self.0.constant(true, budget)
            }
            fn copy(
                &self,
                value: &Element,
                budget: &mut Budget,
            ) -> Result<Element, CoefficientFailure> {
                self.0.copy(value, budget)
            }
            fn neg(
                &self,
                value: &Element,
                budget: &mut Budget,
            ) -> Result<Element, CoefficientFailure> {
                self.0.neg(value, budget)
            }
            fn add(
                &self,
                left: &Element,
                right: &Element,
                budget: &mut Budget,
            ) -> Result<Element, CoefficientFailure> {
                self.0.add(left, right, budget)
            }
            fn sub(
                &self,
                left: &Element,
                right: &Element,
                budget: &mut Budget,
            ) -> Result<Element, CoefficientFailure> {
                self.0.sub(left, right, budget)
            }
            fn mul(
                &self,
                left: &Element,
                right: &Element,
                budget: &mut Budget,
            ) -> Result<Element, CoefficientFailure> {
                self.0.mul(left, right, budget)
            }
            fn pow(
                &self,
                value: &Element,
                exponent: u32,
                budget: &mut Budget,
            ) -> Result<Element, CoefficientFailure> {
                self.0.pow(value, exponent, budget)
            }
            fn reduce(
                &self,
                value: &Element,
                budget: &mut Budget,
            ) -> Result<Element, CoefficientFailure> {
                self.0.reduce(value, budget)
            }
            fn zero_test(
                &self,
                value: &Element,
                scope: &EvaluationScope<'_>,
                budget: &mut Budget,
            ) -> Result<ZeroDecision, CoefficientFailure> {
                self.0.zero_test(value, scope, budget)
            }
            fn inverse(
                &self,
                value: &Element,
                scope: &EvaluationScope<'_>,
                budget: &mut Budget,
            ) -> Result<UnitDecision<Element>, CoefficientFailure> {
                self.0.inverse(value, scope, budget)
            }
            fn map(
                &self,
                value: &Element,
                map: &CoefficientMapData<Handle>,
                scope: &EvaluationScope<'_>,
                budget: &mut Budget,
            ) -> Result<Element, CoefficientFailure> {
                self.0.map(value, map, scope, budget)
            }
        }
    };
}

coefficient_domain!(
    RationalCoefficients,
    CoefficientDomainData::Rational,
    "Sealed arithmetic over the rational ground field."
);
coefficient_domain!(
    GaussianCoefficients,
    CoefficientDomainData::Gaussian,
    "Sealed arithmetic over the Gaussian rational ground field."
);
coefficient_domain!(
    FractionCoefficients,
    CoefficientDomainData::Fraction { .. },
    "Sealed canonical rational-function arithmetic over an admitted ground field."
);

coefficient_domain!(
    TowerCoefficients,
    CoefficientDomainData::Quotient { .. },
    "Sealed arithmetic in a monic finite quotient, without an implicit field assertion."
);
