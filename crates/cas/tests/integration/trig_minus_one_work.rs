// SPDX-License-Identifier: AGPL-3.0-only
//! Literal complete-operation boundaries include prior Work and retained payload.

use crate::testing::common::TestResult;
use crate::testing::half_pi_limits::{KINDS, verify};
use crate::testing::trig_minus_one_work::{Operation, attempt};
use nrz_cas::limits::BudgetKind;

/// One complete operation's measured ceilings, successful Work and refusal prefixes.
type ResourceRow = (usize, Operation, [u64; 18], u64, u64, [u64; 18]);

/// Independently measured literal, negation, conditional, quotient and sum portable operations.
const ROWS: [ResourceRow; 10] = [
    (
        0,
        Operation::Export,
        [
            3_566, 0, 0, 0, 3_585, 0, 3_566, 3_566, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        3_566,
        3_566,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    (
        0,
        Operation::Replay,
        [
            814_680, 48, 460, 8, 112_642, 52_840, 3_566, 0, 814_680, 98, 280, 98, 280, 32, 6, 16,
            0, 0,
        ],
        814_680,
        789_960,
        [
            814_636, 613_789, 576_730, 576_731, 726_486, 577_201, 260, 0, 814_636, 577_201,
            577_201, 577_201, 577_201, 577_207, 445_164, 1_520, 0, 0,
        ],
    ),
    (
        1,
        Operation::Export,
        [
            3_494, 0, 0, 0, 3_513, 0, 3_494, 3_494, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        3_494,
        3_494,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    (
        1,
        Operation::Replay,
        [
            773_151, 48, 449, 8, 107_900, 53_199, 3_494, 0, 773_151, 98, 281, 98, 281, 32, 6, 21,
            0, 0,
        ],
        773_151,
        747_066,
        [
            773_107, 580_304, 539_805, 539_806, 686_944, 540_276, 260, 0, 773_107, 540_276,
            540_276, 540_276, 540_276, 540_282, 504_135, 93_121, 0, 0,
        ],
    ),
    (
        2,
        Operation::Export,
        [
            4_305, 0, 0, 0, 4_324, 0, 4_305, 4_305, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        4_305,
        4_305,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    (
        2,
        Operation::Replay,
        [
            1_098_487, 125, 724, 8, 135_794, 65_463, 4_305, 0, 1_098_487, 114, 345, 114, 345, 36,
            11, 21, 0, 0,
        ],
        1_098_487,
        1_049_137,
        [
            1_098_377, 846_348, 1_098_197, 790_171, 978_491, 790_975, 264, 0, 1_098_377, 790_975,
            790_975, 790_975, 790_975, 790_981, 631_217, 124_962, 0, 0,
        ],
    ),
    (
        3,
        Operation::Export,
        [
            4_148, 0, 0, 0, 4_167, 0, 4_148, 4_148, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        4_148,
        4_148,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    (
        3,
        Operation::Replay,
        [
            1_042_958, 78, 616, 8, 130_834, 63_980, 4_148, 0, 1_042_958, 114, 343, 114, 343, 36,
            11, 21, 0, 0,
        ],
        1_042_958,
        999_483,
        [
            1_042_914, 801_908, 751_440, 751_441, 929_106, 752_109, 260, 0, 1_042_914, 752_109,
            752_109, 752_109, 752_109, 752_115, 599_211, 118_688, 0, 0,
        ],
    ),
    (
        4,
        Operation::Export,
        [
            4_098, 0, 0, 0, 4_117, 0, 4_098, 4_098, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        4_098,
        4_098,
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    (
        4,
        Operation::Replay,
        [
            994_553, 68, 539, 8, 129_698, 61_266, 4_098, 0, 994_553, 112, 325, 112, 325, 36, 8, 21,
            0, 0,
        ],
        994_553,
        962_358,
        [
            994_509, 756_822, 709_048, 709_049, 882_170, 709_585, 260, 0, 994_509, 709_585,
            709_585, 709_585, 709_585, 709_591, 566_550, 115_494, 0, 0,
        ],
    ),
];

/// Export prepays the copy; production and replay retain their completed incremental charges.
/// Every refusal compares the complete graph snapshot; zero ceilings still execute.
/// ´test:integration:trig-minus-one-portable-resource-boundaries´
#[test]
fn trig_minus_one_portable_resource_boundaries() -> TestResult {
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
