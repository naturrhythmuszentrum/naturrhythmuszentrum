// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical export advertises and reserves the exact copy before allocating.

use crate::testing::common::TestResult;
use crate::testing::proofs::cancellation;
use crate::testing::readings::{Fixture, ledger, matrix};
use crate::testing::witness::equality;
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::proof::{Primitive, VerifiedWitness};
use nrz_cas::syntax::Operator;
use nrz_cas::{Cas, Fault, FaultKind, Location};

const AT: Location = Location::new("witness export test");

fn empty() -> TestResult<(Cas, VerifiedWitness)> {
    let mut fixture = Fixture::new(&[])?;
    let value = fixture.term(
        Operator::Matrix {
            rows: 0,
            columns: u32::MAX,
        },
        matrix(0, u32::MAX),
        &[],
    )?;
    let domain = fixture.cas.admissible_region(&fixture.context)?;
    let step = equality(
        &fixture.context,
        &domain,
        Primitive::Reflexivity,
        &value,
        &value,
    );
    let witness = fixture
        .cas
        .check_proof(&fixture.context, &[step], &mut ledger()?)?;
    Ok((fixture.cas, witness))
}

fn conditional(reflexive: bool) -> TestResult<(Cas, VerifiedWitness)> {
    let (mut fixture, _, mut step) = cancellation()?;
    if reflexive {
        step = equality(
            &fixture.context,
            &fixture.domain,
            Primitive::Reflexivity,
            &fixture.x,
            &fixture.x,
        );
    }
    let witness = fixture
        .cas
        .check_proof(&fixture.context, &[step], &mut ledger()?)?;
    Ok((fixture.cas, witness))
}

/// Exact reported lengths match actual copies for scalar, conditional and zero-entry shapes.
/// ´test:integration:witness-export-exact-lengths´
#[test]
fn witness_export_exact_lengths() -> TestResult {
    for (cas, witness) in [conditional(true)?, conditional(false)?, empty()?] {
        let before = cas.counts();
        let size = witness.exported_byte_len();
        let mut budget = ledger()?;
        let bytes = cas.export_witness(&witness, &mut budget)?;
        assert_eq!(size, bytes.len());
        assert_eq!(budget.usage().work, u64::try_from(size)?);
        assert_eq!(cas.counts(), before);
        let mut receiver = Cas::new(Limits::default())?;
        let imported = receiver.inspect(&bytes, &mut ledger()?)?;
        assert_eq!(imported.exported_byte_len(), size);
        assert_eq!(receiver.export_witness(&imported, &mut ledger()?)?, bytes);
    }
    Ok(())
}

fn restricted(kind: BudgetKind, limit: u64) -> Result<Budget, Fault> {
    let mut limits = Limits::default();
    match kind {
        BudgetKind::Work => limits.work = limit,
        BudgetKind::StagedPayloadBytes => limits.staged_payload_bytes = limit,
        BudgetKind::WitnessBytes => limits.witness_bytes = limit,
        BudgetKind::OutputBytes => limits.output_bytes = limit,
        _ => return Err(Fault::invalid(nrz_cas::InvalidCode::Policy, AT)),
    }
    Budget::new(limits)
}

/// Every export ceiling has an exact N boundary; refusals precede allocation and ledger changes.
/// ´test:integration:witness-export-exact-boundaries´
#[test]
fn witness_export_exact_boundaries() -> TestResult {
    let (cas, witness) = conditional(false)?;
    let count = u64::try_from(witness.exported_byte_len())?;
    for kind in [
        BudgetKind::Work,
        BudgetKind::StagedPayloadBytes,
        BudgetKind::WitnessBytes,
        BudgetKind::OutputBytes,
    ] {
        let mut exact = restricted(kind, count)?;
        assert_eq!(
            cas.export_witness(&witness, &mut exact)?.len(),
            witness.exported_byte_len()
        );
        let mut short = restricted(kind, count - 1)?;
        let limits = short.limits().clone();
        let usage = short.usage();
        let fault = cas
            .export_witness(&witness, &mut short)
            .err()
            .ok_or("short export accepted")?;
        assert!(
            matches!(fault.kind, FaultKind::Exhausted { budget, needed, .. } if budget == kind && needed == count)
        );
        assert_eq!(short.usage(), usage);
        assert_eq!(short.limits(), &limits);
    }
    Ok(())
}

/// Retained copies and prior Work share the same allowance, and foreign proofs remain invalid.
/// ´test:integration:witness-export-retained-and-cumulative´
#[test]
fn witness_export_retained_and_cumulative() -> TestResult {
    let (cas, witness) = empty()?;
    let count = u64::try_from(witness.exported_byte_len())?;
    let mut exact = restricted(BudgetKind::StagedPayloadBytes, count * 2)?;
    let bytes =
        exact.with_retained_payload(count, AT, |budget| cas.export_witness(&witness, budget))?;
    assert_eq!(bytes.len(), witness.exported_byte_len());
    let mut short = restricted(BudgetKind::StagedPayloadBytes, count * 2 - 1)?;
    let usage = short.usage();
    assert!(
        short
            .with_retained_payload(count, AT, |budget| cas.export_witness(&witness, budget))
            .is_err()
    );
    assert_eq!(short.usage(), usage);
    let mut cumulative = restricted(BudgetKind::Work, count)?;
    cumulative.work(1, AT)?;
    assert!(cas.export_witness(&witness, &mut cumulative).is_err());
    assert_eq!(cumulative.usage().work, 1);
    let receiver = Cas::new(Limits::default())?;
    let mut budget = ledger()?;
    let usage = budget.usage();
    assert!(receiver.export_witness(&witness, &mut budget).is_err());
    assert_eq!(budget.usage(), usage);
    Ok(())
}
