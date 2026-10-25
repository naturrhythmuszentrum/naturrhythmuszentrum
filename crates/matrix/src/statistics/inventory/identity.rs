// SPDX-License-Identifier: AGPL-3.0-only
//! Equal shapes cannot substitute a different original operation, model owner or source law.

use super::Run;
use crate::{
    CheckedOperation, OperationError, PrivateAllocationError as Error, binding::NativeRecipe,
    statistics::store::Lineage,
};
use std::rc::Rc;

impl Run {
    /// Evaluation and publication retain the exact immutable original checked recipe.
    pub(crate) fn validate_operation(
        &self,
        actual: &CheckedOperation,
    ) -> Result<(), OperationError> {
        let original = self.0.operation.as_ref().ok_or(Error::ForeignInventory)?;
        let same = original.revision == actual.revision
            && Rc::ptr_eq(&original.inputs, &actual.inputs)
            && Rc::ptr_eq(&original.outputs, &actual.outputs)
            && same_recipe(&original.recipe, &actual.recipe);
        if !same {
            return Err(Error::ForeignInventory.into());
        }
        Ok(())
    }
    /// The retained law and complete ordered source union come only from the original captured bindings.
    pub(crate) fn validate_lineage(&self, actual: &Lineage) -> Result<(), OperationError> {
        let original = self.binding(0)?;
        let same_model = Rc::ptr_eq(&original.lineage.model.data, &actual.model.data)
            && original.lineage.model.id == actual.model.id
            && original.lineage.model.sources == actual.model.sources
            && original.lineage.model.terms == actual.model.terms;
        let same_columns = actual.columns == self.0.columns;
        let same_law = actual.assurance == self.0.assurance.get();
        let ordered_roots = actual.roots.windows(2).all(|pair| pair[0] < pair[1]);
        let original_roots = actual.roots.iter().all(|root| {
            self.0
                .bindings
                .iter()
                .any(|input| input.lineage.roots.binary_search(root).is_ok())
        });
        let complete_roots = self.0.bindings.iter().all(|input| {
            input
                .lineage
                .roots
                .iter()
                .all(|root| actual.roots.binary_search(root).is_ok())
        });
        if !same_model
            || !same_columns
            || !same_law
            || !ordered_roots
            || !original_roots
            || !complete_roots
        {
            return Err(Error::ForeignInventory.into());
        }
        Ok(())
    }
}
/// Shared original closed definitions are compared by their declaration owner, never reconstructed values.
fn same_recipe(original: &NativeRecipe, actual: &NativeRecipe) -> bool {
    match (original, actual) {
        (NativeRecipe::Copy, NativeRecipe::Copy) => true,
        (NativeRecipe::Kernel(left), NativeRecipe::Kernel(right)) => Rc::ptr_eq(left, right),
        (NativeRecipe::Solve(left), NativeRecipe::Solve(right)) => Rc::ptr_eq(left, right),
        (NativeRecipe::Affine(left), NativeRecipe::Affine(right)) => Rc::ptr_eq(left, right),
        (
            NativeRecipe::Precision {
                source: left,
                target: first,
            },
            NativeRecipe::Precision {
                source: right,
                target: second,
            },
        ) => left == right && first == second,
        (
            NativeRecipe::Reciprocal {
                kernel: left,
                definition: first,
            },
            NativeRecipe::Reciprocal {
                kernel: right,
                definition: second,
            },
        ) => Rc::ptr_eq(left, right) && Rc::ptr_eq(first, second),
        (
            NativeRecipe::ReciprocalStatistics {
                kernel: left,
                definition: first,
            },
            NativeRecipe::ReciprocalStatistics {
                kernel: right,
                definition: second,
            },
        ) => Rc::ptr_eq(left, right) && Rc::ptr_eq(first, second),
        _ => false,
    }
}
