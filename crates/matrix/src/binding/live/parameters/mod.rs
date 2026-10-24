// SPDX-License-Identifier: AGPL-3.0-only
//! Execute companion maps from explicitly bound original parameters and their numerical errors.

pub(super) mod numeric;
mod phasor;
mod plan;
pub use phasor::PhasorStatistics;

pub use plan::CompanionExecution;

use super::{LiveBindingError, ReadingBinding};
use crate::{Heap, HeapError};
use nrz_bindings::readings::algebra::{Fault, Location, limits::BudgetUsage};
use nrz_bindings::{
    Budget,
    readings::{CompanionReadingError, ParameterErrorReading},
};
use nrz_scalar::{ArithmeticContext, Scalar};
use std::rc::Rc;

/// Shared caller-ledger location for companion preparation, lowering and evaluation.
const AT: Location = Location::new("source companion execution");

/// A companion execution refusal leaves every output unavailable.
#[derive(Clone, Debug, thiserror::Error)]
pub enum CompanionExecutionError {
    /// Original symbols, ordered targets or parameter units disagree.
    #[error("companion source definition: {0}")]
    Definition(#[from] CompanionReadingError),
    /// A selected source, error parameter or target is no longer live.
    #[error("companion source binding: {0}")]
    Live(#[from] LiveBindingError),
    /// Numerical storage or finite allocation refused.
    #[error("companion source storage: {0}")]
    Heap(#[from] HeapError),
    /// Exact construction or cumulative resource accounting refused.
    #[error("companion source resources: {0}")]
    Exact(#[from] Fault),
    /// Outward arithmetic or coefficient import refused.
    #[error("companion source arithmetic: {0}")]
    Arithmetic(#[from] nrz_scalar::ArithmeticFailure),
    /// The ordered targets, input coverage or arithmetic identity differ.
    #[error("companion source execution identity mismatch")]
    Identity,
}

/// Separate lowering and arithmetic charges on the same cumulative caller ledger.
#[derive(Clone, Debug)]
pub struct CompanionExecutionUsage {
    /// Before coefficient import, including every earlier caller charge.
    pub before_lowering: BudgetUsage,
    /// After exact coefficient import and before numerical source access.
    pub before_evaluation: BudgetUsage,
    /// After complete output construction.
    pub after: BudgetUsage,
}

/// Complete immutable parameter result with both original sources and all ordered target versions.
#[derive(Clone, Debug)]
pub struct CompanionEvaluation<P: Scalar> {
    /// Exact law and live source/target dependency closure, shared without another graph.
    execution: CompanionExecution<P>,
    /// Complete private centers and numerical-error radii.
    payload: Rc<numeric::Values<P>>,
    /// Arithmetic theorem revision used for coefficient import and evaluation.
    revision: u32,
    /// Separate cumulative lowering and evaluation usage snapshots.
    usage: CompanionExecutionUsage,
}

impl<P: Scalar + 'static> CompanionEvaluation<P> {
    /// Read all parameter centers only while both sources and every target remain current.
    /// # Errors
    /// Refuses stale or foreign inputs and changed arithmetic revisions.
    pub fn values<'a>(
        &'a self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<&'a [P], CompanionExecutionError> {
        self.validate(heap, context)?;
        Ok(&self.payload.centers)
    }
    /// Numerical parameter error includes source error, coefficient import and contraction rounding.
    /// # Errors
    /// Refuses stale or foreign inputs and changed arithmetic revisions.
    pub fn errors<'a>(
        &'a self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<&'a [P], CompanionExecutionError> {
        self.validate(heap, context)?;
        Ok(&self.payload.errors)
    }
    /// Retained exact companion law and source parameter identity.
    #[must_use]
    pub fn exact(&self) -> &ParameterErrorReading {
        self.execution.exact()
    }
    /// Original parameter and parameter-error bindings, in that order.
    #[must_use]
    pub fn sources(&self) -> &[ReadingBinding<P>; 2] {
        self.execution.sources()
    }
    /// Ordered output targets; two-sided covariance retains both selections and shared family.
    #[must_use]
    pub fn targets(&self) -> &[ReadingBinding<P>] {
        self.execution.targets()
    }
    /// Complete cumulative usage, without a per-phase ledger reset.
    #[must_use]
    pub const fn usage(&self) -> &CompanionExecutionUsage {
        &self.usage
    }
    /// Revalidate the complete numerical closure, even when current center bytes agree.
    fn validate(
        &self,
        heap: &Heap,
        context: &ArithmeticContext,
    ) -> Result<(), CompanionExecutionError> {
        if self.revision != context.revision() {
            return Err(CompanionExecutionError::Identity);
        }
        self.execution.validate(heap)
    }
}

impl Heap {
    /// Evaluate a checked companion from its original source parameters exactly once.
    ///
    /// The returned snapshot is complete or absent. It retains both covariance target versions
    /// and the original shared-source map. This evaluates declared parameters; it does not
    /// establish positive semidefiniteness or a statistical model from their numerical values.
    /// # Errors
    /// Refuses stale targets, missing source errors, negative radii, overflow and cumulative limits.
    pub fn evaluate_companion<P: Scalar + 'static>(
        &self,
        execution: &CompanionExecution<P>,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<CompanionEvaluation<P>, CompanionExecutionError> {
        execution.validate(self)?;
        let before_lowering = budget.usage();
        let (payload, before_evaluation) =
            budget.with_retained_payload(execution.bytes(), AT, |budget| {
                numeric::evaluate(self, execution, context, budget)
            })?;
        Ok(CompanionEvaluation {
            execution: execution.clone(),
            payload: Rc::new(payload),
            revision: context.revision(),
            usage: CompanionExecutionUsage {
                before_lowering,
                before_evaluation,
                after: budget.usage(),
            },
        })
    }
}
