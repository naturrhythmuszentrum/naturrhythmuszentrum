// SPDX-License-Identifier: AGPL-3.0-only
//! Admission controls enumerate actual independent allocation owners and preserve original requests.

use super::super::{
    admission::Refusal,
    node::{Node, State},
    owner::{Capacities, Owner},
};
use super::fixture::TestResult;
use crate::LoanError;
use crate::loan::graph::{sink, walk::Error};
use crate::loan::lc_shape::tests::output::{Setup, setup};
use nrz_bindings::Limits;

/// One input reserves an outcome, a probe and exhaustion: 6 + 8 + 5 codec words.
const CAPACITY: Capacities = Capacities {
    nodes: 1,
    edges: 0,
    scopes: 0,
    drive: 20,
    records: 3,
    words: 19,
    witnesses: 0,
};

/// Adapter slots/boundaries/kinds; five O allocations; destinations; four K allocations; gate/header.
/// The enclosing adapter guard owns only slots, boundaries, kinds, destinations, gate and header.
pub(super) fn prefixes() -> TestResult {
    for (failed, acquired) in [0, 1, 2, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 5]
        .into_iter()
        .enumerate()
    {
        let Setup {
            heap,
            mut account,
            inventory,
            axes,
            registry,
            ..
        } = setup::<f64>(Limits::default(), [2, 3, 2])?;
        let declaration = inventory
            .input(0)
            .ok_or("original source")?
            .source(&axes, &mut account)?;
        let nodes = vec![Node::input(declaration)];
        let original = nodes.as_ptr();
        let before = account.usage().work;
        let injection = crate::loan::fault::Injection::after(failed);
        let refused = Owner::admit(
            nodes,
            &[1],
            CAPACITY,
            &inventory,
            &registry,
            &[],
            &heap,
            &mut account,
        );
        assert_eq!(crate::loan::fault::Injection::attempts(), failed + 1);
        assert_eq!(
            crate::loan::fault::Injection::prefix(),
            [acquired, 0, acquired]
        );
        drop(injection);
        let Err(Refusal { nodes, error }) = refused else {
            return Err("injected admission refusal".into());
        };
        assert!(matches!(
            error,
            Error::Native(LoanError::AllocationRefused)
                | Error::Delivery(sink::Error::Native(LoanError::AllocationRefused))
        ));
        assert_eq!(nodes.as_ptr(), original);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].state, State::Pending);
        assert!(account.usage().work > before);
        // The account registration's 24-byte shared cell survives its creation; no sink debit does.
        assert_eq!(
            sink::live_bytes(&account)?,
            if failed >= 12 { 24 } else { 0 }
        );
        let spent = account.usage().work;
        let owner = Owner::admit(
            nodes,
            &[1],
            CAPACITY,
            &inventory,
            &registry,
            &[],
            &heap,
            &mut account,
        )
        .map_err(|Refusal { error, .. }| error)?;
        assert_eq!(owner.backing.cleanup.allocations, 4);
        assert!(account.usage().work > spent);
        drop(owner);
        assert_eq!(sink::live_bytes(&account)?, 24);
    }
    Ok(())
}
