// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable current and historical seals reject all directed cross-family substitutions.

use super::{
    TestResult, candidate, published,
    seals::{MEMBERS, owned},
};
use crate::loan::{application::Evidence, fault, registry::State};
use crate::{Heap, LoanError, PublishedTile};
use std::rc::Rc;

/// Temporarily detach registry aliases to mutate test evidence, then restore every alias.
fn alter(
    tile: &mut PublishedTile<f64>,
    heap: &mut Heap,
    action: impl FnOnce(&mut Evidence) -> TestResult,
) -> TestResult {
    let root = tile.pin().ticket()?.identity.root.slot;
    let pin_slot = heap.loans.groups[root].as_ref().ok_or("group")?.pin;
    let mut index = heap.loans.pins[pin_slot].take().ok_or("index")?;
    index.witness = None;
    heap.loans.groups[root].as_mut().ok_or("group")?.state = State::Reserved;
    let pin = Rc::get_mut(&mut tile.pin.record)
        .and_then(|record| record.payload.as_mut())
        .ok_or("unique pin")?;
    let bundle = Rc::get_mut(&mut pin.bundle)
        .and_then(|owner| owner.payload.as_mut())
        .ok_or("unique bundle")?;
    let result = action(bundle.origin.evidence.as_mut().ok_or("origin")?);
    heap.loans.groups[root].as_mut().ok_or("group")?.state =
        State::Published(Rc::clone(&pin.bundle));
    index.witness = Some(Rc::downgrade(&tile.pin.record));
    heap.loans.pins[pin_slot] = Some(index);
    result
}
/// Each substituted object comes from a successfully published, individually valid family.
fn exchange(
    tile: &mut PublishedTile<f64>,
    heap: &mut Heap,
    donor: &mut PublishedTile<f64>,
    other: &mut Heap,
    choice: usize,
) -> TestResult {
    alter(tile, heap, |evidence| {
        alter(donor, other, |replacement| {
            match choice {
                0 => std::mem::swap(&mut evidence.operation, &mut replacement.operation),
                1 => std::mem::swap(&mut evidence.law, &mut replacement.law),
                2 => std::mem::swap(evidence, replacement),
                _ => return Err("substitution".into()),
            }
            Ok(())
        })
    })
}
/// Equal native centers at one cannot bypass identity; prepared retains its distinct admitted input.
/// ´test:unit:square-root-all-directed-current-and-historical-family-substitutions´
#[test]
fn square_root_all_directed_current_and_historical_family_substitutions() -> TestResult {
    let mut pairs = 0;
    for member in MEMBERS {
        for other in MEMBERS.into_iter().filter(|other| *other != member) {
            pairs += 1;
            let (mut heap, mut account, owner) = owned(member)?;
            let (mut donor_heap, mut donor_account, donor) = owned(other)?;
            let owner = candidate(owner, &mut account)?;
            let donor = candidate(donor, &mut donor_account)?;
            let (mut tile, _) = published(heap.publish_application(owner, &mut account))?;
            let (mut donor, _) =
                published(donor_heap.publish_application(donor, &mut donor_account))?;
            for choice in 0..3 {
                exchange(&mut tile, &mut heap, &mut donor, &mut donor_heap, choice)?;
                fault::reset_reads();
                fault::reset_scalars();
                for historical in [false, true] {
                    let result = if historical {
                        tile.pin().historical_reader(&mut account)
                    } else {
                        tile.reader(&heap, &mut account)
                    };
                    if choice < 2 {
                        assert!(
                            matches!(result, Err(LoanError::UnsupportedContract)),
                            "{result:?}"
                        );
                    } else {
                        assert!(matches!(result, Err(LoanError::WrongLoan)), "{result:?}");
                    }
                }
                assert_eq!([fault::reads(), fault::scalars()], [0; 2]);
                exchange(&mut tile, &mut heap, &mut donor, &mut donor_heap, choice)?;
                let reader = tile.reader(&heap, &mut account)?;
                assert!(matches!(
                    reader.application_outcome(0, &mut account)?,
                    crate::CellOutcome::Completed
                ));
                if member.is_some() {
                    assert_eq!(
                        reader.read_component(crate::HostChannel::Center, 0, 0, &mut account)?,
                        1.0
                    );
                }
            }
            drop(heap);
            assert!(
                tile.pin()
                    .historical_reader(&mut account)?
                    .contains(0, &mut account)?
            );
        }
    }
    assert_eq!(pairs, 12);
    Ok(())
}
