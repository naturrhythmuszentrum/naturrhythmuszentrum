// SPDX-License-Identifier: AGPL-3.0-only
//! Each inventory and write form has exact caller-ledger and simultaneous-payload boundaries.

use crate::testing::common::TestResult;
use crate::testing::readings::{ReadingCase, ledger, state_case};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::proof::VerifiedWitness;
use nrz_cas::readings::{DependencyInventory, EntryIndex, SelectionSpec};
use nrz_cas::{Fault, FaultKind, Handle, Location};

struct Case {
    reading: ReadingCase,
    witness: VerifiedWitness,
    inventory: DependencyInventory,
    base: Handle,
    write: Handle,
}

impl Case {
    fn new() -> TestResult<Self> {
        let mut reading = state_case(0)?;
        let witness = reading.prove(&mut ledger()?)?;
        let inventory = reading
            .fixture
            .cas
            .reading_dependencies(&reading.left, &mut ledger()?)?;
        let base = reading.fixture.symbol("z")?;
        let write = reading.fixture.cas.select_entries(
            &base,
            &SelectionSpec::Entry(EntryIndex { row: 0, column: 0 }),
            &mut ledger()?,
        )?;
        Ok(Self {
            reading,
            witness,
            inventory,
            base,
            write,
        })
    }

    fn run(&self, operation: u32, budget: &mut Budget) -> Result<(), Fault> {
        let cas = &self.reading.fixture.cas;
        match operation {
            0 => cas
                .reading_dependencies(&self.reading.left, budget)
                .map(|_| ()),
            1 => cas.witness_dependencies(&self.witness, budget).map(|_| ()),
            2 => cas
                .dependency_selection_intersection(&self.inventory, &self.write, budget)
                .map(|_| ()),
            3 => cas
                .dependency_entry_intersection(
                    &self.inventory,
                    &self.base,
                    &[EntryIndex { row: 0, column: 0 }],
                    budget,
                )
                .map(|_| ()),
            _ => Err(Fault::invalid(
                nrz_cas::InvalidCode::Reference,
                Location::new("inventory boundary"),
            )),
        }
    }
}

fn limits(kind: BudgetKind, limit: u64) -> TestResult<Budget> {
    let mut limits = Limits::default();
    match kind {
        BudgetKind::Work => limits.work = limit,
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = limit,
        BudgetKind::OutputBytes => limits.output_bytes = limit,
        _ => return Err("unexpected inventory budget".into()),
    }
    Ok(Budget::new(limits)?)
}

fn payload_boundary(case: &Case, operation: u32, kind: BudgetKind) -> TestResult<u64> {
    let mut low = 0;
    let mut high = match kind {
        BudgetKind::StagedPayloadBytes => Limits::default().staged_payload_bytes,
        BudgetKind::OutputBytes => Limits::default().output_bytes,
        _ => return Err("unexpected payload budget".into()),
    };
    while low < high {
        let middle = low + (high - low) / 2;
        match case.run(operation, &mut limits(kind, middle)?) {
            Ok(()) => high = middle,
            Err(Fault {
                kind: FaultKind::Exhausted { budget: actual, .. },
                ..
            }) if actual == kind => low = middle + 1,
            Err(fault) => return Err(fault.into()),
        }
    }
    Ok(high)
}

fn boundaries(operation: u32) -> TestResult {
    let case = Case::new()?;
    let before = case.reading.fixture.cas.counts();
    let mut measured = ledger()?;
    case.run(operation, &mut measured)?;
    let work = measured.usage().work;
    let staged = payload_boundary(&case, operation, BudgetKind::StagedPayloadBytes)?;
    let output = payload_boundary(&case, operation, BudgetKind::OutputBytes)?;
    for (kind, needed) in [
        (BudgetKind::Work, work),
        (BudgetKind::StagedPayloadBytes, staged),
        (BudgetKind::OutputBytes, output),
    ] {
        assert!(needed > 0);
        case.run(operation, &mut limits(kind, needed)?)?;
        let refusal = case.run(operation, &mut limits(kind, needed - 1)?);
        assert!(
            matches!(refusal, Err(Fault { kind: FaultKind::Exhausted { budget: actual, .. }, .. }) if actual == kind)
        );
        assert_eq!(case.reading.fixture.cas.counts(), before);
    }
    let mut cumulative = limits(BudgetKind::Work, work)?;
    cumulative.work(1, Location::new("earlier inventory work"))?;
    assert!(matches!(
        case.run(operation, &mut cumulative),
        Err(Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            },
            ..
        })
    ));
    assert_eq!(case.reading.fixture.cas.counts(), before);
    Ok(())
}

/// Reading closure extraction succeeds at each exact ceiling and refuses one unit below it.
/// ´test:integration:dependency-reading-work-and-payload-boundaries´
#[test]
fn dependency_reading_work_and_payload_boundaries() -> TestResult {
    boundaries(0)
}

/// Witness extraction charges authentication, canonical closure and all dependency projections.
/// ´test:integration:dependency-witness-work-and-payload-boundaries´
#[test]
fn dependency_witness_work_and_payload_boundaries() -> TestResult {
    boundaries(1)
}

/// An authenticated selection write shares the caller's cumulative ledger and output capacity.
/// ´test:integration:dependency-selection-write-work-and-payload-boundaries´
#[test]
fn dependency_selection_write_work_and_payload_boundaries() -> TestResult {
    boundaries(2)
}

/// A raw write list prepays validation and intersection with exact payload boundaries.
/// ´test:integration:dependency-entry-write-work-and-payload-boundaries´
#[test]
fn dependency_entry_write_work_and_payload_boundaries() -> TestResult {
    boundaries(3)
}
