// SPDX-License-Identifier: AGPL-3.0-only
//! Original correspondence counts four new shared backings, transferring existing region owners.

use super::{BinaryElementwise, Limits, TestResult, fixture};
use std::rc::Rc;

/// Shared-allocation counters precede the aligned payload.
#[repr(C)]
struct Shared<T> {
    strong: usize,
    weak: usize,
    payload: T,
}
/// Region field inventory is independent of the production constructor's byte calculation.
#[repr(C)]
struct Region {
    shape: crate::CheckedShape,
    descriptor: nrz_bindings::Descriptor,
    admission: nrz_bindings::HomogeneousRecipe,
    domain: crate::CoordinateMap,
    codomain: crate::CoordinateMap,
}
/// Scalar quantity alternatives bound their retained original recipe payload.
#[expect(
    dead_code,
    reason = "ABI alternatives are measured without creating a recipe"
)]
enum Quantity {
    Product,
    Quotient,
    IntegerPower(i64),
    SquareRoot,
}
/// Independent closed recipe alternatives retain the original largest entry-map payload.
#[expect(
    dead_code,
    reason = "ABI alternatives are measured without producing authority"
)]
enum Recipe {
    PositiveReciprocal(Box<()>),
    Sum {
        subtract: bool,
        right: crate::EntryMap,
    },
    Linear(Box<()>),
    Transform(crate::EntryMap),
    Product,
    Quantity {
        operation: Quantity,
        allowance: u64,
    },
    Elementwise(crate::Elementwise),
    PreparedExpm1,
    Binary(BinaryElementwise),
    Cartesian(
        crate::CellProduct,
        crate::CellLayoutVersion,
        BinaryElementwise,
    ),
}
/// The checked kernel owns one shared extent array and no additional binary program.
#[repr(C)]
struct Kernel {
    inputs: Rc<[crate::Extent]>,
    output: crate::Extent,
    recipe: Recipe,
    work: usize,
}
/// Every new shared header and payload is counted once; supplied regions already own their contents.
const fn bytes() -> usize {
    size_of::<Shared<[Region; 2]>>()
        + size_of::<Shared<[Region; 1]>>()
        + size_of::<Shared<[crate::Extent; 2]>>()
        + size_of::<Shared<Kernel>>()
}
/// The independent mirrors reproduce layouts before any constructor boundary is exercised.
#[test]
fn binary_original_constructor_independent_allocation_inventory() {
    assert_eq!(size_of::<Region>(), size_of::<crate::RegionContract>());
    assert_eq!(
        size_of::<Recipe>(),
        size_of::<crate::kernels::contract::Recipe>()
    );
    assert_eq!(
        size_of::<Kernel>(),
        size_of::<crate::kernels::KernelContract>()
    );
    assert_eq!(
        bytes(),
        4 * 16 + 3 * size_of::<Region>() + 2 * 24 + size_of::<Kernel>()
    );
}
/// New borrowed uses have their own immutable account; no original account is replenished.
#[test]
fn binary_original_constructor_both_byte_ceiling_neighbours() -> TestResult {
    for staging in [false, true] {
        for short in [false, true] {
            let mut setup = fixture(
                1,
                1,
                [1; 2],
                2,
                false,
                BinaryElementwise::Product,
                Limits::default(),
            )?;
            let mut limits = Limits::default();
            let required = u64::try_from(bytes())?;
            if staging {
                limits.staged_payload_bytes = required - u64::from(short);
            } else {
                limits.output_bytes = required - u64::from(short);
            }
            let mut account = crate::ExecutionAccount::new(nrz_bindings::Budget::new(limits)?)?;
            let left = setup
                .heap
                .acquire_pin::<f64>(setup.request.left.ticket()?, &mut account)?;
            let right = setup
                .heap
                .acquire_pin::<f64>(setup.request.right.ticket()?, &mut account)?;
            let operation = &setup.request.operation.operation;
            let inputs = [operation.inputs()[0].clone(), operation.inputs()[1].clone()];
            let output = operation.outputs()[0].clone();
            let before = account.usage().work;
            let result = crate::PointwiseBinaryOperation::check(
                inputs,
                output,
                [&left, &right],
                setup.request.destination.symbols.clone(),
                setup.request.destination.axis,
                BinaryElementwise::Product,
                crate::ExecutionLimits::default(),
                &mut account,
            );
            assert_eq!(result.is_ok(), !short);
            assert_eq!(account.usage().work - before, 16);
        }
    }
    Ok(())
}
