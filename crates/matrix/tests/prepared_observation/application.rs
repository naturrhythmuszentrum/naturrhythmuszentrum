// SPDX-License-Identifier: AGPL-3.0-only
//! Actual masked publication agrees with observation disabled and retains complete native receipts.
use super::{ArithmeticContext, Counts, Fixture, Policy, TestResult, broad, ledger};
use nrz_bindings::{BindingSymbols, Context, Descriptor, ExponentPolicy, Registry};
use nrz_matrix::{
    ApplicationEvaluation, ApplicationPublication, Axis, AxisEntry, BlockDraft, Coverage,
    EntryDraft, ExecutionAccount, ExecutionLimits, Expm1ApplicationRequest, Heap, HeapLimits,
    HostChannel, HostTileDeclaration, OperationDraft, OwnedApplication, Partition,
    PointwiseExpm1Request, PreparedExpm1Operation, PublishedTile, RegionContract, ShapeDraft,
    ShapeLimits, StructureScope,
};
use std::rc::Rc;

/// A coherent assembly supplies original checked region witnesses, without retagging a Square.
fn region(cells: usize) -> TestResult<RegionContract> {
    let limits = ShapeLimits::default();
    let owner = Rc::new(
        Context::new(
            Registry::new(ExponentPolicy::Rational),
            nrz_bindings::readings::algebra::limits::Limits::default(),
            nrz_bindings::readings::algebra::syntax::Environment::default(),
        )?
        .freeze()?,
    );
    let domain = Axis::new(
        vec![AxisEntry::new("component", Descriptor::ratio())?],
        Partition::whole(1, limits)?,
        limits,
    )?;
    let entries = (0..cells)
        .map(|cell| AxisEntry::new(format!("cell-{cell}"), Descriptor::ratio()))
        .collect::<Result<Vec<_>, _>>()?;
    let codomain = Axis::new(entries, Partition::whole(cells, limits)?, limits)?;
    let blocks = if cells == 0 {
        Vec::new()
    } else {
        vec![BlockDraft::new(
            0..cells,
            0..1,
            vec![EntryDraft::Declared(Descriptor::ratio()); cells],
        )]
    };
    let shape = ShapeDraft::new(domain, codomain, blocks, StructureScope::WholeFamily)
        .check(owner, limits)?;
    let checked = OperationDraft::copy(vec![shape]).check(ExecutionLimits::default())?;
    Ok(checked.inputs()[0].clone())
}
/// Filled host values and cleared invalid holes use only the public host protocol.
fn publish_host(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    input: HostTileDeclaration<f64>,
    values: &[Option<f64>],
) -> TestResult<(PublishedTile<f64>, nrz_matrix::PublicationReceipt)> {
    let mut loan = match heap.request_loan(input, account) {
        nrz_matrix::LoanRequest::Granted(loan) => loan,
        other @ nrz_matrix::LoanRequest::Refused(_) => return Err(format!("{other:?}").into()),
    };
    for (cell, value) in values.iter().enumerate() {
        loan.write_component(
            HostChannel::Center,
            cell,
            0,
            value.unwrap_or(f64::NAN),
            account,
        )?;
        if value.is_none() {
            loan.clear_cell(HostChannel::Center, cell, account)?;
        }
    }
    Ok(match heap.handback(loan, account) {
        nrz_matrix::HandbackOutcome::Published(tile, receipt) => (tile, receipt),
        other => return Err(format!("{other:?}").into()),
    })
}
/// Setup ends before observation so the bounded receipt describes actual masked reuse.
fn ready() -> TestResult<(Heap, ExecutionAccount, OwnedApplication<f64>)> {
    let mut fixture = Fixture::new()?;
    let original = broad(&mut fixture, 40)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = ExecutionAccount::new(ledger()?)?;
    let law = heap
        .prepare_retained_expm1(
            fixture.cas,
            &original,
            [-2.0, 0.0],
            Policy::default(),
            &ArithmeticContext::new()?,
            &mut Counts::default(),
            &mut account,
        )
        .map_err(|(error, _)| error)?;
    let mut symbolic = ledger()?;
    let source = BindingSymbols::declare(&Descriptor::ratio(), [3, 1], [false; 2], &mut symbolic)?;
    let destination =
        BindingSymbols::declare(&Descriptor::ratio(), [3, 1], [true; 2], &mut symbolic)?;
    let axis = heap.create_cell_axis(3, &mut symbolic)?;
    let input = HostTileDeclaration::check(
        source.clone(),
        axis,
        Coverage::RepresentedInputs,
        &mut account,
    )?;
    let output = HostTileDeclaration::check(
        destination.clone(),
        axis,
        Coverage::RepresentedInputs,
        &mut account,
    )?;
    let (tile, receipt) = publish_host(
        &mut heap,
        &mut account,
        input,
        &[Some(-0.5), Some(-1.0), Some(0.0)],
    )?;
    let layout = receipt
        .filled(HostChannel::Center)
        .ok_or("filled")?
        .native()
        .layout();
    let demand = heap.create_mask(layout, &[0, 1, 2], &mut symbolic)?;
    let input = region(3)?;
    let operation = PreparedExpm1Operation::check(
        &law,
        &original.original.target,
        &original.original.context,
        &original.original.coordinates[0].coordinate,
        [-2.0, 0.0],
        40,
        1,
        input.clone(),
        input,
        source,
        destination,
        axis,
        &mut account,
    )?;
    let owner = match heap.request_pointwise_expm1(
        PointwiseExpm1Request {
            operation,
            law,
            input: tile.into_pin(),
            destination: output,
            demand,
            limits: ExecutionLimits::new(1, 15, 15),
        },
        &mut account,
    ) {
        Expm1ApplicationRequest::ApplicationReady(owner) => owner,
        other @ Expm1ApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    Ok((heap, account, owner))
}
/// Complete native traversal, publication and five selected result loads have distinct receipts.
fn finish(
    mut heap: Heap,
    mut account: ExecutionAccount,
    owner: OwnedApplication<f64>,
) -> TestResult<([[u64; 5]; 3], u64)> {
    let before = account.usage().work;
    let owner = match owner.evaluate(&mut account) {
        ApplicationEvaluation::ApplicationCandidate(owner) => owner,
        other => return Err(format!("{other:?}").into()),
    };
    let evaluation = account.usage().work - before;
    let tile = match heap.publish_application(owner, &mut account) {
        ApplicationPublication::ApplicationPublished(tile, _) => tile,
        other => return Err(format!("{other:?}").into()),
    };
    let reader = tile.reader(&heap, &mut account)?;
    let mut bits = [[0; 5]; 3];
    for (cell, values) in bits.iter_mut().enumerate() {
        let [lower, upper] = reader.expm1_endpoints(cell, 0, &mut account)?;
        *values = [
            reader.read_component(HostChannel::Center, cell, 0, &mut account)?,
            reader.read_component(HostChannel::Total, cell, 0, &mut account)?,
            reader.read_component(HostChannel::Error, cell, 0, &mut account)?,
            lower,
            upper,
        ]
        .map(f64::to_bits);
    }
    Ok((bits, evaluation))
}
/// Zero native exact Work has a completed receipt beside the positive wrapper prepayment.
#[test]
fn masked_prepared_expm1_observation_agrees_through_last_native_exit() -> TestResult {
    let _guard = super::observation::DISPATCHERS
        .lock()
        .map_err(|_| "dispatcher guard")?;
    let (heap, account, owner) = ready()?;
    let (enabled, report) = super::trace::observed(64, || finish(heap, account, owner));
    let enabled = enabled?;
    let (heap, account, owner) = ready()?;
    let disabled =
        tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
            finish(heap, account, owner)
        })?;
    assert_eq!(enabled, disabled);
    assert_eq!(enabled.1, 3 + 2 + 3 * (6 + 2));
    assert!(report.complete);
    assert_eq!(report.observer.capacity, 64);
    assert_eq!(
        report
            .clocks
            .get(&(String::new(), "nrz.kernel".into(), "prepared-expm1".into()))
            .map(|clock| clock.1),
        Some(3)
    );
    let native: Vec<_> = report
        .receipts
        .iter()
        .filter(|(key, _)| key.3 == "prepared native traversal")
        .collect();
    assert_eq!(native.len(), 1);
    assert_eq!(
        (
            native[0].1.inclusive,
            native[0].1.exclusive,
            native[0].1.intervals
        ),
        (0, 0, 1)
    );
    let wrapper: Vec<_> = report
        .receipts
        .iter()
        .filter(|(key, _)| key.3 == "pointwise evaluation")
        .collect();
    assert_eq!(wrapper.len(), 1);
    assert_eq!(wrapper[0].1.inclusive, enabled.1);
    assert_eq!(report.counters.len(), 0);
    assert_eq!(report.ledgers.len(), 1);
    for ledger in report.ledgers.values() {
        assert!(ledger.closed);
        assert_eq!(
            ledger.last - ledger.first,
            ledger.classified + ledger.remainder
        );
    }
    Ok(())
}
