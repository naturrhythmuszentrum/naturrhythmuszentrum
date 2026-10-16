// SPDX-License-Identifier: AGPL-3.0-only
//! Shared solve fixtures use the public operation and heap boundary.

pub(crate) use crate::kernel_testing::TestResult;
use crate::kernel_testing::{axis, owner, shape};
use crate::test_api::{CheckedShape, OperationDraft, ShapeLimits};

/// One immutable owner and three explicitly shared coordinate axes.
pub(crate) fn shapes(order: usize, columns: usize) -> TestResult<[CheckedShape; 3]> {
    let declarations = owner()?;
    let variables = axis(order)?;
    let equations = axis(order)?;
    let right_columns = axis(columns)?;
    Ok([
        shape(&declarations, &variables, &equations)?,
        shape(&declarations, &right_columns, &equations)?,
        shape(&declarations, &right_columns, &variables)?,
    ])
}

/// A checked native solve draft, without hidden oracle arithmetic.
pub(crate) fn draft(order: usize, columns: usize) -> TestResult<OperationDraft> {
    let [operator, rhs, solution] = shapes(order, columns)?;
    Ok(OperationDraft::solve(
        operator,
        rhs,
        solution,
        ShapeLimits::default(),
    )?)
}
