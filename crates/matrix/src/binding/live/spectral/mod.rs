// SPDX-License-Identifier: AGPL-3.0-only
//! Strict positive-family spectral bounds retain the original theorem and every live input version.

mod input;
mod moments;
mod numeric;
mod plan;
mod publication;
mod statistics;
pub use plan::LcSpectrum;
pub use publication::{LcSpectralPublication, PublishedLcSpectrum};

use super::LiveBindingError;
use crate::{Coverage, Heap, HeapError};
use nrz_bindings::{
    Budget,
    readings::{
        ReadingError, SpectralLawError,
        algebra::{
            Fault, Location,
            limits::{BudgetKind, BudgetUsage},
        },
    },
};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Accounting location for spectral lowering and outward numerical execution.
const AT: Location = Location::new("LC spectral execution");

/// An exact, live-source, whole-family domain or outward-arithmetic refusal.
#[derive(Clone, Debug, thiserror::Error)]
pub enum SpectralError {
    /// Exact physical law admission refused.
    #[error("spectral law: {0}")]
    Law(#[from] SpectralLawError),
    /// Original reading or reference metadata differs.
    #[error("spectral reading: {0}")]
    Reading(#[from] ReadingError),
    /// A selected source or companion version is unavailable.
    #[error("spectral live input: {0}")]
    Live(#[from] LiveBindingError),
    /// Original evaluated derivative admission or validation refused.
    #[error("spectral derivative: {0}")]
    Derivative(#[from] crate::ReciprocalDerivativeError),
    /// Original joint model, target coverage or covariance parameter access refused.
    #[error("spectral statistical input: {0}")]
    Joint(#[from] crate::JointError),
    /// Numerical storage or extent admission refused.
    #[error("spectral storage: {0}")]
    Heap(#[from] HeapError),
    /// Caller-owned cumulative resources refused.
    #[error("spectral resources: {0}")]
    Exact(#[from] Fault),
    /// Scalar arithmetic could not produce finite outward evidence.
    #[error("spectral arithmetic: {0}")]
    Arithmetic(#[from] nrz_scalar::ArithmeticFailure),
    /// The full input box is nonpositive.
    #[error("spectral input {input} is nonpositive")]
    NonpositiveInput {
        /// Ordered original input: zero for L, one for C.
        input: usize,
    },
    /// A positive center has no separated positive whole-family input enclosure.
    #[error("spectral input {input} is not separated from zero")]
    InputNotSeparated {
        /// Ordered original input: zero for L, one for C.
        input: usize,
    },
    /// Rounded coefficient/product or time bounds cannot establish strict positivity.
    #[error("spectral coefficient, product or time enclosure is not separated positive")]
    NotSeparated,
    /// Symbol, reference, map, companion target or arithmetic revision differs.
    #[error("spectral execution identity mismatch")]
    Identity,
}

/// A finite center with total and numerical-only radii about the same original nominal target.
#[derive(Clone, Copy, Debug)]
pub struct SpectralValue<P: Scalar> {
    /// Rounded candidate in the declared coordinate.
    pub center: P,
    /// Complete deterministic radius, already including the numerical error.
    pub total: P,
    /// Numerical-only radius; it must not be added to the total again.
    pub error: P,
}

/// Both conjugate rates and the characteristic scale, with complete original live dependencies.
///
/// This immutable evaluation publishes no heap reading. The real parts are proved exact zero;
/// `imaginary_rates` orders the positive imaginary root before its conjugate, independently of
/// CAS selection order. An explicitly attached original Jacobian and joint model additionally
/// supply local linear covariance without changing these deterministic endpoints.
#[derive(Clone, Debug)]
pub struct LcSpectralEvaluation<P: Scalar> {
    /// Prechecked original law, exact constants and original selected inputs.
    plan: LcSpectrum<P>,
    /// Normalized positive imaginary magnitude, restored rate and characteristic impedance.
    values: [SpectralValue<P>; 3],
    /// Original physical versus represented-input coverage scopes.
    coverage: [Coverage; 2],
    /// Arithmetic theorem revision used by every enclosure.
    revision: u32,
    /// Caller usage before and after complete numerical execution.
    usage: [BudgetUsage; 2],
}

impl<P: Scalar + 'static> LcSpectralEvaluation<P> {
    /// Revalidated normalized positive imaginary magnitude in dimensionless time.
    /// # Errors
    /// Refuses any stale source, companion or arithmetic revision.
    pub fn normalized(
        &self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<SpectralValue<P>, SpectralError> {
        self.validate(heap, context)?;
        Ok(self.values[0])
    }
    /// Both physical imaginary rates in inverse seconds, positive first, each with multiplicity one.
    /// # Errors
    /// Refuses stale original input or companion versions and changed arithmetic revisions.
    pub fn imaginary_rates(
        &self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<[SpectralValue<P>; 2], SpectralError> {
        self.validate(heap, context)?;
        let positive = self.values[1];
        let negative = SpectralValue {
            center: -positive.center,
            ..positive
        };
        Ok([positive, negative])
    }
    /// Positive characteristic impedance `sqrt(L/C)` in ohms with both restored error channels.
    /// # Errors
    /// Refuses stale original input or companion versions and changed arithmetic revisions.
    pub fn characteristic_scale(
        &self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<SpectralValue<P>, SpectralError> {
        self.validate(heap, context)?;
        Ok(self.values[2])
    }
    /// Original exact law, numerical schedule and selected source bindings.
    #[must_use]
    pub const fn plan(&self) -> &LcSpectrum<P> {
        &self.plan
    }
    /// Original coverage scopes; a generated radius cannot promote represented input to a physical bound.
    #[must_use]
    pub const fn coverage(&self) -> [Coverage; 2] {
        self.coverage
    }
    /// Cumulative execution usage, separate from exact proof replay and schedule lowering.
    #[must_use]
    pub const fn usage(&self) -> [BudgetUsage; 2] {
        self.usage
    }
    /// Every original source and radius is live even when its center has unchanged bits.
    fn validate(&self, heap: &Heap, context: &ArithmeticContext) -> Result<(), SpectralError> {
        if self.revision != context.revision() {
            return Err(SpectralError::Identity);
        }
        self.plan.validate(heap)
    }
}

impl Heap {
    /// Enclose the complete positive two-state family and restore both rates through its time map.
    /// # Errors
    /// Refuses stale or foreign bindings, nonpositive or zero-containing boxes, insufficient
    /// numerical separation, nonfinite arithmetic and exhausted caller resources.
    pub fn evaluate_lc_spectrum<P: Scalar + 'static>(
        &self,
        plan: &LcSpectrum<P>,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<LcSpectralEvaluation<P>, SpectralError> {
        let before = budget.usage();
        plan.validate(self)?;
        let bytes = statistics::evaluation_bytes(plan)?;
        let (values, coverage, moments) = budget.with_retained_payload(bytes, AT, |budget| {
            let constants = numeric::constants(plan, context, budget)?;
            let (inputs, coverage) = input::capture(self, plan, budget)?;
            Ok::<_, SpectralError>((
                numeric::evaluate(&inputs, &constants, context, budget)?,
                coverage,
                statistics::evaluate(self, plan, &constants, context, budget)?,
            ))
        })?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        Ok(LcSpectralEvaluation {
            plan: statistics::evaluated(plan, moments),
            values,
            coverage,
            revision: context.revision(),
            usage: [before, budget.usage()],
        })
    }
}
