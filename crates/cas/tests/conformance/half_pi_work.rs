// SPDX-License-Identifier: AGPL-3.0-only
//! Literal operation boundaries retain successful charge prefixes and atomic publication.

use crate::testing::common::TestResult;
use crate::testing::half_pi_limits::{KINDS, verify};
use crate::testing::half_pi_work::{Operation, attempt, success};
use nrz_cas::limits::BudgetKind;

/// Costs and failed prefixes exclude 37 prior Work; staging includes 19 caller bytes.
/// Each cold and warm row is a separate complete-operation server measurement.
/// Cold Check/Replay adds 8 retained-kind replacements +2 first-kind entries
/// +12 identity-index insertions +12 vertex admissions = 34 Work; cold Guarded
/// adds 22+2+26+26 = 76. Warm operations and Export add zero. Refusal
/// prefixes pay only the local entries reached before the typed refusal; the
/// commit ledger expands every moved prefix. Old/new lookup floors are equal.
const COSTS: [(Operation, bool, [u64; 18], [u64; 18]); 4] = [
    // Check, cold: positive prefixes retain prior successful charges; zero prefixes are initial preflights.
    (
        Operation::Check,
        false,
        [
            42_002, 70, 14, 2, 17_971, 7_372, 735, 0, 0, 16, 32, 15, 32, 3, 1, 20, 0, 0,
        ],
        [
            41_998, 41_089, 11_226, 4_277, 36_987, 11_826, 41_998, 0, 0, 35_998, 11_826, 11_826,
            11_826, 0, 2_886, 11_236, 0, 0,
        ],
    ),
    // Check, warm: positive prefixes retain prior successful charges; zero prefixes are initial preflights.
    (
        Operation::Check,
        true,
        [
            41_968, 70, 14, 2, 18_099, 210, 735, 0, 0, 16, 0, 0, 0, 3, 1, 20, 0, 0,
        ],
        [
            41_964, 41_055, 11_204, 4_267, 36_953, 11_558, 41_964, 0, 0, 35_964, 0, 0, 0, 0, 2_886,
            11_214, 0, 0,
        ],
    ),
    // Guarded, cold: positive prefixes retain prior successful charges; zero prefixes are initial preflights.
    (
        Operation::Guarded,
        false,
        [
            106_476, 100, 66, 4, 31_523, 14_542, 1_172, 0, 0, 28, 72, 28, 72, 6, 2, 20, 0, 0,
        ],
        [
            106_472, 105_101, 51_666, 51_667, 96_695, 52_284, 106_472, 0, 0, 52_284, 52_284,
            52_284, 52_284, 0, 40_940, 49_308, 0, 0,
        ],
    ),
    // Guarded, warm: positive prefixes retain prior successful charges; zero prefixes are initial preflights.
    (
        Operation::Guarded,
        true,
        [
            106_400, 100, 66, 4, 31_651, 258, 1_172, 0, 0, 28, 0, 28, 72, 6, 2, 20, 0, 0,
        ],
        [
            106_396, 105_025, 51_602, 51_603, 96_619, 51_926, 106_396, 0, 0, 95_630, 0, 30_144,
            30_144, 0, 40_888, 49_244, 0, 0,
        ],
    ),
];

/// Every positive ceiling has a typed one-below refusal; zero ceilings execute successfully.
/// ´test:integration:half-pi-checking-boundaries´
#[test]
fn half_pi_checking_boundaries() -> TestResult {
    for (operation, warm, costs, prefixes) in COSTS {
        verify(
            &KINDS,
            |kind| success(operation, warm, kind, costs[0]),
            &costs,
            &prefixes,
            |kind, amount| attempt(operation, warm, kind, amount),
        )?;
    }
    Ok(())
}

/// Operation, warmth, complete Work, bit ceilings and failed Work prefixes.
type BitRow = (Operation, bool, u64, [u64; 2], [u64; 2]);

/// Bit refusals retain the literal successful prefix for each independently measured row.
const BITS: [BitRow; 4] = [
    (Operation::Check, false, 42_002, [65, 66], [525, 10_111]),
    (Operation::Check, true, 41_968, [65, 66], [525, 10_089]),
    (Operation::Guarded, false, 106_476, [2, 2], [34_083, 33_955]),
    (Operation::Guarded, true, 106_400, [2, 2], [34_019, 33_891]),
];

/// Concrete integer components and exact arithmetic refuse before oversized allocation.
/// ´test:integration:half-pi-checking-bits´
#[test]
fn half_pi_checking_bits() -> TestResult {
    let kinds = [BudgetKind::ComponentBits, BudgetKind::IntermediateBits];
    for (operation, warm, work, costs, prefixes) in BITS {
        verify(
            &kinds,
            |kind| success(operation, warm, kind, work),
            &costs,
            &prefixes,
            |kind, amount| attempt(operation, warm, kind, amount),
        )?;
    }
    Ok(())
}
