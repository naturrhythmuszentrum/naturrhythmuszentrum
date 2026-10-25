// SPDX-License-Identifier: AGPL-3.0-only
//! Native reciprocals consume one original evaluated Jacobian and retain first-order assurance.

use super::{Affine, StatisticsError, inventory::Role};
use crate::binding::NativeRecipe;
use crate::{
    Binding, CheckedOperation, Handle, Heap, HeapError, OperationError,
    ReciprocalDerivativeContract, ReciprocalDerivativeError, ReciprocalDerivativeEvaluation, Value,
};
use nrz_bindings::{
    Budget,
    readings::{PositiveReciprocalMap, ReciprocalJacobian},
};
use nrz_scalar::{ArithmeticContext, Scalar};
use std::rc::Rc;

/// Exact operation evidence shared by execution and historical records, with no point arrays.
#[derive(Debug)]
pub(crate) struct Definition {
    /// Original typed operator, references, positive domain and correspondence.
    pub(crate) map: Rc<PositiveReciprocalMap>,
    /// Original shared source derivative; numerical point storage belongs to the binding.
    pub(crate) derivative: ReciprocalDerivativeContract,
}

impl<P: Scalar + 'static> Binding<P> {
    /// Supply the original evaluated Jacobian for first-order reciprocal covariance transport.
    ///
    /// The evaluation must name every physical input as a derivative direction. Admission
    /// rechecks its exact selected readings and versions. Output covariance describes the
    /// local linear model only; it supplies neither nonlinear means nor support bounds.
    /// # Errors
    /// Refuses other recipes, substituted derivative definitions, omitted directions and
    /// exhausted caller resources. Live point and source-model checks remain at admission.
    pub fn with_reciprocal_statistics(
        mut self,
        evaluation: ReciprocalDerivativeEvaluation<P>,
        budget: &mut Budget,
    ) -> Result<Self, ReciprocalDerivativeError> {
        let NativeRecipe::Reciprocal { kernel, definition } = &self.operation.recipe else {
            return Err(ReciprocalDerivativeError::Unsupported);
        };
        check_definition(definition, &evaluation, budget)?;
        self.operation.recipe = NativeRecipe::ReciprocalStatistics {
            kernel: Rc::clone(kernel),
            definition: Rc::new(Definition {
                map: Rc::clone(definition),
                derivative: evaluation.contract().clone(),
            }),
        };
        self.reciprocal_statistics = Some(Rc::new(evaluation));
        Ok(self)
    }
}

/// Original mathematical identity is checked without rebuilding or replaying another derivative.
fn check_definition<P: Scalar + 'static>(
    definition: &PositiveReciprocalMap,
    evaluation: &ReciprocalDerivativeEvaluation<P>,
    budget: &mut Budget,
) -> Result<(), ReciprocalDerivativeError> {
    use nrz_bindings::readings::algebra::{Location, limits::BudgetKind};
    const AT: Location = Location::new("native reciprocal statistics");
    let exact = evaluation.contract().exact();
    let count = definition
        .reuse_key()
        .as_bytes()
        .len()
        .checked_add(definition.inputs().len())
        .ok_or(HeapError::ExtentOverflow)?;
    budget.work(
        u64::try_from(count).map_err(|_| HeapError::ExtentOverflow)?,
        AT,
    )?;
    if definition.reuse_key() != exact.definition().reuse_key()
        || definition.inputs() != exact.definition().inputs()
        || definition.frame().context() != exact.definition().frame().context()
        || definition.frame().entries() != exact.definition().frame().entries()
        || exact.key().positions().len() != definition.inputs().len()
        || exact
            .key()
            .positions()
            .iter()
            .copied()
            .ne(0..definition.inputs().len())
    {
        return Err(ReciprocalDerivativeError::Identity);
    }
    let metadata = size_of::<Definition>()
        + size_of::<ReciprocalDerivativeEvaluation<P>>()
        + 4 * size_of::<usize>();
    let bytes = evaluation
        .native_payload_bytes()?
        .checked_add(u64::try_from(metadata).map_err(|_| HeapError::ExtentOverflow)?)
        .ok_or(HeapError::ExtentOverflow)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.check(BudgetKind::OutputBytes, bytes, AT)?;
    Ok(())
}

impl CheckedOperation {
    /// Shared original source Jacobian retained by the native first-order reciprocal recipe.
    #[must_use]
    pub fn reciprocal_jacobian(&self) -> Option<&ReciprocalJacobian> {
        match &self.recipe {
            NativeRecipe::ReciprocalStatistics { definition, .. } => {
                Some(definition.derivative.exact())
            }
            _ => None,
        }
    }
}

/// Point data never substitutes another reading, owner, selected version or derivative identity.
pub(super) fn validate<P: Scalar + 'static>(
    heap: &Heap,
    operation: &CheckedOperation,
    handles: &[Handle<Value, P>],
    evaluation: Option<&ReciprocalDerivativeEvaluation<P>>,
) -> Result<(), OperationError> {
    match (&operation.recipe, evaluation) {
        (NativeRecipe::ReciprocalStatistics { definition, .. }, Some(evaluation)) => {
            if !std::ptr::eq(
                definition.derivative.exact().native(),
                evaluation.contract().exact().native(),
            ) {
                return Err(StatisticsError::Contract.into());
            }
            evaluation.validate_native(heap, handles, &ArithmeticContext::new()?)?;
            Ok(())
        }
        (NativeRecipe::ReciprocalStatistics { .. }, None) | (_, Some(_)) => {
            Err(StatisticsError::Contract.into())
        }
        (_, None) => Ok(()),
    }
}

/// Copy enclosed original partials into the predeclared Jacobian owners before contraction.
pub(super) fn map<P: Scalar + 'static, Q: Scalar + 'static>(
    operation: &CheckedOperation,
    evaluation: Option<&ReciprocalDerivativeEvaluation<P>>,
    context: &ArithmeticContext,
    run: &super::inventory::Run,
) -> Result<Affine<Q>, OperationError> {
    let evaluation = evaluation.ok_or(StatisticsError::Contract)?;
    let rows = operation.outputs()[0].shape().extent().entries();
    let columns = operation.inputs().len();
    let values = evaluation.native_values();
    if values.len()
        != rows
            .checked_mul(columns)
            .ok_or(StatisticsError::ResourceLimit)?
    {
        return Err(StatisticsError::Contract.into());
    }
    let mut result = Affine {
        jacobian: run.matrix_at(Role::Jacobian, 0, crate::Extent::new(rows, columns)?)?,
        offsets: run.matrix_at(Role::Offsets, 0, crate::Extent::new(rows, 1)?)?,
    };
    for (index, &truth) in values.iter().enumerate() {
        let (center, enclosure) = super::precision::entry(truth.lower().get(), truth, context)?;
        result
            .jacobian
            .set(index % rows, index / rows, center, enclosure)?;
    }
    Ok(result)
}
