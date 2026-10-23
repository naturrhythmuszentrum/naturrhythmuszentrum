// SPDX-License-Identifier: AGPL-3.0-only
//! Every new map boundary shares one cumulative ledger and rolls back graph publication.

use crate::testing::common::{TestResult, real};
use crate::testing::map_inverses::{constants, expected, ledger, limits, rational_case};
use crate::testing::readings::{Fixture, map};
use nrz_cas::limits::{Budget, BudgetKind};
use nrz_cas::proof::{Statement, VerifiedWitness};
use nrz_cas::readings::{AffineMap, ExactAffineMap};
use nrz_cas::{ExactConstant, Fault, FaultKind, Handle, Location};

struct Case {
    fixture: Fixture,
    original: ExactAffineMap,
    oracle: ExactAffineMap,
    rational: AffineMap,
    coefficients: Vec<ExactConstant>,
    offsets: Vec<ExactConstant>,
    entries: Vec<Handle>,
    shifts: Vec<Handle>,
    target: nrz_cas::proof::Judgment,
    witness: Option<VerifiedWitness>,
    bytes: Vec<u8>,
}

impl Case {
    fn new(operation: u32) -> TestResult<Self> {
        let (mut fixture, original, oracle) = rational_case(2)?;
        let target = expected(&mut fixture, &original, &oracle)?;
        let coefficients = constants(&["2", "3", "5", "7"])?;
        let offsets = constants(&["11", "13"])?;
        let mut entries = Vec::new();
        let mut shifts = Vec::new();
        for value in &coefficients {
            entries.push(fixture.term(
                nrz_cas::syntax::Operator::Rational(value.as_rational().ok_or("literal")?.clone()),
                real(),
                &[],
            )?);
        }
        for value in &offsets {
            shifts.push(fixture.term(
                nrz_cas::syntax::Operator::Rational(value.as_rational().ok_or("literal")?.clone()),
                real(),
                &[],
            )?);
        }
        let witness = if operation == 6 {
            Some(
                fixture
                    .cas
                    .invert_reading_map(&fixture.context, &original, &mut ledger()?)?
                    .witness()
                    .clone(),
            )
        } else {
            None
        };
        let bytes = if operation == 7 {
            let (mut source, original, _) = rational_case(2)?;
            let inverse =
                source
                    .cas
                    .invert_reading_map(&source.context, &original, &mut ledger()?)?;
            source
                .cas
                .export_witness(inverse.witness(), &mut ledger()?)?
        } else {
            Vec::new()
        };
        Ok(Self {
            fixture,
            original,
            oracle,
            rational: map(2, 2, &[2, 3, 5, 7], &[11, 13])?,
            coefficients,
            offsets,
            entries,
            shifts,
            target,
            witness,
            bytes,
        })
    }

    fn run(&mut self, operation: u32, budget: &mut Budget) -> Result<(), Fault> {
        let cas = &mut self.fixture.cas;
        let context = &self.fixture.context;
        match operation {
            0 => {
                self.rational.inverse(budget)?;
            }
            1 => {
                cas.exact_reading_map(context, (2, 2), &self.entries, &self.shifts, budget)?;
            }
            2 => {
                cas.constant_reading_map(
                    context,
                    (2, 2),
                    &self.coefficients,
                    &self.offsets,
                    budget,
                )?;
            }
            3 => {
                cas.compose_reading_maps(&self.original, &self.oracle, budget)?;
            }
            4 => {
                cas.invert_reading_map(context, &self.original, budget)?;
            }
            5 => {
                let Statement::Equality { left, right, .. } = &self.target.statement else {
                    return Err(invalid());
                };
                cas.prove_reading_equality(context, left, right, budget)?;
            }
            6 => {
                cas.export_witness(self.witness.as_ref().ok_or_else(invalid)?, budget)?;
            }
            7 => {
                cas.replay(&self.bytes, &self.target, budget)?;
            }
            _ => return Err(invalid()),
        }
        Ok(())
    }
}

const fn invalid() -> Fault {
    Fault::invalid(
        nrz_cas::InvalidCode::Reference,
        Location::new("map boundary fixture"),
    )
}

fn bounded(kind: BudgetKind, limit: u64) -> Result<Budget, Fault> {
    let mut policy = limits();
    match kind {
        BudgetKind::Work => policy.work = limit,
        BudgetKind::StagedPayloadBytes => policy.staged_payload_bytes = limit,
        BudgetKind::PayloadBytes => policy.payload_bytes = limit,
        BudgetKind::OutputBytes => policy.output_bytes = limit,
        BudgetKind::WitnessBytes => policy.witness_bytes = limit,
        _ => return Err(invalid()),
    }
    Budget::new(policy)
}

