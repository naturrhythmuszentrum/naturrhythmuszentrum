// SPDX-License-Identifier: AGPL-3.0-only
//! Coordinate-map operations share the caller's exact Work and retained-payload ceilings.

use crate::testing::normalization::{entries, frame};
use crate::testing::readings::{Fixture, TestResult, ledger};
use nrz_bindings::readings::{CoordinateDirection, CoordinateNormalization};
use nrz_bindings::{Budget, Limits};

/// Every new exact operation succeeds at N and refuses at N minus one without a policy reset.
/// ´test:integration:coordinate-normalization-resource-boundaries´
#[test]
fn coordinate_normalization_resource_boundaries() -> TestResult {
    for (index, [work, staged, _]) in profiles().into_iter().enumerate() {
        let operation = u32::try_from(index)?;
        assert_eq!(attempt(operation, Limits::default(), 0)?, work);
        attempt(
            operation,
            Limits {
                work,
                ..Limits::default()
            },
            0,
        )?;
        assert!(
            attempt(
                operation,
                Limits {
                    work: work - 1,
                    ..Limits::default()
                },
                0
            )
            .is_err()
        );
        assert!(
            attempt(
                operation,
                Limits {
                    work,
                    ..Limits::default()
                },
                1
            )
            .is_err()
        );
        attempt(
            operation,
            Limits {
                staged_payload_bytes: staged,
                ..Limits::default()
            },
            0,
        )?;
        assert!(
            attempt(
                operation,
                Limits {
                    staged_payload_bytes: staged - 1,
                    ..Limits::default()
                },
                0
            )
            .is_err()
        );
    }
    Ok(())
}

/// Build independent inputs before constraining the one measured operation.
fn attempt(operation: u32, limits: Limits, precharge: u64) -> TestResult<u64> {
    let mut budget = Budget::new(limits)?;
    budget.work(
        precharge,
        nrz_cas::Location::new("coordinate normalization precharge"),
    )?;
    if operation == 0 {
        let mut case = Fixture::new(&[])?;
        CoordinateNormalization::new(&mut case.cas, &case.context, &entries()?, &mut budget)?;
    } else {
        let (mut case, coordinates) = frame()?;
        match operation {
            1 => {
                coordinates.image(&mut case.cas, CoordinateDirection::Normalize, &mut budget)?;
            }
            2 => {
                coordinates.image(&mut case.cas, CoordinateDirection::Restore, &mut budget)?;
            }
            3 => {
                coordinates.conversion_to(&mut case.cas, &coordinates, &mut budget)?;
            }
            4 => {
                coordinates.reuse_key(&mut case.cas, &mut budget)?;
            }
            5 => {
                let image = coordinates.image(
                    &mut case.cas,
                    CoordinateDirection::Restore,
                    &mut ledger()?,
                )?;
                image.export(&mut budget)?;
            }
            _ => {
                coordinates.check_entries(coordinates.entries(), &mut budget)?;
            }
        }
    }
    Ok(budget.usage().work)
}

/// Complete exact results fit their output payload boundary, while a pure identity check owns no output.
/// ´test:integration:coordinate-normalization-output-boundaries´
#[test]
fn coordinate_normalization_output_boundaries() -> TestResult {
    for (index, [_, _, output]) in profiles().into_iter().enumerate() {
        let operation = u32::try_from(index)?;
        attempt(
            operation,
            Limits {
                output_bytes: output,
                ..Limits::default()
            },
            0,
        )?;
        if output > 0 {
            assert!(
                attempt(
                    operation,
                    Limits {
                        output_bytes: output - 1,
                        ..Limits::default()
                    },
                    0
                )
                .is_err()
            );
        }
    }
    Ok(())
}

/// Measured default-profile Work, staged bytes and output bytes for the independent mixed reference fixture.
const fn profiles() -> [[u64; 3]; 7] {
    [
        [285_787, 8_390_928, 15_664],
        [97_953, 8_405_680, 23_961],
        [95_905, 8_405_680, 23_961],
        [232_979, 8_421_344, 42_964],
        [71_487, 65_190, 3_327],
        [1_024, 24_689, 560],
        [19, 16, 0],
    ]
}
