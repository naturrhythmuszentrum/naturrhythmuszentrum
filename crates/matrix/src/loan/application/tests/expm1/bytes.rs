// SPDX-License-Identifier: AGPL-3.0-only
//! Internal common-admission boundaries isolate application bytes from prior law preparation.

use super::{Fixture, TestResult, fixture, ready};
use crate::loan::{abi, fault::Injection};
use nrz_bindings::{
    Budget, Limits,
    readings::algebra::{Exhaustion, FaultKind, Location, limits::BudgetKind},
};

/// Host publication leaves nine rows, of which four are vacant; law owner/use occupy two.
/// New application keeps 23 rows and grows the table by 21, independently of live observations.
fn inventory(cells: usize, components: usize) -> [usize; 2] {
    let length = cells * components;
    let q = cells.div_ceil(64);
    let t = length.div_ceil(64);
    let rounded = 8 * length.div_ceil(8);
    let delta = 3 * (256 + abi::group_row()) + 2 * 32 + 21 * 264 + 40;
    let retained = 5 * (8 * rounded + 8 * t + 8 * q)
        + 8 * q
        + cells * abi::outcome_width()
        + abi::application_headers::<f64>()
        + delta;
    let old = 256 + abi::group_row() + 3 * 32 + 9 * 264 + 40;
    let temporary = 23 * 264 + 5 * 16 + 3 * 248 + 2 * 24;
    assert_eq!(temporary, 6944);
    [retained, retained + 2 * old + temporary]
}
/// Independent complete B/S literals come from the mirror widths and exact-growth enumeration above.
#[test]
fn prepared_application_literal_complete_bytes() {
    assert_eq!(
        [
            inventory(0, 1),
            inventory(1, 1),
            inventory(7, 1),
            inventory(8, 1),
            inventory(9, 1),
            inventory(65, 3)
        ],
        [
            [16_624, 31_168],
            [17_064, 31_608],
            [17_256, 31_800],
            [17_288, 31_832],
            [17_640, 32_184],
            [26_960, 41_504]
        ]
    );
}
/// Complete growth includes empty headers, rounded neighboring capacities and both endpoint maps.
#[test]
fn prepared_application_independent_complete_byte_inventory() -> TestResult {
    for (cells, components) in [(0, 1), (1, 1), (7, 1), (8, 1), (9, 1), (65, 3)] {
        let Fixture {
            mut heap,
            mut account,
            request,
        } = fixture(cells, components, [-2.0, 0.0], 2, false)?;
        let [retained, _] = inventory(cells, components);
        let before = heap.resource_bytes()?;
        let owner = ready(heap.request_pointwise_expm1(request, &mut account))?;
        assert_eq!(heap.resource_bytes()? - before, retained);
        assert_eq!(heap.loans.elements()?, [0, 6 * cells * components]);
        let endpoints = owner.core.endpoints.as_ref().ok_or("endpoints")?;
        for endpoint in endpoints {
            let payload = heap.loans.debits[endpoint.tokens[0].slot]
                .as_ref()
                .ok_or("endpoint debit")?;
            assert_eq!(payload.bytes, 64 * (cells * components).div_ceil(8));
            assert_eq!(payload.elements, cells * components);
        }
        assert!(matches!(
            heap.cancel_application(owner, &mut account),
            crate::ApplicationCancellation::ApplicationCancelled(_)
        ));
    }
    Ok(())
}
/// Scope: the exact internal application plan, with fixed test ceilings established after separate setup.
/// No public setup refusal is advertised as the application boundary and no ceiling is mutated.
#[test]
fn prepared_application_scoped_byte_neighbours() -> TestResult {
    for (cells, components) in [(0, 1), (1, 1), (8, 1), (9, 1), (65, 3)] {
        let Fixture { heap, request, .. } = fixture(cells, components, [-2.0, 0.0], 2, false)?;
        let counts = inventory(cells, components);
        for staged in [false, true] {
            for short in [0, 1] {
                let required = u64::try_from(counts[usize::from(staged)])?;
                let mut limits = super::laws::limits();
                if staged {
                    limits.staged_payload_bytes = required - short;
                } else {
                    limits.output_bytes = required - short;
                }
                let mut budget = Budget::new(limits)?;
                budget.work(7, crate::loan::AT)?;
                let injection = Injection::after(0);
                let result =
                    super::super::super::admission::check_plan(&heap, &request, &mut budget);
                assert_eq!(result.is_ok(), short == 0);
                if short != 0 {
                    let error = result.err().ok_or("byte refusal")?;
                    if staged {
                        let crate::LoanError::Budget(fault) = error else {
                            return Err("staged type".into());
                        };
                        assert_eq!(
                            fault.kind,
                            FaultKind::Exhausted {
                                reason: Exhaustion::Limit,
                                budget: BudgetKind::StagedPayloadBytes,
                                limit: required - 1,
                                needed: required
                            }
                        );
                    } else {
                        assert!(
                            matches!(error, crate::LoanError::Heap(crate::HeapError::ResourceBytes {
                            staged: false, used, limit }) if used == required && limit == required - 1)
                        );
                    }
                }
                assert_eq!(Injection::attempts(), 0);
                drop(injection);
            }
        }
    }
    Ok(())
}
/// Persistent public region/witness/reading memo records and an active caller reservation compose once.
#[test]
fn prepared_application_and_shell_persistent_staged_boundaries() -> TestResult {
    use crate::loan::retained_expm1::tests::memos;
    const PREFIX: u64 = 1_000_000;
    const AT: Location = Location::new("prepared application caller prefix");
    let Fixture { heap, request, .. } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
    let Fixture {
        heap: mut shell_heap,
        mut account,
        request: shell_request,
    } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
    let owner = ready(shell_heap.request_pointwise_expm1(shell_request, &mut account))?;
    let (mut cas, original, reading, _) = memos::seed()?;
    memos::populate(
        &mut cas,
        &original,
        &reading,
        &mut Budget::new(super::laws::limits())?,
    )?;
    let persistent = cas.retained_judgment_bytes() + 64 + memos::memo_bytes()?;
    assert!(persistent > 0);
    for shell in [false, true] {
        let staged = if shell {
            64
        } else {
            u64::try_from(inventory(1, 1)[1])?
        };
        for short in [0, 1] {
            let boundary = PREFIX + persistent + staged;
            let mut budget = Budget::new(Limits {
                staged_payload_bytes: boundary - short,
                ..super::laws::limits()
            })?;
            memos::populate(&mut cas, &original, &reading, &mut budget)?;
            budget.work(7, AT)?;
            let before = budget.usage().work;
            let injection = Injection::after(0);
            let result = budget.with_retained_payload(PREFIX, AT, |budget| {
                if shell {
                    super::super::super::publication::shell_preflight(
                        &shell_heap,
                        &owner.core,
                        budget,
                    )
                } else {
                    super::super::super::admission::check_plan(&heap, &request, budget)
                }
            });
            assert_eq!(result.is_ok(), short == 0);
            if let Err(error) = result {
                let crate::LoanError::Budget(fault) = error else {
                    return Err("ledger-aware refusal".into());
                };
                assert_eq!(
                    fault.kind,
                    FaultKind::Exhausted {
                        reason: Exhaustion::Limit,
                        budget: BudgetKind::StagedPayloadBytes,
                        limit: persistent + staged - 1,
                        needed: persistent + staged
                    }
                );
                assert_eq!(fault.location, crate::loan::AT);
                assert!(fault.series_evidence.is_none());
            }
            assert_eq!(Injection::attempts(), 0);
            assert!(budget.usage().work > before);
            assert_eq!(budget.limits().staged_payload_bytes, boundary - short);
            drop(injection);
        }
    }
    Ok(())
}

