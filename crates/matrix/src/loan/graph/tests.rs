// SPDX-License-Identifier: AGPL-3.0-only
//! Independent layout mirrors and literal scoped histories do not call the cost planner.

mod boundaries;
mod layout;
mod native;
use super::inventory::Row;
use super::{Arguments, Inventory, Reservation};
use crate::ExecutionAccount;
use nrz_bindings::{Budget, Limits};
use nrz_scalar::Scalar;

/// Typed fixture failures retain their original diagnostic causes.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
/// Independent two-node/one-edge storage history: 648 header/node + 88 incidence + 96 scratch.
const BYTES: usize = 832;
/// Literal-event oracle: H=2 (operation entry, initial control cursors);
/// N=6 (two proposal inspections, two slot initializations, two attempt writes), E=1;
/// I=13 (ready 1, marks 2, roots/closure 2, stack 4, scope 4); M=X=A=T=0;
/// P=9 (eight distinct allocations and one fixed owner cleanup record). Total: 31.
const WORK: u64 = 31;
/// Caller arguments deliberately contain many more attempts than the Work boundary.
fn args() -> Arguments<'static> {
    Arguments {
        nodes: 2,
        edges: 1,
        proposed_edges: 1,
        scopes: 1,
        drive: 1_000,
        attempts: &[100, 200],
    }
}
/// Start one immutable original ledger, with explicit prior consumption.
fn account(limits: Limits) -> TestResult<ExecutionAccount> {
    let mut budget = Budget::new(limits)?;
    budget.work(5, crate::loan::AT)?;
    Ok(ExecutionAccount::new(budget)?)
}

/// Actual capacities, aliases and external retaining readers keep disjoint projections.
/// ´test:unit:execution-graph-owner-overlap´
#[test]
fn execution_graph_owner_overlap() -> TestResult {
    layout::check();
    overlap::<f32>()?;
    overlap::<f64>()?;
    native::outside::<f32>()?;
    native::outside::<f64>()
}
/// Independently enumerate one owner, two distinct owners and one surviving outside reader.
fn overlap<P: Scalar + 'static>() -> TestResult {
    let mut account = account(Limits::default())?;
    let mut old = Reservation::reserve::<P>(args(), &[], &mut account)?;
    assert!(std::rc::Rc::get_mut(&mut old.backing).is_some());
    let before = account.usage().work;
    let first = Inventory::inspect(&[old.borrowed()], &mut account)?;
    assert_eq!(
        first.rows,
        [
            Row::Covered(648),
            Row::Covered(88),
            Row::Covered(96),
            Row::OutsideScope,
            Row::OutsideScope,
            Row::Deferred,
            Row::Deferred
        ]
    );
    assert_eq!(first.covered_bytes, BYTES);
    // H scope entry + H distinct backing inspection; no account-authentication unit.
    assert_eq!(account.usage().work - before, 2);
    assert_eq!((first.owners, first.references), (1, 1));
    assert_eq!(old.backing.nodes.capacity(), 2);
    assert_eq!(old.backing.edges.capacity(), 1);
    assert_eq!(old.backing.ready.capacity(), 1);
    assert_eq!(old.backing.stack.capacity(), 2);
    assert_eq!(old.backing.marks.capacity(), 2);
    assert_eq!(old.backing.scopes.capacity(), 1);
    assert_eq!(old.backing.membership.capacity(), 2);
    assert_eq!(old.backing.drive, 1_000);
    assert_eq!(old.backing.control.ready, 0);
    assert_eq!(old.backing.cleanup.allocations, 8);
    assert_eq!(old.backing.cleanup.records, 1);
    assert_eq!(old.backing.cleanup.edges, 0);
    assert_eq!(old.backing.format, crate::storage::Format::of::<P>());
    let before = account.usage().work;
    let reader = old.reader(&mut account)?;
    assert_eq!(account.usage().work - before, 2); // H entry + P retaining edge.
    assert!(std::rc::Rc::get_mut(&mut old.backing).is_none());
    let before = account.usage().work;
    let shared = Inventory::inspect(
        &[old.borrowed(), reader.borrowed(), old.borrowed()],
        &mut account,
    )?;
    assert_eq!(
        (shared.owners, shared.references, shared.covered_bytes),
        (1, 3, BYTES)
    );
    // H scope + H distinct backing + A for each of two actual pointer comparisons.
    assert_eq!(account.usage().work - before, 4);
    let new =
        Reservation::reserve::<P>(args(), &[old.borrowed(), reader.borrowed()], &mut account)?;
    let simultaneous = Inventory::inspect(
        &[old.borrowed(), new.borrowed(), reader.borrowed()],
        &mut account,
    )?;
    assert_eq!(
        (simultaneous.owners, simultaneous.covered_bytes),
        (2, 1_664)
    );
    let weak = std::rc::Rc::downgrade(&old.backing);
    drop(old);
    assert!(weak.upgrade().is_some());
    let held = Inventory::inspect(&[reader.borrowed(), new.borrowed()], &mut account)?;
    assert_eq!(held.covered_bytes, 1_664);
    drop(reader);
    assert!(weak.upgrade().is_none());
    assert_eq!(
        Inventory::inspect(&[new.borrowed()], &mut account)?.covered_bytes,
        BYTES
    );
    drop(new);
    assert_eq!(Inventory::inspect(&[], &mut account)?.owners, 0);
    Ok(())
}

/// Exact/one-below boundaries use the actual unchanged account and preserve rightful handles.
/// ´test:unit:execution-graph-resource-boundaries´
#[test]
fn execution_graph_resource_boundaries() -> TestResult {
    boundaries::run::<f32>()?;
    boundaries::run::<f64>()
}
