// SPDX-License-Identifier: AGPL-3.0-only
//! Live estimates are observations; only receipt and fixed-row invariants are checked.

use super::BenchResult;
use nrz_matrix::sampling::{
    AnalysisReport, IterationOutcome, ReceiptStatus, SamplingReport, SamplingStop, WorkReceipt,
    analyze,
};
use std::{fmt::Debug, io::Write};

/// Export returned data explicitly, after the silent sampling interval has ended.
pub(super) fn export<E: Copy + Debug>(
    phase: &str,
    report: &SamplingReport<'_, u64, E>,
) -> BenchResult {
    let mut output = std::io::stdout().lock();
    writeln!(
        output,
        "host export {phase}: operation={}; stop={:?}; seed={}; rows={}; iterations={}; warm_up={}; step={:?}; measured={:?}; total={:?}",
        report.identity().representative,
        report.stop(),
        report.settings().analysis.seed(),
        report.rows().len(),
        report.iterations().len(),
        report.warm_up_iterations(),
        report.linear_step(),
        report.measured_elapsed(),
        report.total_elapsed()
    )?;
    if let Some(analysis) = report.analysis() {
        writeln!(
            output,
            "host export {phase}: slope_seconds={:?}; mean_seconds={:?}; throughput={:?}; serial_1000={:?}; U={:?}; C={:?}",
            analysis.estimates().slope,
            analysis.estimates().mean,
            analysis.throughput(),
            report.extrapolate_batch(1_000)?,
            report.uninterrupted_bound(),
            report.finalization_bound()
        )?;
    }
    Ok(())
}

/// Fixed rows and explicit seed isolate statistical state from live timing noise.
pub(super) fn replay<E>(report: &SamplingReport<'_, u64, E>) -> BenchResult<AnalysisReport> {
    Ok(analyze(
        report.rows(),
        report.settings().analysis,
        report.elements_per_item(),
        |_| false,
    )?)
}

/// Check every receipt independently against the native operation's zero-Work contract.
fn validate<E: Copy + Debug + Eq>(report: &SamplingReport<'_, u64, E>) -> BenchResult {
    if report.stop() != &SamplingStop::Complete {
        return Err(format!("incomplete host observation: {:?}", report.stop()).into());
    }
    let expected = Some(WorkReceipt {
        exit: Some(0),
        operation: Some(0),
        status: ReceiptStatus::Complete,
    });
    for record in report.iterations() {
        let returned = record.outcome == IterationOutcome::Returned(Ok(()));
        let entry = record.entry == Some(0);
        let receipt = record.receipt == expected;
        let disposed = record.disposal == Some(Ok(()));
        let cleaned = record.cleanup == Some(Ok(()));
        if !(returned && entry && receipt && disposed && cleaned) {
            return Err(format!("unexpected native receipt: {record:?}").into());
        }
    }
    Ok(())
}

/// Compare discrete report fields and replay original rows after Criterion has run.
pub(super) fn compare<E: Copy + Debug + Eq>(
    before: &SamplingReport<'_, u64, E>,
    after: &SamplingReport<'_, u64, E>,
    fixed: &AnalysisReport,
) -> BenchResult {
    validate(before)?;
    validate(after)?;
    let same_policy = before.settings() == after.settings();
    let same_rows = before.rows().len() == after.rows().len();
    let same_iterations = before.iterations().len() == after.iterations().len();
    let same_warm_up = before.warm_up_iterations() == after.warm_up_iterations();
    if !(same_policy && same_rows && same_iterations && same_warm_up) {
        return Err("Criterion coexistence changed deterministic sampling fields".into());
    }
    for (left, right) in before.rows().iter().zip(after.rows()) {
        if left.iterations != right.iterations {
            return Err("Criterion coexistence changed the linear schedule".into());
        }
    }
    for (left, right) in before.iterations().iter().zip(after.iterations()) {
        let same_position = left.position == right.position;
        let same_entry = left.entry == right.entry;
        let same_receipt = left.receipt == right.receipt;
        if !(same_position && same_entry && same_receipt) {
            return Err("Criterion coexistence changed a Work receipt".into());
        }
    }
    let replayed = replay(before)?;
    if fixed.estimates() != replayed.estimates() {
        return Err("Criterion coexistence changed seeded fixed-row estimates".into());
    }
    writeln!(
        std::io::stdout().lock(),
        "coexistence verified: operation={}; seed={}; rows={}; receipts={}; exact_Work=0; fixed_row_replay=identical",
        before.identity().representative,
        before.settings().analysis.seed(),
        before.rows().len(),
        before.iterations().len()
    )?;
    Ok(())
}
