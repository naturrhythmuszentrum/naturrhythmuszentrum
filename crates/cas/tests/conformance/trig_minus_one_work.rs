// SPDX-License-Identifier: AGPL-3.0-only
//! Literal complete-operation boundaries include prior Work and retained payload.

use crate::testing::common::TestResult;
use crate::testing::half_pi_limits::{KINDS, verify};
use crate::testing::trig_minus_one_work::{Operation, attempt};
use nrz_cas::limits::BudgetKind;

/// One production row and its complete dimension and prefix pins.
type Row = (usize, Operation, [u64; 18], u64, u64, [u64; 18]);

/// Independently measured production resource rows.
const ROWS: [Row; 5] = [
    (
        0,
        Operation::Produce,
        [
            718_811, 38, 465, 8, 112_770, 51_913, 3_566, 0, 0, 96, 275, 96, 275, 32, 6, 16, 0, 0,
        ],
        718_811,
        694_331,
        [
            718_807, 518_666, 621_904, 485_104, 630_709, 485_574, 718_807, 0, 0, 485_574, 485_574,
            485_574, 485_574, 485_580, 401_819, 196_285, 0, 0,
        ],
    ),
    (
        1,
        Operation::Produce,
        [
            657_185, 37, 451, 8, 108_028, 52_272, 3_494, 0, 0, 96, 276, 96, 276, 32, 6, 21, 0, 0,
        ],
        657_185,
        631_335,
        [
            657_181, 465_119, 563_595, 428_047, 571_070, 428_517, 657_181, 0, 0, 428_517, 428_517,
            428_517, 428_517, 428_523, 347_534, 142_908, 0, 0,
        ],
    ),
    (
        2,
        Operation::Produce,
        [
            937_093, 38, 720, 8, 135_922, 64_856, 4_305, 0, 0, 113, 342, 113, 342, 36, 11, 21, 0, 0,
        ],
        937_093,
        891_503,
        [
            937_089, 686_265, 807_355, 636_201, 817_490, 637_005, 937_089, 0, 0, 637_005, 637_005,
            637_005, 637_005, 637_011, 524_264, 263_386, 0, 0,
        ],
    ),
    (
        3,
        Operation::Produce,
        [
            906_124, 39, 617, 8, 130_962, 62_984, 4_148, 0, 0, 112, 338, 112, 338, 36, 11, 21, 0, 0,
        ],
        906_124,
        863_354,
        [
            906_120, 665_991, 782_229, 619_545, 792_364, 620_213, 906_120, 0, 0, 620_213, 620_213,
            620_213, 620_213, 620_219, 513_336, 263_190, 0, 0,
        ],
    ),
    (
        4,
        Operation::Produce,
        [
            865_054, 38, 545, 8, 129_826, 60_339, 4_098, 0, 0, 110, 320, 110, 320, 36, 8, 21, 0, 0,
        ],
        865_054,
        833_094,
        [
            865_050, 628_104, 742_628, 584_317, 752_763, 584_853, 865_050, 0, 0, 584_853, 584_853,
            584_853, 584_853, 584_859, 487_590, 263_081, 0, 0,
        ],
    ),
];

/// Export prepays the copy; production and replay retain their completed incremental charges.
/// Every refusal compares the complete graph snapshot; zero ceilings still execute.
/// ´test:integration:trig-minus-one-checking-resource-boundaries´
#[test]
fn trig_minus_one_checking_resource_boundaries() -> TestResult {
    for (input, operation, costs, work, name_work, prefixes) in ROWS {
        verify(
            &KINDS,
            |kind| {
                if kind == BudgetKind::NameBytes {
                    name_work
                } else {
                    work
                }
            },
            &costs,
            &prefixes,
            |resource, amount| attempt(input, operation, resource, amount),
        )?;
    }
    Ok(())
}
