// SPDX-License-Identifier: AGPL-3.0-only
//! Independent ABI mirrors enumerate every owner and replacement backing in the byte fixture.

use super::{Fixture, TestResult, fixture, ready};
use crate::heap::{descriptors::DescriptorRecord, identity::Key};
use crate::loan::registry::{
    BundleOwner, CleanupReceipt, Debit, Declaration, Group, Identity, Liveness, PinIndex,
    PinRecord, QualifiedMaskVersion, Token,
};
use crate::mask::storage::{LayoutRecord, MaskRecord, Native};
use nrz_bindings::Limits;
use nrz_scalar::Scalar;
use std::rc::Rc;

/// The explicit shared-allocation ABI has two counters followed by its aligned payload.
#[repr(C)]
struct Shared<T> {
    strong: usize,
    weak: usize,
    payload: T,
}
/// Native and descriptor slot backings retain their occupation generation and optional record.
#[repr(C)]
struct Slot<T> {
    generation: u64,
    value: Option<T>,
}
/// Independent host-core field inventory, including the final strong liveness handle.
#[repr(C)]
struct HostMirror {
    identity: Identity,
    declaration: Declaration,
    channels: [Option<crate::loan::payload::LoanChannel>; 3],
    layout: Rc<LayoutRecord>,
    bundle: Rc<BundleOwner>,
    pin: Rc<PinRecord>,
    headers: [Token; 5],
    cleanup: CleanupReceipt,
    witness: Rc<Liveness>,
}
/// Fixed symbolic/evidence fields and the independently owned outcomes have no hidden allocation.
#[repr(C)]
struct EvidenceMirror {
    operation: crate::CheckedOperation,
    inputs: Rc<[crate::RegionContract]>,
    outputs: Rc<[crate::RegionContract]>,
    law: Rc<crate::kernels::KernelContract>,
    source: Declaration,
    source_identity: Identity,
    attachments: [Option<QualifiedMaskVersion>; 3],
    demand: crate::MaskVersion,
    destination: Identity,
    revision: u32,
    outcomes: Vec<crate::CellOutcome>,
    token: Token,
}
/// One application box includes the shared host fields and every additional inline owner.
#[repr(C)]
struct CoreMirror<P: Scalar> {
    input: Option<crate::BundlePin<P>>,
    contract: crate::kernels::KernelContract,
    extent_token: Token,
    filled: Vec<u64>,
    filled_token: Token,
    evidence: EvidenceMirror,
    population: usize,
    candidate: bool,
    host: HostMirror,
}
/// Independent field layouts are checked against actual Rust owners before byte-neighbour tests.
fn headers<P: Scalar>() -> usize {
    assert_eq!(
        size_of::<HostMirror>(),
        size_of::<crate::loan::lifecycle::HostCore>()
    );
    assert_eq!(
        size_of::<EvidenceMirror>(),
        size_of::<crate::loan::application::Evidence>()
    );
    assert_eq!(
        size_of::<CoreMirror<P>>(),
        size_of::<crate::loan::application::ApplicationCore<P>>()
    );
    size_of::<CoreMirror<P>>()
        + size_of::<Shared<LayoutRecord>>()
        + size_of::<Shared<Liveness>>()
        + size_of::<Shared<BundleOwner>>()
        + size_of::<Shared<PinRecord>>()
        + size_of::<Shared<[crate::Extent; 1]>>()
}
/// Initial host publication frees 2h tracking rows and two headers before application admission.
fn byte_fixture<P: Scalar>(cells: usize, components: usize, channels: usize) -> [usize; 2] {
    let length = cells * components;
    let rounded = length.div_ceil(64 / size_of::<P>()) * 64;
    let words = cells.div_ceil(64);
    let tracking = length.div_ceil(64);
    let old_debits = 3 * channels + 6;
    let new_debits = old_debits + 17 - (2 * channels + 2);
    let descriptor_delta = 3 * size_of::<Slot<DescriptorRecord>>();
    let group_delta = 3 * size_of::<Option<Group>>();
    let delta = descriptor_delta
        + group_delta
        + size_of::<Slot<Native<LayoutRecord>>>()
        + size_of::<Slot<Native<MaskRecord>>>()
        + (new_debits - old_debits) * size_of::<Option<Debit>>()
        + size_of::<Option<PinIndex>>();
    let retained = 3 * (rounded + 8 * tracking + 8 * words)
        + 8 * words
        + cells * size_of::<crate::CellOutcome>()
        + headers::<P>()
        + delta;
    let old = channels * size_of::<Slot<DescriptorRecord>>()
        + channels * size_of::<Option<Group>>()
        + size_of::<Slot<Native<LayoutRecord>>>()
        + 2 * size_of::<Slot<Native<MaskRecord>>>()
        + old_debits * size_of::<Option<Debit>>()
        + size_of::<Option<PinIndex>>();
    let temporary = 17 * size_of::<Debit>()
        + 5 * size_of::<Key>()
        + 3 * size_of::<DescriptorRecord>()
        + size_of::<Native<LayoutRecord>>()
        + size_of::<Native<MaskRecord>>();
    [retained, retained + 2 * old + temporary]
}
/// Independent exact-growth totals are enforced at N and N minus one for both byte ceilings.
fn boundaries<P: Scalar + 'static>() -> TestResult {
    for cells in [0, 1, 7, 8, 9, 15, 16, 17, 65] {
        for channels in 1..=3 {
            let [retained, staged] = byte_fixture::<P>(cells, 1, channels);
            for staging in [false, true] {
                let required = if staging { staged } else { retained };
                for succeeds in [false, true] {
                    let mut limits = Limits::default();
                    if staging {
                        limits.staged_payload_bytes =
                            u64::try_from(required - usize::from(!succeeds))?;
                    } else {
                        limits.output_bytes = u64::try_from(required - usize::from(!succeeds))?;
                    }
                    let Fixture {
                        mut heap,
                        mut account,
                        request,
                    } = fixture::<P>(cells, 1, channels, 2, false, limits)?;
                    let before = heap.resource_bytes()?;
                    match heap.request_pointwise_square(request, &mut account) {
                        crate::ApplicationRequest::ApplicationReady(owner) => {
                            assert!(succeeds);
                            assert_eq!(heap.resource_bytes()? - before, retained);
                            assert!(matches!(
                                heap.cancel_application(owner, &mut account),
                                crate::ApplicationCancellation::ApplicationCancelled(_)
                            ));
                        }
                        crate::ApplicationRequest::ApplicationRefused(
                            crate::ApplicationError::Loan(crate::LoanError::Heap(
                                crate::HeapError::ResourceBytes {
                                    staged: actual,
                                    used,
                                    ..
                                },
                            )),
                            _,
                        ) => {
                            assert!(!succeeds);
                            assert_eq!(actual, staging);
                            assert_eq!(used, u64::try_from(required)?);
                            assert_eq!(heap.resource_bytes()?, before);
                        }
                        other @ crate::ApplicationRequest::ApplicationRefused(..) => {
                            return Err(format!("{other:?}").into());
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
/// Byte fixtures count input residence once and admit only this call's exact replacement peak.
/// ´test:unit:pointwise-byte-neighbours-independent-abi-and-rounding´
#[test]
fn pointwise_byte_neighbours_independent_abi_and_rounding() -> TestResult {
    boundaries::<f32>()?;
    boundaries::<f64>()
}
/// An already populated application table supplies spare capacity without a replacement charge.
/// ´test:unit:pointwise-spare-capacity-byte-inventory´
#[test]
fn pointwise_spare_capacity_byte_inventory() -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture::<f64>(3, 1, 3, 2, false, Limits::default())?;
    let external = heap.retain_pin(&request.input, &mut account)?;
    let operation = request.operation.clone();
    let symbols = request.destination.symbols.clone();
    let axis = request.destination.axis;
    let demand = request.demand;
    let owner = ready(heap.request_pointwise_square(request, &mut account))?;
    assert!(matches!(
        heap.cancel_application(owner, &mut account),
        crate::ApplicationCancellation::ApplicationCancelled(_)
    ));
    assert!(matches!(
        heap.reconcile_detached(&mut account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    assert!(matches!(
        heap.reclaim(&mut account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    let destination = crate::HostTileDeclaration::check(
        symbols,
        axis,
        crate::Coverage::RepresentedInputs,
        &mut account,
    )?;
    let request = crate::PointwiseRequest {
        operation,
        input: external,
        destination,
        demand,
        limits: crate::ExecutionLimits::default(),
    };
    let before = heap.resource_bytes()?;
    let owner = ready(heap.request_pointwise_square(request, &mut account))?;
    // Three rounded slabs, six tracking words, one prospective word and three inline outcomes.
    let retained = 3 * (64 + 8 + 8) + 8 + 3 * size_of::<crate::CellOutcome>() + headers::<f64>();
    assert_eq!(heap.resource_bytes()? - before, retained);
    assert!(matches!(
        heap.cancel_application(owner, &mut account),
        crate::ApplicationCancellation::ApplicationCancelled(_)
    ));
    Ok(())
}

/// Independently counted vacant table capacities after source and result are reclaimed.
pub(super) const fn vacant_after_single_application() -> usize {
    size_of::<crate::mask::storage::Store>()
        + 6 * size_of::<Slot<DescriptorRecord>>()
        + 6 * size_of::<Option<Group>>()
        + 2 * size_of::<Slot<Native<LayoutRecord>>>()
        + 3 * size_of::<Slot<Native<MaskRecord>>>()
        + 24 * size_of::<Option<Debit>>()
        + 2 * size_of::<Option<PinIndex>>()
}
/// The publication shell's independent plan is tested without altering a live account's ceilings.
/// ´test:unit:pointwise-publication-shell-isolated-byte-neighbours´
#[test]
fn pointwise_publication_shell_isolated_byte_neighbours() -> TestResult {
    for staging in [false, true] {
        for succeeds in [false, true] {
            let mut limits = Limits::default();
            let required = size_of::<Shared<MaskRecord>>();
            if staging {
                limits.staged_payload_bytes = u64::try_from(required - usize::from(!succeeds))?;
            } else {
                limits.output_bytes = u64::try_from(required - usize::from(!succeeds))?;
            }
            let heap = crate::Heap::new(crate::HeapLimits::default())?;
            let mut budget = nrz_bindings::Budget::new(limits)?;
            let result =
                crate::loan::application::publication::shell_preflight(&heap, 1, &mut budget);
            assert_eq!(result.is_ok(), succeeds);
            if let Err(crate::LoanError::Heap(crate::HeapError::ResourceBytes {
                staged,
                used,
                ..
            })) = result
            {
                assert_eq!(staged, staging);
                assert_eq!(used, u64::try_from(required)?);
            }
        }
    }
    Ok(())
}
/// Fixed ABI literals are obtained from these separately laid-out mirrors, never the planner.
/// ´test:unit:pointwise-literal-abi-byte-totals´
#[test]
fn pointwise_literal_abi_byte_totals() {
    let independently_laid_out = [
        byte_fixture::<f32>(0, 1, 1),
        byte_fixture::<f64>(0, 1, 3),
        byte_fixture::<f32>(3, 1, 3),
        byte_fixture::<f64>(3, 1, 3),
        byte_fixture::<f32>(17, 1, 2),
        byte_fixture::<f64>(9, 1, 2),
        [headers::<f64>(), vacant_after_single_application()],
        [
            size_of::<crate::CellOutcome>(),
            size_of::<Shared<MaskRecord>>(),
        ],
    ];
    // Slot/header field mirrors give descriptor 256, group 888, native 32,
    // debit 248 and weak pin 40 bytes. Temporary backings are
    // 17*248 + 5*16 + 3*248 + 2*24 = 5088 bytes.
    // Retained table delta is 3*(256+888) + 2*32 + 40 + (15-2*h)*248.
    // H=6008 is the sum of the independent core/shared field mirrors above.
    // Empty B is 6008 + 7256 - 496*h; replaced old backings are
    // h*(256+888) + 3*32 + (3*h+6)*248 + 40.
    // C=3 adds 3*(64+8+8)+8+3*24=320 in either format.
    // Binary32 C=17 rounds to 128 slab bytes and adds 432+8+408=848;
    // binary64 C=9 also rounds to 128 and adds 432+8+216=656.
    // Final vacant capacity: 48 + 6*256 + 6*888 + 5*32 + 24*248 + 2*40.
    assert_eq!(
        [
            size_of::<Slot<DescriptorRecord>>(),
            size_of::<Option<Group>>(),
            size_of::<Slot<Native<MaskRecord>>>(),
            size_of::<Option<Debit>>(),
            size_of::<Option<PinIndex>>()
        ],
        [256, 888, 32, 248, 40]
    );
    assert_eq!(
        independently_laid_out,
        [
            [12768, 24880],
            [11776, 31440],
            [12096, 31760],
            [12096, 31760],
            [13120, 29008],
            [12928, 28816],
            [6008, 13104],
            [24, 64]
        ]
    );
}
