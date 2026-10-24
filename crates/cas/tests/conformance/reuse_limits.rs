// SPDX-License-Identifier: AGPL-3.0-only
//! Reuse extraction and local authentication have cumulative Work and payload boundaries.

use crate::testing::common::{TestResult, real};
use crate::testing::map_inverses::{fixture as map_fixture, ledger, limits};
use crate::testing::readings::Fixture;
use nrz_cas::limits::{Budget, BudgetKind};
use nrz_cas::proof::{Primitive, VerifiedWitness};
use nrz_cas::readings::ReuseKey;
use nrz_cas::syntax::Operator;
use nrz_cas::{Fault, FaultKind, Handle, Location, Rational};

struct Case {
    fixture: Fixture,
    expression: Handle,
    witness: Option<VerifiedWitness>,
    key: Option<ReuseKey>,
}

impl Case {
    fn new(operation: u32) -> TestResult<Self> {
        let mut fixture = map_fixture(&["p"])?;
        let parameter = fixture.symbol("p")?;
        let one = fixture.term(Operator::Rational(Rational::one()), real(), &[])?;
        let expression = fixture.term(Operator::Div, real(), &[one, parameter])?;
        let witness = if operation == 0 {
            None
        } else {
            let domain = fixture.cas.admissible_region(&fixture.context)?;
            let step = crate::testing::witness::equality(
                &fixture.context,
                &domain,
                Primitive::Reflexivity,
                &expression,
                &expression,
            );
            Some(
                fixture
                    .cas
                    .check_proof(&fixture.context, &[step], &mut ledger()?)?,
            )
        };
        let key = if operation == 2 {
            Some(
                fixture
                    .cas
                    .witness_reuse_key(witness.as_ref().ok_or("witness")?, &mut ledger()?)?,
            )
        } else {
            None
        };
        Ok(Self {
            fixture,
            expression,
            witness,
            key,
        })
    }

    fn run(&mut self, operation: u32, budget: &mut Budget) -> Result<(), Fault> {
        match operation {
            0 => {
                self.fixture.cas.expression_reuse_key(
                    &self.fixture.context,
                    &self.expression,
                    budget,
                )?;
            }
            1 => {
                self.fixture
                    .cas
                    .witness_reuse_key(self.witness.as_ref().ok_or_else(invalid)?, budget)?;
            }
            2 => {
                self.fixture
                    .cas
                    .validate_reuse_key(self.key.as_ref().ok_or_else(invalid)?, budget)?;
            }
            _ => return Err(invalid()),
        }
        Ok(())
    }
}

const fn invalid() -> Fault {
    Fault::invalid(
        nrz_cas::InvalidCode::Reference,
        Location::new("reuse boundary fixture"),
    )
}

fn bounded(kind: BudgetKind, limit: u64) -> Result<Budget, Fault> {
    let mut policy = limits();
    match kind {
        BudgetKind::Work => policy.work = limit,
        BudgetKind::StagedPayloadBytes => policy.staged_payload_bytes = limit,
        BudgetKind::OutputBytes => policy.output_bytes = limit,
        BudgetKind::PayloadBytes => policy.payload_bytes = limit,
        _ => return Err(invalid()),
    }
    Budget::new(policy)
}

fn capacity(operation: u32, kind: BudgetKind) -> TestResult<u64> {
    let mut low = 0;
    let mut high = if kind == BudgetKind::OutputBytes {
        limits().output_bytes
    } else {
        limits().staged_payload_bytes
    };
    while low < high {
        let middle = low + (high - low) / 2;
        match Case::new(operation)?.run(operation, &mut bounded(kind, middle)?) {
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

/// Every key form succeeds at N and refuses at N minus one without publishing a partial identity.
/// ´test:integration:reuse-key-work-and-payload-boundaries´
#[test]
fn reuse_key_work_and_payload_boundaries() -> TestResult {
    for operation in 0..3 {
        let mut measured = ledger()?;
        Case::new(operation)?.run(operation, &mut measured)?;
        let work = measured.usage().work;
        let staged = capacity(operation, BudgetKind::StagedPayloadBytes)?;
        let output = capacity(operation, BudgetKind::OutputBytes)?;
        let payload = capacity(operation, BudgetKind::PayloadBytes)?;
        for (kind, needed) in [
            (BudgetKind::Work, work),
            (BudgetKind::StagedPayloadBytes, staged),
            (BudgetKind::OutputBytes, output),
            (BudgetKind::PayloadBytes, payload),
        ] {
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
        cumulative.work(1, Location::new("earlier reuse work"))?;
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
    }
    Ok(())
}

/// Record complete Work and storage capacities for guarded reciprocal reuse.
/// ´test:integration:reuse-resource-measurements´
#[test]
fn reuse_resource_measurements() -> TestResult {
    let mut rows = Vec::new();
    for operation in 0..3 {
        let mut measured = ledger()?;
        Case::new(operation)?.run(operation, &mut measured)?;
        let usage = measured.usage();
        rows.push([
            u64::from(operation),
            usage.work,
            usage.arithmetic_steps,
            usage.condition_steps,
            capacity(operation, BudgetKind::StagedPayloadBytes)?,
            capacity(operation, BudgetKind::PayloadBytes)?,
            capacity(operation, BudgetKind::OutputBytes)?,
        ]);
    }
    assert_eq!(
        rows,
        [
            [0, 64_156, 3, 30, 12_304, 4_833, 3_179],
            [1, 41_977, 2, 8, 11_984, 110, 3_179],
            [2, 28_003, 1, 4, 14_238, 110, 3_179],
        ]
    );
    Ok(())
}
