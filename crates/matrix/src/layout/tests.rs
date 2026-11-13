// SPDX-License-Identifier: AGPL-3.0-only
//! Enumerated address sets independently decide affine writable incidence.

use super::Layout;
use crate::Extent;
use std::collections::BTreeSet;

/// Signed strides, singleton axes, empty maps and interior collisions use a set-cardinality oracle.
/// ´test:unit:layout-affine-injectivity-independent-address-sets´
#[test]
fn layout_affine_injectivity_independent_address_sets() -> Result<(), crate::HeapError> {
    for rows in 0..6 {
        for columns in 0..6 {
            let extent = Extent::new(rows, columns)?;
            for row_stride in -5..=5 {
                for column_stride in -5..=5 {
                    let mut addresses = BTreeSet::new();
                    for row in 0..rows {
                        for column in 0..columns {
                            let row = isize::try_from(row)
                                .map_err(|_| crate::HeapError::ExtentOverflow)?;
                            let column = isize::try_from(column)
                                .map_err(|_| crate::HeapError::ExtentOverflow)?;
                            addresses.insert(row * row_stride + column * column_stride);
                        }
                    }
                    let minimum = addresses.first().copied().unwrap_or(0);
                    let maximum = addresses.last().copied().unwrap_or(-1);
                    let length = usize::try_from(maximum - minimum + 1)
                        .map_err(|_| crate::HeapError::ExtentOverflow)?;
                    let layout =
                        Layout::strided(extent, length, -minimum, row_stride, column_stride)?;
                    assert_eq!(
                        layout.writable_affine()?,
                        addresses.len() == extent.entries(),
                        "{rows}x{columns}, strides {row_stride}/{column_stride}"
                    );
                }
            }
        }
    }
    Ok(())
}

/// Large nonseparated maps need no entry-sized scratch; explicit gathers keep their own boundary.
/// ´test:unit:layout-affine-injectivity-large-and-gather-boundaries´
#[test]
fn layout_affine_injectivity_large_and_gather_boundaries() -> Result<(), crate::HeapError> {
    let extent = Extent::new(1_000_000, 2)?;
    assert!(Layout::strided(extent, 2_000_002, 0, 2, 3)?.writable_affine()?);
    assert!(!Layout::strided(extent, 2_000_003, 0, 2, 4)?.writable_affine()?);
    let extent = Extent::new(2, 2)?;
    assert!(!Layout::gather(extent, 4, &[0, 2, 1, 3], 4)?.writable_affine()?);
    Ok(())
}
