// SPDX-License-Identifier: AGPL-3.0-only
//! A bounded subscriber checks matrix receipts on the enclosing CAS ledger incarnation.

use super::{TestResult, fixture};
use nrz_bindings::{Budget, Limits};
use std::sync::{Arc, Mutex};
use tracing::{
    Event, Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};

/// All subscriber lifetimes in this matrix test target share one interest-rebuild guard.
static DISPATCHERS: Mutex<()> = Mutex::new(());

/// Only fixed receipt fields enter this test observer.
#[derive(Clone, Debug, Default)]
struct Fields {
    /// Original ledger incarnation.
    ledger: u64,
    /// Current boundary snapshot.
    work: u64,
    /// Static consumer family.
    family: String,
    /// Original ledger namespace.
    owner: String,
    /// Static helper name.
    operation: String,
    /// Final result classification.
    outcome: String,
}

impl Visit for Fields {
    fn record_u64(&mut self, field: &Field, value: u64) {
        match field.name() {
            "ledger" => self.ledger = value,
            "work" => self.work = value,
            _ => {}
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "family" => value.clone_into(&mut self.family),
            "owner" => value.clone_into(&mut self.owner),
            "operation" => value.clone_into(&mut self.operation),
            "outcome" => value.clone_into(&mut self.outcome),
            _ => {}
        }
    }

    fn record_debug(&mut self, _field: &Field, _value: &dyn std::fmt::Debug) {}
}

/// One synchronous interval; this fixture contains no suspension or foreign ledger.
#[derive(Debug)]
struct Receipt {
    /// Latest structured fields.
    fields: Fields,
    /// Work on actual entry.
    before: u64,
    /// Work on actual exit.
    after: u64,
    /// Nearest active interval on the same ledger.
    parent: Option<usize>,
}

/// The fixture retains at most sixteen intervals and reports loss explicitly.
#[derive(Debug, Default)]
struct State {
    /// Bounded receipt storage.
    receipts: Vec<Receipt>,
    /// Actual synchronous entered stack.
    active: Vec<usize>,
    /// Overflow or invalid callbacks invalidate the test evidence.
    lost: bool,
}

/// Shared observer state carries no production mask or ledger owner.
#[derive(Clone, Debug)]
struct Observer(Arc<Mutex<State>>);

impl Subscriber for Observer {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.target() == "nrz::work"
    }

    fn new_span(&self, attributes: &Attributes<'_>) -> Id {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.receipts.len() >= 16 {
            state.lost = true;
            return Id::from_u64(17);
        }
        let mut fields = Fields::default();
        attributes.record(&mut fields);
        state.receipts.push(Receipt {
            fields,
            before: 0,
            after: 0,
            parent: None,
        });
        Id::from_u64(u64::try_from(state.receipts.len()).unwrap_or(17))
    }

    fn record(&self, id: &Id, values: &Record<'_>) {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(receipt) = state.receipts.get_mut(index(id)) {
            values.record(&mut receipt.fields);
        } else {
            state.lost = true;
        }
    }

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, _event: &Event<'_>) {}

    fn enter(&self, id: &Id) {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = index(id);
        let parent = state.active.last().copied();
        if let Some(receipt) = state.receipts.get_mut(current) {
            receipt.before = receipt.fields.work;
            receipt.parent = parent;
            state.active.push(current);
        } else {
            state.lost = true;
        }
    }

    fn exit(&self, id: &Id) {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = index(id);
        if state.active.pop() != Some(current) {
            state.lost = true;
        }
        if let Some(receipt) = state.receipts.get_mut(current) {
            receipt.after = receipt.fields.work;
        } else {
            state.lost = true;
        }
    }
}

/// Subscriber IDs are one-based indices into bounded test storage.
fn index(id: &Id) -> usize {
    usize::try_from(id.into_u64() - 1).unwrap_or(usize::MAX)
}

