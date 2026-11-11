// SPDX-License-Identifier: AGPL-3.0-only
//! Public binary lifecycles check exact images, whole-cell completion and joint selected loads.

use super::{
    BinaryElementwise, Limits, TestResult, admit, candidate, custom, fixture, oracle, published,
    ready,
};
use crate::loan::fault;
use crate::{CellOutcome, DomainReason, HostChannel, KernelError, LoanError};

/// Read every output through the public selected reader and refuse prepared endpoints.
pub(super) fn readings(
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
/// Nine ordered role combinations and shifted holes determine A independently of native words.
#[test]
fn binary_joint_selection_roles_and_component_order() -> TestResult {
    for function in [BinaryElementwise::Product, BinaryElementwise::Quotient] {
        for cells in [0, 1, 63, 64, 65] {
            for components in [1, 3] {
                for left in 1..=3 {
                    for right in 1..=3 {
                        for selection in 0..3 {
                            selection_case(cells, components, [left, right], selection, function)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
/// One complete attempt has literal selected load, scalar and store counts even across holes.
fn selection_case(
    cells: usize,
    components: usize,
    roles: [usize; 2],
    selection: usize,
    function: BinaryElementwise,
) -> TestResult {
    let mut setup = fixture(
        cells,
        components,
        roles,
        selection,
        true,
        function,
        Limits::default(),
    )?;
    let expected = (0..cells)
        .filter(|cell| {
            super::requested(*cell, selection)
                && super::fixture::available(*cell, components, roles, true)
        })
        .collect::<Vec<_>>();
    assert_ne!(
        setup.request.left.ticket()?.identity.layout,
        setup.request.right.ticket()?.identity.layout
    );
    let owner = ready(admit(
        &mut setup.heap,
        setup.request,
        function,
        &mut setup.account,
    ))?;
    fault::reset_reads();
    fault::reset_scalars();
    fault::reset_stores();
    let injection = fault::Injection::after(0);
    let owner = candidate(owner, &mut setup.account)?;
    assert_eq!(fault::Injection::attempts(), 0);
    assert_eq!(
        fault::reads(),
        (2 + usize::from(roles[0] > 1) + usize::from(roles[1] > 1)) * components * expected.len()
    );
    assert_eq!(fault::scalars(), components * expected.len());
    assert_eq!(fault::stores(), 3 * components * expected.len());
    drop(injection);
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    let reader = tile.reader(&setup.heap, &mut setup.account)?;
    for cell in 0..cells {
        assert_eq!(
            reader.contains(cell, &mut setup.account)?,
            expected.contains(&cell)
        );
        if expected.contains(&cell) {
            for component in 0..components {
                let sign = if (cell + component) % 2 == 0 {
                    1.0
                } else {
                    -1.0
                };
                let radii = roles.map(|roles| if roles > 1 { 0.25 } else { 0.0 });
                oracle::check(
                    [2.0 * sign, -3.0 * sign],
                    radii,
                    function,
                    readings(&reader, cell, component, &mut setup.account)?,
                )?;
            }
        } else {
            let outcome = reader.application_outcome(cell, &mut setup.account)?;
            if super::requested(cell, selection) {
                assert!(matches!(outcome, CellOutcome::MissingInput));
            } else {
                assert!(matches!(outcome, CellOutcome::NotRequested));
            }
            let before = fault::reads();
            assert!(matches!(
                reader.read_component(HostChannel::Center, cell, 0, &mut setup.account),
                Err(LoanError::CoverageMismatch)
            ));
            assert_eq!(fault::reads(), before);
        }
    }
    Ok(())
}
/// Nonrepresentable centers, mixed signs, identities, underflow and extreme ratios have exact truth.
#[test]
fn binary_published_exact_rational_images_and_nearest_centers() -> TestResult {
    use BinaryElementwise::{Product, Quotient};
    let tiny = f64::from_bits(1);
    for (function, centers, radii) in [
        (Product, [1.1, 1.1], [0.0; 2]),
        (
            Product,
            [1.5, f64::from_bits(0x3ff0_0000_0000_0001)],
            [0.0; 2],
        ),
        (
            Product,
            [1.5, f64::from_bits(0x3ff0_0000_0000_0003)],
            [0.0; 2],
        ),
        (Quotient, [1.0, 3.0], [0.0; 2]),
        (Product, [-1.1, 1.1], [0.25; 2]),
        (Quotient, [1.0, -3.0], [0.25; 2]),
        (Product, [0.0, -2.0], [0.0; 2]),
        (Quotient, [-0.0, -2.0], [0.0; 2]),
        (Product, [-1.0, 2.0], [0.0; 2]),
        (Quotient, [2.0, -1.0], [0.0; 2]),
        (Product, [tiny, 0.5], [0.0; 2]),
        (Product, [-tiny, 0.5], [0.0; 2]),
        (Product, [f64::from_bits(3), 0.5], [0.0; 2]),
        (Product, [f64::from_bits(5), 0.5], [0.0; 2]),
        (Quotient, [f64::from_bits(3), 2.0], [0.0; 2]),
        (Quotient, [f64::from_bits(5), 2.0], [0.0; 2]),
        (Product, [f64::MIN_POSITIVE, 0.5], [0.0; 2]),
        (
            Product,
            [f64::MIN_POSITIVE.next_down(), 1.0_f64.next_up()],
            [0.0; 2],
        ),
        (Product, [1e-200, 1e-200], [0.0; 2]),
        (Product, [1e200, 1e-200], [0.0; 2]),
        (Quotient, [1e300, 1e300], [0.0; 2]),
        (Quotient, [1e-300, 1e300], [0.0; 2]),
        (Quotient, [f64::from_bits(2), f64::from_bits(2)], [0.0; 2]),
    ] {
        let mut setup = custom(
            1,
            1,
            [2; 2],
            2,
            false,
            function,
            Limits::default(),
            |operand, _, _| (centers[operand], radii[operand]),
        )?;
        let owner = candidate(
            ready(admit(
                &mut setup.heap,
                setup.request,
                function,
                &mut setup.account,
            ))?,
            &mut setup.account,
        )?;
        assert!(
            matches!(owner.core.evidence.outcomes[0], CellOutcome::Completed),
            "{centers:?}: {:?}",
            owner.core.evidence.outcomes
        );
        let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
        let reader = tile.reader(&setup.heap, &mut setup.account)?;
        oracle::check(
            centers,
            radii,
            function,
            readings(&reader, 0, 0, &mut setup.account)?,
        )?;
        drop(setup.heap);
        let reader = tile.pin().historical_reader(&mut setup.account)?;
        oracle::check(
            centers,
            radii,
            function,
            readings(&reader, 0, 0, &mut setup.account)?,
        )?;
    }
    assert!((1.0 / f64::from_bits(2)).is_infinite());
    Ok(())
}
/// Nominal error never loads or adds either declared overlapping input error a second time.
#[test]
fn binary_error_roles_gate_without_double_counting() -> TestResult {
    for function in [BinaryElementwise::Product, BinaryElementwise::Quotient] {
        let mut results = Vec::new();
        for roles in [[2, 2], [2, 3], [3, 2], [3, 3]] {
            let mut setup = fixture(1, 1, roles, 2, false, function, Limits::default())?;
            let owner = ready(admit(
                &mut setup.heap,
                setup.request,
                function,
                &mut setup.account,
            ))?;
            fault::reset_reads();
            let owner = candidate(owner, &mut setup.account)?;
            assert_eq!(fault::reads(), 4);
            let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
            results.push(
                readings(
                    &tile.reader(&setup.heap, &mut setup.account)?,
                    0,
                    0,
                    &mut setup.account,
                )?
                .map(f64::to_bits),
            );
        }
        assert!(results.iter().all(|result| result == &results[0]));
    }
    Ok(())
}
/// Both exact and outward-only zero contact refuse before shortcuts; every admitted suffix runs.
#[test]
fn binary_domain_refusal_retains_original_entry_and_continues() -> TestResult {
    use BinaryElementwise::{Product, Quotient};
    for (function, centers, radii, reason) in [
        (Quotient, [0.0, 0.0], [0.0; 2], DomainReason::ZeroInDivisor),
        (Quotient, [0.0, -0.0], [0.0; 2], DomainReason::ZeroInDivisor),
        (
            Quotient,
            [0.0, 1.0],
            [0.0, 1.0],
            DomainReason::ZeroInDivisor,
        ),
        (
            Quotient,
            [1.0, -1.0],
            [0.0, 1.0],
            DomainReason::ZeroInDivisor,
        ),
        (
            Quotient,
            [0.0, f64::from_bits(1)],
            [0.0; 2],
            DomainReason::ZeroInDivisor,
        ),
        (
            Product,
            [f64::MAX, 0.0],
            [0.0; 2],
            DomainReason::UnboundedImage,
        ),
        (
            Product,
            [1e200, 1e200],
            [0.0; 2],
            DomainReason::UnboundedImage,
        ),
        (
            Quotient,
            [f64::MAX, 1.0],
            [0.0; 2],
            DomainReason::UnboundedImage,
        ),
        (
            Quotient,
            [1e300, 1e-300],
            [0.0; 2],
            DomainReason::UnboundedImage,
        ),
        (
            Product,
            [0.0, 2.0],
            [f64::MAX, 0.0],
            DomainReason::UnboundedImage,
        ),
    ] {
        for first in [0, 1, 2] {
            let mut setup = custom(
                3,
                3,
                [2; 2],
                2,
                false,
                function,
                Limits::default(),
                |operand, cell, component| {
                    if cell == 1 && component >= first {
                        (centers[operand], radii[operand])
                    } else {
                        (2.0, 0.0)
                    }
                },
            )?;
            let owner = ready(admit(
                &mut setup.heap,
                setup.request,
                function,
                &mut setup.account,
            ))?;
            fault::reset_reads();
            fault::reset_scalars();
            fault::reset_stores();
            let owner = candidate(owner, &mut setup.account)?;
            assert_eq!(
                [fault::reads(), fault::scalars(), fault::stores()],
                [36, 9, 27]
            );
            assert!(
                matches!(owner.core.evidence.outcomes[1], CellOutcome::CellRefused { component, reason: KernelError::BinaryDomain { function: actual, entry, reason: cause } } if component == first && actual == function && entry == 1 + 3 * first && cause == reason)
            );
            let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
            let reader = tile.reader(&setup.heap, &mut setup.account)?;
            assert!(!reader.contains(1, &mut setup.account)?);
            for cell in [0, 2] {
                assert!(reader.contains(cell, &mut setup.account)?);
            }
        }
    }
    Ok(())
}

/// Center-only distinct tiny bundles succeed by direct division; the adjacent divisor touches zero.
#[test]
fn binary_center_only_subnormal_direct_division_and_all_refused_completion() -> TestResult {
    for divisor in [1, 2] {
        let mut setup = custom(
            65,
            2,
            [1; 2],
            2,
            false,
            BinaryElementwise::Quotient,
            Limits::default(),
            |operand, _, _| (f64::from_bits(if operand == 0 { 2 } else { divisor }), 0.0),
        )?;
        assert_ne!(setup.request.left.ticket()?, setup.request.right.ticket()?);
        let owner = candidate(
            ready(admit(
                &mut setup.heap,
                setup.request,
                BinaryElementwise::Quotient,
                &mut setup.account,
            ))?,
            &mut setup.account,
        )?;
        if divisor == 1 {
            assert_eq!(owner.core.filled, [0; 2]);
        }
        let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
        let reader = tile.reader(&setup.heap, &mut setup.account)?;
        for cell in 0..65 {
            assert_eq!(reader.contains(cell, &mut setup.account)?, divisor == 2);
            if divisor == 2 {
                oracle::check(
                    [f64::from_bits(2); 2],
                    [0.0; 2],
                    BinaryElementwise::Quotient,
                    readings(&reader, cell, 1, &mut setup.account)?,
                )?;
            } else {
                assert!(matches!(
                    reader.application_outcome(cell, &mut setup.account)?,
                    CellOutcome::CellRefused {
                        component: 0,
                        reason: KernelError::BinaryDomain {
                            function: BinaryElementwise::Quotient,
                            reason: DomainReason::ZeroInDivisor,
                            ..
                        }
                    }
                ));
            }
        }
    }
    Ok(())
}
