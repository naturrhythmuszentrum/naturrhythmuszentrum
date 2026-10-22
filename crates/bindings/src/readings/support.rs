// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded arrays and complete typed syntax inspection.

use super::ReadingError;
use nrz_cas::limits::{Budget, BudgetKind};
use nrz_cas::source::SyntaxNode;
use nrz_cas::syntax::{AtomicSort, Draft, ExpressionDraft, Extent, Field, Operator, Sort};
use nrz_cas::{Cas, Handle, Location, Reference};

/// Location for physical adapter accounting and refusals.
pub(super) const AT: Location = Location::new("physical reading");
/// Complete sort, operator, operands and declaration header of an inspected expression.
type Inspected = (Sort, Operator, Vec<Handle>, Handle);

/// Check a complete metadata array before storing it, without a disposable allocation.
pub(super) fn capacity<T>(count: usize, budget: &mut Budget) -> Result<(), ReadingError> {
    let length = u64::try_from(count).map_err(|_| ReadingError::Shape)?;
    let bytes = length
        .checked_mul(u64::try_from(std::mem::size_of::<T>()).map_err(|_| ReadingError::Shape)?)
        .ok_or(ReadingError::Shape)?;
    budget.check(BudgetKind::MatrixEntries, length, AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(length, AT)?;
    Ok(())
}

/// Fallibly reserve the actual bounded output vector before traversal.
pub(super) fn reserve<T>(count: usize, budget: &mut Budget) -> Result<Vec<T>, ReadingError> {
    capacity::<T>(count, budget)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| nrz_cas::Fault::allocation(AT))?;
    Ok(values)
}

/// Inspect the complete original expression and declaration snapshot.
pub(super) fn inspect(
    cas: &Cas,
    node: &Handle,
    budget: &mut Budget,
) -> Result<Inspected, ReadingError> {
    let syntax = cas.describe_syntax(node, budget)?;
    if !syntax.scope.is_empty() || syntax.binding.is_some() {
        return Err(ReadingError::Definition);
    }
    let SyntaxNode::Expression(value) = syntax.node else {
        return Err(ReadingError::Definition);
    };
    let mut operands = reserve(syntax.dependencies.len(), budget)?;
    for dependency in syntax.dependencies {
        operands.push(dependency.target);
    }
    Ok((value.sort, value.operator, operands, syntax.header))
}

/// Accept only literal Real rectangles for storage coordinate families.
pub(super) const fn shape(sort: &Sort) -> Result<[u32; 2], ReadingError> {
    match sort {
        Sort::Value(AtomicSort::Matrix {
            field: Field::Real,
            rows: Extent::Literal(rows),
            columns: Extent::Literal(columns),
        }) => Ok([*rows, *columns]),
        _ => Err(ReadingError::Shape),
    }
}

/// Count ordered Real coordinates without inferring a numerical representation.
pub(super) fn coordinates(sort: &Sort) -> Result<usize, ReadingError> {
    let count = match sort {
        Sort::Value(AtomicSort::Scalar(Field::Real)) => 1,
        Sort::Value(AtomicSort::Scalar(Field::Complex)) => 2,
        _ => {
            let [rows, columns] = shape(sort)?;
            u64::from(rows) * u64::from(columns)
        }
    };
    usize::try_from(count).map_err(|_| ReadingError::Shape)
}

/// Admit one closed expression under the exact retained declaration header.
pub(super) fn expression(
    cas: &mut Cas,
    context: &Handle,
    operator: Operator,
    sort: Sort,
    operands: &[Handle],
    budget: &mut Budget,
) -> Result<Handle, ReadingError> {
    let header = cas.context_header(context)?;
    let draft = Draft::Expression(ExpressionDraft {
        operator,
        sort,
        header: Reference::Existing(header),
        scope: Vec::new(),
        binding: None,
        space: None,
        operands: operands.iter().cloned().map(Reference::Existing).collect(),
    });
    cas.admit(&[draft], budget)?
        .pop()
        .ok_or(ReadingError::Definition)
}