/// Mask and allocation receipts retain one ledger and disabled runs prepare no identity.
/// ´test:unit:mask-shared-ledger-boundary-receipts´
#[test]
fn mask_shared_ledger_boundary_receipts() -> TestResult {
    let _guard = DISPATCHERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (mut heap, layout, _, _) = fixture(65)?;
    let state = Arc::new(Mutex::new(State::default()));
    let subscriber = Observer(Arc::clone(&state));
    let observed_work = tracing::subscriber::with_default(subscriber, || -> TestResult<u64> {
        let mut budget = Budget::new(Limits::default())?;
        budget.with_work_phase("cas", "enclosing CAS", |budget| {
            heap.create_mask(layout, &[1, 64], budget)
        })?;
        Ok(budget.usage().work)
    })?;
    let state = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(!state.lost);
    assert_eq!(state.active, []);
    assert_eq!(state.receipts.len(), 3);
    let root = state.receipts.first().ok_or("missing enclosing receipt")?;
    assert_eq!(root.fields.family, "cas");
    assert_eq!(root.fields.operation, "enclosing CAS");
    assert_eq!(root.after - root.before, observed_work);
    let mut exclusive = 0_u64;
    for (index, receipt) in state.receipts.iter().enumerate() {
        assert_eq!(receipt.fields.ledger, root.fields.ledger);
        assert_ne!(receipt.fields.ledger, 0);
        assert_eq!(receipt.fields.owner, "nrz-cas");
        assert_eq!(receipt.fields.outcome, "completed");
        if index != 0 {
            assert_eq!(receipt.fields.family, "matrix.mask");
        }
        let children: u64 = state
            .receipts
            .iter()
            .filter(|child| child.parent == Some(index))
            .map(|child| child.after - child.before)
            .sum();
        exclusive += (receipt.after - receipt.before)
            .checked_sub(children)
            .ok_or("invalid exclusive receipt")?;
    }
    assert_eq!(exclusive, observed_work);
    drop(state);
    tracing::subscriber::with_default(
        tracing::subscriber::NoSubscriber::default(),
        || -> TestResult {
            let (mut heap, layout, _, _) = fixture(65)?;
            let mut budget = Budget::new(Limits::default())?;
            heap.create_mask(layout, &[1, 64], &mut budget)?;
            assert_eq!(budget.usage().work, observed_work);
            // The public Debug view exposes the lazy counter without adding a production observer API.
            let identity_is_unassigned =
                format!("{budget:?}").contains("observation: Identity { generation: 0 }");
            assert!(identity_is_unassigned);
            Ok(())
        },
    )
}

