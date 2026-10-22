// SPDX-License-Identifier: AGPL-3.0-only
//! Public layout arithmetic and explicit work-limit contracts.

use nrz_bindings::Descriptor;
use nrz_matrix::{Extent, Heap, HeapError, HeapLimits, Layout};

use crate::testing::TestResult;

/// Both signed extreme products and their sums must fit before an address is admitted.
/// ´test:integration:affine-overflow-and-region-refusals´
#[test]
fn affine_overflow_and_region_refusals() -> TestResult {
    let pair = Extent::new(2, 1)?;
    assert_eq!(
        Layout::strided(pair, 2, 0, -1, 0),
        Err(HeapError::InvalidLayout)
    );
    assert_eq!(
        Layout::strided(pair, 2, 1, 1, 0),
        Err(HeapError::InvalidLayout)
    );
    assert_eq!(
        Layout::strided(pair, 2, isize::MAX, 1, 0),
        Err(HeapError::ExtentOverflow)
    );
    assert_eq!(
        Layout::strided(pair, 2, isize::MIN, -1, 0),
        Err(HeapError::ExtentOverflow)
    );
    assert_eq!(
        Layout::strided(Extent::new(3, 1)?, 3, 0, isize::MAX, 0),
        Err(HeapError::ExtentOverflow)
    );
    assert_eq!(
        Layout::strided(Extent::new(1, 3)?, 3, 0, 0, isize::MIN),
        Err(HeapError::ExtentOverflow)
    );
    assert_eq!(
        Layout::strided(Extent::new(2, 2)?, 4, 0, isize::MAX, 1),
        Err(HeapError::ExtentOverflow)
    );
    assert_eq!(
        Layout::contiguous(Extent::new(usize::MAX, 0)?),
        Err(HeapError::ExtentOverflow)
    );
    assert_eq!(
        Layout::strided(pair, usize::MAX, 0, 1, 0),
        Err(HeapError::ExtentOverflow)
    );
    let reverse = Layout::strided(Extent::new(2, 3)?, 6, 5, -1, -2)?;
    assert_eq!((reverse.index(0, 0)?, reverse.index(1, 2)?), (5, 0));
    assert_eq!(reverse.index(2, 0), Err(HeapError::InvalidLayout));
    Ok(())
}

/// An independently enumerated small integer oracle checks all sign combinations and corners.
/// ´test:integration:affine-extremes-match-integer-enumeration´
#[test]
fn affine_extremes_match_integer_enumeration() -> TestResult {
    for rows in 0..=3 {
        for columns in 0..=3 {
            for offset in -2..=7 {
                for row_stride in -3..=3 {
                    for column_stride in -3..=3 {
                        let extent = Extent::new(rows, columns)?;
                        let mut expected = Vec::new();
                        for column in 0..columns {
                            for row in 0..rows {
                                expected.push(
                                    offset
                                        + isize::try_from(row)? * row_stride
                                        + isize::try_from(column)? * column_stride,
                                );
                            }
                        }
                        let admitted =
                            Layout::strided(extent, 6, offset, row_stride, column_stride);
                        assert_eq!(
                            admitted.is_ok(),
                            expected.iter().all(|&address| (0..6).contains(&address))
                        );
                        if let Ok(layout) = admitted {
                            for (logical, address) in expected.iter().enumerate() {
                                assert_eq!(
                                    layout.index(logical % rows, logical / rows)?,
                                    usize::try_from(*address)?
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Gather validation scans every entry at the limit and admits repeated read indices.
/// ´test:integration:gather-indices-and-work-boundaries´
#[test]
fn gather_indices_and_work_boundaries() -> TestResult {
    let extent = Extent::new(2, 2)?;
    assert_eq!(
        Layout::gather(extent, 3, &[2, 0, 2, 1], 3),
        Err(HeapError::WorkLimit)
    );
    for limit in [4, 5] {
        let layout = Layout::gather(extent, 3, &[2, 0, 2, 1], limit)?;
        assert_eq!((layout.index(0, 0)?, layout.index(0, 1)?), (2, 2));
        assert_eq!(layout.index(1, 1)?, 1);
    }
    assert_eq!(
        Layout::gather(extent, 3, &[0, 1, 2, 3], 4),
        Err(HeapError::InvalidLayout)
    );
    assert_eq!(
        Layout::gather(extent, 3, &[0, 1, 2], 4),
        Err(HeapError::LengthMismatch)
    );
    let empty = Layout::strided(Extent::new(0, 3)?, 0, isize::MIN, isize::MAX, isize::MIN)?;
    assert_eq!(empty.index(0, 0), Err(HeapError::InvalidLayout));
    assert_eq!(
        Layout::gather(Extent::new(3, 0)?, 0, &[], 0)?
            .extent()
            .entries(),
        0
    );
    Ok(())
}

/// A failed alias publication consumes no descriptor slot and cannot escape its base range.
/// ´test:integration:alias-refusals-preserve-capacity´
#[test]
fn alias_refusals_preserve_capacity() -> TestResult {
    let mut heap = Heap::new(HeapLimits::new(2, 3, 1).with_layout_work(2))?;
    let original = heap.import_represented(
        Extent::new(3, 1)?,
        &Descriptor::ratio(),
        &[1.0_f64, 2.0, 3.0],
    )?;
    let wrong = Layout::strided(Extent::new(1, 1)?, 4, 3, 0, 0)?;
    assert!(matches!(
        heap.alias(original.value(), &wrong),
        Err(HeapError::InvalidLayout)
    ));
    let gathered = Layout::gather(Extent::new(3, 1)?, 3, &[2, 0, 1], 3)?;
    assert!(matches!(
        heap.alias(original.value(), &gathered),
        Err(HeapError::WorkLimit)
    ));
    let broadcast = Layout::strided(Extent::new(3, 1)?, 3, 1, 0, 0)?;
    let selected = heap.alias(original.value(), &broadcast)?;
    assert!(matches!(
        heap.value(selected)?.region().export(),
        Err(HeapError::WorkLimit)
    ));
    assert!(matches!(
        heap.value(selected)?.region().pack(),
        Err(HeapError::WorkLimit)
    ));
    assert_eq!(
        heap.value(selected)?.region().get(2, 0)?.to_bits(),
        2.0_f64.to_bits()
    );
    assert!(matches!(
        heap.alias(original.value(), &broadcast),
        Err(HeapError::Capacity)
    ));
    Ok(())
}
