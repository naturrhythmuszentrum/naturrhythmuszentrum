// SPDX-License-Identifier: AGPL-3.0-only
//! Literal visit inventories check exact neighbours on the same previously spent ledger.
use super::super::{
    admission::Refusal,
    exact::{Report, Scope, Source},
    gate::{self, Gate},
    owner::{Capacities, Owner},
    snapshot::Snapshot,
};
use super::{
    fixture::{self, TestResult},
    pending,
};
use crate::loan::{
    charge,
    graph::walk::Error,
    lc_shape::{
        output::bridge::retained_overlap::Observation,
        tests::output::{Setup, setup},
    },
};
use crate::{ExecutionAccount, Heap, HeapLimits, LoanError};
use nrz_bindings::{Budget, LcOmegaCorrespondence, Limits};
use std::rc::Rc;

/// One original correspondence is observed through one or two retaining positions, without cloning it.
#[derive(Debug)]
struct Law<'a>(&'a Rc<LcOmegaCorrespondence>, usize);
impl Source for Law<'_> {
    fn slots(&self) -> Result<usize, LoanError> {
        Ok(4 * self.1)
    }
    fn observation(&self, position: usize) -> Result<Option<Observation<'_>>, Error> {
        Ok(match position % 4 {
            0 => Some(Observation::captured(self.0).map_err(|error| {
                Error::Adapter(super::super::native::Failure::Preparation(error.into()))
            })?),
            1 => Some(Observation::correspondence(self.0)),
            2 => Some(Observation::native(self.0)),
            _ => None,
        })
    }
}

/// H1 + four(A1+N1) + three comparisons(visit2+A1) =18.
/// Two copies: H1 + eight visits2 + comparisons3+6+4+6+9 =45;
/// the repeated captured comparison includes its existing owning predicate's one Work visit.
pub(super) fn exact() -> TestResult {
    for (copies, needed) in [(1, 18), (2, 45)] {
        for short in [false, true] {
            let Setup {
                mut heap,
                mut account,
                inventory,
                axes,
                registry,
                radian,
            } = setup::<f64>(Limits::default(), [2, 3, 2])?;
            let (mut owner, _) = fixture::admit(
                &inventory,
                &axes,
                &registry,
                &radian,
                &mut heap,
                &mut account,
            )?;
            for position in 0..5 {
                fixture::publish_input(&mut owner, position, &mut heap, &mut account)?;
            }
            pending::prepared(&mut owner, &mut heap, &mut account)?;
            let correspondence = owner.backing.nodes[10]
                .native
                .as_ref()
                .and_then(super::super::native::Native::retained_correspondence)
                .ok_or("prepared law")?;
            let source = Law(correspondence, copies);
            let empty = Heap::new(HeapLimits::default())?;
            remaining(&mut account, needed - u64::from(short))?;
            let before = account.usage().work;
            let result = Scope {
                current: Some(&source),
                prior: &[],
                heap: &empty,
            }
            .inspect(&mut account);
            assert_eq!(account.usage().work - before, needed - u64::from(short));
            if short {
                assert!(matches!(result, Err(Error::Native(LoanError::Budget(_)))));
            } else {
                assert_eq!(
                    result?,
                    Report {
                        logical: 416
                            + crate::loan::lc_shape::tests::output::bridge::oracle::retained(false),
                        observed: 16,
                        owners: 3
                    }
                );
            }
        }
    }
    Ok(())
}

/// Gate H2 + M5 + P2 =9; an empty native heap adds no inventory visits.
pub(super) fn gate() -> TestResult {
    for short in [false, true] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let mut account = ExecutionAccount::new(Budget::new(Limits::default())?)?;
        let gate = Rc::new(Gate::new(&account));
        remaining(&mut account, 9 - u64::from(short))?;
        let before = account.usage().work;
        let result =
            gate::run(&gate, (0, 0), &mut heap, &mut account, |_, _| ()).map_err(Error::from);
        gate.observe(&result);
        assert_eq!(account.usage().work - before, if short { 0 } else { 9 });
        assert_eq!(gate.stopped(), short);
        assert!(heap.graph_gate.is_none());
        if short {
            assert!(matches!(result, Err(Error::Native(LoanError::Budget(_)))));
        } else {
            result?;
        }
    }
    Ok(())
}

/// An empty graph's report pays nativeH1, graphH1+O(H1+H1), deliveryH1 and exact(H1+A1)=7.
/// A duplicate borrow adds graphA1 and exactA1, without another owner or payload.
pub(super) fn snapshot() -> TestResult {
    for (copies, needed, prefix) in [(1, 7, 5), (2, 9, 6)] {
        for short in [false, true] {
            let Setup {
                heap,
                mut account,
                inventory,
                registry,
                ..
            } = setup::<f64>(Limits::default(), [2, 3, 2])?;
            let mut owner = Owner::admit(
                Vec::new(),
                &[],
                Capacities {
                    nodes: 0,
                    edges: 0,
                    scopes: 0,
                    drive: 100,
                    records: 0,
                    words: 0,
                    witnesses: 0,
                },
                &inventory,
                &registry,
                &[],
                &heap,
                &mut account,
            )
            .map_err(|Refusal { error, .. }| error)?;
            remaining(&mut account, needed - u64::from(short))?;
            let before = account.usage().work;
            let borrows = [owner.borrowed(), owner.borrowed()];
            let result = Snapshot::inspect(&borrows[..copies], &heap, &mut account);
            assert_eq!(
                account.usage().work - before,
                if short { prefix } else { needed }
            );
            if short {
                assert!(matches!(result, Err(Error::Native(LoanError::Budget(_)))));
                assert!(matches!(
                    owner.begin_capacity_epoch(&mut account),
                    Err(Error::WorkStopped)
                ));
                assert_eq!(account.usage().work - before, prefix);
            } else {
                let report = result?;
                assert_eq!(
                    (
                        report.graph_bytes,
                        report.observer_bytes,
                        report.delivery_history_bytes
                    ),
                    (376 + 144, 72 + 16, 144 + 24)
                );
                assert_eq!(report.graph_owners, 1);
                assert_eq!(report.exact_children, Report::default());
            }
        }
    }
    Ok(())
}

/// Explicit earlier work is consumed before the exact neighbour; no limit or ledger is replaced.
fn remaining(account: &mut ExecutionAccount, amount: u64) -> TestResult {
    charge(&mut account.ledger, 13)?;
    let burn = account.limits().work - account.usage().work - amount;
    account.ledger.work(burn, crate::loan::AT)?;
    Ok(())
}
