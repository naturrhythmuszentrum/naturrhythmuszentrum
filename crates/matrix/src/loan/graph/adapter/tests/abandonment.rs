// SPDX-License-Identifier: AGPL-3.0-only
//! A common timeline separates graph storage, exact children, numerical payload and surviving history.
use super::super::{input::InputRequest, node::Node, snapshot::Snapshot};
use super::{
    fixture::{self, TestResult},
    lifecycle,
};
use crate::loan::{
    graph::{inventory::Row, sink::Status},
    lc_shape::tests::output::{Setup, setup},
};
use nrz_scalar::Scalar;

/// Abandonment releases graph storage while both original public native routes remain usable.
pub(super) fn run<P: Scalar + 'static>(cancel: bool) -> TestResult {
    let Setup {
        mut heap,
        mut account,
        inventory,
        axes,
        registry,
        radian,
    } = setup::<P>(nrz_bindings::Limits::default(), [2, 3, 2])?;
    let (mut owner, demands) = fixture::admit_extended(
        &inventory,
        &axes,
        &registry,
        &radian,
        &mut heap,
        &mut account,
        ([8; 12], &(0..12).collect::<Vec<_>>(), None, true),
    )?;
    super::originals::run(&owner, &inventory, &axes, &registry, &heap, &mut account)?;
    let original = owner.originals(&mut account)?;
    let original_measure = original
        .input(0)
        .ok_or("original question")?
        .question
        .measure;
    let initial = Snapshot::inspect(&[owner.borrowed()], &heap, &mut account)?;
    fixed::<P>(initial);
    assert_eq!(initial.native.elements, [0; 2]);
    assert_eq!(initial.native.numerical_bytes, 0);
    assert_eq!(initial.exact_children.logical, 0);
    let extra = owner.handle(12, &mut account)?;
    let InputRequest::Granted {
        loan,
        evidence: Ok(evidence),
        control,
    } = owner.request_input(extra, &mut heap, &mut account, &[])?
    else {
        return Err("extra host occupation".into());
    };
    control?;
    let witness = owner.witness(evidence, &mut account)?;
    owner.acknowledge_probe(evidence, &mut account)?;
    for position in 0..5 {
        fixture::publish_input(&mut owner, position, &mut heap, &mut account)?;
    }
    let output = owner.handle(11, &mut account)?;
    let omega = owner.handle(10, &mut account)?;
    let outside = lifecycle::drive(&mut owner, output, omega, &mut heap, &mut account)?;
    let published = Snapshot::inspect(&[owner.borrowed()], &heap, &mut account)?;
    fixed::<P>(published);
    assert_eq!(
        Snapshot::inspect(&[owner.borrowed(), owner.borrowed()], &heap, &mut account)?,
        published
    );
    numerical::<P>(published, true);
    let before = account.usage();
    drop(owner);
    assert_eq!(account.usage(), before);
    assert_eq!(witness.status(&mut account)?, Status::Abandoned);
    let abandoned = Snapshot::inspect(&[], &heap, &mut account)?;
    assert_eq!(
        (
            abandoned.graph_bytes,
            abandoned.observer_bytes,
            abandoned.graph_owners
        ),
        (0, 0, 0)
    );
    assert_eq!(abandoned.delivery_history_bytes, 19_424);
    numerical::<P>(abandoned, true);
    assert_eq!(abandoned.exact_children, published.exact_children);
    release(loan, outside, demands, &mut heap, &mut account, cancel)?;
    assert_eq!(
        original
            .input(0)
            .ok_or("retained original")?
            .question
            .measure,
        original_measure
    );
    assert!(original.registry().same_owner(
        &crate::loan::lc_shape::exact_scope::ExactScope::new(&inventory, &registry).registry()
    ));
    assert_eq!(witness.status(&mut account)?, Status::Abandoned);
    let before = account.usage();
    drop(witness);
    assert_eq!(account.usage(), before);
    assert_eq!(
        Snapshot::inspect(&[], &heap, &mut account)?.delivery_history_bytes,
        24
    );
    Ok(())
}