fn capacity(operation: u32, kind: BudgetKind) -> TestResult<u64> {
    let mut low = 0;
    let mut high = match kind {
        BudgetKind::OutputBytes => limits().output_bytes,
        BudgetKind::WitnessBytes => limits().witness_bytes,
        _ => limits().staged_payload_bytes,
    };
    while low < high {
        let middle = low + (high - low) / 2;
        let mut case = Case::new(operation)?;
        match case.run(operation, &mut bounded(kind, middle)?) {
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

fn boundary(operation: u32) -> TestResult {
    let mut case = Case::new(operation)?;
    let mut measured = ledger()?;
    case.run(operation, &mut measured)?;
    let work = measured.usage().work;
    let kinds = if operation == 6 {
        [BudgetKind::OutputBytes, BudgetKind::WitnessBytes]
    } else {
        [BudgetKind::StagedPayloadBytes, BudgetKind::PayloadBytes]
    };
    let first = capacity(operation, kinds[0])?;
    let second = capacity(operation, kinds[1])?;
    let mut measurements = vec![
        (BudgetKind::Work, work),
        (kinds[0], first),
        (kinds[1], second),
    ];
    if operation == 0 || operation == 4 {
        measurements.push((
            BudgetKind::OutputBytes,
            capacity(operation, BudgetKind::OutputBytes)?,
        ));
        measurements.push((
            BudgetKind::WitnessBytes,
            capacity(operation, BudgetKind::WitnessBytes)?,
        ));
    }
    for (kind, needed) in measurements {
        assert!(needed > 0);
        Case::new(operation)?.run(operation, &mut bounded(kind, needed)?)?;
        let mut case = Case::new(operation)?;
        let before = case.fixture.cas.counts();
        let result = case.run(operation, &mut bounded(kind, needed - 1)?);
        assert!(
            matches!(result, Err(Fault { kind: FaultKind::Exhausted { budget: actual, .. }, .. }) if actual == kind)
        );
        assert_eq!(case.fixture.cas.counts(), before);
    }
    let mut cumulative = bounded(BudgetKind::Work, work)?;
    cumulative.work(1, Location::new("earlier map work"))?;
    assert!(matches!(
        Case::new(operation)?.run(operation, &mut cumulative),
        Err(Fault {
            kind: FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            },
            ..
        })
    ));
    Ok(())
}

/// Rational specialization, symbolic construction, constant import and composition charge exact ceilings.
/// ´test:integration:map-construction-work-and-payload-boundaries´
#[test]
fn map_construction_work_and_payload_boundaries() -> TestResult {
    for operation in 0..4 {
        boundary(operation)?;
    }
    Ok(())
}

/// Inversion, checking, export and fresh replay retain spent Work and publish nothing on failure.
/// ´test:integration:map-proof-work-and-payload-boundaries´
#[test]
fn map_proof_work_and_payload_boundaries() -> TestResult {
    for operation in 4..8 {
        boundary(operation)?;
    }
    Ok(())
}

/// Record complete Work and minimum storage capacities for the independent general-map fixture.
/// ´test:integration:map-resource-measurements´
#[test]
fn map_resource_measurements() -> TestResult {
    let mut rows = Vec::new();
    for operation in 0..8 {
        let mut measured = ledger()?;
        Case::new(operation)?.run(operation, &mut measured)?;
        let usage = measured.usage();
        let (staged, payload) = if operation == 6 {
            (0, 0)
        } else {
            (
                capacity(operation, BudgetKind::StagedPayloadBytes)?,
                capacity(operation, BudgetKind::PayloadBytes)?,
            )
        };
        let (output, witness) = if matches!(operation, 0 | 4 | 6) {
            (
                capacity(operation, BudgetKind::OutputBytes)?,
                capacity(operation, BudgetKind::WitnessBytes)?,
            )
        } else {
            (0, 0)
        };
        rows.push([
            u64::from(operation),
            usage.work,
            usage.arithmetic_steps,
            usage.condition_steps,
            staged,
            payload,
            output,
            witness,
        ]);
    }
    assert_eq!(
        rows,
        [
            [0, 408_941, 3_420, 69, 84_798, 28_242, 2_956, 1_247],
            [1, 3_390, 4, 0, 2_910, 147, 0, 0],
            [2, 9_796, 16, 0, 12_668, 147, 0, 0],
            [3, 1_110, 0, 0, 753, 68, 0, 0],
            [4, 344_458, 2_932, 69, 88_548, 28_242, 2_424, 1_247],
            [5, 177_809, 1_516, 34, 33_024, 12_104, 0, 0],
            [6, 1_247, 0, 0, 0, 0, 1_247, 1_247],
            [7, 240_505, 1_600, 32, 33_024, 12_127, 0, 0],
        ]
    );
    Ok(())
}
