// SPDX-License-Identifier: AGPL-3.0-only
//! Exclusive checked bindings, frozen dependencies and private output reservations.
//!
//! # Execution lease and private reservations
//!
//! `Binding::new` requires a `CheckedOperation`. `validate` takes the exclusive heap borrow and captures every actual value identity/version, schema, logical selection and declared-bound occupation, schema and target version. It checks every concrete slot and structural zero, including zero radii for `WholeFamily` zeros. Inputs can overlap or repeat. `ExecutionLimits` bounds the slot count and the total selected centers plus carried companions per scan or private payload set. Heap limits also apply to the complete output reservation.
//!
//! `Bound` owns the checked evidence and real private destination slabs and descriptor reservations. It uses the existing output proof to establish injectivity and mutual disjointness for all destination regions, including companions. `output_reservations` exposes future identities and schemas, never output handles. Dropping a binding releases the reservations without publishing a value. Binding preparation can coexist with one temporary zero payload set and its reserved backend slabs; both sets are bounded by the scratch entry limit. No caller callback runs in publication preparation.
//!
//! `physical_input` lends an immutable `PhysicalView`. `normalized_inputs` lends the complete operation-ordered `NormalizedInputs` bundle with the binding's unique execution identity and its explicit coherent unit-reference interpretation. Companion entries follow exactly the same selected logical map. Neither view grants raw mutation. The exclusive binding and its shorter borrows prevent mutation, growth, compaction and another execution through the same heap until they end. Execution and reserved value identities use the checked nonwrapping process-local allocator. A published ordered shape retains its coordinate identities: a later binding must use those same ordered axes. A projected alias of a derived value needs a matching supported shape contract; matching dimensions or newly repeated coordinate names cannot reinterpret an output.
//!
//! ```rust
//! use nrz_matrix::{Binding, CheckedShape, ExecutionLimits, Handle, Heap, OperationDraft, Value};
//!
//! fn copy_value(heap: &mut Heap, shape: CheckedShape, input: Handle<Value, f64>)
//!     -> Result<Handle<Value, f64>, Box<dyn std::error::Error>>
//! {
//!     let limits = ExecutionLimits::default();
//!     let operation = OperationDraft::copy(vec![shape]).check(limits)?;
//!     let bound = Binding::new(operation, vec![input], limits).validate(heap)?;
//!     let candidate = bound.evaluate(bound.normalized_inputs()?)?;
//!     let outcome = bound.publish(candidate)?;
//!     Ok(outcome.output().first().ok_or("missing copy output")?.value())
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod error;
pub(crate) mod lifecycle;
mod limits;
mod operation;
mod reservation;
mod snapshot;
pub(crate) mod symbols;
mod views;

pub use error::OperationError;
pub use lifecycle::{SymbolState, ValueBinding};
pub use limits::ExecutionLimits;
pub(crate) use operation::NativeRecipe;
pub use operation::{CheckedOperation, OperationDraft, RegionContract};
pub use reservation::OutputReservation;
pub use snapshot::{CompanionSnapshot, InputSnapshot};
pub use views::{NormalizedInputs, NormalizedView, PhysicalView};

use crate::heap::identity::identity;
use crate::{Handle, Heap, HeapError, Value};
use nrz_scalar::Scalar;
use reservation::Reservations;

/// Unique process-local identity of one validated execution, never a buffer address.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionId(pub(crate) u64);

/// Proposed concrete operands; only validation can lend numerical inputs.
#[derive(Debug)]
pub struct Binding<P: Scalar, Q: Scalar = P> {
    /// Private checked recipe, never an unchecked draft.
    pub(crate) operation: CheckedOperation,
    /// Optional exact selected bindings supplied through symbol lookup.
    pub(crate) symbol_inputs: Option<Vec<ValueBinding<P>>>,
    /// Operation-ordered input locators.
    pub(crate) inputs: Vec<Handle<Value, P>>,
    /// Explicit work and private scratch limits.
    pub(crate) limits: ExecutionLimits,
    /// Output scalar type, fixed independently of the input handle type.
    pub(crate) output: std::marker::PhantomData<Q>,
}

impl<P: Scalar + 'static> Binding<P> {
    /// Bind a checked recipe to concrete operands, with no access granted yet.
    #[must_use]
    pub const fn new(
        operation: CheckedOperation,
        inputs: Vec<Handle<Value, P>>,
        limits: ExecutionLimits,
    ) -> Self {
        Self {
            operation,
            symbol_inputs: None,
            inputs,
            limits,
            output: std::marker::PhantomData,
        }
    }
}

impl<P: Scalar + 'static> Binding<P> {
    /// Bind a checked recipe to exact symbol-selected inputs.
    #[must_use]
    pub fn from_symbols(
        operation: CheckedOperation,
        inputs: Vec<ValueBinding<P>>,
        limits: ExecutionLimits,
    ) -> Self {
        Self {
            inputs: inputs.iter().map(ValueBinding::handle).collect(),
            symbol_inputs: Some(inputs),
            operation,
            limits,
            output: std::marker::PhantomData,
        }
    }
}

