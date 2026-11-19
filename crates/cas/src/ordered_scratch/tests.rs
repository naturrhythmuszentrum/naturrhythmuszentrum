// SPDX-License-Identifier: AGPL-3.0-only
//! Independent small tariffs exercise ordering, replacement and exhaustion before mutation.

use super::Map;
use crate::budget::{Budget, BudgetKind, Limits};
use crate::error::{Exhaustion, Fault, FaultKind, Location};
use crate::testing::common::TestResult;

/// Stable location for ordered scratch controls.
const AT: Location = Location::new("ordered scratch control");

/// Three out-of-order keys force both search directions and suffix movement.
fn numeric(budget: &mut Budget) -> Result<(), Fault> {
    let mut map = Map::new();
    map.reserve(3, budget, AT)?;
    map.insert(30_u64, 3, budget, AT)?;
    map.insert(10, 1, budget, AT)?;
    map.insert(20, 2, budget, AT)?;
    assert_eq!(map.keys().copied().collect::<Vec<_>>(), [10, 20, 30]);
    assert_eq!(map.get(&20, budget, AT)?, Some(&2));
    assert_eq!(map.insert(20, 4, budget, AT)?, Some(2));
    assert_eq!(map.remove(&10, budget, AT)?, Some(1));
    assert_eq!(map.get(&40, budget, AT)?, None);
    Ok(())
}

/// ´test:crate:ordered-scratch-numeric-work-boundary´
#[test]
fn numeric_work_boundary() -> TestResult {
    // Inserts 1+3+4, lookup 1, replacement 3, removal 5, absent lookup 1.
    for prior in [0, 37] {
        for short in [0, 1] {
            let mut budget = Budget::new(Limits {
                work: prior + 18 - short,
                ..Limits::default()
            })?;
            budget.work(prior, AT)?;
            let result = numeric(&mut budget);
            if short == 0 {
                result?;
                assert_eq!(budget.usage().work, prior + 18);
            } else {
                assert!(
                    matches!(result, Err(Fault { kind: FaultKind::Exhausted { budget: BudgetKind::Work, needed, .. }, .. }) if needed == prior + 18)
                );
            }
        }
    }
    Ok(())
}

/// ´test:crate:ordered-scratch-string-refusal-preserves-entries´
#[test]
fn string_refusal_preserves_entries() -> TestResult {
    let mut budget = Budget::new(Limits {
        work: 8,
        ..Limits::default()
    })?;
    let mut map = Map::new();
    map.reserve(2, &mut budget, AT)?;
    map.insert("aaaa1", 1, &mut budget, AT)?;
    // One six-unit prefix comparison leaves insufficient Work for two moved entries.
    let failure = map.insert("aaaa0", 0, &mut budget, AT);
    assert!(matches!(
        failure,
        Err(Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::Work,
                needed: 9,
                ..
            },
            ..
        })
    ));
    assert_eq!(budget.usage().work, 7);
    assert_eq!(map.iter().copied().collect::<Vec<_>>(), [("aaaa1", 1)]);
    let mut exact = Budget::new(Limits {
        work: 8,
        ..Limits::default()
    })?;
    map.insert("aaaa0", 0, &mut exact, AT)?;
    assert_eq!(exact.usage().work, 8);
    assert_eq!(map.keys().copied().collect::<Vec<_>>(), ["aaaa0", "aaaa1"]);
    Ok(())
}

