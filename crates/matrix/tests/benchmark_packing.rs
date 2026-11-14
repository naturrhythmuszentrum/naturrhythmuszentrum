// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`large_supported_shapes_select_proven_dispatch_conditions`] | matrix | Supported large raw shapes retain contiguous operands and serial dispatch. |
//! | [`opaque_packing_refuses_before_input_or_buffer_admission`] | matrix | Unsupported shapes refuse before input validation or buffer reservation. |

#![cfg(feature = "benchmark")]

use nrz_matrix::benchmark::{RawError, RawKernel, RawOperation, RawShape};

/// Check the concrete conditions of the pinned source proof, without timing an allocator.
fn supported(operation: RawOperation, shape: RawShape) -> Result<(), RawError> {
    let left = vec![2.0; shape.rows * shape.contracted];
    let right = if operation == RawOperation::Factor {
        Vec::new()
    } else {
        vec![1.0; shape.contracted * shape.columns]
    };
    assert_eq!(RawKernel::packing_bytes(operation, shape), Some(0));
    let mut kernel = RawKernel::prepare(operation, shape, left, right, 1_073_741_824)?;
    assert_eq!(kernel.shape(), shape);
    assert_eq!(kernel.backend_workers(), 1);
    let layouts = kernel.buffer_layouts();
    for (index, rows) in [(0, shape.rows), (1, shape.contracted), (2, shape.rows)] {
        assert_eq!(layouts[index].row_stride_bytes, size_of::<f64>());
        assert_eq!(layouts[index].column_stride_bytes, rows * size_of::<f64>());
    }
    kernel.execute()?;
    assert_eq!(kernel.buffer_layouts(), layouts);
    Ok(())
}

/// Supported large raw shapes retain contiguous operands and serial dispatch.
/// ´claim:matrix:benchmark-proven-packing-dispatch-conditions´
/// ´test:integration:benchmark-proven-packing-dispatch-conditions´
#[test]
fn large_supported_shapes_select_proven_dispatch_conditions() -> Result<(), RawError> {
    assert!(
        include_str!("../../../Cargo.lock")
            .contains("[[package]]\nname = \"faer\"\nversion = \"0.24.4\"\n")
    );
    for shape in [
        RawShape {
            rows: 1,
            contracted: 4096,
            columns: 4096,
        },
        RawShape {
            rows: 4096,
            contracted: 4096,
            columns: 1,
        },
        RawShape {
            rows: 4096,
            contracted: 1,
            columns: 4096,
        },
    ] {
        supported(RawOperation::Gemm, shape)?;
    }
    supported(
        RawOperation::Gemv,
        RawShape {
            rows: 4096,
            contracted: 4096,
            columns: 1,
        },
    )?;
    supported(
        RawOperation::Factor,
        RawShape {
            rows: 1,
            contracted: 1,
            columns: 1,
        },
    )?;
    supported(
        RawOperation::Solve,
        RawShape {
            rows: 1,
            contracted: 1,
            columns: 4096,
        },
    )?;
    Ok(())
}

/// Unsupported shapes refuse before input validation or buffer reservation.
/// ´claim:matrix:benchmark-opaque-packing-refuses-before-reservation´
/// ´test:integration:benchmark-opaque-packing-refuses-before-reservation´
#[test]
fn opaque_packing_refuses_before_input_or_buffer_admission() {
    for (operation, shape) in [
        (
            RawOperation::Gemm,
            RawShape {
                rows: 1024,
                contracted: 1024,
                columns: 1024,
            },
        ),
        (
            RawOperation::Factor,
            RawShape {
                rows: 2,
                contracted: 2,
                columns: 2,
            },
        ),
        (
            RawOperation::Solve,
            RawShape {
                rows: 2,
                contracted: 2,
                columns: 1,
            },
        ),
    ] {
        assert_eq!(RawKernel::packing_bytes(operation, shape), None);
        assert!(matches!(
            RawKernel::prepare(operation, shape, Vec::new(), Vec::new(), 0),
            Err(RawError::UnaccountedPacking)
        ));
    }
}
