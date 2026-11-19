// SPDX-License-Identifier: AGPL-3.0-only
//! Independent small tariffs exercise ordering, replacement and exhaustion before mutation.

use super::Map;
use crate::budget::{Budget, BudgetKind, Limits};
use crate::error::{Fault, FaultKind, Location};
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
