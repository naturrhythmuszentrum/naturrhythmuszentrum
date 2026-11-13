// SPDX-License-Identifier: AGPL-3.0-only
//! Independent field inventories and enumerated vacancy reuse pin retained and simultaneous bytes.

use super::{BinaryElementwise, Limits, TestResult, admit, fixture, ready};
use crate::loan::{
    abi,
    registry::{Declaration, Identity, QualifiedMaskVersion, Token},
};
use crate::{ApplicationCancellation, BinaryApplicationRequest};

/// Independent original correspondence field inventory.
#[repr(C)]
struct Operation {
    checked: crate::CheckedOperation,
    sources: [nrz_bindings::BindingSymbols; 2],
    identities: [Identity; 2],
    destination: nrz_bindings::BindingSymbols,
    axis: crate::CellAxis,
}
/// Independent right capture includes its one already-funded pin and one new allocation token.
#[repr(C)]
struct Side {
    operation: Operation,
    source: Declaration,
    identity: Identity,
    attachments: [Option<QualifiedMaskVersion>; 3],
    input: Option<crate::BundlePin<f64>>,
    token: Token,
}
/// The closed alternative remains the size of the pre-existing optional endpoint pair.
#[expect(
    dead_code,
    reason = "independent ABI alternatives are measured without creating authority"
)]
enum Auxiliary {
    Absent,
    Endpoints([(usize, u64, u64); 2]),
    Binary(Box<Side>),
}

