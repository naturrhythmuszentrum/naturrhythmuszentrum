// SPDX-License-Identifier: AGPL-3.0-only
//! Real published native intersections select full, empty and unsupported partial composition.

use super::super::{
    drive::Event,
    input::{InputRequest, InputReturn},
    node::State,
    owner::Owner,
};
use super::fixture::{self, TestResult};
use crate::loan::lc_shape::tests::output::{Setup, setup};
use crate::{ExecutionAccount, Heap};
use nrz_bindings::Limits;
use nrz_scalar::Scalar;

/// Refusal dominates cancellation; otherwise all five required positions determine coverage.
pub(super) fn run<P: Scalar + 'static>() -> TestResult {
    for (instances, empty_demand, mixed, expected) in [
        (1, false, false, State::UnsupportedPartial),
        (0, false, false, State::MissingInput),
        (2, true, false, State::Published),
        (2, false, true, State::DependencyRefused(5)),
    ] {
        let Setup {
            mut heap,
            mut account,
            inventory,
            axes,
            registry,
            radian,
        } = setup::<P>(Limits::default(), [2, 3, 2])?;
        let selected = if empty_demand {
            Vec::new()
        } else {
            (0..12).collect()
        };
        let mut attempts = [8; 12];
        if mixed {
            attempts[0] = 0;
        }
        let (mut owner, _) = fixture::admit_selected(
            &inventory,
            &axes,
            &registry,
            &radian,
            &mut heap,
            &mut account,
            (attempts, &selected, None),
        )?;
        issue(&mut owner, instances, mixed, &mut heap, &mut account)?;
        drain(&mut owner, expected, empty_demand, &mut heap, &mut account)?;
    }
    Ok(())
}

/// Only the last original position is partial; it is not a chosen arithmetic operand.
fn issue<P: Scalar + 'static>(
    owner: &mut Owner<'_, P, 5>,
    instances: usize,
    mixed: bool,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> TestResult {
    for position in 0..5 {
        let handle = owner.handle(position, account)?;
        if mixed && position == 1 {
            let Event::Terminal {
                evidence, control, ..
            } = owner.cancel_node(handle, heap, account)?
            else {
                return Err("genuine cancelled expectation".into());
            };
            control?;
            owner.acknowledge_outcome(handle, evidence, heap, account)?;
            continue;
        }
        match owner.request_input(handle, heap, account, &[])? {
            InputRequest::Granted {
                mut loan,
                evidence: Ok(evidence),
                control,
            } => {
                control?;
                owner.acknowledge_probe(evidence, account)?;
                fixture::fill_prefix(
                    &mut loan,
                    position,
                    if position == 4 { instances } else { 2 },
                    account,
                )?;
                let InputReturn::Published { control } =
                    owner.return_input(handle, loan, heap, account)
                else {
                    return Err("original partial handback".into());
                };
                control?;
            }
            InputRequest::AttemptsExhausted {
                evidence, control, ..
            } if mixed && position == 0 => {
                control?;
                owner.acknowledge_probe(evidence, account)?;
            }
            other => return Err(format!("host input: {other:?}").into()),
        }
    }
    Ok(())
}

/// Empty requested coverage must reach sealed native publication without fake input values.
pub(super) fn drain<P: Scalar + 'static>(
    owner: &mut Owner<'_, P, 5>,
    expected: State,
    empty_demand: bool,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> TestResult {
    let output = owner.handle(11, account)?;
    let mut delivered = false;
    let mut omega = false;
    for _ in 0..256 {
        match owner.drive_one(heap, account, &[])? {
            Event::Advanced { probe, control, .. } => {
                control?;
                owner.acknowledge_probe(probe, account)?;
            }
            Event::Terminal {
                node,
                evidence,
                control,
            } => {
                control?;
                if node.slot == 10 {
                    assert_eq!(owner.backing.nodes[10].state, expected);
                    omega = true;
                }
                if node == output {
                    delivered = true;
                    if empty_demand {
                        let reader = owner
                            .result(output, heap, account)?
                            .current_reader(heap, account)?;
                        for cell in 0..12 {
                            assert_eq!(
                                reader.outcome(cell, account)?.kind,
                                crate::LcOutcomeKind::NotRequested
                            );
                        }
                    } else {
                        assert!(owner.result(output, heap, account).is_err());
                        assert_eq!(owner.backing.core.reservation.backing.nodes[10].attempts, 8);
                    }
                }
                owner.acknowledge_outcome(node, evidence, heap, account)?;
            }
            Event::Refused {
                node,
                error,
                probe,
                control,
            } if expected == State::DependencyRefused(5) => {
                assert!((5..10).contains(&node.slot));
                assert_eq!(
                    super::super::refusal::classify(&error, None),
                    super::super::refusal::Class::Terminal
                );
                control?;
                owner.acknowledge_probe(probe.ok_or("zero source probe")?, account)?;
            }
            Event::Control => {}
            Event::Quiescent => break,
            other => return Err(format!("coverage progression: {other:?}").into()),
        }
    }
    assert!(omega);
    assert!(delivered);
    Ok(())
}

/// Zero-capacity host publication supplies no fabricated scalar; mapped admission retains its refusal.
pub(super) fn zero_source<P: Scalar + 'static>() -> TestResult {
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        radian,
    } = setup::<P>(Limits::default(), [0, 3, 2])?;
    let (mut owner, _) = fixture::admit_selected(
        &inventory,
        &axes,
        &registry,
        &radian,
        &mut heap,
        &mut account,
        ([8; 12], &[], None),
    )?;
    for position in 0..5 {
        let handle = owner.handle(position, &mut account)?;
        let InputRequest::Granted {
            loan,
            evidence: Ok(evidence),
            control,
        } = owner.request_input(handle, &mut heap, &mut account, &[])?
        else {
            return Err("zero-capacity host admission".into());
        };
        control?;
        owner.acknowledge_probe(evidence, &mut account)?;
        let InputReturn::Published { control } =
            owner.return_input(handle, loan, &mut heap, &mut account)
        else {
            return Err("zero-capacity host publication".into());
        };
        control?;
    }
    crate::loan::fault::reset_reads();
    drain(
        &mut owner,
        State::DependencyRefused(5),
        false,
        &mut heap,
        &mut account,
    )?;
    assert_eq!(crate::loan::fault::reads(), 0);
    Ok(())
}
