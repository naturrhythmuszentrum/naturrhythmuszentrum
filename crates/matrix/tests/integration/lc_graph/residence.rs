// SPDX-License-Identifier: AGPL-3.0-only
//! F3 observes a common native/graph/exact/history timeline through abandonment and release.
use super::{
    drive,
    fixture::{self, Result, Setup},
};
use nrz_matrix::*;
use nrz_scalar::Scalar;

pub(super) fn run<P: Scalar + 'static>() -> Result {
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        radian,
        certificate,
        equal_question,
    } = Setup::<P>::new(false)?;
    let equal_inventory =
        LcInputInventory::<P, 5>::declare(&equal_question, [2, 3, 2], &mut account)?;
    let question = inventory.input(0).ok_or("original question")?.question();
    // Complete numerical cycles share the unchanged two-million-unit ledger.
    for round in 0..2 {
        let inventory = LcInputInventory::<P, 5>::declare(question, [2, 3, 2], &mut account)?;
        let (mut owner, masks) = fixture::admit(
            &inventory,
            &axes,
            &registry,
            &radian,
            &mut heap,
            &mut account,
            (&certificate, &(0..12).collect::<Vec<_>>(), true),
        )?;
        super::originals::run(
            &owner,
            &inventory,
            &equal_inventory,
            &axes,
            &registry,
            &heap,
            &mut account,
        )?;
        let original = owner.originals(&mut account)?;
        let measure = original
            .input(0)
            .ok_or("original question")?
            .question
            .measure;
        let initial = LcGraphSnapshot::inspect(&[owner.borrowed()], &heap, &mut account)?;
        fixed::<P>(initial);
        assert_eq!(initial.native.elements, [0; 2]);
        let extra = owner.handle(12, &mut account)?;
        let LcGraphInputRequest::Granted {
            loan,
            evidence: Ok(evidence),
            control,
        } = owner.request_input(extra, &mut heap, &mut account, &[])?
        else {
            return Err("detached host occupation".into());
        };
        control?;
        let witness = owner.witness(evidence, &mut account)?;
        owner.acknowledge_probe(evidence, &mut account)?;
        drive::issue(&mut owner, 2, &mut heap, &mut account)?;
        let outside = drive::drain(
            &mut owner,
            LcGraphState::Published,
            false,
            &mut heap,
            &mut account,
        )?
        .ok_or("outside sealed result")?;
        let published = LcGraphSnapshot::inspect(&[owner.borrowed()], &heap, &mut account)?;
        fixed::<P>(published);
        numerical::<P>(published);
        assert_eq!(
            LcGraphSnapshot::inspect(&[owner.borrowed(), owner.borrowed()], &heap, &mut account)?,
            published
        );
        abandon(owner, &witness, published, &heap, &mut account)?;
        release(
            loan,
            outside,
            masks,
            &mut heap,
            &mut account,
            round % 2 == 0,
        )?;
        assert_eq!(
            original
                .input(0)
                .ok_or("retained original")?
                .question
                .measure,
            measure
        );
        history(witness, &heap, &mut account)?;
    }
    super::history::run::<P>()?;
    super::stopped::run::<P>()
}

