// SPDX-License-Identifier: AGPL-3.0-only
//! Observed and unobserved native reuse have identical outcomes and separate exact-work receipts.

use super::{BinaryElementwise, TestResult, admit, candidate, fixture, ready};
use nrz_bindings::Limits;
use std::sync::{Arc, Mutex};
use tracing::{
    Event, Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};

/// Serialize this module's subscriber lifetimes.
static DISPATCHERS: Mutex<()> = Mutex::new(());

/// Only fixed receipt fields enter this test observer.
#[derive(Clone, Debug, Default)]
struct Fields {
    /// Span name distinguishes native entry from exact-work receipts.
    name: String,
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
        metadata.target() == "nrz::work" || metadata.name() == "nrz.kernel"
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
        let mut fields = Fields {
            name: attributes.metadata().name().to_owned(),
            ..Fields::default()
        };
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

/// A two-cell fixture observes one wrapper prepayment, one native interval and two scalar calls.
fn evaluate(function: BinaryElementwise) -> TestResult<(u64, String)> {
    let mut setup = fixture(2, 1, [1; 2], 2, false, function, Limits::default())?;
    let owner = ready(admit(
        &mut setup.heap,
        setup.request,
        function,
        &mut setup.account,
    ))?;
    let before = setup.account.usage().work;
    let owner = candidate(owner, &mut setup.account)?;
    Ok((
        setup.account.usage().work - before,
        format!("{:?}", owner.core.evidence.outcomes),
    ))
}
/// Selected native spans contain no exact arithmetic; scalar and wrapper observations stay disjoint.
fn observe(function: BinaryElementwise) -> TestResult {
    let _guard = DISPATCHERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut setup = fixture(2, 1, [1; 2], 2, false, function, Limits::default())?;
    let owner = ready(admit(
        &mut setup.heap,
        setup.request,
        function,
        &mut setup.account,
    ))?;
    let state = Arc::new(Mutex::new(State::default()));
    let subscriber = Observer(Arc::clone(&state));
    let before = setup.account.usage().work;
    crate::loan::fault::reset_reads();
    crate::loan::fault::reset_scalars();
    crate::loan::fault::reset_stores();
    let owner =
        tracing::subscriber::with_default(subscriber, || candidate(owner, &mut setup.account))?;
    let observed = (
        setup.account.usage().work - before,
        format!("{:?}", owner.core.evidence.outcomes),
    );
    assert_eq!(observed.0, 60);
    assert_eq!(
        [
            crate::loan::fault::reads(),
            crate::loan::fault::scalars(),
            crate::loan::fault::stores()
        ],
        [4, 2, 6]
    );
    let state = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(!state.lost);
    assert_eq!(state.active, [] as [usize; 0]);
    assert_eq!(state.receipts.len(), 4);
    let wrapper = state.receipts.first().ok_or("wrapper")?;
    assert_eq!(wrapper.fields.operation, "pointwise evaluation");
    assert_eq!(wrapper.fields.family, "matrix.kernel");
    assert_eq!(wrapper.after - wrapper.before, 60);
    assert_ne!(wrapper.fields.ledger, 0);
    let native = state.receipts.get(1).ok_or("native traversal")?;
    assert_eq!(
        native.fields.operation,
        match function {
            BinaryElementwise::Product => "product native traversal",
            BinaryElementwise::Quotient => "quotient native traversal",
        }
    );
    assert_eq!(native.fields.ledger, wrapper.fields.ledger);
    assert_eq!(native.after - native.before, 0);
    for scalar in &state.receipts[2..] {
        assert_eq!(scalar.fields.name, "nrz.kernel");
        assert_eq!(
            scalar.fields.family,
            match function {
                BinaryElementwise::Product => "product",
                BinaryElementwise::Quotient => "quotient",
            }
        );
        assert_eq!(scalar.parent, Some(1));
        assert_eq!(scalar.after - scalar.before, 0);
    }
    drop(state);
    let disabled =
        tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
            evaluate(function)
        })?;
    assert_eq!(disabled, observed);
    Ok(())
}

/// Both named native intervals report zero exact Work and preserve the literal wrapper allowance.
#[test]
fn binary_observation_distinguishes_both_members_and_no_native_exact_work() -> TestResult {
    observe(BinaryElementwise::Product)?;
    observe(BinaryElementwise::Quotient)
}
