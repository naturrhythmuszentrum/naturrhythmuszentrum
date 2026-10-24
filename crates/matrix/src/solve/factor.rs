// SPDX-License-Identifier: AGPL-3.0-only
//! Opaque factors depend only on the exact checked operator and never become value handles.
//!
//! # Opaque factors and exact reuse dependencies
//!
//! `Bound::factor` returns an opaque `FactorRef` with an independently checked inverse witness, and `Bound::evaluate_factored` accepts it in any execution whose exact operator dependencies match. No LU entries or private inverse candidate can be exported as a certified value. The factor's identity and inverse-norm accessor report historical facts of its original factorization execution. The ordinary `evaluate` path creates the same private resource internally. All success paths use the existing single-candidate, complete-bundle publication protocol and retain both operand dependencies; a failed inverse check, singular candidate or work limit publishes nothing.
//!
//! `FactorDependency` records the exact operator identity/version, immutable heap schema, selected logical region, precision, checked operator recipe revision, LU algorithm and inverse-witness arithmetic/norm revision. Cloning a solve draft or checked operation preserves its operator revision; constructing a new draft creates a new checked revision and conservatively refuses an older factor. Output budgets, restoration, client keys and RHS values can change on a cloned draft without changing that operator revision. `Bound::validate_factor` checks eligibility without claiming a new solve certificate. Physical compaction and unrelated allocation changes preserve eligibility; changed operator versions or selections refuse even when bytes agree. `SolveEvidence::factor_use` names the originating and current executions and the validated key. Every reused solve recomputes its residual, perturbation bound, statistics and budget.

use super::backend::Lu;
use crate::solve_verification::{InverseWitness, SolveWork};
use crate::{Bound, ExecutionId, Failure, NormalizedView, OperationError, SolveError, ValueStamp};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Opaque LU and independently validated inverse witness for one unchanged checked operator.
///
/// The factors have no exported entries or forward-accuracy claim. Their inverse
/// witness proves an operator norm bound; it is not a published inverse value.
pub struct FactorRef<P: Scalar> {
    /// Execution whose checked operator was packed.
    pub(crate) execution: ExecutionId,
    /// Exact operator identity and version in that execution.
    dependency: FactorDependency,
    /// Private faer factors and permutation.
    pub(crate) backend: Lu,
    /// Independent inverse-defect proof against the saved heap operator.
    pub(crate) witness: InverseWitness<P>,
    /// Original allocation binding and complete factor-construction receipts.
    inventory: super::FactorInventory,
}

impl<P: Scalar> std::fmt::Debug for FactorRef<P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FactorRef")
            .field("execution", &self.execution)
            .field("dependency", &self.dependency)
            .finish_non_exhaustive()
    }
}

impl<P: Scalar> FactorRef<P> {
    /// Complete factor-construction receipts; later RHS solve allocations are separate owners.
    #[must_use]
    pub const fn allocation_inventory(&self) -> &super::FactorInventory {
        &self.inventory
    }

