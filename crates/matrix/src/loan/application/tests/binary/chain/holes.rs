// SPDX-License-Identifier: AGPL-3.0-only
//! Upstream refusal and unavailable roles propagate as holes without loading placeholders.

use super::super as binary;
use super::super::super as common;
use super::{TestResult, admit, candidate, published, request as make_request};
use crate::{BinaryElementwise, CellOutcome, Elementwise};
use nrz_bindings::Limits;

/// Original host holes, upstream demand and downstream demand are three independent sets.
#[test]
fn chain_sparse_upstream_availability_and_selected_loads() -> TestResult {
    for cells in [0, 1, 63, 64, 65] {
        for components in [1, 3] {
            for upstream in 0..3 {
                for downstream in 0..3 {
                    let binary::Fixture {
                        mut heap,
                        mut account,
                        request: original,
                    } = binary::custom::<f64>(
                        cells,
                        components,
                        [3; 2],
                        upstream,
                        true,
                        BinaryElementwise::Product,
                        Limits::default(),
                        |operand, _, _| (if operand == 0 { 2.0 } else { 3.0 }, 0.125),
                    )?;
                    let owner =
                        binary::ready(heap.request_pointwise_product(original, &mut account))?;
                    let owner = candidate(owner, &mut account)?;
                    let (tile, _) = published(heap.publish_application(owner, &mut account))?;
                    let request = make_request(
                        &mut heap,
                        tile.into_pin(),
                        Elementwise::SquareRoot,
                        downstream,
                        &mut account,
                    )?;
                    let owner = common::ready(admit(
                        &mut heap,
                        request,
                        Elementwise::SquareRoot,
                        &mut account,
                    ))?;
                    crate::loan::fault::reset_reads();
                    crate::loan::fault::reset_scalars();
                    let owner = candidate(owner, &mut account)?;
                    let selected = |cell| {
                        common::requested(cell, upstream)
                            && common::requested(cell, downstream)
                            && binary::fixture::available(cell, components, [3; 2], true)
                    };
                    let count = (0..cells).filter(|cell| selected(*cell)).count();
                    assert_eq!(crate::loan::fault::reads(), 2 * components * count);
                    assert_eq!(crate::loan::fault::scalars(), components * count);
                    for cell in 0..cells {
                        let outcome = owner.application_outcome(cell, &mut account)?;
                        if selected(cell) {
                            assert!(matches!(outcome, CellOutcome::Completed));
                        } else if common::requested(cell, downstream) {
                            assert!(matches!(outcome, CellOutcome::MissingInput));
                        } else {
                            assert!(matches!(outcome, CellOutcome::NotRequested));
                        }
                    }
                    let _result = published(heap.publish_application(owner, &mut account))?;
                }
            }
        }
    }
    Ok(())
}

