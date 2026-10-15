// SPDX-License-Identifier: AGPL-3.0-only
//! Checked homogeneous extents, without an inferred physical operator axis.

use super::HeapError;

/// Checked rectangular extents in logical column order.
///
/// This storage shape does not establish ordered physical axes or a linear operator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Extent {
    /// Number of rows.
    rows: usize,
    /// Number of columns.
    columns: usize,
    /// Checked product of the two extents.
    entries: usize,
}

impl Extent {
    /// Check extents before any allocation or indexing.
    ///
    /// # Errors
    /// Returns `HeapError::ExtentOverflow` if the product cannot be represented.
    pub const fn new(rows: usize, columns: usize) -> Result<Self, HeapError> {
        let Some(entries) = rows.checked_mul(columns) else {
            return Err(HeapError::ExtentOverflow);
        };
        Ok(Self {
            rows,
            columns,
            entries,
        })
    }

    /// Number of rows.
    #[must_use]
    pub const fn rows(self) -> usize {
        self.rows
    }

    /// Number of columns.
    #[must_use]
    pub const fn columns(self) -> usize {
        self.columns
    }

    /// Number of addressable entries, including structural zeros.
    #[must_use]
    pub const fn entries(self) -> usize {
        self.entries
    }

    /// Checked column-vectorization index.
    pub(crate) const fn index(self, row: usize, column: usize) -> Result<usize, HeapError> {
        if row >= self.rows || column >= self.columns {
            return Err(HeapError::InvalidLayout);
        }
        let Some(start) = column.checked_mul(self.rows) else {
            return Err(HeapError::ExtentOverflow);
        };
        match start.checked_add(row) {
            Some(index) => Ok(index),
            None => Err(HeapError::ExtentOverflow),
        }
    }
}
