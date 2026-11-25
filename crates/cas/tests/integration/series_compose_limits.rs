// SPDX-License-Identifier: AGPL-3.0-only
//! Complete-operation measurements include the full independent checking lifecycle.

use crate::testing::common::TestResult;

/// The composed object has independent cold and retained admission boundaries.
/// ´test:integration:compose-object-admission-boundaries´
#[test]
fn compose_object_admission_boundaries() -> TestResult {
    use crate::testing::series_work::{KINDS, attempt_values, attempt_values_detail};
    use nrz_cas::limits::BudgetKind;
    use nrz_cas::{Exhaustion, FaultKind};
    for (warm, ceilings, prefixes) in [
        (
            false,
            [1627, 6837, 5229, 12, 21, 12, 21, 2, 3, 4, 0],
            [1626, 1622, 1622, 1622, 1622, 1622, 1622, 1441, 37, 37, 0],
        ),
        (
            true,
            [1622, 6901, 93, 0, 8, 12, 21, 2, 3, 4, 0],
            [1535, 130, 122, 0, 1449, 130, 130, 1441, 37, 37, 0],
        ),
    ] {
        for ((dimension, ceiling), prefix) in KINDS.into_iter().zip(ceilings).zip(prefixes) {
            assert_eq!(
                attempt_values(warm, dimension, ceiling, &["0", "1", "0", "-2"])?,
                (true, if warm { 1_622 } else { 1_627 })
            );
            if ceiling > 0 {
                let (success, work, fault) =
                    attempt_values_detail(warm, dimension, ceiling - 1, &["0", "1", "0", "-2"])?;
                assert_eq!((success, work), (false, prefix));
                let needed = if dimension == BudgetKind::StagedPayloadBytes {
                    ceiling - 113
                } else {
                    ceiling
                };
                assert_eq!(
                    fault.ok_or("admission fault")?.kind,
                    FaultKind::Exhausted {
                        reason: if dimension == BudgetKind::ComponentBits {
                            Exhaustion::ArithmeticRange
                        } else {
                            Exhaustion::Limit
                        },
                        budget: dimension,
                        limit: needed - 1,
                        needed,
                    }
                );
            }
        }
    }
    Ok(())
}