    /// Execution that owns the factor's normalized coordinate contract.
    #[must_use]
    pub const fn execution(&self) -> ExecutionId {
        self.execution
    }
    /// Exact mathematical operator identity and content version.
    #[must_use]
    pub const fn operator(&self) -> ValueStamp {
        self.dependency.stamp
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
    /// Later executions may reuse it through `evaluate_factored` only after matching
    /// the exact operator version, selected map, schema and checked recipe revision.
    /// RHS values, statistical source models and output budgets are separate dependencies.
    /// # Errors
    /// Refuses another recipe, backend failure or an unresolved inverse defect.
    pub fn factor(&self) -> Result<FactorRef<P>, Failure<P>> {
        self.make_factor(None)
            .map_err(|cause| self.failure(cause, Vec::new()))
    }

    /// Factor with an explicit caller ceiling for the newly owned allocation plan and receipt.
    ///
    /// Both staged and retained output byte limits are checked before backend allocation. The
    /// numerical work/scratch policy and the caller's other exact-construction limits are unchanged.
    /// # Errors
    /// Refuses exhausted metadata bytes or any ordinary factorization/certification failure.
    pub fn factor_with_inventory_budget(
        &self,
        budget: &nrz_bindings::Budget,
    ) -> Result<FactorRef<P>, Failure<P>> {
        self.make_factor(Some(budget))
            .map_err(|cause| self.failure(cause, Vec::new()))
    }

    /// Keep normalized borrowing and factor construction behind the checked solve recipe.
    fn make_factor(
        &self,
        budget: Option<&nrz_bindings::Budget>,
    ) -> Result<FactorRef<P>, OperationError> {
        let crate::binding::NativeRecipe::Solve(contract) = &self.operation.recipe else {
            return Err(SolveError::Contract.into());
        };
        super::recovery::preflight(contract, self.statistics.work)?;
        let inputs = self.normalized_inputs()?;
        let operator = inputs.views().first().ok_or(OperationError::InputCount)?;
        let input = self
            .dependencies
            .first()
            .ok_or(OperationError::InputCount)?;
        budget.map_or_else(
            || build(operator, self.execution, input, contract),
            |budget| build_with_budget(operator, self.execution, input, contract, budget),
        )
    }
}

/// The candidate inverse remains local and is discarded after its independent proof.
pub(crate) fn build<P: Scalar + 'static>(
    operator: &NormalizedView<'_, P>,
    execution: ExecutionId,
    input: &crate::InputSnapshot<P>,
    contract: &super::SolveContract,
) -> Result<FactorRef<P>, OperationError> {
    let budget = nrz_bindings::Budget::new(nrz_bindings::Limits::default())?;
    build_with_budget(operator, execution, input, contract, &budget)
}

/// The caller's existing byte policy admits metadata before any numerical owner is created.
fn build_with_budget<P: Scalar + 'static>(
    operator: &NormalizedView<'_, P>,
    execution: ExecutionId,
    input: &crate::InputSnapshot<P>,
    contract: &super::SolveContract,
    budget: &nrz_bindings::Budget,
) -> Result<FactorRef<P>, OperationError> {
    let context = ArithmeticContext::new().map_err(SolveError::from)?;
    let dependency = FactorDependency::capture(input, contract, context.revision());
    super::allocations::Run::check_budget(&dependency, budget)?;
    let run = super::allocations::Run::new(execution, input, contract, &dependency)?;
    let backend = Lu::factor(operator, &run)?;
    let candidate = backend.inverse_witness::<P>(operator.extent().rows(), &run)?;
    let mut defects = run.buffer(super::FactorAllocationStage::InverseDefects)?;
    for _ in 0..candidate.len() {
        defects.push(P::ZERO)?;
    }
    let witness = crate::solve_verification::inverse_with_defects(
        operator,
        &candidate,
        &mut defects,
        &mut SolveWork::new(contract.work),
        &context,
    )?;
    drop(defects);
    drop(candidate);
    let inventory = run.complete(backend.inline_bytes(), backend.ticket())?;
    Ok(FactorRef {
        execution,
        dependency,
        backend,
        witness,
        inventory,
    })
}