/// Product overflow, negative roots and zero-reaching root enclosures leave later cells executable.
/// The third exact host product is zero; the following reciprocal must refuse locally.
#[test]
fn chain_refusals_become_holes_and_zero_reaching_root_is_not_clipped() -> TestResult {
    let (mut heap, mut account, tile) = mixed_product()?;
    let request = make_request(
        &mut heap,
        tile.into_pin(),
        Elementwise::SquareRoot,
        2,
        &mut account,
    )?;
    let owner = common::ready(admit(
        &mut heap,
        request,
        Elementwise::SquareRoot,
        &mut account,
    ))?;
    crate::loan::fault::reset_reads();
    crate::loan::fault::reset_scalars();
    let owner = candidate(owner, &mut account)?;
    assert_eq!(
        [crate::loan::fault::reads(), crate::loan::fault::scalars()],
        [12, 6]
    );
    assert!(matches!(
        owner.application_outcome(0, &mut account)?,
        CellOutcome::MissingInput
    ));
    assert!(matches!(
        owner.application_outcome(1, &mut account)?,
        CellOutcome::CellRefused { .. }
    ));
    assert!(matches!(
        owner.application_outcome(2, &mut account)?,
        CellOutcome::Completed
    ));
    let (tile, _) = published(heap.publish_application(owner, &mut account))?;
    let request = make_request(
        &mut heap,
        tile.into_pin(),
        Elementwise::Reciprocal,
        2,
        &mut account,
    )?;
    let owner = common::ready(admit(
        &mut heap,
        request,
        Elementwise::Reciprocal,
        &mut account,
    ))?;
    crate::loan::fault::reset_reads();
    crate::loan::fault::reset_scalars();
    let owner = candidate(owner, &mut account)?;
    assert_eq!(
        [crate::loan::fault::reads(), crate::loan::fault::scalars()],
        [8, 4]
    );
    for cell in 0..2 {
        assert!(matches!(
            owner.application_outcome(cell, &mut account)?,
            CellOutcome::MissingInput
        ));
    }
    assert!(matches!(
        owner.application_outcome(2, &mut account)?,
        CellOutcome::CellRefused {
            reason: crate::KernelError::Domain {
                reason: crate::DomainReason::ZeroInDivisor,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        owner.application_outcome(3, &mut account)?,
        CellOutcome::Completed
    ));
    let _result = published(heap.publish_application(owner, &mut account))?;
    Ok(())
}

/// All-refused publication is valid and later full demand cannot read its initialized placeholders.
#[test]
fn chain_completion_becomes_empty_midway() -> TestResult {
    for value in [0.0, f64::from_bits(1_u64 << 48)] {
        let binary::Fixture {
            mut heap,
            mut account,
            request: original,
        } = binary::custom::<f64>(
            3,
            1,
            [1; 2],
            2,
            false,
            BinaryElementwise::Product,
            Limits::default(),
            |operand, _, _| (if operand == 0 { value } else { 1.0 }, 0.0),
        )?;
        let owner = binary::ready(heap.request_pointwise_product(original, &mut account))?;
        let owner = candidate(owner, &mut account)?;
        let (tile, _) = published(heap.publish_application(owner, &mut account))?;
        let tile = super::apply(
            &mut heap,
            tile.into_pin(),
            Elementwise::Reciprocal,
            2,
            &mut account,
        )?;
        // The second exact center is 2^-1026, whose reciprocal exceeds finite binary64.
        let reason = if value == 0.0 {
            crate::DomainReason::ZeroInDivisor
        } else {
            crate::DomainReason::UnboundedImage
        };
        let reader = tile.reader(&heap, &mut account)?;
        for cell in 0..3 {
            assert!(
                matches!(reader.application_outcome(cell, &mut account)?, CellOutcome::CellRefused {
                reason: crate::KernelError::Domain { reason: actual, .. }, ..
            } if *actual == reason)
            );
        }
        let request = make_request(
            &mut heap,
            tile.into_pin(),
            Elementwise::Square,
            2,
            &mut account,
        )?;
        let owner = common::ready(admit(&mut heap, request, Elementwise::Square, &mut account))?;
        crate::loan::fault::reset_reads();
        crate::loan::fault::reset_scalars();
        let owner = candidate(owner, &mut account)?;
        assert_eq!(
            [crate::loan::fault::reads(), crate::loan::fault::scalars()],
            [0; 2]
        );
        assert!(
            owner
                .core
                .evidence
                .outcomes
                .iter()
                .all(|outcome| matches!(outcome, CellOutcome::MissingInput))
        );
        let _result = published(heap.publish_application(owner, &mut account))?;
    }
    Ok(())
}

/// Exact independent host boxes produce overflow, a negative image, zero, and a valid suffix.
fn mixed_product() -> TestResult<(
    crate::Heap,
    crate::ExecutionAccount,
    crate::PublishedTile<f64>,
)> {
    let binary::Fixture {
        mut heap,
        mut account,
        request: original,
    } = binary::custom::<f64>(
        4,
        2,
        [3; 2],
        2,
        false,
        BinaryElementwise::Product,
        Limits::default(),
        |operand, cell, _| match (cell, operand) {
            (0, 0) => (f64::MAX, 0.0),
            (1, 0) => (-2.0, 0.0),
            (2, 0) => (0.0, 0.0),
            (_, 0) => (2.0, 0.0),
            _ => (3.0, 0.0),
        },
    )?;
    let owner = binary::ready(heap.request_pointwise_product(original, &mut account))?;
    let owner = candidate(owner, &mut account)?;
    assert!(matches!(
        owner.application_outcome(0, &mut account)?,
        CellOutcome::CellRefused { .. }
    ));
    let (tile, _) = published(heap.publish_application(owner, &mut account))?;
    Ok((heap, account, tile))
}

/// The exact root of the smallest positive dyadic is positive, but its published enclosure reaches zero.
#[test]
fn chain_positive_exact_root_can_refuse_reciprocal() -> TestResult {
    let mut setup = common::fixture::<f64>(1, 1, 1, 2, false, Limits::default())?;
    common::refusal::replace_source(&mut setup, |_, _| (f64::from_bits(1), 0.0))?;
    setup.request.operation = crate::OperationDraft::map_elementwise(
        setup.request.operation.inputs()[0].shape().clone(),
        setup.request.operation.outputs()[0].shape().clone(),
        Elementwise::SquareRoot,
    )?
    .check(crate::ExecutionLimits::default())?;
    let owner = common::ready(
        setup
            .heap
            .request_pointwise_square_root(setup.request, &mut setup.account),
    )?;
    let owner = candidate(owner, &mut setup.account)?;
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    let reader = tile.reader(&setup.heap, &mut setup.account)?;
    let mut readings = [0.0; 3];
    for (value, channel) in readings.iter_mut().zip(crate::HostChannel::ALL) {
        *value = reader.read_component(channel, 0, 0, &mut setup.account)?;
    }
    super::oracle::root([f64::from_bits(1), 0.0, 0.0], readings)?;
    assert_eq!(readings[0].to_bits(), 0x1e60_0000_0000_0000);
    assert!(readings[1] >= readings[0]);
    let request = make_request(
        &mut setup.heap,
        tile.into_pin(),
        Elementwise::Reciprocal,
        2,
        &mut setup.account,
    )?;
    let owner = common::ready(admit(
        &mut setup.heap,
        request,
        Elementwise::Reciprocal,
        &mut setup.account,
    ))?;
    let owner = candidate(owner, &mut setup.account)?;
    assert!(matches!(
        owner.application_outcome(0, &mut setup.account)?,
        CellOutcome::CellRefused {
            reason: crate::KernelError::Domain {
                reason: crate::DomainReason::ZeroInDivisor,
                ..
            },
            ..
        }
    ));
    let _tile = published(setup.heap.publish_application(owner, &mut setup.account))?;
    Ok(())
}

/// Exact rational corners independently give [1/4,77/4] although the nominal product is 6.
/// A symmetric published radius is at least 53/4, so its lower endpoint is at most -29/4.
/// The chained root must use that published box, not the positive exact product rectangle.
#[test]
fn chain_positive_product_with_negative_published_lower_refuses_root() -> TestResult {
    use binary::oracle::Rational;
    use std::cmp::Ordering;

    let centers = [2.0, 3.0];
    let radii = [1.5, 2.5];
    let range = super::oracle::rectangle(centers, radii)?;
    assert_eq!(range[0].compare(&Rational::binary(0.25)?), Ordering::Equal);
    assert_eq!(range[1].compare(&Rational::binary(19.25)?), Ordering::Equal);
    let binary::Fixture {
        mut heap,
        mut account,
        request,
    } = binary::custom::<f64>(
        1,
        1,
        [2; 2],
        2,
        false,
        BinaryElementwise::Product,
        Limits::default(),
        |operand, _, _| (centers[operand], radii[operand]),
    )?;
    let owner = candidate(
        binary::ready(heap.request_pointwise_product(request, &mut account))?,
        &mut account,
    )?;
    let (tile, _) = published(heap.publish_application(owner, &mut account))?;
    let readings = super::oracle::readings(&tile, &heap, &mut account, 0, 0)?;
    binary::oracle::check(centers, radii, BinaryElementwise::Product, readings)?;
    assert_eq!(readings[0].to_bits(), 6.0_f64.to_bits());
    let center = Rational::binary(readings[0])?;
    let total = Rational::binary(readings[1])?;
    let required = range[1].sub(&center);
    assert_eq!(required.compare(&Rational::binary(13.25)?), Ordering::Equal);
    assert_ne!(total.compare(&required), Ordering::Less);
    assert_ne!(
        center.sub(&total).compare(&Rational::binary(-7.25)?),
        Ordering::Greater
    );
    let tile = super::apply(
        &mut heap,
        tile.into_pin(),
        Elementwise::SquareRoot,
        2,
        &mut account,
    )?;
    let reader = tile.reader(&heap, &mut account)?;
    assert!(!reader.contains(0, &mut account)?);
    assert!(matches!(
        reader.application_outcome(0, &mut account)?,
        CellOutcome::CellRefused {
            component: 0,
            reason: crate::KernelError::Domain {
                function: Elementwise::SquareRoot,
                entry: 0,
                reason: crate::DomainReason::NegativeRoot,
            },
        }
    ));
    Ok(())
}
