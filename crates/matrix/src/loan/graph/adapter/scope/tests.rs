// SPDX-License-Identifier: AGPL-3.0-only
//! Isolated scope counters reach the actual admission transaction without global seeds.

use super::{Boundary, Progress, begin, step};
use crate::loan::graph::{
    Arguments, Reservation,
    walk::{Admission, Connection, Core, Error, Kind},
};
use crate::{ExecutionAccount, LoanError};
use nrz_bindings::{Budget, Limits};

/// Preserve diagnostic causes without printing from tests.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Three native positions share fixed topology and one admitted scope descriptor.
fn owner(account: &mut ExecutionAccount) -> TestResult<Core> {
    let reservation = Reservation::reserve::<f64>(
        Arguments {
            nodes: 3,
            edges: 2,
            proposed_edges: 2,
            scopes: 1,
            drive: 100,
            attempts: &[1, 1, 1],
        },
        &[],
        account,
    )?;
    let edges = [
        Connection {
            producer: 0,
            consumer: 2,
            demand: 0,
            semantic: false,
        },
        Connection {
            producer: 1,
            consumer: 2,
            demand: 1,
            semantic: false,
        },
    ];
    match Core::admit(reservation, &[Kind::Fixture; 3], &edges, account) {
        Admission::Admitted(owner) => Ok(owner),
        other => Err(format!("scope owner admission: {other:?}").into()),
    }
}

/// The maximal allocated generation remains usable; the following admission changes no gate.
/// ´test:unit:lc-omega-graph-scope-generation´
#[test]
fn lc_omega_graph_scope_generation() -> TestResult {
    for generation in [u64::MAX - 1, u64::MAX] {
        let mut account = ExecutionAccount::new(Budget::new(Limits::default())?)?;
        let mut core = owner(&mut account)?;
        let root = core.handle(2, &mut account)?;
        let source = core.handle(0, &mut account)?;
        let backing =
            std::rc::Rc::get_mut(&mut core.reservation.backing).ok_or(LoanError::WrongLoan)?;
        backing.control.epochs[2] = generation;
        let mut boundaries = [Boundary::default(); 3];
        let flags = core.reservation.backing.control.flags;
        let drive = core.reservation.backing.drive;
        let epochs = core.reservation.backing.control.epochs;
        let links: Vec<_> = core
            .reservation
            .backing
            .edges
            .iter()
            .map(|edge| edge.links)
            .collect();
        let result = begin(
            &mut core,
            &mut boundaries,
            &[root],
            &[source, root],
            &mut account,
        );
        if generation == u64::MAX {
            assert!(matches!(
                result,
                Err(Error::Native(LoanError::CountOverflow))
            ));
            assert_eq!(core.reservation.backing.control.flags, flags);
            assert_eq!(core.reservation.backing.drive, drive);
            assert_eq!(core.reservation.backing.control.epochs, epochs);
            assert_eq!(
                core.reservation
                    .backing
                    .edges
                    .iter()
                    .map(|edge| edge.links)
                    .collect::<Vec<_>>(),
                links
            );
            assert_eq!(core.reservation.backing.marks, [0, 0, 0]);
            assert_eq!(core.reservation.backing.membership, [0, 0]);
            assert_eq!(boundaries.map(|b| b.generation), [0, 0, 0]);
        } else {
            let scope = result?;
            assert_eq!(scope.generation, u64::MAX);
            let mut published = false;
            // Three root inspections, two dependency edges and fixed push/pop completion.
            for _ in 0..16 {
                if step(&mut core, &boundaries, scope, &mut account)? == Progress::Published {
                    published = true;
                    break;
                }
            }
            assert!(published);
            assert_eq!(core.reservation.backing.membership, [0b100, 0b101]);
            assert_eq!(core.reservation.backing.control.epochs[2], u64::MAX);
            let drive = core.reservation.backing.drive;
            assert_eq!(
                step(&mut core, &boundaries, scope, &mut account)?,
                Progress::Published
            );
            assert_eq!(core.reservation.backing.drive, drive);
        }
    }
    Ok(())
}