/// Owned-loan phases retain one original ledger and prepaid cancellation emits zero new Work.
/// ´test:unit:loan-original-ledger-observation-does-not-change-work´
#[test]
fn loan_original_ledger_observation_does_not_change_work() -> TestResult {
    let _guard = DISPATCHERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (mut heap, mut account, declaration) = loan_fixture()?;
    let state = Arc::new(Mutex::new(State::default()));
    let subscriber = Observer(Arc::clone(&state));
    let observed = tracing::subscriber::with_default(subscriber, || {
        loan_phases(&mut heap, &mut account, declaration)
    })?;
    let state = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(!state.lost);
    assert_eq!(state.active, []);
    assert_eq!(state.receipts.len(), 5);
    let ledger = state
        .receipts
        .first()
        .ok_or("missing receipt")?
        .fields
        .ledger;
    assert_ne!(ledger, 0);
    for receipt in &state.receipts {
        assert_eq!(receipt.fields.family, "matrix.loan");
        assert_eq!(receipt.fields.ledger, ledger);
    }
    let cancellation = state.receipts.last().ok_or("cancellation receipt")?;
    assert_eq!(cancellation.fields.operation, "prepaid cancellation");
    assert_eq!(cancellation.before, cancellation.after);
    assert_eq!(
        state
            .receipts
            .iter()
            .map(|receipt| receipt.after - receipt.before)
            .sum::<u64>(),
        observed
    );
    drop(state);
    tracing::subscriber::with_default(
        tracing::subscriber::NoSubscriber::default(),
        || -> TestResult {
            let (mut heap, mut account, declaration) = loan_fixture()?;
            assert_eq!(loan_phases(&mut heap, &mut account, declaration)?, observed);
            let identity_is_unassigned =
                format!("{account:?}").contains("observation: Identity { generation: 0 }");
            assert!(identity_is_unassigned);
            Ok(())
        },
    )
}
/// Exact declaration work precedes observation of native ownership phases.
fn loan_fixture() -> TestResult<(
    crate::Heap,
    crate::ExecutionAccount,
    crate::HostTileDeclaration<f64>,
)> {
    let heap = crate::Heap::new(crate::HeapLimits::default())?;
    let mut declarations = Budget::new(Limits::default())?;
    let symbols = nrz_bindings::BindingSymbols::declare(
        &nrz_bindings::Descriptor::ratio(),
        [1, 1],
        [false, false],
        &mut declarations,
    )?;
    let axis = heap.create_cell_axis(1, &mut declarations)?;
    let mut budget = Budget::new(Limits::default())?;
    budget.work(
        3,
        nrz_bindings::readings::algebra::Location::new("loan receipt prior usage"),
    )?;
    let mut account = crate::ExecutionAccount::new(budget)?;
    let declaration = crate::HostTileDeclaration::check(
        symbols,
        axis,
        crate::Coverage::RepresentedInputs,
        &mut account,
    )?;
    Ok((heap, account, declaration))
}
/// Payload refusal retains the same owner for prepaid cancellation without constructing a child budget.
fn loan_phases(
    heap: &mut crate::Heap,
    account: &mut crate::ExecutionAccount,
    declaration: crate::HostTileDeclaration<f64>,
) -> TestResult<u64> {
    let before = account.usage().work;
    let mut loan = match heap.request_loan(declaration, account) {
        crate::LoanRequest::Granted(loan) => loan,
        other @ crate::LoanRequest::Refused(_) => return Err(format!("{other:?}").into()),
    };
    loan.write_cell(crate::HostChannel::Center, 0, &[f64::NAN], account)?;
    let loan = match heap.handback(loan, account) {
        crate::HandbackOutcome::Repair(crate::LoanError::InvalidPayload { .. }, loan) => loan,
        other => return Err(format!("{other:?}").into()),
    };
    assert!(matches!(
        heap.cancel_loan(loan, account),
        crate::CancelOutcome::Cancelled(_)
    ));
    Ok(account.usage().work - before)
}

/// Transport records its allocation child on the same ledger without changing literal Work.
/// ´test:unit:mask-transport-ledger-receipts´
#[test]
fn mask_transport_ledger_receipts() -> TestResult {
    let guard = DISPATCHERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (mut heap, layout, _, mut preparation) = fixture(65)?;
    let source = heap.create_mask(layout, &[1, 64], &mut preparation)?;
    let axis = heap.mask(source, layout)?.axis();
    let target = heap.create_cell_layout(
        axis,
        crate::Layout::contiguous(crate::Extent::new(65, 1)?)?,
        &mut preparation,
    )?;
    let state = Arc::new(Mutex::new(State::default()));
    let subscriber = Observer(Arc::clone(&state));
    tracing::subscriber::with_default(subscriber, || -> TestResult {
        let mut budget = Budget::new(Limits::default())?;
        heap.transport_mask(source, target, &mut budget)?;
        // Two authenticators, three occupied slots, two zeroes, two copies, one scan.
        assert_eq!(budget.usage().work, 10);
        Ok(())
    })?;
    drop(guard);
    let mut state = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(!state.lost);
    assert_eq!(state.active, []);
    let receipts = std::mem::take(&mut state.receipts);
    drop(state);
    assert_eq!(receipts.len(), 2);
    let root = receipts.first().ok_or("transport receipt")?;
    let allocation = receipts.get(1).ok_or("allocation receipt")?;
    assert_eq!(root.fields.operation, "mask cell transport");
    assert_eq!(allocation.fields.operation, "mask allocation");
    assert_eq!(allocation.parent, Some(0));
    assert_eq!(root.fields.ledger, allocation.fields.ledger);
    assert_ne!(root.fields.ledger, 0);
    assert_eq!(root.after - root.before, 10);
    assert_eq!(allocation.after - allocation.before, 2);
    Ok(())
}
