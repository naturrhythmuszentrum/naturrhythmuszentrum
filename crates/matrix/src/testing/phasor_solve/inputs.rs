// SPDX-License-Identifier: AGPL-3.0-only
//! Every stored source copy has independently declared deterministic and numerical radii.

use crate::phasor_testing::tickets;
use crate::reading_testing::{Fixture, TestResult, ledger};
use crate::test_api::{CompanionAdapter, Extent, ReadingAdapter, ReadingRegion};
use nrz_bindings::{
    CompanionKind,
    readings::{CompanionParameters, CompanionReading, CompanionReadingKind},
};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// Input radius parameters are distinct per original stored copy and coverage channel.
pub(super) fn companions(
    fixture: &mut Fixture,
    adapters: &[ReadingAdapter<usize>],
) -> TestResult<Vec<CompanionAdapter<usize>>> {
    let mut result = Vec::new();
    for (start, prefix) in [(0, "impedance"), (5, "voltage")] {
        for (index, target) in adapters[start + 1..start + 5].iter().enumerate() {
            for (channel, kind) in [
                ("total", CompanionKind::DeterministicBound),
                ("error", CompanionKind::NumericalError),
            ] {
                let name = format!("{prefix}_{index}_{channel}");
                let parameters = CompanionParameters::new(
                    &fixture.cas,
                    &fixture.context,
                    &fixture.symbol(&format!("{name}_source"))?,
                    CompanionReadingKind::Radius(kind),
                    std::slice::from_ref(target.reading()),
                    &mut ledger()?,
                )?;
                let symbol = fixture.symbol(&format!("{name}_output"))?;
                let companion = CompanionReading::define(
                    &mut fixture.cas,
                    &symbol,
                    &parameters,
                    std::slice::from_ref(target.reading()),
                    &mut ledger()?,
                )?;
                result.push(CompanionAdapter::new(
                    companion,
                    tickets(2, &[u64::try_from(result.len())?]),
                    vec![target.clone()],
                    &mut ledger()?,
                )?);
            }
        }
    }
    Ok(result)
}

/// Dyadic discrepancies and input boxes keep independent exact-rational test oracles finite.
pub(super) fn regions<P: Scalar>(
    reactance: P,
    radius: P,
    rounded: bool,
) -> TestResult<Vec<ReadingRegion<P>>> {
    let discrepancy = if rounded {
        P::ONE / P::from_u8(8)
    } else {
        P::ZERO
    };
    let impedance = vec![P::ZERO, reactance, -reactance + discrepancy, P::ZERO];
    let voltage = vec![P::from_u8(3), P::ZERO, P::ZERO, P::from_u8(3) + discrepancy];
    let mut parameters = Vec::new();
    let context = ArithmeticContext::new()?;
    for (region, values) in [&impedance, &voltage].iter().enumerate() {
        for (index, _) in values.iter().enumerate() {
            let changed = (region == 0 && index == 2) || (region == 1 && index == 3);
            let expected = if region == 0 {
                -reactance
            } else {
                P::from_u8(3)
            };
            let error = if changed {
                Enclosure::singleton(Finite::new(values[index])?)
                    .displacement_from(Finite::new(expected)?, &context)?
                    .get()
            } else {
                P::ZERO
            };
            let uncertain = if region == 0 && (index == 1 || index == 2) {
                radius
            } else {
                P::ZERO
            };
            let total = Enclosure::singleton(Finite::new(uncertain)?)
                .add(Enclosure::singleton(Finite::new(error)?), &context)?
                .upper()
                .get();
            parameters.extend([total, error]);
        }
    }
    Ok(vec![
        ReadingRegion {
            extent: Extent::new(2, 2)?,
            values: impedance,
        },
        ReadingRegion {
            extent: Extent::new(2, 2)?,
            values: voltage,
        },
        ReadingRegion {
            extent: Extent::new(16, 1)?,
            values: parameters,
        },
    ])
}