fn fixed<P: Scalar>(report: LcGraphSnapshot) {
    assert_eq!(
        report.graph_bytes,
        3440 + 144 + 13 * (size_of::<LcGraphNode<'_, P, 5>>() + 8)
    );
    assert_eq!(report.observer_bytes, 88);
    assert_eq!(report.graph_owners, 1);
    assert_eq!(
        report.delivery_history_bytes,
        144 + 130 * 88 + 975 * 8 + 16 + 24
    );
    assert_eq!(
        report.walk_rows,
        [
            LcGraphInventoryRow::Covered(2144),
            LcGraphInventoryRow::Covered(888),
            LcGraphInventoryRow::Covered(408),
            LcGraphInventoryRow::OutsideScope,
            LcGraphInventoryRow::OutsideScope,
            LcGraphInventoryRow::Deferred,
            LcGraphInventoryRow::Deferred
        ]
    );
    assert_eq!(report.original_exact_capacity_bytes, None);
    assert_eq!(report.provenance_capacity_bytes, None);
}
fn numerical<P: Scalar>(report: LcGraphSnapshot) {
    let mut elements = [0; 2];
    elements[usize::from(size_of::<P>() == 8)] = 3 * (6 * 2 + 6 * 12);
    assert_eq!(report.native.elements, elements);
    assert_eq!(
        report.native.numerical_bytes,
        3 * (6 * 64 + 6 * (12 * size_of::<P>()).div_ceil(64) * 64)
    );
    assert_eq!(report.exact_children.observed, 16);
    assert_eq!(report.exact_children.owners, 3);
    assert_eq!(report.exact_children.logical, exact());
}
fn release<P: Scalar + 'static>(
    mut loan: OwnedLoan<P>,
    outside: LcPublishedResult<P>,
    masks: Vec<MaskVersion>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    cancel: bool,
) -> Result {
    if cancel {
        assert!(matches!(
            heap.cancel_loan(loan, account),
            CancelOutcome::Cancelled(_)
        ));
    } else {
        fixture::fill(&mut loan, 0, 2, account)?;
        let HandbackOutcome::Published(tile, _) = heap.handback(loan, account) else {
            return Err("native handback after abandonment".into());
        };
        drop(tile);
    }
    {
        let reader = outside.current_reader(heap, account)?;
        for cell in 0..12 {
            drive::enclosure(&reader, cell, account)?;
        }
    }
    outside.release(account).map_err(|(_, error)| error)?;
    fixture::retire(masks, heap, account)?;
    let mut empty = false;
    for _ in 0..8 {
        if matches!(heap.residence(account, 100_000)?, Residence::Empty { .. }) {
            empty = true;
            break;
        }
    }
    assert!(empty);
    let report = LcGraphSnapshot::inspect(&[], heap, account)?;
    assert_eq!(report.native.elements, [0; 2]);
    assert_eq!(report.native.numerical_bytes, 0);
    assert_eq!(report.exact_children.logical, 0);
    Ok(())
}

/// Dropping graph capabilities cannot erase live native or history occupations.
fn abandon<P: Scalar + 'static>(
    owner: LcGraphOwner<'_, P, 5>,
    witness: &LcGraphWitness,
    published: LcGraphSnapshot,
    heap: &Heap,
    account: &mut ExecutionAccount,
) -> Result {
    let before = account.usage();
    drop(owner);
    assert_eq!(account.usage(), before);
    assert_eq!(witness.status(account)?, LcGraphDeliveryStatus::Abandoned);
    let abandoned = LcGraphSnapshot::inspect(&[], heap, account)?;
    assert_eq!(
        (
            abandoned.graph_bytes,
            abandoned.observer_bytes,
            abandoned.graph_owners
        ),
        (0, 0, 0)
    );
    numerical::<P>(abandoned);
    assert_eq!(abandoned.exact_children, published.exact_children);
    Ok(())
}

/// Forty-four literal canonical records, grouped by key length and dependency arity.
fn exact() -> u64 {
    let graph = [
        (17, 1, 1),
        (41, 2, 7),
        (46, 2, 3),
        (50, 2, 1),
        (50, 3, 5),
        (51, 2, 1),
        (52, 2, 10),
        (59, 4, 11),
        (60, 4, 1),
        (244, 2, 1),
        (249, 2, 1),
    ]
    .into_iter()
    .map(|(key, edges, count)| count * (3 * key + 256 + 32 * edges))
    .sum::<u64>()
        + (3 * 20 + 256)
        + (3 * 23 + 256);
    assert_eq!(graph, 22_522);
    // Captured416; correspondence carrier48, store ABI+384, root16, records, wire10342, maps1170.
    416 + 48
        + size_of::<nrz_bindings::readings::algebra::Cas>() as u64
        + 384
        + 16
        + graph
        + 10_342
        + 1170
}

/// Only the final prepaid witness release removes the abandoned account-resident history.
fn history(witness: LcGraphWitness, heap: &Heap, account: &mut ExecutionAccount) -> Result {
    assert_eq!(witness.status(account)?, LcGraphDeliveryStatus::Abandoned);
    assert_eq!(
        LcGraphSnapshot::inspect(&[], heap, account)?.delivery_history_bytes,
        19_424
    );
    let before = account.usage();
    drop(witness);
    assert_eq!(account.usage(), before);
    assert_eq!(
        LcGraphSnapshot::inspect(&[], heap, account)?.delivery_history_bytes,
        24
    );
    Ok(())
}
