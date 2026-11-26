// SPDX-License-Identifier: AGPL-3.0-only
//! Public native readers supply sealed values only after original graph publication.

use super::super::{
    drive::Event,
    input::{InputRequest, InputReturn},
    owner::Owner,
    residence::Completion,
};
use super::fixture::{self, TestResult};
use crate::loan::lc_shape::tests::output::{Setup, setup};
use nrz_scalar::Scalar;

/// The twelve-node graph runs on the same account that declared all five original constraints.
pub(super) fn published<P: Scalar + 'static>(replay: bool) -> TestResult {
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        radian,
    } = setup::<P>(nrz_bindings::Limits::default(), [2, 3, 2])?;
    let certificate = if replay {
        Some(crate::loan::lc_shape::tests::output::graph_certificate(
            &inventory,
            &registry,
            &radian,
            &mut account,
        )?)
    } else {
        None
    };
    let (mut owner, demands) = fixture::admit_selected(
        &inventory,
        &axes,
        &registry,
        &radian,
        &mut heap,
        &mut account,
        ([8; 12], &(0..12).collect::<Vec<_>>(), certificate.as_ref()),
    )?;
    let output = owner.handle(11, &mut account)?;
    let omega = owner.handle(10, &mut account)?;
    assert!(matches!(
        Owner::completion(&[&owner, &owner], &mut heap, &mut account, 100_000)?,
        Completion::GraphPending(_)
    ));
    exact(&owner, &heap, &mut account, false)?;
    issue(&mut owner, &mut heap, &mut account)?;
    let outside = drive(&mut owner, output, omega, &mut heap, &mut account)?;
    exact(&owner, &heap, &mut account, true)?;
    assert!(matches!(
        Owner::completion(&[&owner], &mut heap, &mut account, 100_000)?,
        Completion::NativePending(..)
    ));
    {
        let reader = outside.current_reader(&heap, &mut account)?;
        for cell in 0..12 {
            enclosure(&reader, cell, &mut account)?;
        }
    }
    super::renewal::run(
        &owner,
        &inventory,
        &axes,
        &registry,
        &mut heap,
        &mut account,
    )?;
    let before = account.usage();
    outside.release(&mut account).map_err(|(_, error)| error)?;
    assert_eq!(account.usage(), before);
    for demand in demands {
        heap.retire_mask(demand, &mut account.ledger)?;
        heap.residence(&mut account, 100_000)?;
        heap.retire_cell_layout(demand.layout(), &mut account.ledger)?;
    }
    let mut complete = false;
    for _ in 0..8 {
        match Owner::completion(&[&owner], &mut heap, &mut account, 100_000)? {
            Completion::Complete {
                retained_native_bytes,
                ..
            } => {
                assert_ne!(retained_native_bytes, 0);
                complete = true;
                break;
            }
            Completion::NativePending(_, residence) => {
                assert!(matches!(residence, crate::Residence::Pending { .. }));
            }
            Completion::GraphPending(_) => return Err("graph obligations remain".into()),
        }
    }
    assert!(complete);
    exact(&owner, &heap, &mut account, false)?;
    named_pending(
        &owner,
        &inventory,
        &axes,
        &registry,
        &mut heap,
        &mut account,
    )?;
    Ok(())
}

