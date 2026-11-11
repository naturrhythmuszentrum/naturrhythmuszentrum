// SPDX-License-Identifier: AGPL-3.0-only
//! Independent selected sets and exact rational reciprocal images cover both signs and extremes.

use super::{TestResult, available, candidate, fixture, oracle, published, ready, requested};
use crate::loan::fault;
use crate::{CellOutcome, DomainReason, Elementwise, HostChannel, KernelError, LoanError};
use nrz_bindings::Limits;

/// Fetch only through the public result reader; endpoints are deliberately unavailable.
fn readings(
    reader: &crate::TileReader<'_, f64>,
    cell: usize,
    component: usize,
    account: &mut crate::ExecutionAccount,
) -> TestResult<[f64; 3]> {
    let values = [
        reader.read_component(HostChannel::Center, cell, component, account)?,
        reader.read_component(HostChannel::Total, cell, component, account)?,
        reader.read_component(HostChannel::Error, cell, component, account)?,
    ];
    let before = fault::reads();
    assert!(matches!(
        reader.expm1_endpoints(cell, component, account),
        Err(LoanError::UnsupportedContract)
    ));
    assert_eq!(fault::reads(), before);
    Ok(values)
}
/// Missing declared error gates availability even though only center/total are loaded.
/// ´test:unit:reciprocal-independent-selected-sets-and-native-visits´
#[test]
fn reciprocal_independent_selected_sets_and_native_visits() -> TestResult {
    for cells in [0, 1, 63, 64, 65, 128] {
        for components in [1, 3] {
            for channels in 1..=3 {
                for selection in 0..3 {
                    let mut setup = fixture::<f64>(
                        cells,
                        components,
                        channels,
                        selection,
                        true,
                        Limits::default(),
                    )?;
                    let owner = ready(
                        setup
                            .heap
                            .request_pointwise_reciprocal(setup.request, &mut setup.account),
                    )?;
                    let expected = (0..cells)
                        .filter(|cell| {
                            requested(*cell, selection)
                                && available(*cell, components, channels, true)
                        })
                        .collect::<Vec<_>>();
                    fault::reset_reads();
                    fault::reset_scalars();
                    fault::reset_stores();
                    let injection = fault::Injection::after(0);
                    let owner = candidate(owner, &mut setup.account)?;
                    assert_eq!(fault::Injection::attempts(), 0);
                    assert_eq!(fault::Injection::kernel_allocations(), [0; 2]);
                    drop(injection);
                    assert_eq!(
                        fault::reads(),
                        (if channels == 1 { 1 } else { 2 }) * components * expected.len()
                    );
                    assert_eq!(fault::scalars(), components * expected.len());
                    assert_eq!(fault::stores(), 3 * components * expected.len());
                    let (tile, _) =
                        published(setup.heap.publish_application(owner, &mut setup.account))?;
                    let reader = tile.reader(&setup.heap, &mut setup.account)?;
                    for cell in 0..cells {
                        assert_eq!(
                            reader.contains(cell, &mut setup.account)?,
                            expected.contains(&cell)
                        );
                        let outcome = reader.application_outcome(cell, &mut setup.account)?;
                        if expected.contains(&cell) {
                            assert!(matches!(outcome, CellOutcome::Completed));
                            for component in 0..components {
                                let actual =
                                    readings(&reader, cell, component, &mut setup.account)?;
                                assert_eq!(actual[0], if cell % 2 == 0 { 0.5 } else { -0.5 });
                            }
                        } else {
                            if requested(cell, selection) {
                                assert!(matches!(outcome, CellOutcome::MissingInput));
                            } else {
                                assert!(matches!(outcome, CellOutcome::NotRequested));
                            }
                            let before = fault::reads();
                            assert!(matches!(
                                reader.read_component(
                                    HostChannel::Center,
                                    cell,
                                    0,
                                    &mut setup.account
                                ),
                                Err(LoanError::CoverageMismatch)
                            ));
                            assert_eq!(fault::reads(), before);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
/// Exact rational images cover singleton non-dyadic quotients, rounding neighbors and finite extremes.
/// ´test:unit:reciprocal-published-exact-rational-value-and-enclosure-oracles´
#[test]
fn reciprocal_published_exact_rational_value_and_enclosure_oracles() -> TestResult {
    let values = [
        (2.0, 0.25),
        (-2.0, 0.25),
        (3.0, 0.0),
        (-3.0, 0.0),
        (1.0, 0.0),
        (-1.0, 0.0),
        (1.0_f64.next_up(), 0.0),
        (1.0_f64.next_down(), 0.0),
        (3.0_f64.next_up(), 0.0),
        (3.0_f64.next_down(), 0.0),
        (f64::MIN_POSITIVE, 0.0),
        (f64::MIN_POSITIVE.next_down(), 0.0),
        (f64::from_bits(1_u64 << 51), 0.0),
        (-f64::from_bits(1_u64 << 51), 0.0),
        (f64::from_bits(0x7fe0_0000_0000_0000), 0.0),
        (-f64::from_bits(0x7fe0_0000_0000_0000), 0.0),
    ];
    let mut setup = fixture::<f64>(values.len(), 1, 2, 2, false, Limits::default())?;
    super::super::refusal::replace_source(&mut setup, |cell, _| values[cell])?;
    let owner = candidate(
        ready(
            setup
                .heap
                .request_pointwise_reciprocal(setup.request, &mut setup.account),
        )?,
        &mut setup.account,
    )?;
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    let reader = tile.reader(&setup.heap, &mut setup.account)?;
    for (cell, (input, radius)) in values.into_iter().enumerate() {
        assert!(matches!(
            reader.application_outcome(cell, &mut setup.account)?,
            CellOutcome::Completed
        ));
        oracle::check(
            input,
            radius,
            readings(&reader, cell, 0, &mut setup.account)?,
        )?;
    }
    drop(setup.heap);
    let reader = tile.pin().historical_reader(&mut setup.account)?;
    for (cell, (input, radius)) in values.into_iter().enumerate() {
        oracle::check(
            input,
            radius,
            readings(&reader, cell, 0, &mut setup.account)?,
        )?;
    }
    Ok(())
}
/// The error role is included in total and has no numerical load or second radius contribution.
#[test]
fn reciprocal_declared_error_is_not_counted_twice() -> TestResult {
    let mut results = Vec::new();
    for channels in [2, 3] {
        let mut setup = fixture::<f64>(2, 1, channels, 2, false, Limits::default())?;
        let owner = ready(
            setup
                .heap
                .request_pointwise_reciprocal(setup.request, &mut setup.account),
        )?;
        fault::reset_reads();
        let owner = candidate(owner, &mut setup.account)?;
        assert_eq!(fault::reads(), 4);
        let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
        let reader = tile.reader(&setup.heap, &mut setup.account)?;
        let pair = [
            readings(&reader, 0, 0, &mut setup.account)?,
            readings(&reader, 1, 0, &mut setup.account)?,
        ];
        for (input, values) in [2.0, -2.0].into_iter().zip(pair) {
            oracle::check(input, 0.25, values)?;
        }
        results.push(pair.map(|values| values.map(f64::to_bits)));
    }
    assert_eq!(results[0], results[1]);
    Ok(())
}
/// Tiny boundaries follow outward division, not a normal-operand threshold from another law.
/// ´test:unit:reciprocal-domain-boundaries-first-refusal-and-continued-traversal´
#[test]
fn reciprocal_domain_boundaries_first_refusal_and_continued_traversal() -> TestResult {
    for (input, radius, reason) in [
        (0.0, 0.0, DomainReason::ZeroInDivisor),
        (-0.0, 0.0, DomainReason::ZeroInDivisor),
        (1.0, 1.0, DomainReason::ZeroInDivisor),
        (-1.0, 1.0, DomainReason::ZeroInDivisor),
        (1.0, 2.0, DomainReason::ZeroInDivisor),
        (-1.0, 2.0, DomainReason::ZeroInDivisor),
        (f64::from_bits(1), 0.0, DomainReason::ZeroInDivisor),
        (-f64::from_bits(1), 0.0, DomainReason::UnboundedImage),
        (f64::MAX, 0.0, DomainReason::UnboundedImage),
        (-f64::MAX, 0.0, DomainReason::UnboundedImage),
    ] {
        for failing in [&[2][..], &[0, 2][..], &[1, 2][..]] {
            let mut setup = fixture::<f64>(3, 3, 2, 2, false, Limits::default())?;
            super::super::refusal::replace_source(&mut setup, |cell, component| {
                if cell == 1 && failing.contains(&component) {
                    (input, radius)
                } else {
                    (2.0, 0.0)
                }
            })?;
            let owner = ready(
                setup
                    .heap
                    .request_pointwise_reciprocal(setup.request, &mut setup.account),
            )?;
            fault::reset_reads();
            fault::reset_scalars();
            fault::reset_stores();
            let owner = candidate(owner, &mut setup.account)?;
            assert_eq!(
                [fault::reads(), fault::scalars(), fault::stores()],
                [18, 9, 27]
            );
            assert!(
                matches!(owner.core.evidence.outcomes[1], CellOutcome::CellRefused { component, reason: KernelError::Domain { function: Elementwise::Reciprocal, entry, reason: actual } } if component == failing[0] && entry == 1 + 3 * failing[0] && actual == reason)
            );
            let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
            let reader = tile.reader(&setup.heap, &mut setup.account)?;
            assert!(!reader.contains(1, &mut setup.account)?);
            for cell in [0, 2] {
                assert!(reader.contains(cell, &mut setup.account)?);
                oracle::check(2.0, 0.0, readings(&reader, cell, 2, &mut setup.account)?)?;
            }
        }
    }
    Ok(())
}
/// An empty completed set is terminal and publishable even when every selected cell refused.
#[test]
fn reciprocal_all_refused_publication() -> TestResult {
    let mut setup = fixture::<f64>(65, 2, 1, 2, false, Limits::default())?;
    super::super::refusal::replace_source(&mut setup, |cell, _| {
        (if cell % 2 == 0 { 0.0 } else { -0.0 }, 0.0)
    })?;
    let owner = candidate(
        ready(
            setup
                .heap
                .request_pointwise_reciprocal(setup.request, &mut setup.account),
        )?,
        &mut setup.account,
    )?;
    assert_eq!(owner.core.filled, [0, 0]);
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    let reader = tile.reader(&setup.heap, &mut setup.account)?;
    for cell in 0..65 {
        assert!(!reader.contains(cell, &mut setup.account)?);
        assert!(matches!(
            reader.application_outcome(cell, &mut setup.account)?,
            CellOutcome::CellRefused { component: 0, .. }
        ));
    }
    Ok(())
}
