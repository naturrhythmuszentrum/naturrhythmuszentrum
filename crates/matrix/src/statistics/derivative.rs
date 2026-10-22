// SPDX-License-Identifier: AGPL-3.0-only
//! Bindings supplies replayed native derivative identities; heap numbers never enter the graph.

use super::StatisticsError;
use crate::{CheckedOperation, Elementwise, OperationError, binding::NativeRecipe};
use nrz_bindings::{
    CheckedSensitivity, Context, Declaration, Evidence, SensitivityError, SensitivityRecipe,
};
use std::rc::Rc;

/// Build one schema-level native proof reused over every homogeneous logical entry.
pub(crate) fn check(
    operation: &CheckedOperation,
) -> Result<Option<Rc<CheckedSensitivity>>, OperationError> {
    if let NativeRecipe::Solve(contract) = &operation.recipe {
        return crate::solve::sensitivity::check(contract).map(Some);
    }
    let NativeRecipe::Kernel(contract) = &operation.recipe else {
        return Ok(None);
    };
    let crate::kernels::contract::Recipe::Elementwise(function) = &contract.recipe else {
        return Ok(None);
    };
    let function = *function;
    let input = operation
        .inputs()
        .first()
        .ok_or(StatisticsError::Contract)?;
    let output = operation
        .outputs()
        .first()
        .ok_or(StatisticsError::Contract)?;
    let keys: &[&str] = if function == Elementwise::SquareRoot {
        &["x", "t"]
    } else {
        &["x"]
    };
    let mut builder = Context::with_real_symbols(
        input.shape().declarations().registry().clone(),
        nrz_bindings::Limits::default(),
        keys,
    )
    .map_err(|cause| refusal(SensitivityError::Algebra(cause)))?;
    builder
        .declare(
            "x".into(),
            Declaration::Input {
                descriptor: input.descriptor().clone(),
                nonzero: false,
                evidence: Evidence::default(),
            },
        )
        .map_err(|cause| refusal(cause.into()))?;
    if function == Elementwise::SquareRoot {
        builder
            .declare(
                "t".into(),
                Declaration::Input {
                    descriptor: output.descriptor().clone(),
                    nonzero: false,
                    evidence: Evidence::default(),
                },
            )
            .map_err(|cause| refusal(cause.into()))?;
    }
    let original = builder
        .parse(
            "matrix native derivative",
            match function {
                Elementwise::Square => "x^2",
                Elementwise::Reciprocal => "1/x",
                Elementwise::SquareRoot => "sqrt(x)",
            },
        )
        .map_err(|cause| refusal(cause.into()))?;
    let coordinates = [String::from("x")];
    let limits = nrz_bindings::LimitsOverride::default();
    let checked = match function {
        Elementwise::Square | Elementwise::Reciprocal => builder.check_sensitivity(
            if function == Elementwise::Square {
                SensitivityRecipe::Square
            } else {
                SensitivityRecipe::Reciprocal
            },
            std::slice::from_ref(&original),
            &coordinates,
            &limits,
        ),
        Elementwise::SquareRoot => builder.check_root_sensitivity(
            std::slice::from_ref(&original),
            &coordinates,
            &[String::from("t")],
            &limits,
        ),
    }
    .map_err(refusal)?;
    if checked.inputs() != std::slice::from_ref(input.descriptor())
        || checked.outputs() != std::slice::from_ref(output.descriptor())
    {
        return Err(StatisticsError::Contract.into());
    }
    Ok(Some(Rc::new(checked)))
}
/// Retain the complete bindings/CAS cause without requiring its non-cloneable witness to clone.
fn refusal(cause: SensitivityError) -> OperationError {
    StatisticsError::Sensitivity(Rc::new(cause)).into()
}
