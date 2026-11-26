// SPDX-License-Identifier: AGPL-3.0-only
//! F1 uses only public graph/native capabilities and independent rational endpoint checks.
use super::fixture::{self, Result, Setup};
use nrz_matrix::*;
use nrz_scalar::Scalar;

pub(super) fn run<P: Scalar + 'static>() -> Result {
    for replay in [false, true] {
        for (instances, empty, expected) in [
            (2, false, LcGraphState::Published),
            (1, false, LcGraphState::UnsupportedPartial),
            (0, false, LcGraphState::MissingInput),
            (2, true, LcGraphState::Published),
        ] {
            let Setup {
                mut heap,
                mut account,
                inventory,
                axes,
                registry,
                radian,
                certificate,
                ..
            } = Setup::<P>::new(replay)?;
            let selected = if empty { Vec::new() } else { (0..12).collect() };
            let (mut owner, masks) = fixture::admit(
                &inventory,
                &axes,
                &registry,
                &radian,
                &mut heap,
                &mut account,
                (&certificate, &selected, false),
            )?;
            let original = owner.originals(&mut account)?;
            for position in 0..5 {
                let observation = original.input(position).ok_or("original position")?;
                assert_eq!(observation.position, position);
                assert!(
                    observation
                        .question
                        .same_owner(&original.input(0).ok_or("shared question")?.question)
                );
                assert_eq!(observation.question.measure.backing_capacity_bytes, None);
            }
            let initial_handle = owner.handle(0, &mut account)?;
            let before = account.usage().work;
            let (result, receipts) = super::receipts::recorded(|| {
                issue(&mut owner, instances, &mut heap, &mut account)?;
                let outside = drain(&mut owner, expected, empty, &mut heap, &mut account)?;
                LcGraphSnapshot::inspect(&[owner.borrowed()], &heap, &mut account)?;
                Ok::<_, Box<dyn std::error::Error>>(outside)
            });
            let outside = result?;
            super::receipts::check(
                &receipts,
                before,
                account.usage().work,
                expected == LcGraphState::Published,
            )?;
            if let Some(outside) = outside {
                assert!(matches!(
                    LcGraphOwner::completion(&[&owner], &mut heap, &mut account, 100_000)?,
                    LcGraphCompletion::NativePending(..)
                ));
                super::renewal::run(
                    &owner,
                    initial_handle,
                    &inventory,
                    &axes,
                    &registry,
                    &mut heap,
                    &mut account,
                )?;
                outside.release(&mut account).map_err(|(_, error)| error)?;
            }
            fixture::retire(masks, &mut heap, &mut account)?;
            complete(&owner, &mut heap, &mut account)?;
        }
    }
    typed::<P>()?;
    super::fanout::run::<P>()?;
    super::preparation::run::<P>()?;
    super::coverage::run::<P>()?;
    super::cancel::run::<P>()
}

pub(super) fn issue<P: Scalar + 'static>(
    owner: &mut LcGraphOwner<'_, P, 5>,
    instances: usize,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result {
    for position in 0..5 {
        let handle = owner.handle(position, account)?;
        let mut loan = fixture::request(owner, handle, heap, account)?;
        fixture::fill(
            &mut loan,
            position,
            if position == 4 { instances } else { 2 },
            account,
        )?;
        let LcGraphInputReturn::Published { control } =
            owner.return_input(handle, loan, heap, account)
        else {
            return Err("native input publication".into());
        };
        control?;
    }
    Ok(())
}

/// Every terminal receipt is acknowledged separately from the sealed numerical result.
pub(super) fn drain<P: Scalar + 'static>(
    owner: &mut LcGraphOwner<'_, P, 5>,
    expected: LcGraphState,
    empty: bool,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result<Option<LcPublishedResult<P>>> {
    let omega = owner.handle(10, account)?;
    let output = owner.handle(11, account)?;
    let mut outside = None;
    let mut seen = [false; 2];
    for _ in 0..256 {
        match owner.drive_one(heap, account, &[])? {
            LcGraphEvent::Advanced { probe, control, .. } => {
                control?;
                owner.acknowledge_probe(probe, account)?;
            }
            LcGraphEvent::Terminal {
                node,
                evidence,
                control,
            } => {
                control?;
                if node == omega {
                    assert_eq!(owner.state(node, heap, account)?, expected);
                    seen[0] = true;
                }
                if node == output {
                    seen[1] = true;
                    if expected == LcGraphState::Published {
                        let result = owner.result(output, heap, account)?;
                        let reader = result.current_reader(heap, account)?;
                        for cell in 0..12 {
                            assert_eq!(
                                reader.outcome(cell, account)?.kind,
                                if empty {
                                    LcOutcomeKind::NotRequested
                                } else {
                                    LcOutcomeKind::Filled
                                }
                            );
                            if !empty {
                                enclosure(&reader, cell, account)?;
                            }
                        }
                        outside = Some(result.retain(heap, account)?);
                    } else {
                        assert!(owner.result(output, heap, account).is_err());
                    }
                }
                owner.acknowledge_outcome(node, evidence, heap, account)?;
            }
            LcGraphEvent::Refused {
                error,
                probe,
                control,
                ..
            } if expected == LcGraphState::DependencyRefused(5) => {
                assert!(matches!(error, LcGraphNativeFailure::Mapped(_)));
                control?;
                owner.acknowledge_probe(probe.ok_or("mapped refusal receipt")?, account)?;
            }
            LcGraphEvent::Control => {}
            LcGraphEvent::Quiescent => break,
            LcGraphEvent::PreparationRefused { error, .. } => {
                return Err(format!("preparation: {error:?}").into());
            }
            other => return Err(format!("public native progression: {other:?}").into()),
        }
    }
    assert_eq!(seen, [true; 2]);
    Ok(outside)
}