/// Native phase progression preserves a consuming preparation refusal and its original declaration.
pub(super) fn drive<P: Scalar + 'static>(
    owner: &mut Owner<'_, P, 5>,
    output: crate::loan::graph::walk::Handle,
    omega: crate::loan::graph::walk::Handle,
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
) -> TestResult<crate::LcPublishedResult<P>> {
    let mut delivered = None;
    let mut returned = false;
    for _ in 0..256 {
        let prepare = !returned
            && owner.backing.core.reservation.backing.nodes[10].counts[0] == 0
            && matches!(
                owner.backing.nodes[10].native,
                Some(super::super::native::Native::OmegaDeclaration(_))
            );
        let injection = prepare.then(|| crate::loan::fault::Injection::after(0));
        let event = owner.drive_one(heap, account, &[])?;
        drop(injection);
        match event {
            Event::Advanced {
                node,
                probe,
                control,
            } => {
                assert!(node.slot < 12);
                if node == omega {
                    exact(owner, heap, account, true)?;
                }
                control?;
                owner.acknowledge_probe(probe, account)?;
            }
            Event::Terminal {
                node,
                evidence,
                control,
            } => {
                control?;
                if node == output {
                    sealed(owner, output, heap, account)?;
                    delivered = Some(owner.result(output, heap, account)?.retain(heap, account)?);
                }
                owner.acknowledge_outcome(node, evidence, heap, account)?;
            }
            Event::PreparationRefused {
                node,
                declaration,
                error,
                probe,
                evidence,
                control,
            } => {
                assert_eq!(node, omega);
                assert!(matches!(
                    error,
                    crate::LcShapeError::Loan(crate::LoanError::AllocationRefused)
                ));
                control?;
                owner.acknowledge_probe(probe?, account)?;
                owner.acknowledge_return(node, evidence?, heap, account)?;
                owner
                    .restore_preparation(node, declaration, heap, account)
                    .map_err(|(_, error)| error)?;
                returned = true;
            }
            Event::Refused {
                node,
                error,
                probe,
                control,
            } => {
                return Err(
                    format!("native refusal {node:?}: {error:?}; {probe:?}; {control:?}").into(),
                );
            }
            Event::AttemptsExhausted {
                node,
                evidence,
                outcome,
                control,
            } => {
                return Err(format!(
                    "unexpected exhaustion {node:?}: {evidence:?}; {outcome:?}; {control:?}"
                )
                .into());
            }
            Event::Control => {}
            Event::Quiescent => break,
            other => return Err(format!("unexpected native graph event: {other:?}").into()),
        }
    }
    assert!(returned);
    delivered.ok_or_else(|| "sealed output was not delivered".into())
}

/// Original L,C dyadics give positive angular frequency with a nonnegative enclosure radius.
fn sealed<P: Scalar + 'static>(
    owner: &Owner<'_, P, 5>,
    output: crate::loan::graph::walk::Handle,
    heap: &crate::Heap,
    account: &mut crate::ExecutionAccount,
) -> TestResult {
    let result = owner.result(output, heap, account)?;
    let reader = result.current_reader(heap, account)?;
    for cell in 0..12 {
        let outcome = reader.outcome(cell, account)?;
        assert_eq!(outcome.kind, crate::LcOutcomeKind::Filled);
        enclosure(&reader, cell, account)?;
    }
    Ok(())
}

/// Squared reciprocal-root bounds use only independently enumerated original input endpoints.
pub(super) fn enclosure<P: Scalar + 'static>(
    reader: &crate::LcResultReader<'_, P>,
    cell: usize,
    account: &mut crate::ExecutionAccount,
) -> TestResult {
    use crate::loan::application::tests::binary::oracle::Rational;
    use crate::{BinaryElementwise::Product, HostChannel};
    use std::cmp::Ordering;
    let center = Rational::binary(reader.read(HostChannel::Center, cell, account)?.to_f64())?;
    let total = Rational::binary(reader.read(HostChannel::Total, cell, account)?.to_f64())?;
    let lower = center.sub(&total);
    let upper = center.add(&total);
    let (minimum, maximum) = if cell < 6 {
        ((5, 32), (35, 128))
    } else {
        ((45, 128), (33, 64))
    };
    let minimum = Rational::fraction(minimum.0, minimum.1)?;
    let maximum = Rational::fraction(maximum.0, maximum.1)?;
    let one = Rational::fraction(1, 1)?;
    assert_eq!(lower.compare(&Rational::fraction(0, 1)?), Ordering::Greater);
    assert_ne!(
        lower
            .image(&lower, Product)?
            .image(&maximum, Product)?
            .compare(&one),
        Ordering::Greater
    );
    assert_ne!(
        upper
            .image(&upper, Product)?
            .image(&minimum, Product)?
            .compare(&one),
        Ordering::Less
    );
    Ok(())
}