impl<P: Scalar + 'static, Q: Scalar + 'static> Binding<P, Q> {
    /// Freeze actual dependencies and reserve complete fresh disjoint output bundles.
    ///
    /// Dropping the result discards all reservations without publishing any handle.
    /// # Errors
    /// Refuses stale/foreign/category/precision mismatches, schemas, structure or limits.
    pub fn validate(self, heap: &mut Heap) -> Result<Bound<'_, P, Q>, OperationError> {
        match self.operation.recipe {
            NativeRecipe::Precision { source, target }
                if source == P::PRECISION && target == Q::PRECISION => {}
            NativeRecipe::Copy
            | NativeRecipe::Affine(_)
            | NativeRecipe::Kernel(_)
            | NativeRecipe::Solve(_)
                if P::PRECISION == Q::PRECISION => {}
            _ => return Err(HeapError::PrecisionMismatch.into()),
        }
        if self.inputs.len() != self.operation.inputs.len() {
            return Err(OperationError::InputCount);
        }
        self.limits.check(self.inputs.len(), 0)?;
        if let NativeRecipe::Solve(contract) = &self.operation.recipe {
            contract.preflight(heap, &self.inputs, self.limits)?;
        }
        if let Some(symbols) = &self.symbol_inputs {
            for binding in symbols {
                heap.value_by_symbol(binding.symbol(), binding)?;
            }
        }
        let dependencies = capture(heap, &self.inputs)?;
        let entries = payload_entries(&dependencies)?;
        if let NativeRecipe::Kernel(contract) = &self.operation.recipe {
            self.limits.check(self.inputs.len(), contract.work)?;
        }
        let output_entries = Reservations::<Q>::payload_entries(&dependencies, &self.operation)?;
        self.limits.check(self.inputs.len(), output_entries)?;
        self.limits.check(self.inputs.len(), entries)?;
        for (slot, (dependency, contract)) in dependencies
            .iter()
            .zip(self.operation.inputs.iter())
            .enumerate()
        {
            dependency.validate(heap, contract, slot)?;
            heap.validate_bound_shape(dependency.handle(), contract, slot)?;
        }
        let statistics =
            crate::statistics::Plan::capture(heap, &self.operation, &self.inputs, self.limits)?;
        let execution = ExecutionId(identity()?);
        let reservations = Reservations::prepare(heap, &dependencies, &self.operation)?;
        reservations.validate(heap)?;
        Ok(Bound {
            heap,
            operation: self.operation,
            dependencies,
            statistics,
            execution,
            reservations,
            issued: std::cell::Cell::new(None),
        })
    }
}

/// Evidence-owning execution lease; every view borrows for less than this exclusive borrow.
#[derive(Debug)]
pub struct Bound<'h, P: Scalar, Q: Scalar = P> {
    /// The only mutable access to the owner throughout this execution.
    pub(crate) heap: &'h mut Heap,
    /// Checked original recipe and exact normalization references.
    pub(crate) operation: CheckedOperation,
    /// Actual ordered input versions, schemas, selections and companion targets.
    pub(crate) dependencies: Vec<InputSnapshot<P>>,
    /// Complete source model and checked statistical execution allowance.
    pub(crate) statistics: crate::statistics::Plan<P>,
    /// Unforgeable association between a binding and its borrowed numerical inputs.
    pub(crate) execution: ExecutionId,
    /// Fully private output storage and descriptor occupations.
    pub(crate) reservations: Reservations<Q>,
    /// The one issued candidate token, unavailable before successful evaluation.
    pub(crate) issued: std::cell::Cell<Option<u64>>,
}

impl<P: Scalar, Q: Scalar> Bound<'_, P, Q> {
    /// Execution identity shared by this binding's views and later candidates.
    #[must_use]
    pub const fn execution(&self) -> ExecutionId {
        self.execution
    }
    /// Checked native recipe and normalization definitions.
    #[must_use]
    pub const fn operation(&self) -> &CheckedOperation {
        &self.operation
    }
    /// Actual operation-ordered dependencies, including companion-only evidence.
    #[must_use]
    pub fn dependencies(&self) -> &[InputSnapshot<P>] {
        &self.dependencies
    }
    /// Future output versions and interpretations; no value handles escape here.
    #[must_use]
    pub fn output_reservations(&self) -> &[OutputReservation] {
        &self.reservations.outputs
    }
}

/// Capture metadata before scanning numerical entries under the total work limit.
fn capture<P: Scalar + 'static>(
    heap: &Heap,
    inputs: &[Handle<Value, P>],
) -> Result<Vec<InputSnapshot<P>>, OperationError> {
    let mut snapshots = buffer(inputs.len())?;
    for &handle in inputs {
        snapshots.push(InputSnapshot::capture(heap, handle)?);
    }
    Ok(snapshots)
}

/// Count centers and all carried companions without confusing base and selected extents.
fn payload_entries<P: Scalar>(inputs: &[InputSnapshot<P>]) -> Result<usize, OperationError> {
    inputs.iter().try_fold(0_usize, |total, input| {
        let count = input
            .layout()
            .extent()
            .entries()
            .checked_mul(
                1 + usize::from(input.companion().is_some())
                    + usize::from(input.evaluation.is_some())
                    + 3 * usize::from(input.contributions.is_some()),
            )
            .ok_or(OperationError::ResourceLimit)?;
        total
            .checked_add(count)
            .ok_or(OperationError::ResourceLimit)
    })
}

/// Fallibly reserve a metadata or scalar vector before entering an append-only phase.
pub(crate) fn buffer<T>(count: usize) -> Result<Vec<T>, HeapError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| HeapError::Capacity)?;
    Ok(result)
}