/// Exact rational inequalities use original L and C endpoints and scalar bit decoding.
pub(super) fn enclosure<P: Scalar + 'static>(
    reader: &LcResultReader<'_, P>,
    cell: usize,
    account: &mut ExecutionAccount,
) -> Result {
    use crate::kernel_oracle::Rational;
    let center = Rational::binary(reader.read(HostChannel::Center, cell, account)?.to_f64())?;
    let total = Rational::binary(reader.read(HostChannel::Total, cell, account)?.to_f64())?;
    let lower = center.subtract(total)?;
    let upper = center.add(total)?;
    let (minimum, maximum) = if cell < 6 {
        ((5, 32), (35, 128))
    } else {
        ((45, 128), (33, 64))
    };
    let minimum = Rational::new(minimum.0, minimum.1)?;
    let maximum = Rational::new(maximum.0, maximum.1)?;
    let one = Rational::new(1, 1)?;
    assert!(lower.numerator > 0);
    assert!(lower.multiply(lower)?.multiply(maximum)?.at_most(one)?);
    assert!(one.at_most(upper.multiply(upper)?.multiply(minimum)?)?);
    Ok(())
}

pub(super) fn complete<P: Scalar + 'static>(
    owner: &LcGraphOwner<'_, P, 5>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result {
    for _ in 0..8 {
        let supplied = [owner];
        let result = LcGraphOwner::completion(&supplied, heap, account, 100_000)?;
        assert_eq!(result.owners().distinct(), 1);
        assert_eq!(result.owners().identities().len(), 1);
        if matches!(result, LcGraphCompletion::Complete { .. }) {
            return Ok(());
        }
    }
    Err("graph/native obligations remain".into())
}

/// A malformed public certificate returns the identical declaration with its typed cause.
fn typed<P: Scalar + 'static>() -> Result {
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        radian,
        certificate,
        ..
    } = Setup::<P>::with_certificate(true, true)?;
    let (mut owner, _) = fixture::admit(
        &inventory,
        &axes,
        &registry,
        &radian,
        &mut heap,
        &mut account,
        (&certificate, &(0..12).collect::<Vec<_>>(), false),
    )?;
    issue(&mut owner, 2, &mut heap, &mut account)?;
    let omega = owner.handle(10, &mut account)?;
    for _ in 0..128 {
        match owner.drive_one(&mut heap, &mut account, &[])? {
            LcGraphEvent::PreparationRefused {
                node,
                declaration,
                error,
                probe,
                evidence,
                control,
            } => {
                assert_eq!(node, omega);
                assert!(std::ptr::eq(declaration.inventory(), &raw const inventory));
                assert!(matches!(
                    error,
                    LcShapeError::Declaration(nrz_bindings::AdmissionError::Algebra(_))
                ));
                control?;
                owner.acknowledge_probe(probe?, &mut account)?;
                owner.acknowledge_return(node, evidence?, &heap, &mut account)?;
                assert_eq!(
                    owner.state(node, &heap, &mut account)?,
                    LcGraphState::Refused
                );
                assert!(matches!(
                    owner.retry_original(node, &heap, &mut account),
                    Err(LcGraphError::InvalidPhase)
                ));
                return Ok(());
            }
            LcGraphEvent::Advanced { probe, control, .. } => {
                control?;
                owner.acknowledge_probe(probe, &mut account)?;
            }
            LcGraphEvent::Terminal {
                node,
                evidence,
                control,
            } => {
                control?;
                owner.acknowledge_outcome(node, evidence, &heap, &mut account)?;
            }
            LcGraphEvent::Control => {}
            other => return Err(format!("typed preparation: {other:?}").into()),
        }
    }
    Err("typed preparation did not run".into())
}