/// A non-issued expectation occupies no numerical region but still blocks its named owner set.
fn named_pending<P: Scalar + 'static>(
    complete: &Owner<'_, P, 5>,
    inventory: &crate::LcInputInventory<P, 5>,
    axes: &crate::LcShapeAxes,
    registry: &nrz_bindings::Registry,
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
) -> TestResult {
    use super::super::{admission::Refusal, node::Node, owner::Capacities};
    let declaration = inventory
        .input(0)
        .ok_or("original input")?
        .source(axes, account)?;
    let other = Owner::admit(
        vec![Node::input(declaration)],
        &[1],
        Capacities {
            nodes: 1,
            edges: 0,
            scopes: 0,
            drive: 10,
            records: 3,
            words: 19,
            witnesses: 0,
        },
        inventory,
        registry,
        &[complete.borrowed()],
        heap,
        account,
    )
    .map_err(|Refusal { error, .. }| error)?;
    assert!(matches!(
        Owner::completion(&[complete, &other, complete], heap, account, 100_000)?,
        Completion::GraphPending(_)
    ));
    assert!(matches!(
        Owner::completion(&[complete], heap, account, 100_000)?,
        Completion::Complete { .. }
    ));
    Ok(())
}

/// Issuing and publishing the five host loans precedes every mapped or checked graph phase.
fn issue<P: Scalar + 'static>(
    owner: &mut Owner<'_, P, 5>,
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
) -> TestResult {
    for position in 0..5 {
        let handle = owner.handle(position, account)?;
        let InputRequest::Granted {
            mut loan,
            evidence: Ok(evidence),
            control,
        } = owner.request_input(handle, heap, account, &[])?
        else {
            return Err("native input admission".into());
        };
        control?;
        owner.acknowledge_probe(evidence, account)?;
        fixture::fill(&mut loan, position, account)?;
        let InputReturn::Published { control } = owner.return_input(handle, loan, heap, account)
        else {
            return Err("native handback".into());
        };
        control?;
    }
    Ok(())
}

/// Literal captured/header bytes and the independent original-record oracle delimit each phase.
fn exact<P: Scalar>(
    owner: &Owner<'_, P, 5>,
    heap: &crate::Heap,
    account: &mut crate::ExecutionAccount,
    live: bool,
) -> TestResult {
    use super::super::{exact::Scope, node::Contract};
    let replay = matches!(
        owner.backing.nodes[10].contract,
        Contract::Omega {
            certificate: Some(_),
            ..
        }
    );
    let expected = super::super::exact::Report {
        logical: if live {
            416 + crate::loan::lc_shape::tests::output::bridge::oracle::retained(replay)
        } else {
            0
        },
        observed: if live { 16 } else { 0 } + if replay { 10_366 } else { 0 },
        owners: if live { 3 } else { 0 } + usize::from(replay),
    };
    let current = Scope {
        current: Some(owner),
        prior: &[],
        heap,
    }
    .inspect(account)?;
    assert_eq!(current, expected);
    // This fresh receiving store has no checked-cache registration. Its LC root
    // therefore remains wholly caller-reserved through the existing owner scope.
    let (reserved, overlap) = Scope {
        current: Some(owner),
        prior: &[],
        heap,
    }
    .reservation(account)?;
    assert_eq!(reserved, expected);
    assert_eq!(overlap, 0);
    let duplicates = [owner.borrowed(), owner.borrowed()];
    assert_eq!(
        Scope {
            current: Some(owner),
            prior: &duplicates,
            heap
        }
        .inspect(account)?,
        expected
    );
    Ok(())
}