/// Literal O and K records plus fixed native carrier slots supply independent storage expectations.
fn fixed<P: Scalar>(report: Snapshot) {
    // O: shared376, 13 nodes136, 11 edges80, ready8, stack13*16, marks13*8,
    // two scopes32 and four membership words8. Adapter excludes the observer here.
    assert_eq!(
        report.graph_bytes,
        3440 + 144 + 13 * (size_of::<Node<'_, P, 5>>() + 8)
    );
    assert_eq!(report.observer_bytes, 72 + 16);
    assert_eq!(report.graph_owners, 1);
    // K: shared144, (26+104) records88, (143+832) words8, two witness words, registration24.
    assert_eq!(
        report.delivery_history_bytes,
        144 + 130 * 88 + 975 * 8 + 16 + 24
    );
    assert_eq!(
        report.walk_rows,
        [
            Row::Covered(2144),
            Row::Covered(888),
            Row::Covered(408),
            Row::OutsideScope,
            Row::OutsideScope,
            Row::Deferred,
            Row::Deferred
        ]
    );
    assert_eq!(report.original_exact_capacity_bytes, None);
    assert_eq!(report.provenance_capacity_bytes, None);
    assert_eq!(report.native.unpartitioned_constant_bytes, 0);
}

/// Six two-cell host triples, five twelve-cell mapped triples and one twelve-cell output triple.
fn numerical<P: Scalar>(report: Snapshot, extra: bool) {
    let hosts = 5 + usize::from(extra);
    let mut elements = [0; 2];
    elements[usize::from(size_of::<P>() == 8)] = 3 * (hosts * 2 + 6 * 12);
    assert_eq!(report.native.elements, elements);
    let rounded = (12 * size_of::<P>()).div_ceil(64) * 64;
    assert_eq!(
        report.native.numerical_bytes,
        3 * (hosts * 64 + 6 * rounded)
    );
    assert_eq!(
        report.exact_children.logical,
        416 + crate::loan::lc_shape::tests::output::bridge::oracle::retained(false)
    );
    assert_eq!(report.exact_children.observed, 16);
    assert_eq!(report.exact_children.owners, 3);
}

/// Native release paths remain authenticated and useful after all graph capabilities have dropped.
fn release<P: Scalar + 'static>(
    mut loan: crate::OwnedLoan<P>,
    outside: crate::LcPublishedResult<P>,
    demands: Vec<crate::MaskVersion>,
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
    cancel: bool,
) -> TestResult {
    if cancel {
        assert!(matches!(
            heap.cancel_loan(loan, account),
            crate::CancelOutcome::Cancelled(_)
        ));
    } else {
        fixture::fill(&mut loan, 0, account)?;
        let crate::HandbackOutcome::Published(tile, _) = heap.handback(loan, account) else {
            return Err("post-abandonment handback".into());
        };
        drop(tile);
    }
    assert!(matches!(
        heap.residence(account, 100_000)?,
        crate::Residence::Pending { .. }
    ));
    {
        let reader = outside.current_reader(heap, account)?;
        for cell in 0..12 {
            lifecycle::enclosure(&reader, cell, account)?;
        }
    }
    outside.release(account).map_err(|(_, error)| error)?;
    for demand in demands {
        heap.retire_mask(demand, &mut account.ledger)?;
        heap.residence(account, 100_000)?;
        heap.retire_cell_layout(demand.layout(), &mut account.ledger)?;
    }
    for _ in 0..8 {
        if matches!(
            heap.residence(account, 100_000)?,
            crate::Residence::Empty { .. }
        ) {
            break;
        }
    }
    let drained = Snapshot::inspect(&[], heap, account)?;
    assert_eq!(drained.native.elements, [0; 2]);
    assert_eq!(drained.native.numerical_bytes, 0);
    assert_eq!(drained.exact_children.logical, 0);
    assert_eq!(drained.delivery_history_bytes, 19_424);
    assert!(drained.native.other_owner_and_table_bytes > 0);
    Ok(())
}
