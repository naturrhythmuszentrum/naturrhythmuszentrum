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
    // Export, cold: complete reservation prepays Work; all failures retain only prior usage.
    (
        Operation::Export,
        false,
        [735, 0, 0, 0, 754, 0, 735, 735, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    // Export, warm: complete reservation prepays Work; all failures retain only prior usage.
    (
        Operation::Export,
        true,
        [735, 0, 0, 0, 754, 0, 735, 735, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ),
    // Replay, cold: positive prefixes retain prior successful charges; zero prefixes are initial preflights.
    (
        Operation::Replay,
        false,
        [
            66_913, 87, 17, 2, 17_971, 8_276, 735, 0, 66_913, 17, 36, 17, 36, 3, 1, 20, 0, 0,
        ],
        [
            66_851, 66_738, 35_136, 17_626, 61_698, 35_736, 263, 0, 66_851, 35_736, 35_736, 35_736,
            35_736, 275, 18_022, 550, 0, 0,
        ],
    ),
    // Replay, warm: positive prefixes retain prior successful charges; zero prefixes are initial preflights.
    (
        Operation::Replay,
        true,
        [
            66_879, 87, 17, 2, 18_099, 210, 735, 0, 66_879, 16, 2, 17, 36, 3, 1, 20, 0, 0,
        ],
        [
            66_817, 66_704, 35_114, 17_626, 61_664, 35_468, 263, 0, 66_817, 618, 10_996, 8_982,
            8_982, 275, 18_022, 550, 0, 0,
        ],
    ),
];

/// Every positive ceiling has a typed one-below refusal; zero ceilings execute successfully.
/// ´test:integration:half-pi-portable-boundaries´
#[test]
fn half_pi_portable_boundaries() -> TestResult {
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
const BITS: [BitRow; 2] = [
    (Operation::Replay, false, 66_913, [65, 66], [1, 34_021]),
    (Operation::Replay, true, 66_879, [65, 66], [1, 33_999]),
];

/// Concrete integer components and exact arithmetic refuse before oversized allocation.
/// ´test:integration:half-pi-portable-bits´
#[test]
fn half_pi_portable_bits() -> TestResult {
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