/// ´test:crate:ordered-scratch-explicit-growth-work-boundary´
#[test]
fn explicit_growth_work_boundary() -> TestResult {
    // Ascending inserts pay 1+2+2+3+7: six comparisons, five entries, four relocations.
    for physical_spare in [0, 16] {
        for short in [0, 1] {
            let mut map = Map::new();
            map.entries.try_reserve_exact(physical_spare)?;
            let mut budget = Budget::new(Limits {
                work: 15 - short,
                ..Limits::default()
            })?;
            for key in 1_u64..=4 {
                map.insert(key, (), &mut budget, AT)?;
            }
            let result = map.insert(5, (), &mut budget, AT);
            if short == 0 {
                result?;
                assert_eq!(budget.usage().work, 15);
                assert_eq!(map.keys().copied().collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
            } else {
                assert!(matches!(
                    result,
                    Err(Fault {
                        kind: FaultKind::Exhausted {
                            budget: BudgetKind::Work,
                            needed: 15,
                            ..
                        },
                        ..
                    })
                ));
                assert_eq!(budget.usage().work, 11);
                assert_eq!(map.keys().copied().collect::<Vec<_>>(), [1, 2, 3, 4]);
            }
        }
    }
    Ok(())
}

/// ´test:crate:ordered-scratch-large-reservation-work-boundary´
#[test]
fn large_reservation_work_boundary() -> TestResult {
    for short in [0, 1] {
        let mut map = Map::new();
        let mut setup = Budget::new(Limits::default())?;
        map.insert(1_u64, (), &mut setup, AT)?;
        map.insert(2, (), &mut setup, AT)?;
        let mut budget = Budget::new(Limits {
            work: 2 - short,
            ..Limits::default()
        })?;
        // Required count 12 exceeds both twice the old target (8) and the floor (4).
        let result = map.reserve(10, &mut budget, AT);
        if short == 0 {
            result?;
            assert_eq!(map.reserved, 12);
            assert_eq!(budget.usage().work, 2);
        } else {
            assert!(matches!(
                result,
                Err(Fault {
                    kind: FaultKind::Exhausted {
                        budget: BudgetKind::Work,
                        needed: 2,
                        ..
                    },
                    ..
                })
            ));
            assert_eq!(map.reserved, 4);
            assert_eq!(budget.usage().work, 0);
        }
        assert_eq!(map.keys().copied().collect::<Vec<_>>(), [1, 2]);
    }
    Ok(())
}

/// ´test:crate:ordered-scratch-second-doubling-work-boundary´
#[test]
fn second_doubling_work_boundary() -> TestResult {
    for physical_spare in [0, 32] {
        for short in [0, 1] {
            let mut map = Map::new();
            map.entries.try_reserve_exact(physical_spare)?;
            let mut budget = Budget::new(Limits {
                work: 37 - short,
                ..Limits::default()
            })?;
            // First five cost 15; sixth through eighth cost 3+3+4.
            for key in 1_u64..=8 {
                map.insert(key, (), &mut budget, AT)?;
            }
            assert_eq!(map.reserved, 8);
            assert_eq!(budget.usage().work, 25);
            // Ninth costs three comparisons, one incoming entry and eight relocations.
            let result = map.insert(9, (), &mut budget, AT);
            if short == 0 {
                result?;
                assert_eq!(map.reserved, 16);
                assert_eq!(budget.usage().work, 37);
                assert_eq!(
                    map.keys().copied().collect::<Vec<_>>(),
                    (1..=9).collect::<Vec<_>>()
                );
            } else {
                assert!(matches!(
                    result,
                    Err(Fault {
                        kind: FaultKind::Exhausted {
                            budget: BudgetKind::Work,
                            needed: 37,
                            ..
                        },
                        ..
                    })
                ));
                assert_eq!(map.reserved, 8);
                assert_eq!(budget.usage().work, 29);
                assert_eq!(
                    map.keys().copied().collect::<Vec<_>>(),
                    (1..=8).collect::<Vec<_>>()
                );
            }
        }
    }
    Ok(())
}

/// ´test:crate:ordered-scratch-full-target-byte-boundary´
#[test]
fn full_target_byte_boundary() -> TestResult {
    // Old live-count bytes were 8, 40, 72 and 96; full target bytes are 32, 64, 128 and 96.
    for (live, additional, target) in [(0_u64, 1, 4), (4, 1, 8), (8, 1, 16), (2, 10, 12)] {
        let bytes = u64::try_from(target * std::mem::size_of::<(u64, ())>())?;
        for short in [0, 1] {
            let mut map = Map::new();
            let mut setup = Budget::new(Limits::default())?;
            for key in 1..=live {
                map.insert(key, (), &mut setup, AT)?;
            }
            let previous_target = map.reserved;
            let mut budget = Budget::new(Limits {
                staged_payload_bytes: bytes - short,
                ..Limits::default()
            })?;
            let result = map.reserve(additional, &mut budget, AT);
            if short == 0 {
                result?;
                assert_eq!(map.reserved, target);
                assert_eq!(budget.usage().work, live);
                // A subsequent fitting request still checks the complete retained target.
                let mut lower = Budget::new(Limits {
                    staged_payload_bytes: bytes - 1,
                    ..Limits::default()
                })?;
                assert!(matches!(map.reserve(0, &mut lower, AT), Err(Fault {
                    kind: FaultKind::Exhausted { budget: BudgetKind::StagedPayloadBytes, needed, .. }, ..
                }) if needed == bytes));
                assert_eq!(lower.usage().work, 0);
            } else {
                assert!(matches!(result, Err(Fault {
                    kind: FaultKind::Exhausted { budget: BudgetKind::StagedPayloadBytes, needed, .. }, ..
                }) if needed == bytes));
                assert_eq!(map.reserved, previous_target);
                assert_eq!(budget.usage().work, 0);
            }
            assert_eq!(
                map.keys().copied().collect::<Vec<_>>(),
                (1..=live).collect::<Vec<_>>()
            );
        }
    }
    Ok(())
}

/// ´test:crate:ordered-scratch-allocation-refusal-retry-charges-relocation´
#[test]
fn allocation_refusal_retry_charges_relocation() -> TestResult {
    let mut map = Map::new();
    let mut setup = Budget::new(Limits::default())?;
    for key in 1_u64..=4 {
        map.insert(key, (), &mut setup, AT)?;
    }
    let capacity = map.entries.capacity();
    // Vec rejects byte sizes above isize::MAX before attempting a physical allocation.
    // The product still fits u64 on the supported platform, so the byte check succeeds.
    let count = (usize::MAX / 2) / std::mem::size_of::<(u64, ())>() + 1;
    let mut budget = Budget::new(Limits {
        staged_payload_bytes: u64::MAX,
        work: 12,
        ..Limits::default()
    })?;
    for attempt in 1..=2 {
        assert!(matches!(
            map.reserve(count - map.len(), &mut budget, AT),
            Err(Fault {
                kind: FaultKind::Exhausted {
                    reason: Exhaustion::Allocation,
                    ..
                },
                ..
            })
        ));
        assert_eq!(budget.usage().work, 4 * attempt);
        assert_eq!(map.reserved, 4);
        assert_eq!(map.entries.capacity(), capacity);
        assert_eq!(map.keys().copied().collect::<Vec<_>>(), [1, 2, 3, 4]);
    }
    // Neither failed attempt advances the target or refunds relocation; retry pays four again.
    map.reserve(1, &mut budget, AT)?;
    assert_eq!(map.reserved, 8);
    assert_eq!(budget.usage().work, 12);
    assert_eq!(map.keys().copied().collect::<Vec<_>>(), [1, 2, 3, 4]);
    Ok(())
}
