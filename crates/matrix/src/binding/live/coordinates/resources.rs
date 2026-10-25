// SPDX-License-Identifier: AGPL-3.0-only
//! Plan actual coordinate byte ceilings from retained images and complete selected array shapes.

use super::{
    CoordinateContract, CoordinateError, CoordinateReading, ReadingBinding, covariance,
    lowering::Lowered, numeric,
};
use crate::HeapError;
use nrz_bindings::readings::CompanionReadingKind;
use nrz_scalar::Scalar;

/// Logical peaks shared with the actual numerical allocation and result checks.
#[derive(Clone, Copy, Debug)]
pub(super) struct ExecutionBytes {
    /// Retained exact/result payload plus the largest simultaneous array reservation.
    pub(super) staged: u64,
    /// Complete result envelope, including original retained evidence.
    pub(super) output: u64,
}

/// Count only companions whose ordered targets include this original reading.
pub(super) fn normalization<P: Scalar>(
    binding: &ReadingBinding<P>,
    contract: &CoordinateContract,
    lowered: &Lowered<P>,
) -> Result<ExecutionBytes, CoordinateError> {
    let (entries, parameters) = binding.companions().iter().try_fold(
        (0_usize, 0_usize),
        |(entries, parameters), companion| {
            let targeted = companion
                .targets()
                .iter()
                .any(|target| target.reading() == binding.reading());
            let statistical = !matches!(
                companion.companion().kind(),
                CompanionReadingKind::Radius(_)
            );
            if targeted && statistical {
                Ok::<_, CoordinateError>((
                    entries
                        .checked_add(companion.entries().len())
                        .ok_or(HeapError::ExtentOverflow)?,
                    parameters.checked_add(1).ok_or(HeapError::ExtentOverflow)?,
                ))
            } else {
                Ok((entries, parameters))
            }
        },
    )?;
    let retained = contract
        .0
        .bytes
        .checked_add(lowered.bytes()?)
        .ok_or(HeapError::ExtentOverflow)?;
    plan::<P>(
        retained,
        binding.reading().entries().len(),
        entries,
        parameters,
        6,
    )
}

/// Restoration retains the complete earlier snapshot and transforms only its recorded parameters.
pub(super) fn restoration<P: Scalar>(
    input: &CoordinateReading<P>,
    lowered: &Lowered<P>,
) -> Result<ExecutionBytes, CoordinateError> {
    let entries = input
        .0
        .covariance
        .iter()
        .try_fold(0_usize, |count, parameter| {
            count
                .checked_add(parameter.centers.len())
                .ok_or(HeapError::ExtentOverflow)
        })?;
    let retained = input
        .0
        .bytes
        .checked_add(lowered.bytes()?)
        .ok_or(HeapError::ExtentOverflow)?;
    plan::<P>(
        retained,
        input.centers().len(),
        entries,
        input.0.covariance.len(),
        3,
    )
}

/// Exact preflight must also fit this finite schedule; no default allowance becomes a reservation.
fn plan<P: Scalar>(
    retained: u64,
    coordinates: usize,
    entries: usize,
    parameters: usize,
    channels: usize,
) -> Result<ExecutionBytes, CoordinateError> {
    let arrays = numeric::array_bytes::<P>(coordinates, 6)?;
    let covariance_peak = if parameters == 0 {
        0
    } else {
        covariance::staged_bytes::<P>(entries, parameters, coordinates, channels)?
    };
    let staged = retained
        .checked_add(arrays.max(covariance_peak))
        .ok_or(HeapError::ExtentOverflow)?;
    let output_retained = retained
        .checked_add(covariance::bytes::<P>(entries, parameters)?)
        .ok_or(HeapError::ExtentOverflow)?;
    let output = numeric::retained_bytes::<P>(coordinates, output_retained)?;
    Ok(ExecutionBytes { staged, output })
}
