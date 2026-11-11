// SPDX-License-Identifier: AGPL-3.0-only
//! Independent ABI mirrors enumerate every owner and replacement backing in the byte fixture.

use super::{Fixture, TestResult, fixture, ready};
use crate::heap::{descriptors::DescriptorRecord, identity::Key};
use crate::loan::abi;
use crate::loan::registry::{Debit, PinIndex};
use crate::mask::storage::{LayoutRecord, MaskRecord, Native};
use nrz_bindings::Limits;
use nrz_scalar::Scalar;

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
/// Initial host publication frees 2h tracking rows and two headers before application admission.
fn byte_fixture<P: Scalar>(cells: usize, components: usize, channels: usize) -> [usize; 2] {
    let length = cells * components;
    let rounded = length.div_ceil(64 / size_of::<P>()) * 64;
    let words = cells.div_ceil(64);
    let tracking = length.div_ceil(64);
    let old_debits = 3 * channels + 6;
    let new_debits = old_debits + 17 - (2 * channels + 2);
    let descriptor_delta = 3 * size_of::<Slot<DescriptorRecord>>();
    let group_delta = 3 * abi::group_row();
    let delta = descriptor_delta
        + group_delta
        + size_of::<Slot<Native<LayoutRecord>>>()
        + size_of::<Slot<Native<MaskRecord>>>()
        + (new_debits - old_debits) * size_of::<Option<Debit>>()
        + size_of::<Option<PinIndex>>();
    let retained = 3 * (rounded + 8 * tracking + 8 * words)
        + 8 * words
        + cells * abi::outcome_width()
        + abi::application_headers::<P>()
        + delta;
    let old = channels * size_of::<Slot<DescriptorRecord>>()
        + channels * abi::group_row()
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
fn boundaries() -> TestResult {
    for cells in [0, 1, 7, 8, 9, 15, 16, 17, 65] {
        for channels in 1..=3 {
            let [retained, staged] = byte_fixture::<f64>(cells, 1, channels);
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
                    } = fixture::<f64>(cells, 1, channels, 2, false, limits)?;
                    let before = heap.resource_bytes()?;
                    match heap.request_pointwise_square_root(request, &mut account) {
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
/// ´test:unit:square_root-byte-neighbours-independent-abi-and-rounding´
#[test]
fn square_root_byte_neighbours_independent_abi_and_rounding() -> TestResult {
    boundaries()
}
/// An already populated application table supplies spare capacity without a replacement charge.
/// ´test:unit:square_root-spare-capacity-byte-inventory´
#[test]
fn square_root_spare_capacity_byte_inventory() -> TestResult {
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
    let owner = ready(heap.request_pointwise_square_root(request, &mut account))?;
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
    let owner = ready(heap.request_pointwise_square_root(request, &mut account))?;
    // Three rounded slabs, six tracking words, one prospective word and three inline outcomes.
    let retained =
        3 * (64 + 8 + 8) + 8 + 3 * abi::outcome_width() + abi::application_headers::<f64>();
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
        + 6 * abi::group_row()
        + 2 * size_of::<Slot<Native<LayoutRecord>>>()
        + 3 * size_of::<Slot<Native<MaskRecord>>>()
        + 24 * size_of::<Option<Debit>>()
        + 2 * size_of::<Option<PinIndex>>()
}
/// The publication shell's independent plan is tested without altering a live account's ceilings.
/// ´test:unit:square_root-publication-shell-isolated-byte-neighbours´
#[test]
fn square_root_publication_shell_isolated_byte_neighbours() -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture::<f64>(3, 1, 3, 2, false, Limits::default())?;
    let owner = ready(heap.request_pointwise_square_root(request, &mut account))?;
    for staging in [false, true] {
        for succeeds in [false, true] {
            let mut limits = Limits::default();
            let required = size_of::<Shared<MaskRecord>>();
            if staging {
                limits.staged_payload_bytes = u64::try_from(required - usize::from(!succeeds))?;
            } else {
                limits.output_bytes = u64::try_from(required - usize::from(!succeeds))?;
            }
            let mut budget = nrz_bindings::Budget::new(limits)?;
            let result = crate::loan::application::publication::shell_preflight(
                &heap,
                &owner.core,
                &mut budget,
            );
            assert_eq!(result.is_ok(), succeeds);
            match result {
                Err(crate::LoanError::Heap(crate::HeapError::ResourceBytes {
                    staged,
                    used,
                    ..
                })) => {
                    assert_eq!((staged, used), (staging, u64::try_from(required)?));
                }
                Err(error) => return Err(error.into()),
                Ok(()) => assert!(succeeds),
            }
        }
    }
    Ok(())
}
/// Fixed ABI literals are obtained from these separately laid-out mirrors, never the planner.
/// ´test:unit:square_root-literal-abi-byte-totals´
#[test]
fn square_root_literal_abi_byte_totals() {
    let independently_laid_out = [
        byte_fixture::<f64>(0, 1, 1),
        byte_fixture::<f64>(1, 1, 1),
        byte_fixture::<f64>(3, 1, 1),
        byte_fixture::<f64>(1, 1, 2),
        byte_fixture::<f64>(1, 1, 3),
        byte_fixture::<f64>(9, 1, 1),
        [
            abi::application_headers::<f64>(),
            vacant_after_single_application(),
        ],
        [abi::outcome_width(), size_of::<Shared<MaskRecord>>()],
    ];
    // Optional endpoints add 2*LoanChannel to the core and 2*PublishedChannel to the bundle.
    // Captured correspondence, endpoint identities and completed-mask identity add 272 to both evidence copies.
    // H = core 3296 + layout 248 + witness 240 + bundle 3216 + pin 72 + extent 40 = 7112.
    // Outcomes grow 24 -> 32 for the lossless prepared refusal; every group row grows 888 -> 1032.
    // Temporary backings remain 17*264 + 5*16 + 3*248 + 2*24 = 5360.
    // Table delta: 3*(256+1032) + 2*32 + 40 + (15-2*h)*264.
    // Empty B: 7112 + 7928 - 528*h. Replaced old backings:
    // h*(256+1032) + 3*32 + (3*h+6)*264 + 40.
    // C=1 adds 280; C=3 adds 344; binary64 C=9 adds 728.
    // Vacant tables: 48 + 6*256 + 6*1032 + 5*32 + 24*264 + 2*40 = 14352.
    assert_eq!(
        [
            size_of::<Slot<DescriptorRecord>>(),
            abi::group_row(),
            size_of::<Slot<Native<MaskRecord>>>(),
            size_of::<Option<Debit>>(),
            size_of::<Option<PinIndex>>()
        ],
        [256, 1032, 32, 264, 40]
    );
    assert_eq!(
        independently_laid_out,
        [
            [14_512, 27_472],
            [14_792, 27_752],
            [14_856, 27_816],
            [14_264, 31_384],
            [13_736, 35_016],
            [15_240, 28_200],
            [7_112, 14_352],
            [32, 64],
        ]
    );
}
