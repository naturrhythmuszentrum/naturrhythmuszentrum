// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit index order is part of a selection's identity.

use super::{AT, AffineMap, invalid, reserve};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode};
use crate::graph::size;
use crate::model::{AtomicSort, Extent, Field, Sort};

/// A canonical index list with its scalar or rectangular result shape.
type SelectedEntries = (Vec<EntryIndex>, Option<(u32, u32)>);

/// A zero-based entry in a declared rectangle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntryIndex {
    /// Row index.
    pub row: u32,
    /// Column index.
    pub column: u32,
}

/// The ordering contract of an explicit list; no constructor silently sorts it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryOrder {
    /// The caller's exact order is meaningful.
    Given,
    /// Strict increasing logical column order.
    ColumnMajor,
    /// Strict increasing row order.
    RowMajor,
}

/// Untrusted finite selection request, checked against a literal matrix sort.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectionSpec {
    /// One scalar entry.
    Entry(EntryIndex),
    /// An ordered column vector of distinct entries.
    Entries {
        /// Original index order.
        entries: Vec<EntryIndex>,
        /// Required order, if any.
        order: EntryOrder,
    },
    /// A complete row, from left to right.
    Row(u32),
    /// A complete column, from top to bottom.
    Column(u32),
    /// A rectangular block in logical column order.
    Block {
        /// First included row.
        row: u32,
        /// First included column.
        column: u32,
        /// Number of selected rows.
        rows: u32,
        /// Number of selected columns.
        columns: u32,
    },
}

/// A checked index list and result shape; the graph retains its source declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntrySelection {
    /// Original declared row extent.
    pub(super) source_rows: u32,
    /// Original declared column extent.
    pub(super) source_columns: u32,
    /// Retained ordered distinct entry identities.
    pub(super) indices: Vec<EntryIndex>,
    /// Selected rectangle, or a scalar entry when absent.
    pub(super) shape: Option<(u32, u32)>,
}

impl EntrySelection {
    /// Check bounds, injectivity, requested order and output shape before copying.
    /// # Errors
    /// Refuses nonliteral matrices, malformed blocks, duplicates, wrong order and limits.
    pub fn new(source: &Sort, spec: &SelectionSpec, budget: &mut Budget) -> Result<Self, Fault> {
        let (_, rows, columns) = rectangle(source)?;
        let (indices, shape) = expand(spec, rows, columns, budget)?;
        let result = Self {
            source_rows: rows,
            source_columns: columns,
            indices,
            shape,
        };
        result.validate(source, budget)?;
        Ok(result)
    }

    /// Ordered entries, without changing their source identities.
    #[must_use]
    pub const fn indices(&self) -> &[EntryIndex] {
        self.indices.as_slice()
    }

    /// Selected result shape; `None` denotes a scalar entry.
    #[must_use]
    pub const fn shape(&self) -> Option<(u32, u32)> {
        self.shape
    }

    /// Compute the exact selected sort in the source entry field.
    /// # Errors
    /// Refuses a different source rectangle, a nonmatrix source or exhausted copy resources.
    pub fn result_sort(&self, source: &Sort, budget: &mut Budget) -> Result<Sort, Fault> {
        let (field, rows, columns) = rectangle(source)?;
        if (rows, columns) != (self.source_rows, self.source_columns) {
            return Err(invalid(InvalidCode::SelectionShape));
        }
        let mut out = crate::encoding::Encoder::new(budget, AT);
        crate::encoding::field(field, &mut out)?;
        let bytes = size(out.finish().len(), AT)?;
        budget.work(bytes, AT)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            bytes.saturating_mul(4).saturating_add(64),
            AT,
        )?;
        Ok(Sort::Value(self.shape.map_or_else(
            || AtomicSort::Scalar(field.clone()),
            |(rows, columns)| AtomicSort::Matrix {
                field: field.clone(),
                rows: Extent::Literal(rows),
                columns: Extent::Literal(columns),
            },
        )))
    }

    /// Compose this exact source-entry selection with an expansion over common coordinates.
    /// # Errors
    /// Refuses a different expansion extent, invalid indices or exhausted resources.
    pub fn compose(&self, expansion: &AffineMap, budget: &mut Budget) -> Result<AffineMap, Fault> {
        let source = Sort::Value(AtomicSort::Matrix {
            field: Field::Real,
            rows: Extent::Literal(self.source_rows),
            columns: Extent::Literal(self.source_columns),
        });
        self.validate(&source, budget)?;
        let extent = self
            .source_rows
            .checked_mul(self.source_columns)
            .ok_or_else(|| invalid(InvalidCode::SelectionShape))?;
        if expansion.outputs() != extent {
            return Err(invalid(InvalidCode::ReadingShape));
        }
        let mut rows = reserve(self.indices.len(), budget)?;
        for index in &self.indices {
            rows.push(
                index
                    .column
                    .checked_mul(self.source_rows)
                    .and_then(|value| value.checked_add(index.row))
                    .ok_or_else(|| invalid(InvalidCode::SelectionShape))?,
            );
        }
        expansion.project(&rows, budget)
    }

    /// Revalidate canonical wire payloads and caller/store limits before use.
    pub(super) fn validate(&self, source: &Sort, budget: &mut Budget) -> Result<(), Fault> {
        let (_, rows, columns) = rectangle(source)?;
        if (rows, columns) != (self.source_rows, self.source_columns) {
            return Err(invalid(InvalidCode::SelectionShape));
        }
        let count = size(self.indices.len(), AT)?;
        budget.check(BudgetKind::MatrixEntries, count, AT)?;
        budget.work(
            count
                .checked_mul(count.saturating_add(1))
                .ok_or_else(|| invalid(InvalidCode::SelectionShape))?,
            AT,
        )?;
        let expected = self
            .shape
            .map_or(1, |(rows, columns)| u64::from(rows) * u64::from(columns));
        if count != expected {
            return Err(invalid(InvalidCode::SelectionShape));
        }
        for (position, index) in self.indices.iter().enumerate() {
            if index.row >= self.source_rows || index.column >= self.source_columns {
                return Err(invalid(InvalidCode::SelectionOutOfRange));
            }
            if self
                .indices
                .get(..position)
                .is_some_and(|prior| prior.contains(index))
            {
                return Err(invalid(InvalidCode::SelectionDuplicate));
            }
        }
        Ok(())
    }
}