/// Literal byte geometry derives from the two sequential host publications and their released rows.
fn bytes(cells: usize, components: usize, roles: [usize; 2]) -> [usize; 2] {
    let length = cells * components;
    let words = cells.div_ceil(64);
    let tracking = length.div_ceil(64);
    let rounded = length.div_ceil(8) * 64;
    let total_roles = roles[0] + roles[1];
    let old_debits = roles[0] + 3 * roles[1] + 10;
    let delta = 3 * (256 + 1032) + 2 * 32 + (16 - 2 * roles[1]) * 264 + 40;
    let old = total_roles * (256 + 1032) + 5 * 32 + old_debits * 264 + 2 * 40;
    let retained = 3 * (rounded + 8 * tracking + 8 * words) + 8 * words + 32 * cells + 8656 + delta;
    [retained, retained + 2 * old + 5624]
}
/// Mirrored fields establish X before comparing any production owner size or byte report.
#[test]
fn binary_independent_abi_and_literal_inventory() {
    assert_eq!(
        [
            size_of::<crate::CheckedOperation>(),
            size_of::<nrz_bindings::BindingSymbols>(),
            size_of::<Identity>(),
            size_of::<crate::CellAxis>()
        ],
        [64, 24, 216, 24]
    );
    assert_eq!(
        [
            size_of::<Operation>(),
            size_of::<Declaration>(),
            size_of::<Option<QualifiedMaskVersion>>(),
            size_of::<Option<crate::BundlePin<f64>>>(),
            size_of::<Token>()
        ],
        [592, 80, 224, 8, 24]
    );
    assert_eq!(size_of::<Side>(), 1592);
    assert_eq!(
        size_of::<Side>(),
        size_of::<crate::loan::application::binary::Side>()
    );
    assert_eq!(
        size_of::<Operation>(),
        size_of::<crate::PointwiseBinaryOperation>()
    );
    assert_eq!(size_of::<Auxiliary>(), 56);
    assert_eq!(
        size_of::<Auxiliary>(),
        size_of::<crate::loan::application::binary::Auxiliary>()
    );
    assert_eq!(
        [
            abi::application_headers::<f64>(),
            abi::outcome_width(),
            abi::group_row()
        ],
        [7040, 32, 1032]
    );
    assert_eq!(7040 + 24 + size_of::<Side>(), 8656);
    assert_eq!(
        [
            bytes(0, 1, [1; 2]),
            bytes(1, 1, [1; 2]),
            bytes(3, 1, [1; 2])
        ],
        [[16_320, 34_968], [16_600, 35_248], [16_664, 35_312]]
    );
    assert_eq!(18 * 264 + 5 * 16 + 3 * 248 + 2 * 24, 5624);
}
/// Both byte ceilings enforce N/N-1 after all setup fits that same immutable account.
#[test]
fn binary_exact_growth_bytes_real_neighbours() -> TestResult {
    for function in [BinaryElementwise::Product, BinaryElementwise::Quotient] {
        for cells in [0, 1, 7, 8, 9, 63, 64, 65] {
            for left in 1..=3 {
                for right in 1..=3 {
                    boundary(cells, [left, right], function)?;
                }
            }
        }
    }
    Ok(())
}
/// Independently enumerated table changes are the only retained additions at exact growth.
fn boundary(cells: usize, roles: [usize; 2], function: BinaryElementwise) -> TestResult {
    let [retained, staged] = bytes(cells, 1, roles);
    for staging in [false, true] {
        for short in [false, true] {
            let required = if staging { staged } else { retained };
            let mut limits = Limits::default();
            if staging {
                limits.staged_payload_bytes = u64::try_from(required - usize::from(short))?;
            } else {
                limits.output_bytes = u64::try_from(required - usize::from(short))?;
            }
            let mut setup = fixture(cells, 1, roles, 2, false, function, limits)?;
            let before = setup.heap.resource_bytes()?;
            let injection =
                crate::loan::fault::Injection::after(if short { 0 } else { usize::MAX });
            match admit(&mut setup.heap, setup.request, function, &mut setup.account) {
                BinaryApplicationRequest::ApplicationReady(owner) => {
                    assert!(!short);
                    assert_eq!(setup.heap.resource_bytes()? - before, retained);
                    assert_eq!(
                        setup
                            .heap
                            .loans
                            .group(owner.ticket().identity)?
                            .debits
                            .entries()
                            .len(),
                        18
                    );
                    assert!(matches!(
                        setup.heap.cancel_application(owner, &mut setup.account),
                        ApplicationCancellation::ApplicationCancelled(_)
                    ));
                }
                BinaryApplicationRequest::ApplicationRefused(
                    crate::ApplicationError::Loan(crate::LoanError::Heap(
                        crate::HeapError::ResourceBytes {
                            staged: actual,
                            used,
                            ..
                        },
                    )),
                    _,
                ) => {
                    assert!(short);
                    assert_eq!(actual, staging);
                    assert_eq!(used, u64::try_from(required)?);
                    assert_eq!(setup.heap.resource_bytes()?, before);
                    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                }
                other @ BinaryApplicationRequest::ApplicationRefused(..) => {
                    return Err(format!("{other:?}").into());
                }
            }
            drop(injection);
        }
    }
    Ok(())
}
/// Sufficient actual table capacity removes every replacement delta and old backing.
#[test]
fn binary_spare_capacity_inventory_and_retained_side() -> TestResult {
    let function = BinaryElementwise::Product;
    let mut setup = fixture(1, 1, [1; 2], 2, false, function, Limits::default())?;
    let left = setup
        .heap
        .retain_pin(&setup.request.left, &mut setup.account)?;
    let right = setup
        .heap
        .retain_pin(&setup.request.right, &mut setup.account)?;
    let operation = setup.request.operation.clone();
    let symbols = setup.request.destination.symbols.clone();
    let axis = setup.request.destination.axis;
    let demand = setup.request.demand;
    let owner = ready(admit(
        &mut setup.heap,
        setup.request,
        function,
        &mut setup.account,
    ))?;
    assert!(matches!(
        setup.heap.cancel_application(owner, &mut setup.account),
        ApplicationCancellation::ApplicationCancelled(_)
    ));
    assert!(matches!(
        setup
            .heap
            .reconcile_detached(&mut setup.account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    assert!(matches!(
        setup.heap.reclaim(&mut setup.account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    let destination = crate::HostTileDeclaration::check(
        symbols,
        axis,
        crate::Coverage::RepresentedInputs,
        &mut setup.account,
    )?;
    let request = crate::PointwiseBinaryRequest {
        operation,
        left,
        right,
        destination,
        demand,
        limits: crate::ExecutionLimits::default(),
    };
    let before = setup.heap.resource_bytes()?;
    let owner = ready(admit(
        &mut setup.heap,
        request,
        function,
        &mut setup.account,
    ))?;
    let expected = 3 * (64 + 8 + 8) + 8 + 32 + 8656;
    assert_eq!(expected, 8936);
    assert_eq!(expected + 5624, 14_560);
    assert_eq!(setup.heap.resource_bytes()? - before, expected);
    let side_key = owner
        .core
        .evidence
        .endpoint_tokens
        .binary()
        .ok_or("side")?
        .token
        .key;
    let owner = super::candidate(owner, &mut setup.account)?;
    let (tile, _) = super::published(setup.heap.publish_application(owner, &mut setup.account))?;
    let bundle = tile
        .pin
        .record
        .payload
        .as_ref()
        .ok_or("pin")?
        .bundle
        .payload
        .as_ref()
        .ok_or("bundle")?;
    let side = bundle
        .origin
        .evidence
        .as_ref()
        .ok_or("evidence")?
        .endpoint_tokens
        .binary()
        .ok_or("side")?;
    assert!(side.input.is_none());
    assert_eq!(side.token.key, side_key);
    Ok(())
}