/// Exact dependency key for the factor and its inverse witness, independent of the RHS.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FactorDependency {
    /// Mathematical operator identity and content version.
    stamp: ValueStamp,
    /// Immutable heap descriptor interpretation and owner scope.
    schema: crate::SchemaId,
    /// Exact selected logical region, stable across physical compaction.
    layout: crate::Layout,
    /// Checked operator shape, original declarations, structure and normalization revision.
    operator_revision: u64,
    /// Backend and verifier representation.
    precision: nrz_scalar::Precision,
    /// Fixed partial-pivoted LU algorithm revision.
    algorithm: u32,
    /// Independent outward arithmetic and normalized Frobenius witness revision.
    arithmetic: u32,
}
impl FactorDependency {
    /// Capture only operator dependencies, excluding every RHS, companion and output policy.
    pub(super) fn capture<P: Scalar>(
        input: &crate::InputSnapshot<P>,
        contract: &super::SolveContract,
        arithmetic: u32,
    ) -> Self {
        Self {
            stamp: input.stamp(),
            schema: input.schema(),
            layout: input.layout().clone(),
            operator_revision: contract.operator_revision,
            precision: P::PRECISION,
            algorithm: 1,
            arithmetic,
        }
    }
    /// Exact operator identity/version served by this resource.
    #[must_use]
    pub const fn stamp(&self) -> ValueStamp {
        self.stamp
    }
    /// Captured homogeneous descriptor revision and heap owner.
    #[must_use]
    pub const fn schema(&self) -> crate::SchemaId {
        self.schema
    }
    /// Exact selected region in logical entry order.
    #[must_use]
    pub const fn layout(&self) -> &crate::Layout {
        &self.layout
    }
    /// Immutable checked operator recipe revision, shared by cloned solve drafts.
    #[must_use]
    pub const fn operator_revision(&self) -> u64 {
        self.operator_revision
    }
    /// Factor and independent witness format.
    #[must_use]
    pub const fn precision(&self) -> nrz_scalar::Precision {
        self.precision
    }
    /// Fixed partial-pivoted LU implementation revision.
    #[must_use]
    pub const fn algorithm_revision(&self) -> u32 {
        self.algorithm
    }
    /// Outward arithmetic and normalized Frobenius witness interpretation revision.
    #[must_use]
    pub const fn arithmetic_revision(&self) -> u32 {
        self.arithmetic
    }
}
/// Factor origin and validated dependency association retained by each new solve certificate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FactorUse {
    /// Original factorization execution, never a previous RHS certificate.
    origin: ExecutionId,
    /// Execution that checked reuse and recomputed its own solve proof.
    execution: ExecutionId,
    /// Exact checked operator key.
    dependency: FactorDependency,
}
impl FactorUse {
    /// Original factor-producing execution.
    #[must_use]
    pub const fn origin(&self) -> ExecutionId {
        self.origin
    }
    /// Current solve execution whose residual and budget are independent.
    #[must_use]
    pub const fn execution(&self) -> ExecutionId {
        self.execution
    }
    /// Whether the factor crossed an execution boundary after dependency validation.
    #[must_use]
    pub fn reused(&self) -> bool {
        self.origin != self.execution
    }
    /// Captured exact operator and witness interpretation.
    #[must_use]
    pub const fn dependency(&self) -> &FactorDependency {
        &self.dependency
    }
}
impl<P: Scalar> FactorRef<P> {
    /// Historical operator key; new use still requires checked binding validation.
    #[must_use]
    pub const fn dependency(&self) -> &FactorDependency {
        &self.dependency
    }
    /// Check actual operator inputs while deliberately excluding RHS and companion revisions.
    pub(crate) fn validate(
        &self,
        input: &crate::InputSnapshot<P>,
        contract: &super::SolveContract,
    ) -> Result<(), OperationError> {
        let context = ArithmeticContext::new().map_err(SolveError::from)?;
        if self.dependency != FactorDependency::capture(input, contract, context.revision()) {
            return Err(SolveError::ForeignFactor.into());
        }
        Ok(())
    }
    /// A successful key comparison is recorded separately from any numerical solve proof.
    pub(crate) fn used_by(&self, execution: ExecutionId) -> FactorUse {
        FactorUse {
            origin: self.execution,
            execution,
            dependency: self.dependency.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::solve_testing::{TestResult, draft};
    use crate::{Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationError, SolveError};
    use nrz_bindings::Descriptor;

    /// A private witness with a substituted arithmetic revision cannot authorize factor reuse.
    /// ´test:unit:factor-reuse-checks-witness-environment-revision´
    #[test]
    fn factor_reuse_checks_witness_environment_revision() -> TestResult {
        let mut heap = Heap::new(HeapLimits::default())?;
        let source =
            heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
        let limits = ExecutionLimits::default();
        let bound = Binding::new(draft(1, 1)?.check(limits)?, vec![source.value(); 2], limits)
            .validate(&mut heap)?;
        let mut factor = bound.factor()?;
        bound.validate_factor(&factor)?;
        factor.dependency.arithmetic = 2;
        let refused = bound
            .validate_factor(&factor)
            .err()
            .ok_or("substituted witness admitted")?;
        assert!(matches!(
            refused.cause(),
            OperationError::Solve(SolveError::ForeignFactor)
        ));
        Ok(())
    }
}