/// Pre-existing vacant capacities require only B and temporary staging, with no replacement peak.
#[test]
fn prepared_application_spare_capacity_bytes_and_scoped_neighbours() -> TestResult {
    let mut setup = fixture(1, 1, [-2.0, 0.0], 2, false)?;
    let second = super::shared::additional(&mut setup)?;
    let Fixture {
        mut heap,
        mut account,
        request,
    } = setup;
    let first = ready(heap.request_pointwise_expm1(request, &mut account))?;
    assert!(matches!(
        heap.cancel_application(first, &mut account),
        crate::ApplicationCancellation::ApplicationCancelled(_)
    ));
    assert!(matches!(
        heap.reconcile_detached(&mut account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    assert!(matches!(
        heap.reclaim(&mut account, 100_000, None),
        crate::ScanOutcome::Complete
    ));
    let retained = abi::application_headers::<f64>() + 5 * (64 + 8 + 8) + 8 + abi::outcome_width();
    let staged = retained + 23 * 264 + 5 * 16 + 3 * 248 + 2 * 24;
    assert_eq!([retained, staged], [7552, 14_496]);
    for staging in [false, true] {
        for short in [0, 1] {
            let required = u64::try_from(if staging { staged } else { retained })?;
            let mut limits = super::laws::limits();
            if staging {
                limits.staged_payload_bytes = required - short;
            } else {
                limits.output_bytes = required - short;
            }
            let mut budget = Budget::new(limits)?;
            let injection = Injection::after(0);
            let result = super::super::super::admission::check_plan(&heap, &second, &mut budget);
            assert_eq!(result.is_ok(), short == 0);
            if let Err(error) = result {
                if staging {
                    let crate::LoanError::Budget(fault) = error else {
                        return Err("staged fault".into());
                    };
                    super::super::bytes::staged_fault(&fault, required);
                } else {
                    assert!(
                        matches!(error, crate::LoanError::Heap(crate::HeapError::ResourceBytes { staged: false, used, limit })
                        if used == required && limit == required - 1)
                    );
                }
            }
            assert_eq!(Injection::attempts(), 0);
            drop(injection);
        }
    }
    let before = heap.resource_bytes()?;
    let owner = ready(heap.request_pointwise_expm1(second, &mut account))?;
    assert_eq!(heap.resource_bytes()? - before, retained);
    drop(owner);
    Ok(())
}
