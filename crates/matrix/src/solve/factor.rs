// SPDX-License-Identifier: AGPL-3.0-only
//! Opaque execution-scoped factors never become ordinary value handles.

use super::backend::Lu;
use crate::solve_verification::{InverseWitness, SolveWork};
use crate::{Bound, ExecutionId, Failure, NormalizedView, OperationError, SolveError, ValueStamp};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Opaque LU and independently validated inverse witness for one frozen execution.
///
/// The factors have no exported entries or forward-accuracy claim. Their inverse
/// witness proves an operator norm bound; it is not a published inverse value.
pub struct FactorRef<P: Scalar> {
    /// Execution whose checked operator was packed.
    pub(crate) execution: ExecutionId,
    /// Exact operator identity and version in that execution.
    operator: ValueStamp,
    /// Private faer factors and permutation.
    pub(crate) backend: Lu,
    /// Independent inverse-defect proof against the saved heap operator.
    pub(crate) witness: InverseWitness<P>,
}

impl<P: Scalar> std::fmt::Debug for FactorRef<P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FactorRef")
            .field("execution", &self.execution)
            .field("operator", &self.operator)
            .finish_non_exhaustive()
    }
}

impl<P: Scalar> FactorRef<P> {
    /// Execution that owns the factor's normalized coordinate contract.
    #[must_use]
    pub const fn execution(&self) -> ExecutionId {
        self.execution
    }
    /// Exact mathematical operator identity and content version.
    #[must_use]
    pub const fn operator(&self) -> ValueStamp {
        self.operator
    }
    /// Independently certified inverse spectral norm in the execution's coordinates.
    #[must_use]
    pub const fn inverse_norm(&self) -> P {
        self.witness.inverse
    }
}

impl<P: Scalar + 'static, Q: Scalar + 'static> Bound<'_, P, Q> {
    /// Factor this solve's checked operator and certify its private inverse witness.
    ///
    /// The resource can be used by `evaluate_factored` in this same execution.
    /// Cross-execution reuse and factor-to-value export are not admitted here.
    /// # Errors
    /// Refuses another recipe, backend failure or an unresolved inverse defect.
    pub fn factor(&self) -> Result<FactorRef<P>, Failure<P>> {
        self.make_factor()
            .map_err(|cause| self.failure(cause, Vec::new()))
    }

    /// Keep normalized borrowing and factor construction behind the checked solve recipe.
    fn make_factor(&self) -> Result<FactorRef<P>, OperationError> {
        let crate::binding::NativeRecipe::Solve(contract) = &self.operation.recipe else {
            return Err(SolveError::Contract.into());
        };
        super::recovery::preflight(contract, self.statistics.work)?;
        let inputs = self.normalized_inputs()?;
        let operator = inputs.views().first().ok_or(OperationError::InputCount)?;
        let stamp = self
            .dependencies
            .first()
            .ok_or(OperationError::InputCount)?
            .stamp();
        build(operator, self.execution, stamp, contract.work)
    }
}

/// The candidate inverse remains local and is discarded after its independent proof.
pub(crate) fn build<P: Scalar + 'static>(
    operator: &NormalizedView<'_, P>,
    execution: ExecutionId,
    stamp: ValueStamp,
    work: usize,
) -> Result<FactorRef<P>, OperationError> {
    let context = ArithmeticContext::new().map_err(SolveError::from)?;
    let backend = Lu::factor(operator)?;
    let candidate = backend.inverse::<P>(operator.extent().rows())?;
    let witness = crate::solve_verification::inverse(
        operator,
        &candidate,
        &mut SolveWork::new(work),
        &context,
    )?;
    Ok(FactorRef {
        execution,
        operator: stamp,
        backend,
        witness,
    })
}
