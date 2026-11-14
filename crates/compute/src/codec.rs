// SPDX-License-Identifier: AGPL-3.0-only
//! Strict current-schema codecs and deterministic canonical bytes.

mod pilot;

mod recipes;

use crate::{
    decimal::{self, MAX_INTEGER, Rounding},
    report::{CaseOutcome, Counts, Observations, RunReport},
    summary::{SummaryDataset, SummaryOutcome},
};
use std::io::{Read, Write};

/// Strict schema, numeric, canonicalization or I/O refusal.
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    /// Owned current-schema data is structurally inconsistent.
    #[error("invalid report: {0}")]
    Invalid(&'static str),
    /// JSON syntax, duplicate fields, unknown fields or malformed variants.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Exact source/grid validation failed.
    #[error(transparent)]
    Decimal(#[from] decimal::DecimalError),
    /// Caller-owned reader or writer failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Common current envelope validation.
fn envelope(
    namespace: &str,
    expected: &str,
    version: [u64; 3],
    owner: &str,
    license: &str,
) -> Result<(), CodecError> {
    if namespace != expected || version != [1, 1, 0] || owner != "NRZ" || license != "CC0-1.0" {
        return Err(CodecError::Invalid("envelope"));
    }
    Ok(())
}

/// Validate all typed integer fields without any floating-point JSON conversion.
fn integer_tree(value: &serde_json::Value) -> Result<(), CodecError> {
    match value {
        serde_json::Value::Number(number) => {
            let bounded = number.as_u64().is_some_and(|n| n <= MAX_INTEGER)
                || number
                    .as_i64()
                    .is_some_and(|n| n.unsigned_abs() <= MAX_INTEGER);
            if !bounded {
                return Err(CodecError::Invalid("integer representation"));
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                integer_tree(value)?;
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values() {
                integer_tree(value)?;
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::String(_) => {}
    }
    Ok(())
}

/// Validate attempt partitions without treating outliers as discarded items.
fn counts(counts: Counts) -> Result<(), CodecError> {
    let attempted = counts
        .pilot
        .checked_add(counts.warm_up)
        .and_then(|n| n.checked_add(counts.measurement));
    let partition = counts.measured.checked_add(counts.discarded);
    if attempted != Some(counts.attempted) || partition != Some(counts.attempted) {
        return Err(CodecError::Invalid("attempt partitions"));
    }
    Ok(())
}

/// Validate compact chronological observations against their row sums.
fn observations(raw: &Observations) -> Result<(), CodecError> {
    counts(raw.counts)?;
    let attempts = u64::try_from(raw.item_ns.len()).map_err(|_| CodecError::Invalid("attempts"))?;
    if raw.counts.warm_up.checked_add(raw.counts.measurement) != Some(attempts) {
        return Err(CodecError::Invalid("item count"));
    }
    let pilot = raw.pilot.empty_ns.len()
        + raw
            .pilot
            .batches
            .iter()
            .map(|batch| batch.elapsed_ns.len())
            .sum::<usize>();
    if u64::try_from(pilot + raw.pilot.failures.len()).ok() != Some(raw.counts.pilot)
        || raw.pilot.attempted != raw.counts.pilot
    {
        return Err(CodecError::Invalid("pilot count"));
    }
    let mut previous = None;
    for exception in &raw.exceptions {
        let ordered = previous.is_none_or(|index| exception.attempt > index);
        if exception.attempt >= attempts || !ordered {
            return Err(CodecError::Invalid("exception index"));
        }
        let index = usize::try_from(exception.attempt)
            .map_err(|_| CodecError::Invalid("exception index"))?;
        let invoked = matches!(
            exception.outcome,
            crate::sampling_evidence::IterationOutcome::Returned(_)
        );
        if invoked != raw.item_ns[index].is_some() {
            return Err(CodecError::Invalid("exception timing"));
        }
        previous = Some(exception.attempt);
    }
    for (index, item) in raw.item_ns.iter().enumerate() {
        let index = u64::try_from(index).map_err(|_| CodecError::Invalid("attempt index"))?;
        if item.is_none()
            && raw
                .exceptions
                .binary_search_by_key(&index, |entry| entry.attempt)
                .is_err()
        {
            return Err(CodecError::Invalid("untimed attempt lacks outcome"));
        }
    }
    timed_phases(raw)?;
    let mut offset =
        usize::try_from(raw.counts.warm_up).map_err(|_| CodecError::Invalid("warm-up count"))?;
    let mut measured = 0_u64;
    for (index, row) in raw.rows.iter().enumerate() {
        let row_index = u64::try_from(index).map_err(|_| CodecError::Invalid("row index"))?;
        let planned = raw
            .linear_step
            .and_then(|step| step.checked_mul(row_index + 1));
        if planned != Some(row.iterations) || row.iterations == 0 || row.elapsed_ns == 0 {
            return Err(CodecError::Invalid("linear row"));
        }
        let end = offset
            .checked_add(
                usize::try_from(row.iterations).map_err(|_| CodecError::Invalid("row count"))?,
            )
            .ok_or(CodecError::Invalid("row count"))?;
        let items = raw
            .item_ns
            .get(offset..end)
            .ok_or(CodecError::Invalid("row extent"))?;
        if raw.exceptions.iter().any(|exception| {
            usize::try_from(exception.attempt).is_ok_and(|index| (offset..end).contains(&index))
        }) {
            return Err(CodecError::Invalid("failed item in complete row"));
        }
        let sum = items
            .iter()
            .try_fold(0_u64, |sum, value| sum.checked_add((*value)?));
        if sum != Some(row.elapsed_ns) {
            return Err(CodecError::Invalid("row sum"));
        }
        let seconds = std::time::Duration::from_nanos(row.elapsed_ns).as_secs_f64();
        if seconds.to_bits() != row.seconds.source_bits.0 {
            return Err(CodecError::Invalid("row seconds conversion"));
        }
        row.seconds.validate(-9, Rounding::Nearest)?;
        offset = end;
        measured = measured
            .checked_add(row.iterations)
            .ok_or(CodecError::Invalid("measured count"))?;
    }
    if measured != raw.counts.measured
        || u64::try_from(raw.rows.len()).ok() != Some(raw.counts.rows)
    {
        return Err(CodecError::Invalid("measured rows"));
    }
    Ok(())
}

/// Exact timed phase totals include failed timed items and unfinished rows.
fn timed_phases(raw: &Observations) -> Result<(), CodecError> {
    let warm =
        usize::try_from(raw.counts.warm_up).map_err(|_| CodecError::Invalid("warm-up count"))?;
    let before = raw
        .item_ns
        .get(..warm)
        .ok_or(CodecError::Invalid("warm-up extent"))?;
    let after = raw
        .item_ns
        .get(warm..)
        .ok_or(CodecError::Invalid("measurement extent"))?;
    let sum = |items: &[Option<u64>]| {
        items
            .iter()
            .flatten()
            .try_fold(0_u64, |n, time| n.checked_add(*time))
    };
    if sum(before) != Some(raw.phases.warm_up_ns) || sum(after) != Some(raw.phases.measurement_ns) {
        return Err(CodecError::Invalid("timed phase totals"));
    }
    Ok(())
}

/// Phase time cannot exceed elapsed time, and overshoot is an exact integer difference.
fn execution(execution: &crate::report::Execution, budget_ns: u64) -> Result<(), CodecError> {
    use crate::report::Reason as Stop;
    if !matches!(
        execution.stop_reason,
        None | Some(Stop::Clock | Stop::Capacity | Stop::Worker | Stop::Deadline | Stop::Cancelled)
    ) {
        return Err(CodecError::Invalid("invalid run stop reason"));
    }
    let timing = (
        execution.phases,
        execution.elapsed_ns,
        execution.overshoot_ns,
    );
    let (phases, elapsed, overshoot) = match timing {
        (Some(phases), Some(elapsed), Some(overshoot)) => (phases, elapsed, overshoot),
        (None, None, None) if execution.stop_reason == Some(Stop::Clock) && !execution.complete => {
            return Ok(());
        }
        _ => {
            return Err(CodecError::Invalid(
                "unavailable execution timing without clock stop",
            ));
        }
    };
    let total = [
        phases.validation_ns,
        phases.preparation_ns,
        phases.pilot_ns,
        phases.warm_up_ns,
        phases.measurement_ns,
        phases.finalization_ns,
        phases.reset_ns,
        phases.harness_ns,
    ]
    .into_iter()
    .try_fold(0_u64, u64::checked_add);
    if total.is_none_or(|total| total > elapsed) || overshoot != elapsed.saturating_sub(budget_ns) {
        return Err(CodecError::Invalid("execution duration ledger"));
    }
    Ok(())
}

/// Known inspection claims require an inspected artifact and valid full content identity.
fn lowering(record: &crate::LoweringRecord) -> Result<(), CodecError> {
    use crate::Evidence;
    let inspected = record.inspected == Evidence::Known(true);
    if let Evidence::Known(artifact) = &record.artifact
        && (!artifact.is_valid() || !inspected)
    {
        return Err(CodecError::Invalid("lowering artifact"));
    }
    let known = matches!(record.symbols, Evidence::Known(_))
        || matches!(record.scalar_vectorization, Evidence::Known(_))
        || matches!(record.splitting, Evidence::Known(_))
        || matches!(record.helpers, Evidence::Known(_))
        || matches!(record.native_fma, Evidence::Known(_));
    if known && !inspected {
        return Err(CodecError::Invalid("uninspected lowering claim"));
    }
    if let Evidence::Known(symbols) = &record.symbols
        && (symbols.is_empty() || symbols.iter().any(String::is_empty))
    {
        return Err(CodecError::Invalid("lowering symbols"));
    }
    Ok(())
}

/// Validate the current raw schema, exact views and reconstruction invariants.
/// # Errors
/// Rejects malformed identities, counts, source/grid disagreements and inconsistent outcomes.
pub fn validate_report(raw: &RunReport) -> Result<(), CodecError> {
    envelope(
        &raw.namespace,
        "org.naturrhythmuszentrum.compute.raw",
        raw.version,
        &raw.owner,
        &raw.license,
    )?;
    integer_tree(&serde_json::to_value(raw)?)?;
    let payload = &raw.payload;
    lowering(&payload.build.lowering)?;
    lowering(&payload.publication.build.lowering)?;
    if !public_projection(
        &serde_json::to_value(&payload.build)?,
        &serde_json::to_value(&payload.publication.build)?,
    ) || !public_projection(
        &serde_json::to_value(&payload.environment)?,
        &serde_json::to_value(&payload.publication.environment)?,
    ) {
        return Err(CodecError::Invalid("invented public evidence"));
    }
    if let crate::Evidence::Known(attachment) = &payload.identity.source_attachment
        && !source_attachment(attachment)
    {
        return Err(CodecError::Invalid("source attachment"));
    }
    let request = &payload.request;
    execution(&payload.execution, request.resources.budget_ns)?;
    request
        .validate(payload.execution.bounds)
        .map_err(|_| CodecError::Invalid("request"))?;
    let expected = request
        .cases
        .len()
        .checked_mul(request.seeds.len())
        .ok_or(CodecError::Invalid("outcome count"))?;
    if payload.cases.len() != expected || payload.identity.run_id != request.run_id {
        return Err(CodecError::Invalid("case identity"));
    }
    raw_case_records(payload)?;
    let complete = payload
        .cases
        .iter()
        .all(|record| matches!(record.outcome, CaseOutcome::Complete(_)));
    if (complete && payload.execution.stop_reason.is_none()) != payload.execution.complete {
        return Err(CodecError::Invalid("run completion"));
    }
    Ok(())
}

/// Validate each chronological case identity and its recorded outcome.
fn raw_case_records(payload: &crate::report::RunPayload) -> Result<(), CodecError> {
    let request = &payload.request;
    for (position, record) in payload.cases.iter().enumerate() {
        let case = position % request.cases.len();
        let repetition = position / request.cases.len();
        let seed = request.seeds[repetition];
        let identity = usize::try_from(record.case_index).ok() == Some(case)
            && usize::try_from(record.repetition).ok() == Some(repetition);
        let seed_identity =
            record.seed == seed && u128::from(record.effective_offset) == seed.offset();
        if !(identity && seed_identity) {
            return Err(CodecError::Invalid("chronological identity"));
        }
        let lc_ledger = (request.cases[case].dependency == crate::Dependency::PreparedTraversal)
            .then(|| crate::recipe::LcLedger::from_case(&request.cases[case]))
            .transpose()
            .map_err(|_| CodecError::Invalid("LC ledger"))?;
        if record.lc_ledger != lc_ledger {
            return Err(CodecError::Invalid("LC ledger"));
        }
        let packing = (request.cases[case].dependency == crate::Dependency::BackendKernel)
            .then(|| crate::recipe::PackingEvidence::from_case(&request.cases[case]))
            .transpose()
            .map_err(|_| CodecError::Invalid("packing evidence"))?;
        if record.packing != packing {
            return Err(CodecError::Invalid("packing evidence"));
        }
        raw_case_capture(
            record,
            payload
                .execution
                .elapsed_ns
                .unwrap_or(crate::decimal::MAX_INTEGER),
        )?;
        let started = matches!(
            record.outcome,
            CaseOutcome::Complete(_) | CaseOutcome::Incomplete(_)
        );
        let prework_capacity = matches!(&record.outcome, CaseOutcome::Incomplete(partial)
            if partial.reason == crate::report::Reason::Capacity
                && partial.observations.counts.attempted == 0
                && partial.observations.pilot.stop == crate::pilot::PilotStop::Capacity);
        let expected_inputs =
            if started && !(prework_capacity && record.arithmetic_inputs.is_none()) {
                crate::kernels::arithmetic_inputs(&request.cases[case], seed)
                    .map_err(|_| CodecError::Invalid("arithmetic inputs"))?
            } else {
                None
            };
        if record.arithmetic_inputs != expected_inputs {
            return Err(CodecError::Invalid("arithmetic coefficient evidence"));
        }
        match &record.outcome {
            CaseOutcome::Complete(measurement) => {
                raw_measurement(
                    measurement,
                    record,
                    &request.cases[case],
                    request,
                    payload.execution.bounds,
                )?;
            }
            CaseOutcome::Incomplete(partial) => {
                observations(&partial.observations)?;
                worker_layouts(
                    &partial.observations.worker_layouts,
                    request.cases[case].workers,
                    false,
                )?;
                pilot::validate(
                    &partial.observations.pilot,
                    &request.cases[case],
                    payload.execution.bounds,
                )?;
                if partial.observations.counts.resamples != 0 {
                    return Err(CodecError::Invalid("partial estimates"));
                }
            }
            CaseOutcome::Unsupported(_) | CaseOutcome::NotStarted(_) => {}
        }
    }
    Ok(())
}

/// Check accepted pilot and load chronology before interpreting a case outcome.
fn raw_case_capture(record: &crate::report::CaseRecord, elapsed_ns: u64) -> Result<(), CodecError> {
    for snapshot in [record.load_before.as_ref(), record.load_after.as_ref()]
        .into_iter()
        .flatten()
    {
        crate::load_summary::project(snapshot)?;
    }
    let accepted = match &record.outcome {
        CaseOutcome::Complete(value) => value.observations.pilot.accepted_length,
        CaseOutcome::Incomplete(value) => value.observations.pilot.accepted_length,
        CaseOutcome::Unsupported(_) | CaseOutcome::NotStarted(_) => None,
    };
    if accepted != record.accepted_length {
        return Err(CodecError::Invalid("accepted length evidence"));
    }
    let before = record.load_before.as_ref().map(|value| value.offset_ns);
    let after = record.load_after.as_ref().map(|value| value.offset_ns);
    if before.zip(after).is_some_and(|(a, b)| a > b)
        || [before, after]
            .into_iter()
            .flatten()
            .any(|offset| offset > elapsed_ns)
    {
        return Err(CodecError::Invalid("load capture offsets"));
    }
    Ok(())
}

/// Reconstruct complete raw counts, exact statistics and recipe work identity.
fn raw_measurement(
    measurement: &crate::Measurement,
    record: &crate::report::CaseRecord,
    case: &crate::CaseRequest,
    request: &crate::RunRequest,
    bounds: Option<crate::ExecutionBounds>,
) -> Result<(), CodecError> {
    observations(&measurement.observations)?;
    worker_layouts(&measurement.observations.worker_layouts, case.workers, true)?;
    pilot::validate(&measurement.observations.pilot, case, bounds)?;
    let counts = measurement.observations.counts;
    if counts.rows != u64::from(request.sampling.samples)
        || counts.resamples != u64::from(request.sampling.resamples)
    {
        return Err(CodecError::Invalid("incomplete complete analysis"));
    }
    let length = record
        .accepted_length
        .ok_or(CodecError::Invalid("accepted length"))?;
    if measurement.observations.pilot.accepted_length != Some(length)
        || measurement.observations.sampler_stop != crate::sampling_evidence::SamplingStop::Complete
        || crate::kernels::counts(case, length).ok() != Some(measurement.numerators)
    {
        return Err(CodecError::Invalid("complete identity or work numerator"));
    }
    let analysis = &measurement.analysis;
    raw_analysis(analysis, request.sampling.confidence, counts.rows)?;
    let n = measurement.numerators;
    for (rate, numerator) in measurement.rates.iter().zip([
        Some(n.items),
        Some(n.elements),
        n.vector_operations,
        n.floating_operations,
        n.logical_bytes,
    ]) {
        let expected = numerator
            .map(|count| crate::statistics::RawRate::from_time(&analysis.estimates[4], count))
            .transpose()?;
        if rate != &expected {
            return Err(CodecError::Invalid("reciprocal source/view agreement"));
        }
    }
    Ok(())
}

/// Preserve the host's fixed statistics, confidence and outlier partition exactly.
fn raw_analysis(
    analysis: &crate::statistics::RawAnalysis,
    confidence: crate::Binary64,
    rows: u64,
) -> Result<(), CodecError> {
    for (index, estimate) in analysis.estimates.iter().enumerate() {
        estimate.validate()?;
        let time = matches!(index, 0 | 1 | 4);
        for (endpoint, value) in [&estimate.point, &estimate.lower, &estimate.upper]
            .into_iter()
            .enumerate()
        {
            if index == 4 && endpoint != 0 {
                continue;
            }
            let exact = decimal::Exact::from_bits(value.source_bits.0)?;
            if value.decimal.starts_with('-') || (time && exact.is_zero()) {
                return Err(CodecError::Invalid("time or dispersion sign"));
            }
        }
    }
    analysis.confidence.validate(-6, Rounding::Nearest)?;
    if analysis.confidence.source_bits != confidence {
        return Err(CodecError::Invalid("confidence source"));
    }
    let mut labels = [0_u64; 5];
    for label in &analysis.outlier_labels {
        let slot = labels
            .get_mut(usize::from(*label))
            .ok_or(CodecError::Invalid("outlier label"))?;
        *slot += 1;
    }
    let mut fence_grid = analysis.estimates[1].grid_power;
    for fence in &analysis.fences {
        let exact = decimal::Exact::from_bits(fence.source_bits.0)?;
        if !exact.is_zero() {
            fence_grid = fence_grid.min(exact.exponent()? - 2);
        }
    }
    if fence_grid != analysis.fence_grid {
        return Err(CodecError::Invalid("canonical fence grid"));
    }
    for (index, fence) in analysis.fences.iter().enumerate() {
        fence.validate(
            analysis.fence_grid,
            if index < 2 {
                Rounding::Lower
            } else {
                Rounding::Upper
            },
        )?;
    }
    if labels != analysis.outlier_counts || labels.iter().sum::<u64>() != rows {
        return Err(CodecError::Invalid("outlier partition"));
    }
    Ok(())
}

/// Write validated canonical raw bytes, ending with exactly one newline.
/// # Errors
/// Returns schema or caller-writer failures without sampling or command execution.
pub fn write_report(raw: &RunReport, mut writer: impl Write) -> Result<(), CodecError> {
    validate_report(raw)?;
    serde_json::to_writer(&mut writer, raw)?;
    writer.write_all(b"\n")?;
    Ok(())
}

/// Decode a bounded canonical raw artifact; alternate encodings are refused.
/// # Errors
/// Rejects capacity, strict schema and byte-identity failures.
pub fn read_report(reader: impl Read, byte_cap: u64) -> Result<RunReport, CodecError> {
    let bytes = read_bounded(reader, byte_cap)?;
    let raw: RunReport = serde_json::from_slice(&bytes)?;
    let mut canonical = Vec::new();
    write_report(&raw, &mut canonical)?;
    if canonical != bytes {
        return Err(CodecError::Invalid("noncanonical raw bytes"));
    }
    Ok(raw)
}

/// Validate summaries without reconstructing absent raw observations.
/// # Errors
/// Rejects malformed current schema, fixed shapes and integer/count/grid invariants.
pub fn validate_summary(summary: &SummaryDataset) -> Result<(), CodecError> {
    envelope(
        &summary.namespace,
        "org.naturrhythmuszentrum.compute.summary",
        summary.version,
        &summary.owner,
        &summary.license,
    )?;
    integer_tree(&serde_json::to_value(summary)?)?;
    if !load_values(&serde_json::to_value(summary)?) {
        return Err(CodecError::Invalid("load quantity"));
    }
    let payload = &summary.payload;
    if let crate::Evidence::Known(attachment) = &payload.identity.source_attachment
        && !source_attachment(attachment)
    {
        return Err(CodecError::Invalid("source attachment"));
    }
    lowering(&payload.build.lowering)?;
    execution(&payload.execution, payload.request.resources.budget_ns)?;
    if !payload.raw.is_valid() || payload.raw_version != [1, 1, 0] {
        return Err(CodecError::Invalid("raw reference"));
    }
    let expected = payload
        .request
        .cases
        .checked_mul(payload.request.repetitions)
        .ok_or(CodecError::Invalid("case count"))?;
    if u64::try_from(payload.cases.len()).ok() != Some(expected) {
        return Err(CodecError::Invalid("case count"));
    }
    if payload.request.cases == 0
        || payload.request.repetitions == 0
        || payload.request.samples < 10
        || payload.request.resamples < 1000
    {
        return Err(CodecError::Invalid("requested counts"));
    }
    if payload.raw.algorithm != payload.identity.object_format
        || payload.request.confidence_input.raw != payload.raw
        || payload.request.confidence_input.pointer != "/payload/request/sampling/confidence"
    {
        return Err(CodecError::Invalid("exact input reference"));
    }
    let complete = payload
        .cases
        .iter()
        .all(|record| matches!(record.outcome, SummaryOutcome::Complete(_)));
    if (complete && payload.execution.stop_reason.is_none()) != payload.execution.complete {
        return Err(CodecError::Invalid("run completion"));
    }
    if !summary_fixed(&payload.request.confidence, -6)
        || !summary_ordered("0.000000", &payload.request.confidence)
        || !summary_ordered(&payload.request.confidence, "1.000000")
    {
        return Err(CodecError::Invalid("rounded confidence"));
    }
    let case_count =
        usize::try_from(payload.request.cases).map_err(|_| CodecError::Invalid("case count"))?;
    for (index, record) in payload.cases.iter().enumerate() {
        let initial = &payload.cases[index % case_count];
        let repetition = &payload.cases[index / case_count * case_count];
        if record.recipe != initial.recipe || record.seed != repetition.seed {
            return Err(CodecError::Invalid("repetition identity"));
        }
        if index % case_count == 0
            && payload.cases[..index]
                .iter()
                .step_by(case_count)
                .any(|previous| previous.effective_offset == record.effective_offset)
        {
            return Err(CodecError::Invalid("colliding repetition offsets"));
        }
    }
    for (position, record) in payload.cases.iter().enumerate() {
        summary_case(record, position, payload)?;
    }
    Ok(())
}

/// Write canonical summary bytes after strict structural validation.
/// # Errors
/// Returns summary validation or caller-writer failures.
pub fn write_summary(summary: &SummaryDataset, mut writer: impl Write) -> Result<(), CodecError> {
    validate_summary(summary)?;
    serde_json::to_writer(&mut writer, summary)?;
    writer.write_all(b"\n")?;
    Ok(())
}

/// Decode a summary without reading raw files or probing any host state.
/// # Errors
/// Rejects oversized input, unknown fields, malformed variants and schema invariants.
pub fn read_summary(reader: impl Read, byte_cap: u64) -> Result<SummaryDataset, CodecError> {
    let bytes = read_bounded(reader, byte_cap)?;
    let summary = serde_json::from_slice(&bytes)?;
    validate_summary(&summary)?;
    Ok(summary)
}

/// Read within a caller-supplied cap and reject truncation instead of accepting a prefix.
fn read_bounded(reader: impl Read, cap: u64) -> Result<Vec<u8>, CodecError> {
    let limit = cap.checked_add(1).ok_or(CodecError::Invalid("byte cap"))?;
    let mut bytes = Vec::new();
    reader.take(limit).read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).map_err(|_| CodecError::Invalid("byte cap"))? > cap {
        return Err(CodecError::Invalid("byte cap"));
    }
    Ok(bytes)
}

/// Public declarations may redact evidence, but may not manufacture a different known fact.
fn public_projection(raw: &serde_json::Value, public: &serde_json::Value) -> bool {
    match (raw, public) {
        (serde_json::Value::Object(original), serde_json::Value::Object(projected)) => {
            let redacted =
                projected.get("state").and_then(serde_json::Value::as_str) == Some("Redacted");
            if redacted {
                return projected
                    .get("value")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|reason| !reason.is_empty());
            }
            original.len() == projected.len()
                && original.iter().all(|(key, value)| {
                    projected
                        .get(key)
                        .is_some_and(|public| public_projection(value, public))
                })
        }
        (serde_json::Value::Array(original), serde_json::Value::Array(projected)) => {
            original.len() == projected.len()
                && original
                    .iter()
                    .zip(projected)
                    .all(|(raw, public)| public_projection(raw, public))
        }
        _ => raw == public,
    }
}

/// Positive canonical reciprocal rates cannot be zero or negative.
fn positive_rate(text: &str) -> bool {
    !text.starts_with('-') && text != "0.00000e0"
}

/// Compare positive six-figure values through exact exponent and mantissa ordering.
fn rate_ordered(lower: &str, upper: &str) -> bool {
    let Some((a, e)) = lower.split_once('e') else {
        return false;
    };
    let Some((b, f)) = upper.split_once('e') else {
        return false;
    };
    let (Ok(e), Ok(f)) = (e.parse::<i32>(), f.parse::<i32>()) else {
        return false;
    };
    !e.cmp(&f).then_with(|| a.cmp(b)).is_gt()
}

/// Shared fixed-grid grammar for report-level confidence.
fn summary_fixed(text: &str, power: i32) -> bool {
    decimal::validate_fixed(text, power).is_ok()
}

/// Ordered canonical same-grid strings need no floating-point parser.
fn summary_ordered(lower: &str, upper: &str) -> bool {
    let a_negative = lower.starts_with('-');
    let b_negative = upper.starts_with('-');
    if a_negative != b_negative {
        return a_negative;
    }
    let a = lower.trim_start_matches('-');
    let b = upper.trim_start_matches('-');
    let order = a.len().cmp(&b.len()).then_with(|| a.cmp(b));
    if a_negative {
        !order.is_lt()
    } else {
        !order.is_gt()
    }
}

/// Validate one fixed case record without reconstructing raw observations.
fn summary_case(
    record: &crate::summary::SummaryCase,
    position: usize,
    payload: &crate::summary::SummaryPayload,
) -> Result<(), CodecError> {
    recipes::protocol(&record.recipe).map_err(CodecError::Invalid)?;
    worker_layouts(
        &record.worker_layouts,
        record.recipe.workers,
        matches!(record.outcome, SummaryOutcome::Complete(_)),
    )?;
    let position = u64::try_from(position).map_err(|_| CodecError::Invalid("case position"))?;
    if record.case_index != position % payload.request.cases
        || record.repetition != position / payload.request.cases
        || u128::from(record.effective_offset) != record.seed.offset()
    {
        return Err(CodecError::Invalid("case identity"));
    }
    if record.recipe.exact_input_pointer.raw != payload.raw
        || record.recipe.exact_input_pointer.pointer
            != format!("/payload/request/cases/{}", record.case_index)
    {
        return Err(CodecError::Invalid("recipe reference"));
    }
    counts(record.counts)?;
    match &record.outcome {
        SummaryOutcome::Complete(_) => {
            if record.counts.rows != u64::from(payload.request.samples)
                || record.counts.resamples != u64::from(payload.request.resamples)
                || record.accepted_length.is_none_or(|length| length == 0)
            {
                return Err(CodecError::Invalid("complete count"));
            }
        }
        SummaryOutcome::Incomplete(_) => {
            if record.counts.resamples != 0 {
                return Err(CodecError::Invalid("partial resamples"));
            }
        }
        SummaryOutcome::Unsupported(_) | SummaryOutcome::NotStarted(_) => {
            if record.counts != Counts::default() || record.accepted_length.is_some() {
                return Err(CodecError::Invalid("unstarted work"));
            }
        }
    }

    let started = matches!(
        record.outcome,
        SummaryOutcome::Complete(_) | SummaryOutcome::Incomplete(_)
    );
    if started != record.pilot.is_some() {
        return Err(CodecError::Invalid("pilot presence"));
    }
    arithmetic_identity(record, position, &payload.raw)?;
    summary_lc_identity(record)?;
    recipes::packing(record).map_err(CodecError::Invalid)?;
    summary_pilot(record)?;
    if let SummaryOutcome::Complete(analysis) = &record.outcome {
        summary_analysis(analysis, record.counts.rows, &payload.request.confidence)?;
        summary_numerators(record, analysis)?;
    }

    Ok(())
}

/// Validate fixed pilot evidence independently of case identity and analysis.
fn summary_pilot(record: &crate::summary::SummaryCase) -> Result<(), CodecError> {
    if let Some(pilot) = &record.pilot {
        let accepted = record.accepted_length.is_some();
        if accepted != (pilot.stop == crate::pilot::PilotStop::Complete)
            || accepted != pilot.accepted_median_seconds.is_some()
            || accepted != pilot.accepted_maximum_ns.is_some()
        {
            return Err(CodecError::Invalid("pilot acceptance evidence"));
        }
        if accepted
            && (pilot.control_median_seconds.is_none()
                || pilot.control_maximum_ns.is_none()
                || pilot.target_ns.is_none()
                || pilot.observed_tick_ns.is_none())
        {
            return Err(CodecError::Invalid("accepted pilot controls"));
        }
        if pilot.observed_tick_ns == Some(0)
            || pilot.declared_resolution_ns == crate::Evidence::Known(0)
        {
            return Err(CodecError::Invalid("pilot resolution"));
        }

        for text in [
            pilot.control_median_seconds.as_ref(),
            pilot.accepted_median_seconds.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            if !summary_fixed(text, -10) || text.starts_with('-') {
                return Err(CodecError::Invalid("pilot median grid"));
            }
        }
    }
    Ok(())
}

/// Static LC evidence is validated even when timing never started.
fn summary_lc_identity(record: &crate::summary::SummaryCase) -> Result<(), CodecError> {
    if record.recipe.dependency != crate::Dependency::PreparedTraversal {
        return if record.recipe.lc_ledger.is_none() {
            Ok(())
        } else {
            Err(CodecError::Invalid("unexpected LC ledger"))
        };
    }
    let ledger = record
        .recipe
        .lc_ledger
        .as_ref()
        .ok_or(CodecError::Invalid("missing LC ledger"))?;
    let expected = crate::recipe::LcLedger::from_populations(
        record.recipe.operation,
        ledger.input_populations.clone(),
        ledger.cycle_population,
    )
    .map_err(|_| CodecError::Invalid("LC populations"))?;
    if *ledger != expected {
        return Err(CodecError::Invalid("LC ledger"));
    }
    let shape_matches = if record.recipe.operation == crate::Operation::InverseTime {
        record.recipe.shape.as_slice()
            == [
                ledger.input_populations.first().copied().unwrap_or(0),
                ledger.cycle_population,
            ]
    } else {
        record.recipe.shape == ledger.input_populations
    };
    if !shape_matches {
        return Err(CodecError::Invalid("LC shape"));
    }
    Ok(())
}

/// Reconstruct the public LC ledger and every rate numerator from its population metadata.
fn summary_lc_numerators(
    record: &crate::summary::SummaryCase,
    analysis: &crate::summary::SummaryAnalysis,
) -> Result<(), CodecError> {
    let ledger = record
        .recipe
        .lc_ledger
        .as_ref()
        .ok_or(CodecError::Invalid("missing LC ledger"))?;
    let expected = crate::recipe::LcLedger::from_populations(
        record.recipe.operation,
        ledger.input_populations.clone(),
        ledger.cycle_population,
    )
    .map_err(|_| CodecError::Invalid("LC populations"))?;
    if *ledger != expected
        || record.recipe.workers != 1
        || record.recipe.width != crate::Width::Scalar
    {
        return Err(CodecError::Invalid("LC ledger"));
    }
    let counts = ledger
        .numerators(
            record.recipe.scope,
            record
                .accepted_length
                .ok_or(CodecError::Invalid("LC length"))?,
        )
        .map_err(|_| CodecError::Invalid("LC counts"))?;
    if analysis.elements_per_second.numerator != counts.elements
        || analysis
            .floating_operations_per_second
            .as_ref()
            .map(|rate| rate.numerator)
            != counts.floating_operations
        || analysis
            .logical_bytes_per_second
            .as_ref()
            .map(|rate| rate.numerator)
            != counts.logical_bytes
        || analysis.vector_operations_per_second.is_some()
    {
        return Err(CodecError::Invalid("LC rate numerators"));
    }
    Ok(())
}

/// Check exact arithmetic rate numerators from the public recipe, without reconstructing observations.
fn summary_numerators(
    record: &crate::summary::SummaryCase,
    analysis: &crate::summary::SummaryAnalysis,
) -> Result<(), CodecError> {
    use crate::{Dependency, Operation, Scope, Width};
    if analysis.items_per_second.numerator != 1 {
        return Err(CodecError::Invalid("aggregate item numerator"));
    }
    if record.recipe.dependency == Dependency::PreparedTraversal {
        return summary_lc_numerators(record, analysis);
    }
    if record.recipe.lc_ledger.is_some() {
        return Err(CodecError::Invalid("unexpected LC ledger"));
    }
    let states = match record.recipe.dependency {
        Dependency::Dependent => 1_u64,
        Dependency::Independent => 8,
        _ => return recipes::numerators(record, analysis).map_err(CodecError::Invalid),
    };
    if !record.recipe.shape.is_empty() || record.recipe.scope != Scope::Prepared {
        return Err(CodecError::Invalid("arithmetic recipe shape or scope"));
    }
    let factor = match record.recipe.operation {
        Operation::Add | Operation::Multiply | Operation::Divide => 1,
        Operation::FmaAddend
        | Operation::FmaMultiplicand
        | Operation::MultiplyAdd
        | Operation::SquareRoot => 2,
        _ => return Err(CodecError::Invalid("arithmetic operation")),
    };
    let width = match record.recipe.width {
        Width::Scalar | Width::Simd1 => 1,
        Width::Simd2 => 2,
        Width::Simd4 => 4,
        Width::Simd8 => 8,
        Width::BackendSelectedBinary64 => return Err(CodecError::Invalid("arithmetic width")),
    };
    let operations = record
        .accepted_length
        .and_then(|length| length.checked_mul(states))
        .and_then(|n| n.checked_mul(record.recipe.workers))
        .ok_or(CodecError::Invalid("arithmetic count"))?;
    let elements = operations
        .checked_mul(width)
        .ok_or(CodecError::Invalid("arithmetic count"))?;
    let floating = elements
        .checked_mul(factor)
        .ok_or(CodecError::Invalid("arithmetic count"))?;
    let vectors = (record.recipe.width != Width::Scalar).then_some(operations);
    if analysis.elements_per_second.numerator != elements
        || analysis
            .floating_operations_per_second
            .as_ref()
            .map(|rate| rate.numerator)
            != Some(floating)
        || analysis
            .vector_operations_per_second
            .as_ref()
            .map(|rate| rate.numerator)
            != vectors
        || analysis.logical_bytes_per_second.is_some()
    {
        return Err(CodecError::Invalid("arithmetic rate numerators"));
    }
    Ok(())
}

/// Check fixed statistic slots, directed interval order and reciprocal notation.
fn summary_analysis(
    analysis: &crate::summary::SummaryAnalysis,
    rows: u64,
    confidence: &str,
) -> Result<(), CodecError> {
    for (index, estimate) in analysis.estimates.iter().enumerate() {
        let time = matches!(index, 0 | 1 | 4);
        for (endpoint, value) in [&estimate.point, &estimate.lower, &estimate.upper]
            .into_iter()
            .enumerate()
        {
            if index == 4 && endpoint != 0 {
                continue;
            }
            let zero = value.bytes().all(|byte| byte == b'0' || byte == b'.');
            if value.starts_with('-') || (time && zero) {
                return Err(CodecError::Invalid("time or dispersion sign"));
            }
        }
        for text in [
            &estimate.point,
            &estimate.standard_error,
            &estimate.lower,
            &estimate.upper,
        ] {
            decimal::validate_fixed(text, estimate.grid_power)?;
        }
        let lower = decimal::Exact::from_grid(&estimate.lower, estimate.grid_power)?;
        let upper = decimal::Exact::from_grid(&estimate.upper, estimate.grid_power)?;
        if lower.compare(upper)?.is_gt() || estimate.standard_error.starts_with('-') {
            return Err(CodecError::Invalid("interval order or dispersion sign"));
        }
    }
    decimal::validate_fixed(&analysis.confidence, -6)?;
    for fence in &analysis.fences {
        decimal::validate_fixed(fence, analysis.fence_grid)?;
    }
    if analysis.confidence != confidence {
        return Err(CodecError::Invalid("confidence mismatch"));
    }
    for pair in analysis.fences.windows(2) {
        if !summary_ordered(&pair[0], &pair[1]) {
            return Err(CodecError::Invalid("fence order"));
        }
    }
    summary_rates(analysis)?;
    if analysis.outlier_counts.iter().sum::<u64>() != rows {
        return Err(CodecError::Invalid("outlier count"));
    }

    Ok(())
}

/// Reciprocal intervals can be unavailable without removing the time analysis or point rate.
fn summary_rates(analysis: &crate::summary::SummaryAnalysis) -> Result<(), CodecError> {
    for rate in [
        Some(&analysis.items_per_second),
        Some(&analysis.elements_per_second),
        analysis.vector_operations_per_second.as_ref(),
        analysis.floating_operations_per_second.as_ref(),
        analysis.logical_bytes_per_second.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        let interval = match (&rate.lower, &rate.upper, &rate.interval_unavailable) {
            (Some(lower), Some(upper), None) => {
                if !positive_rate(lower) || !positive_rate(upper) || !rate_ordered(lower, upper) {
                    return Err(CodecError::Invalid("rate sign or endpoint order"));
                }
                Some([lower, upper])
            }
            (None, None, Some(reason)) if reason == "NonpositiveTimeEndpoint" => None,
            _ => return Err(CodecError::Invalid("reciprocal interval availability")),
        };
        if rate.numerator == 0 || !positive_rate(&rate.point) {
            return Err(CodecError::Invalid("rate numerator or point"));
        }
        for text in std::iter::once(&rate.point).chain(interval.into_iter().flatten()) {
            if decimal::validate_significant(text).is_err() {
                return Err(CodecError::Invalid("rate representation"));
            }
        }
    }

    Ok(())
}

/// Validate the fixed numeric telemetry shape without interpreting strings as floats.
fn load_values(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(fields) => {
            if fields.contains_key("resolution_numerator") {
                let Some(text) = fields.get("value").and_then(serde_json::Value::as_str) else {
                    return false;
                };
                let Some(power) = fields
                    .get("grid_power")
                    .and_then(serde_json::Value::as_i64)
                    .and_then(|n| i32::try_from(n).ok())
                else {
                    return false;
                };
                let numerator = fields
                    .get("resolution_numerator")
                    .and_then(serde_json::Value::as_u64);
                let denominator = fields
                    .get("resolution_denominator")
                    .and_then(serde_json::Value::as_u64);
                let unit = fields.get("unit").and_then(serde_json::Value::as_str);
                let source = numerator.is_some_and(|n| n > 0) && denominator.is_some_and(|n| n > 0);
                let native = matches!(unit, Some("runnable tasks" | "percent" | "one" | "MHz"));
                if !source || !native || text.starts_with('-') || !summary_fixed(text, power) {
                    return false;
                }
            }
            fields.values().all(load_values)
        }
        serde_json::Value::Array(values) => values.iter().all(load_values),
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => true,
    }
}

/// Arithmetic summaries bind active byte counts and coefficient references to exact case identity.
fn arithmetic_identity(
    record: &crate::summary::SummaryCase,
    position: u64,
    raw: &crate::report::RawContentId,
) -> Result<(), CodecError> {
    let states = match record.recipe.dependency {
        crate::Dependency::Dependent => Some(1_u64),
        crate::Dependency::Independent => Some(8),
        _ => None,
    };
    let started = matches!(
        record.outcome,
        SummaryOutcome::Complete(_) | SummaryOutcome::Incomplete(_)
    );
    let prework_capacity = matches!(
        record.outcome,
        SummaryOutcome::Incomplete(crate::report::Reason::Capacity)
    ) && record.counts.attempted == 0
        && record
            .pilot
            .as_ref()
            .is_some_and(|pilot| pilot.stop == crate::pilot::PilotStop::Capacity);
    let missing_capacity_inputs = prework_capacity && record.arithmetic.is_none();
    if (started && states.is_some() && !missing_capacity_inputs) != record.arithmetic.is_some() {
        return Err(CodecError::Invalid("arithmetic evidence presence"));
    }
    if let Some(arithmetic) = &record.arithmetic {
        let width = match record.recipe.width {
            crate::Width::Scalar | crate::Width::Simd1 => 1,
            crate::Width::Simd2 => 2,
            crate::Width::Simd4 => 4,
            crate::Width::Simd8 => 8,
            crate::Width::BackendSelectedBinary64 => {
                return Err(CodecError::Invalid("arithmetic width"));
            }
        };
        let fields = if matches!(
            record.recipe.operation,
            crate::Operation::FmaAddend
                | crate::Operation::FmaMultiplicand
                | crate::Operation::MultiplyAdd
        ) {
            2
        } else {
            1
        };
        let bytes = states.ok_or(CodecError::Invalid("arithmetic dependency"))? * width * 8;
        if arithmetic.cycle != 2
            || arithmetic.state_bytes != bytes
            || arithmetic.coefficient_bytes != bytes * fields * 2
            || arithmetic.exact_inputs.raw != *raw
            || arithmetic.exact_inputs.pointer
                != format!("/payload/cases/{position}/arithmetic_inputs")
        {
            return Err(CodecError::Invalid("arithmetic byte identity or reference"));
        }
    }
    Ok(())
}

/// Worker evidence has stable worker ordering, bounded storage and no exported address.
fn worker_layouts(
    layouts: &[crate::layout::WorkerLayout],
    workers: u64,
    complete: bool,
) -> Result<(), CodecError> {
    let count = u64::try_from(layouts.len()).map_err(|_| CodecError::Invalid("worker layouts"))?;
    if count > workers || (complete && count != workers) {
        return Err(CodecError::Invalid("worker layout population"));
    }
    for (index, layout) in layouts.iter().enumerate() {
        if usize::try_from(layout.worker).ok() != Some(index)
            || layout.first_touch_worker != layout.worker
        {
            return Err(CodecError::Invalid("worker layout ownership"));
        }
        if complete && layout.buffers.iter().all(Option::is_none) {
            return Err(CodecError::Invalid("missing numerical storage"));
        }
        let capacity = layout
            .buffers
            .iter()
            .flatten()
            .try_fold(0_u64, |sum, buffer| sum.checked_add(buffer.capacity_bytes));
        let live = layout
            .buffers
            .iter()
            .flatten()
            .try_fold(0_u64, |sum, buffer| sum.checked_add(buffer.live_bytes));
        if capacity != Some(layout.numerical_capacity_bytes)
            || live != Some(layout.numerical_live_bytes)
        {
            return Err(CodecError::Invalid("worker buffer total"));
        }
        for buffer in layout.buffers.iter().flatten() {
            let numerical = buffer.role != crate::layout::BufferRole::Scratch;
            let aligned = buffer.address_modulo_64 % 8 == 0;
            if numerical && !aligned {
                return Err(CodecError::Invalid("worker buffer alignment"));
            }
            if buffer.live_bytes > buffer.capacity_bytes
                || buffer.address_modulo_64 >= 64
                || buffer.row_stride_bytes == 0
            {
                return Err(CodecError::Invalid("worker buffer layout"));
            }
        }
    }
    Ok(())
}

/// Reviewed source links stay in a fixed repository partition and carry full blob identities.
fn source_attachment(attachment: &crate::report::SourceAttachment) -> bool {
    let Some(name) = attachment.path.strip_prefix("docs/attachments/compute/") else {
        return false;
    };
    let Some(slug) = name.strip_suffix(".patch") else {
        return false;
    };
    let nonempty = !slug.is_empty();
    let grammar = slug
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    let name = nonempty && grammar;
    let digest = attachment.blob.is_valid();
    name && digest
}

#[cfg(test)]
mod tests;