/// Literal mathematical shapes include empty rectangles without inventing entries.
pub(super) const fn rectangle(sort: &Sort) -> Result<(&Field, u32, u32), Fault> {
    match sort {
        Sort::Value(AtomicSort::Matrix {
            field,
            rows: Extent::Literal(rows),
            columns: Extent::Literal(columns),
        }) => Ok((field, *rows, *columns)),
        _ => Err(invalid(InvalidCode::SelectionShape)),
    }
}

/// Expand every convenience spelling into one retained list and selected result shape.
fn expand(
    spec: &SelectionSpec,
    rows: u32,
    columns: u32,
    budget: &mut Budget,
) -> Result<SelectedEntries, Fault> {
    match spec {
        SelectionSpec::Entry(index) => {
            let mut indices = reserve(1, budget)?;
            indices.push(*index);
            Ok((indices, None))
        }
        SelectionSpec::Entries { entries, order } => {
            check_order(entries, *order, budget)?;
            let mut indices = reserve(entries.len(), budget)?;
            indices.extend_from_slice(entries);
            let count =
                u32::try_from(entries.len()).map_err(|_| invalid(InvalidCode::SelectionShape))?;
            Ok((indices, Some((count, 1))))
        }
        SelectionSpec::Row(row) => block((*row, 0), (1, columns), (rows, columns), budget),
        SelectionSpec::Column(column) => block((0, *column), (rows, 1), (rows, columns), budget),
        SelectionSpec::Block {
            row,
            column,
            rows: height,
            columns: width,
        } => block((*row, *column), (*height, *width), (rows, columns), budget),
    }
}

/// Validate endpoints before expanding; an empty block must still start inside its bounds.
fn block(
    origin: (u32, u32),
    shape: (u32, u32),
    source: (u32, u32),
    budget: &mut Budget,
) -> Result<SelectedEntries, Fault> {
    let (row, column) = origin;
    let (rows, columns) = shape;
    let end_row = row
        .checked_add(rows)
        .ok_or_else(|| invalid(InvalidCode::SelectionShape))?;
    let end_column = column
        .checked_add(columns)
        .ok_or_else(|| invalid(InvalidCode::SelectionShape))?;
    if row > source.0 || column > source.1 || end_row > source.0 || end_column > source.1 {
        return Err(invalid(InvalidCode::SelectionOutOfRange));
    }
    let count = usize::try_from(u64::from(rows) * u64::from(columns))
        .map_err(|_| invalid(InvalidCode::SelectionShape))?;
    let mut indices = reserve(count, budget)?;
    for column in column..end_column {
        for row in row..end_row {
            indices.push(EntryIndex { row, column });
        }
    }
    Ok((indices, Some(shape)))
}

/// Ordering and duplicate failures remain distinct even before source bounds are checked.
fn check_order(
    entries: &[EntryIndex],
    order: EntryOrder,
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(size(entries.len(), AT)?, AT)?;
    for pair in entries.windows(2) {
        let [left, right] = pair else {
            return Err(invalid(InvalidCode::SelectionShape));
        };
        let ordered = match order {
            EntryOrder::Given => true,
            EntryOrder::ColumnMajor => (left.column, left.row) <= (right.column, right.row),
            EntryOrder::RowMajor => (left.row, left.column) <= (right.row, right.column),
        };
        if !ordered {
            return Err(invalid(InvalidCode::SelectionOrder));
        }
    }
    Ok(())
}
