// SPDX-License-Identifier: AGPL-3.0-only
//! Root ownership determines which no-loan expectations close; outside authority stays independent.

use super::super::{
    drive::Event,
    input::{InputRequest, InputReturn},
    node::State,
    owner::Owner,
    residence::Completion,
    scope::Progress,
};
use super::{
    coverage,
    fixture::{self, TestResult},
};
use crate::loan::lc_shape::tests::output::{Setup, setup};
use nrz_scalar::Scalar;

/// One unowned original remains pending even though every owned expectation has closed without a loan.
pub(super) fn run<P: Scalar + 'static>(external: bool) -> TestResult {
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        radian,
    } = setup::<P>(nrz_bindings::Limits::default(), [2, 3, 2])?;
    let (mut owner, demands) = fixture::admit(
        &inventory,
        &axes,
        &registry,
        &radian,
        &mut heap,
        &mut account,
    )?;
    let handles = (0..12)
        .map(|slot| owner.handle(slot, &mut account))
        .collect::<Result<Vec<_>, _>>()?;
    let scope = owner.begin_scope(
        &[handles[11]],
        &handles[usize::from(external)..],
        &mut account,
    )?;
    let mut closed = false;
    for _ in 0..128 {
        if owner.close_scope(scope, &mut account)? == Progress::Published {
            closed = true;
            break;
        }
    }
    assert!(closed);
    let mut notified = false;
    for _ in 0..13 {
        if owner.notify_one(scope, &heap, &mut account)? == Progress::Published {
            notified = true;
            break;
        }
    }
    assert!(notified);
    for node in &owner.backing.nodes[usize::from(external)..5] {
        assert_eq!(node.state, State::Absent);
        assert_eq!(node.loan, None);
    }
    if external {
        assert_eq!(owner.backing.nodes[0].state, State::Pending);
        pending(&mut owner, &mut heap, &mut account)?;
        let InputRequest::Granted {
            mut loan,
            evidence: Ok(evidence),
            control,
        } = owner.request_input(handles[0], &mut heap, &mut account, &[])?
        else {
            return Err("outside expectation issuance".into());
        };
        control?;
        owner.acknowledge_probe(evidence, &mut account)?;
        fixture::fill(&mut loan, 0, &mut account)?;
        let InputReturn::Published { control } =
            owner.return_input(handles[0], loan, &mut heap, &mut account)
        else {
            return Err("outside native handback".into());
        };
        control?;
    }
    coverage::drain(
        &mut owner,
        State::MissingInput,
        false,
        &mut heap,
        &mut account,
    )?;
    for demand in demands {
        heap.retire_mask(demand, &mut account.ledger)?;
        heap.residence(&mut account, 100_000)?;
        heap.retire_cell_layout(demand.layout(), &mut account.ledger)?;
    }
    let mut complete = false;
    for _ in 0..8 {
        match Owner::completion(&[&owner], &mut heap, &mut account, 100_000)? {
            Completion::Complete { .. } => {
                complete = true;
                break;
            }
            Completion::NativePending(..) => {}
            Completion::GraphPending(_) => return Err("scoped graph obligation remains".into()),
        }
    }
    assert!(complete);
    Ok(())
}

/// A bounded quiescent prefix does not manufacture absence for its unowned original input.
fn pending<P: Scalar + 'static>(
    owner: &mut Owner<'_, P, 5>,
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
) -> TestResult {
    let mut quiet = false;
    for _ in 0..128 {
        match owner.drive_one(heap, account, &[])? {
            Event::Terminal {
                node,
                evidence,
                control,
            } => {
                assert!(node.slot < 10);
                control?;
                owner.acknowledge_outcome(node, evidence, heap, account)?;
            }
            Event::Control => {}
            Event::Quiescent => {
                quiet = true;
                break;
            }
            other => return Err(format!("outside expectation prefix: {other:?}").into()),
        }
    }
    assert!(quiet);
    assert_eq!(owner.backing.nodes[0].state, State::Pending);
    assert!(matches!(
        Owner::completion(&[owner], heap, account, 100_000)?,
        Completion::GraphPending(_)
    ));
    Ok(())
}
