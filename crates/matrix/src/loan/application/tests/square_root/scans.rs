// SPDX-License-Identifier: AGPL-3.0-only
//! Independently enumerated scan visits include vacancies, weak pins and the final unchanged pass.

use super::{TestResult, candidate, fixture, published, ready};
use crate::{Residence, ScanOutcome};
use nrz_bindings::Limits;

/// Exact-growth tables retain h+3 groups, two pins, h+21 debits, two layouts and three masks.
/// Reconciliation visits every row and removes two expired pins: 2*h+28.
/// Reclaim visits all groups twice, with 2*h+10 source and 17 result removal steps: 4*h+33.
/// Empty residence scans cost 3*h+29, then inventory costs 4*h+78, for 7*h+107.
#[test]
fn square_root_scan_and_residence_work_neighbors() -> TestResult {
    for channels in 1..=3 {
        for phase in 0..3 {
            for short in [false, true] {
                let mut setup = fixture::<f64>(1, 1, channels, 2, false, Limits::default())?;
                let demand = setup.request.demand;
                let owner = ready(
                    setup
                        .heap
                        .request_pointwise_square_root(setup.request, &mut setup.account),
                )?;
                setup.heap.retire_mask(demand, &mut setup.account.ledger)?;
                let owner = candidate(owner, &mut setup.account)?;
                let (tile, _) =
                    published(setup.heap.publish_application(owner, &mut setup.account))?;
                drop(tile);
                assert_eq!(setup.heap.loans.groups.len(), channels + 3);
                assert_eq!(setup.heap.loans.pins.len(), 2);
                assert_eq!(setup.heap.loans.debits.len(), channels + 21);
                if phase > 0 {
                    assert!(matches!(
                        setup
                            .heap
                            .reconcile_detached(&mut setup.account, 1000, None),
                        ScanOutcome::Complete
                    ));
                }
                if phase > 1 {
                    assert!(matches!(
                        setup.heap.reclaim(&mut setup.account, 1000, None),
                        ScanOutcome::Complete
                    ));
                }
                let required = u64::try_from(
                    [2 * channels + 28, 4 * channels + 33, 7 * channels + 107][phase],
                )?;
                let remaining = required - u64::from(short);
                setup.account.ledger.work(
                    setup.account.limits().work - setup.account.usage().work - remaining,
                    crate::loan::AT,
                )?;
                let before = setup.account.usage().work;
                if phase < 2 {
                    let result = if phase == 0 {
                        setup
                            .heap
                            .reconcile_detached(&mut setup.account, 1000, None)
                    } else {
                        setup.heap.reclaim(&mut setup.account, 1000, None)
                    };
                    if short {
                        assert!(
                            matches!(result, ScanOutcome::WorkExhausted(_)),
                            "{result:?}"
                        );
                    } else {
                        assert!(matches!(result, ScanOutcome::Complete), "{result:?}");
                    }
                    assert_eq!(setup.account.usage().work - before, remaining);
                } else {
                    let result = setup.heap.residence(&mut setup.account, 1000)?;
                    if short {
                        assert!(matches!(
                            result,
                            Residence::Pending {
                                elements: None,
                                bytes: None
                            }
                        ));
                        assert_eq!(
                            setup.account.usage().work - before,
                            u64::try_from(3 * channels + 29)?
                        );
                    } else {
                        assert!(
                            matches!(result, Residence::Empty { retained_bytes } if retained_bytes == [11_248, 12_800, 14_352][channels - 1])
                        );
                        assert_eq!(setup.account.usage().work - before, required);
                    }
                    assert_eq!(setup.heap.loans.elements()?, [0; 2]);
                    assert!(setup.heap.loans.debits.iter().all(Option::is_none));
                }
            }
        }
    }
    Ok(())
}
