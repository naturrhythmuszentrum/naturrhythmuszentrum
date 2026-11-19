// SPDX-License-Identifier: AGPL-3.0-only
//! Reachable finite-format cells are separate from the defensive conversion matrix.
use super::{Scalar, TestResult, checks, demand, failure, oracle, request, setup};
use crate::{LcOmegaOutput, LcProductError, LcRequestedState};
use nrz_bindings::Limits;

pub(super) fn run<P: Scalar + 'static>() -> TestResult {
    for case in 0..11 {
        let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
        let output = LcOmegaOutput::declare(
            &s.inventory,
            request(&s.registry, &s.radian),
            &mut s.account,
        )?;
        let mut values = [[[P::ONE, P::ZERO, P::ZERO]; 2]; 5];
        let (left, right, code) = inputs::<P>(case);
        values[0][0] = left;
        values[1][0] = right;
        if case == 10 {
            values[4][0][0] = -P::ONE;
        }
        let masks = std::array::from_fn(|_| vec![0, 3, 11]);
        let members = super::super::super::inventory::readers::cells::publish_values(
            &s.inventory,
            &s.axes,
            &mut s.heap,
            &mut s.account,
            &masks,
            1,
            &values,
        )?;
        let prepared = output
            .prepare(
                &mut s.heap,
                &members.references(),
                &s.registry,
                &mut s.account,
            )
            .map_err(|f| f.into_parts().1)?;
        let expected = {
            let product = crate::LcInputReaders::captured(
                prepared.declaration.inventory(),
                &prepared.receipt.closure.rows,
            )?
            .into_cells(&mut s.account)?
            .into_product([0, 1], &mut s.account)?;
            let mut expected = Vec::new();
            for cell in [0, 3, 11] {
                let expected_code = if cell == 11 { [2, 0, 0] } else { code };
                let observed = product.read_requested(
                    cell,
                    crate::LcQuantityDemand::Omega,
                    None,
                    &mut s.account,
                );
                let numerical = reference(observed, expected_code)?;
                expected.push((cell, expected_code, numerical));
            }
            expected
        };
        let mask = demand(&mut s.heap, &s.axes, &[0, 3, 11], &mut s.account)?;
        let pending = prepared
            .admit(&mut s.heap, mask, &mut s.account)
            .map_err(|f| f.into_parts().1)?;
        let before = s.account.usage().work;
        crate::loan::fault::reset_reads();
        let injection = crate::loan::fault::Injection::after(0);
        let candidate = pending.evaluate(&mut s.account).map_err(failure)?;
        assert_eq!(s.account.usage().work - before, 436);
        assert_eq!(crate::loan::fault::reads(), 45);
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        drop(injection);
        checks::cells(&candidate, &expected)?;
        super::super::publication::images(candidate, &expected, mask, &mut s.heap, &mut s.account)?;
    }
    Ok(())
}
fn inputs<P: Scalar>(case: usize) -> ([P; 3], [P; 3], [u16; 3]) {
    let plain = |value| [value, P::ZERO, P::ZERO];
    let one = plain(P::ONE);
    match case {
        0 => (plain(-P::ONE), one, [3, 1, 1]),
        1 => (one, plain(-P::ONE), [3, 2, 1]),
        2 => (plain(P::ZERO), one, [4, 1, 2]),
        3 => (one, [P::ONE, P::ONE, P::ZERO], [4, 2, 2]),
        4 => {
            let broad = [P::ONE, P::from_u8(3) / P::from_u8(4), P::ZERO];
            (broad, broad, [4, 3, 2])
        }
        5 => (
            plain(P::min_positive_normal()),
            plain(P::ONE / P::from_u8(2)),
            [2, 0, 0],
        ),
        6 => (
            plain(P::max_finite() / P::from_u8(16)),
            plain(P::from_u8(2)),
            [2, 0, 0],
        ),
        7 => (plain(P::max_finite()), plain(P::from_u8(2)), [5, 3, 0x0104]),
        8 => (
            plain(P::min_positive_normal()),
            plain(P::min_positive_normal()),
            [4, 3, 2],
        ),
        9 => (plain(-P::ONE), plain(-P::ONE), [3, 1, 1]),
        _ => (one, one, [2, 0, 0]),
    }
}
fn reference<P: Scalar>(
    result: Result<crate::LcRequestedCell<'_, '_, P, 5>, LcProductError>,
    code: [u16; 3],
) -> TestResult<Option<[P; 3]>> {
    if code[0] == 5 {
        assert!(matches!(
            result,
            Err(LcProductError::Kernel(crate::KernelError::BinaryDomain {
                function: crate::BinaryElementwise::Product,
                reason: crate::DomainReason::UnboundedImage,
                ..
            }))
        ));
        return Ok(None);
    }
    let result = result?;
    match result.state() {
        LcRequestedState::Ready {
            tau,
            rate: Some(rate),
            ..
        } => {
            assert_eq!(code, [2, 0, 0]);
            let [left, right] =
                [0, 1].map(|position| result.inputs().readings()[position].values());
            oracle::enclosure(left, right, tau, rate)?;
            Ok(Some(rate))
        }
        LcRequestedState::Invalid(target) | LcRequestedState::Undecided(target) => {
            let status = if matches!(result.state(), LcRequestedState::Invalid(_)) {
                3
            } else {
                4
            };
            let target = match target {
                crate::LcFrequencyTarget::Inductance => 1,
                crate::LcFrequencyTarget::Capacitance => 2,
                crate::LcFrequencyTarget::Product => 3,
                crate::LcFrequencyTarget::Root => 4,
            };
            assert_eq!(code, [status, target, status - 2]);
            Ok(None)
        }
        LcRequestedState::Ready { rate: None, .. } => Err("omega dependency missing".into()),
    }
}