/// Literal full-operation ceilings retain prior spend and check exact refusal prefixes.
/// ´test:integration:compose-complete-resource-boundaries´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "Literal complete-operation resource tables remain together for audit"
)]
fn compose_complete_resource_boundaries() -> TestResult {
    use crate::testing::series_boundaries::{KINDS, attempt, attempt_detail};
    use nrz_cas::limits::BudgetKind;
    use nrz_cas::{Exhaustion, FaultKind};
    for (mode, kind, ceilings, prefixes) in [
        (
            0,
            13,
            [
                509_619, 85_386, 18_497, 21, 61, 21, 61, 3442, 8678, 1, 5409, 1, 2, 3, 98, 1, 0, 3,
            ],
            [
                509_615, 91_813, 197_232, 197_232, 191_695, 197_232, 191_695, 509_615, 191_695,
                174_179, 495_408, 174_179, 3750, 83_070, 173_954, 37, 0, 127_659,
            ],
        ),
        (
            1,
            13,
            [
                517_524, 341_162, 17_904, 20, 56, 20, 56, 3442, 8678, 1, 6252, 1, 2, 3, 149, 1, 0,
                3,
            ],
            [
                517_520, 99_718, 205_137, 205_137, 199_600, 205_137, 199_600, 517_520, 199_600,
                34_840, 503_313, 182_084, 21_462, 294, 181_859, 205_142, 0, 22_045,
            ],
        ),
        (
            2,
            13,
            [
                517_365, 349_968, 2755, 17, 62, 20, 62, 3442, 17_356, 1, 6251, 1, 2, 3, 149, 1, 0,
                3,
            ],
            [
                517_361, 99_569, 199_452, 504_344, 199_451, 34_151, 199_451, 517_361, 199_451,
                34_840, 503_154, 181_935, 21_462, 294, 181_710, 204_983, 0, 22_045,
            ],
        ),
        (
            3,
            13,
            [
                3479, 3555, 0, 0, 0, 0, 0, 3442, 3442, 0, 0, 0, 1, 0, 0, 0, 0, 1,
            ],
            [37, 37, 0, 0, 0, 0, 0, 37, 37, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
        (
            4,
            13,
            [
                621_268, 85_386, 21_353, 27, 73, 27, 73, 3442, 8678, 1, 6193, 1, 2, 3, 110, 1,
                621_231, 3,
            ],
            [
                621_217, 181_489, 286_908, 286_908, 281_371, 286_908, 281_371, 329, 281_371,
                15_247, 606_951, 263_855, 21_019, 38, 263_630, 341, 621_217, 217_335,
            ],
        ),
        (
            0,
            14,
            [
                144_609, 28_385, 11_130, 20, 53, 20, 53, 1331, 2892, 2, 996, 5, 1, 2, 26, 3, 0, 2,
            ],
            [
                144_605, 140_780, 48_678, 48_678, 47_530, 48_678, 47_530, 144_605, 47_530, 43_786,
                134_038, 43_786, 0, 11_884, 43_561, 37, 0, 31_964,
            ],
        ),
        (
            1,
            14,
            [
                137_386, 42_593, 10_537, 19, 48, 19, 48, 1331, 2892, 2, 1047, 3, 1, 2, 28, 2, 0, 2,
            ],
            [
                137_382, 133_557, 41_455, 41_455, 40_307, 41_455, 40_307, 137_382, 40_307, 36_563,
                126_815, 36_563, 0, 294, 36_338, 41_461, 0, 5170,
            ],
        ),
        (
            2,
            14,
            [
                137_374, 42_593, 560, 17, 56, 19, 56, 1331, 5049, 2, 1047, 3, 1, 2, 28, 2, 0, 2,
            ],
            [
                137_370, 133_545, 40_308, 116_402, 40_307, 9142, 40_307, 137_370, 40_307, 36_563,
                126_803, 36_563, 0, 294, 36_338, 41_449, 0, 5170,
            ],
        ),
        (
            3,
            14,
            [
                1368, 1444, 0, 0, 0, 0, 0, 1331, 1331, 0, 0, 0, 1, 0, 0, 0, 0, 1,
            ],
            [37, 37, 0, 0, 0, 0, 0, 37, 37, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
        (
            4,
            14,
            [
                218_198, 28_385, 13_986, 26, 65, 26, 65, 1331, 2892, 2, 1122, 4, 1, 2, 27, 2,
                218_161, 2,
            ],
            [
                218_147, 214_263, 114_519, 114_519, 113_371, 114_519, 113_371, 329, 113_371,
                109_627, 207_521, 109_627, 0, 38, 109_402, 341, 218_147, 97_805,
            ],
        ),
        (
            0,
            15,
            [
                46_821, 15_969, 9232, 19, 45, 19, 45, 707, 1013, 1, 181, 1, 3, 0, 5, 1, 0, 4,
            ],
            [
                46_817, 44_833, 9484, 9484, 9057, 9484, 9057, 46_817, 9057, 7617, 41_802, 7617,
                154, 0, 7392, 37, 0, 6368,
            ],
        ),
        (
            1,
            15,
            [
                48_172, 18_497, 8639, 18, 40, 18, 40, 707, 1013, 1, 204, 1, 3, 0, 8, 1, 0, 4,
            ],
            [
                48_168, 46_184, 10_835, 10_835, 10_408, 10_835, 10_408, 48_168, 10_408, 2353,
                43_153, 8968, 756, 0, 8743, 10_840, 0, 1012,
            ],
        ),
        (
            2,
            15,
            [
                48_162, 18_625, 200, 14, 46, 18, 46, 707, 2026, 1, 204, 1, 3, 0, 8, 1, 0, 4,
            ],
            [
                48_158, 46_174, 10_409, 44_333, 10_408, 1661, 10_408, 48_158, 10_408, 2353, 43_143,
                8968, 756, 0, 8743, 10_830, 0, 1012,
            ],
        ),
        (
            3,
            15,
            [744, 820, 0, 0, 0, 0, 0, 707, 707, 0, 0, 0, 1, 0, 0, 0, 0, 1],
            [37, 37, 0, 0, 0, 0, 0, 37, 37, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
        (
            4,
            15,
            [
                74_465, 15_969, 12_088, 25, 57, 25, 57, 707, 1013, 1, 219, 1, 3, 0, 7, 1, 74_428, 4,
            ],
            [
                74_414, 72_371, 33_918, 33_918, 33_491, 33_918, 33_491, 329, 33_491, 13_711,
                69_340, 32_051, 16_138, 0, 31_826, 341, 74_414, 30_802,
            ],
        ),
    ] {
        for ((dimension, n), prefix) in KINDS.into_iter().zip(ceilings).zip(prefixes) {
            assert!(
                attempt(mode, kind, dimension, n)?.0,
                "{mode}/{kind} {dimension:?} at {n}"
            );
            if n > 0 {
                let (success, work, fault) = attempt_detail(mode, kind, dimension, n - 1)?;
                assert_eq!(
                    (success, work),
                    (false, prefix),
                    "mode {mode} kind {kind} {dimension:?}"
                );
                if let Some(fault) = fault {
                    // These effective nested ceilings are measured independently of the outer N.
                    let needed = if dimension == BudgetKind::StagedPayloadBytes {
                        match (mode, kind) {
                            (3, 13) => 3442,
                            (3, 14) => 1331,
                            (3, 15) => 707,
                            (_, 13) => 8,
                            (_, 14) => 27_760,
                            (_, 15) => 15_344,
                            _ => return Err("fixture".into()),
                        }
                    } else {
                        n
                    };
                    assert_eq!(
                        fault.kind,
                        FaultKind::Exhausted {
                            reason: if matches!(
                                dimension,
                                BudgetKind::ComponentBits | BudgetKind::IntermediateBits
                            ) {
                                Exhaustion::ArithmeticRange
                            } else {
                                Exhaustion::Limit
                            },
                            budget: dimension,
                            limit: needed - 1,
                            needed,
                        },
                        "mode {mode} kind {kind} {dimension:?}"
                    );
                }
            }
        }
    }
    Ok(())
}

/// Each searched ceiling is replayed at N and N−1 with canonical rollback and prior usage.
/// ´test:integration:compose-measure-resources´
#[test]
#[ignore = "development measurement of complete operation boundaries"]
#[expect(
    clippy::print_stdout,
    reason = "Explicit development measurements emit the complete resource table"
)]
fn compose_measure_resources() -> TestResult {
    for kind in 13..16 {
        for mode in 0..5 {
            let (ceilings, prefixes) = crate::testing::series_boundaries::measure(mode, kind)?;
            println!("COMPOSE-RESOURCE {mode} {kind} {ceilings:?} {prefixes:?}");
            for (index, (dimension, n)) in crate::testing::series_boundaries::KINDS
                .into_iter()
                .zip(ceilings)
                .enumerate()
            {
                if n > 0 {
                    let (_, _, fault) = crate::testing::series_boundaries::attempt_detail(
                        mode,
                        kind,
                        dimension,
                        n - 1,
                    )?;
                    if let Some(fault) = fault {
                        println!("COMPOSE-FAULT {mode} {kind} {index} {:?}", fault.kind);
                    }
                }
            }
        }
    }
    for warm in [false, true] {
        let mut ceilings = Vec::new();
        let mut prefixes = Vec::new();
        for dimension in crate::testing::series_work::KINDS {
            let mut low = if dimension == nrz_cas::limits::BudgetKind::Work {
                37
            } else {
                0
            };
            let mut high = crate::testing::common::default_budget()?
                .limits()
                .get(dimension);
            while low < high {
                let middle = low + (high - low) / 2;
                if crate::testing::series_work::attempt_values(
                    warm,
                    dimension,
                    middle,
                    &["0", "1", "0", "-2"],
                )?
                .0
                {
                    high = middle;
                } else {
                    low = middle + 1;
                }
            }
            ceilings.push(low);
            if low > 0 {
                let (_, _, fault) = crate::testing::series_work::attempt_values_detail(
                    warm,
                    dimension,
                    low - 1,
                    &["0", "1", "0", "-2"],
                )?;
                println!(
                    "COMPOSE_ADMISSION_FAULT {warm} {dimension:?} {:?}",
                    fault.ok_or("admission fault")?.kind
                );
            }
            prefixes.push(if low > 0 {
                crate::testing::series_work::attempt_values(
                    warm,
                    dimension,
                    low - 1,
                    &["0", "1", "0", "-2"],
                )?
                .1
            } else {
                0
            });
        }
        println!("COMPOSE_ADMISSION {warm} {ceilings:?} {prefixes:?}");
    }
    Ok(())
}
