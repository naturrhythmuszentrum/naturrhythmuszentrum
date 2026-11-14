// SPDX-License-Identifier: AGPL-3.0-only
//! Structural summary invariants using integer/string arithmetic only.

mod lc;

mod recipes;

use super::schema::{ObjectFormat, SummaryDataset, SummaryOutcome};

/// Full lowercase hexadecimal grammar.
fn hex(text: &str, length: usize) -> bool {
    let width = text.len() == length;
    let digits = text
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    width && digits
}

/// Fixed-grid canonical grammar with bounded encoded length and normalized zero.
fn fixed(text: &str, power: i32) -> bool {
    if !(-326..=309).contains(&power) || text.len() > 637 {
        return false;
    }
    let magnitude = text.strip_prefix('-').unwrap_or(text);
    let places = if power < 0 {
        power.unsigned_abs() as usize
    } else {
        0
    };
    let (whole, fraction) = match magnitude.split_once('.') {
        Some(parts) if places > 0 => parts,
        None if places == 0 => (magnitude, ""),
        _ => return false,
    };
    let whole_ok = !whole.is_empty() && (whole == "0" || !whole.starts_with('0'));
    let digits = whole
        .bytes()
        .chain(fraction.bytes())
        .all(|b| b.is_ascii_digit());
    let negative_zero =
        text.starts_with('-') && whole.bytes().chain(fraction.bytes()).all(|b| b == b'0');
    whole_ok && digits && fraction.len() == places && !negative_zero
}

/// Ordered same-grid signed literals, permitting an interval to exclude its point.
fn ordered(lower: &str, upper: &str) -> bool {
    let negative_lower = lower.starts_with('-');
    let negative_upper = upper.starts_with('-');
    if negative_lower != negative_upper {
        return negative_lower;
    }
    let a = lower.trim_start_matches('-');
    let b = upper.trim_start_matches('-');
    let order = a.len().cmp(&b.len()).then_with(|| a.cmp(b));
    if negative_lower {
        !order.is_lt()
    } else {
        !order.is_gt()
    }
}

/// Canonical six-significant-figure grammar.
fn significant(text: &str) -> bool {
    if text == "0.00000e0" {
        return true;
    }
    let Some((mantissa, exponent)) = text.strip_prefix('-').unwrap_or(text).split_once('e') else {
        return false;
    };
    let bytes = mantissa.as_bytes();
    if bytes.len() != 7 {
        return false;
    }
    let leading = (b'1'..=b'9').contains(&bytes[0]);
    let point = bytes[1] == b'.';
    let digits = bytes[2..].iter().all(u8::is_ascii_digit);
    let magnitude = exponent.strip_prefix('-').unwrap_or(exponent);
    let spelling = !magnitude.is_empty() && (magnitude == "0" || !magnitude.starts_with('0'));
    let canonical = spelling && magnitude.bytes().all(|b| b.is_ascii_digit()) && exponent != "-0";
    let range = exponent
        .parse::<i32>()
        .is_ok_and(|value| (-324..=340).contains(&value));
    leading && point && digits && canonical && range
}

/// Check every typed integer against the common JSON domain.
fn integers(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Number(number) => {
            number.as_u64().is_some_and(|n| n < (1_u64 << 53))
                || number
                    .as_i64()
                    .is_some_and(|n| n.unsigned_abs() < (1_u64 << 53))
        }
        serde_json::Value::Array(values) => values.iter().all(integers),
        serde_json::Value::Object(values) => values.values().all(integers),
        serde_json::Value::Null | serde_json::Value::String(_) | serde_json::Value::Bool(_) => true,
    }
}

/// Phase time cannot exceed elapsed time, and overshoot is an exact integer difference.
fn execution(execution: &super::schema::Execution, budget_ns: u64) -> Result<(), String> {
    use super::schema::Reason as Stop;
    if !matches!(
        execution.stop_reason,
        None | Some(Stop::Clock | Stop::Capacity | Stop::Worker | Stop::Deadline | Stop::Cancelled)
    ) {
        return Err(invalid("invalid run stop reason"));
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
        _ => return Err(invalid("unavailable execution timing without clock stop")),
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
        return Err(invalid("execution duration ledger"));
    }
    Ok(())
}

/// Structured lowering cannot promote compiler intent into inspected instructions.
fn lowering(record: &super::schema::LoweringRecord) -> Result<(), String> {
    use super::schema::Evidence;
    let inspected = record.inspected == Evidence::Known(true);
    if let Evidence::Known(artifact) = &record.artifact {
        let length = match artifact.algorithm {
            ObjectFormat::Sha1 => 40,
            ObjectFormat::Sha256 => 64,
        };
        if !inspected || !hex(&artifact.digest, length) {
            return Err(invalid("lowering artifact"));
        }
    }
    let known = matches!(record.symbols, Evidence::Known(_))
        || matches!(record.scalar_vectorization, Evidence::Known(_))
        || matches!(record.splitting, Evidence::Known(_))
        || matches!(record.helpers, Evidence::Known(_))
        || matches!(record.native_fma, Evidence::Known(_));
    if known && !inspected {
        return Err(invalid("uninspected lowering claim"));
    }
    if let Evidence::Known(symbols) = &record.symbols
        && (symbols.is_empty() || symbols.iter().any(String::is_empty))
    {
        return Err(invalid("lowering symbols"));
    }
    Ok(())
}

/// Validate the complete fixed shape without reconstructing omitted observations.
pub(super) fn summary(summary: &SummaryDataset) -> Result<(), String> {
    if summary.namespace != "org.naturrhythmuszentrum.compute.summary"
        || summary.version != [1, 1, 0]
        || summary.owner != "NRZ"
        || summary.license != "CC0-1.0"
    {
        return Err("summary envelope".to_owned());
    }
    let value = serde_json::to_value(summary).map_err(|error| error.to_string())?;
    if !integers(&value) {
        return Err("integer representation".to_owned());
    }
    if !load_values(&value) {
        return Err("load quantity".to_owned());
    }
    let payload = &summary.payload;
    if let super::schema::Evidence::Known(attachment) = &payload.identity.source_attachment
        && !source_attachment(attachment)
    {
        return Err("source attachment".to_owned());
    }
    lowering(&payload.build.lowering)?;
    execution(&payload.execution, payload.request.resources.budget_ns)?;
    let length = match payload.raw.algorithm {
        ObjectFormat::Sha1 => 40,
        ObjectFormat::Sha256 => 64,
    };
    if !hex(&payload.raw.digest, length) || payload.raw_version != [1, 1, 0] {
        return Err("raw content reference".to_owned());
    }
    let total = payload
        .request
        .cases
        .checked_mul(payload.request.repetitions);
    if total != u64::try_from(payload.cases.len()).ok() {
        return Err("case count".to_owned());
    }
    if payload.request.cases == 0
        || payload.request.repetitions == 0
        || payload.request.samples < 10
        || payload.request.resamples < 1000
    {
        return Err("requested counts".to_owned());
    }
    if payload.raw.algorithm != payload.identity.object_format
        || payload.request.confidence_input.raw != payload.raw
        || payload.request.confidence_input.pointer != "/payload/request/sampling/confidence"
    {
        return Err("exact input reference".to_owned());
    }
    let complete = payload
        .cases
        .iter()
        .all(|record| matches!(record.outcome, SummaryOutcome::Complete(_)));
    if (complete && payload.execution.stop_reason.is_none()) != payload.execution.complete {
        return Err(invalid("run completion"));
    }
    if !fixed(&payload.request.confidence, -6)
        || !ordered("0.000000", &payload.request.confidence)
        || !ordered(&payload.request.confidence, "1.000000")
    {
        return Err(invalid("rounded confidence"));
    }
    let case_count = usize::try_from(payload.request.cases).map_err(|_| invalid("case count"))?;
    for (index, record) in payload.cases.iter().enumerate() {
        let initial = &payload.cases[index % case_count];
        let repetition = &payload.cases[index / case_count * case_count];
        if record.recipe != initial.recipe || record.seed != repetition.seed {
            return Err(invalid("repetition identity"));
        }
        if index % case_count == 0
            && payload.cases[..index]
                .iter()
                .step_by(case_count)
                .any(|previous| previous.effective_offset == record.effective_offset)
        {
            return Err(invalid("colliding repetition offsets"));
        }
    }
    for (position, record) in payload.cases.iter().enumerate() {
        summary_case(record, position, payload)?;
    }
    Ok(())
}

/// Stable structural refusal text.
fn invalid(message: &str) -> String {
    message.to_owned()
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

/// Validate one fixed case record without reconstructing raw observations.
fn summary_case(
    record: &super::schema::SummaryCase,
    position: usize,
    payload: &super::schema::SummaryPayload,
) -> Result<(), String> {
    recipes::protocol(&record.recipe).map_err(str::to_owned)?;
    worker_layouts(
        &record.worker_layouts,
        record.recipe.workers,
        matches!(record.outcome, SummaryOutcome::Complete(_)),
    )?;
    let position = u64::try_from(position).map_err(|_| "case position".to_owned())?;
    let seed = u128::from_str_radix(&record.seed, 16).map_err(|_| "seed syntax".to_owned())?;
    let offset = (seed ^ (seed >> 64)) & 1_048_575;
    if record.case_index != position % payload.request.cases
        || record.repetition != position / payload.request.cases
        || u128::from(record.effective_offset) != offset
    {
        return Err("case identity".to_owned());
    }
    if record.recipe.exact_input_pointer.raw != payload.raw
        || record.recipe.exact_input_pointer.pointer
            != format!("/payload/request/cases/{}", record.case_index)
    {
        return Err("recipe reference".to_owned());
    }
    match &record.outcome {
        SummaryOutcome::Complete(_) => {
            if record.counts.rows != u64::from(payload.request.samples)
                || record.counts.resamples != u64::from(payload.request.resamples)
                || record.accepted_length.is_none_or(|length| length == 0)
            {
                return Err("complete count".to_owned());
            }
        }
        SummaryOutcome::Incomplete(_) => {
            if record.counts.resamples != 0 {
                return Err("partial resamples".to_owned());
            }
        }
        SummaryOutcome::Unsupported(_) | SummaryOutcome::NotStarted(_) => {
            if record.counts != super::schema::Counts::default() || record.accepted_length.is_some()
            {
                return Err("unstarted work".to_owned());
            }
        }
    }
    if !hex(&record.seed, 32) {
        return Err("seed encoding".to_owned());
    }
    let counts = record.counts;
    let attempted = counts
        .pilot
        .checked_add(counts.warm_up)
        .and_then(|n| n.checked_add(counts.measurement));
    let partition = counts.measured.checked_add(counts.discarded);
    if attempted != Some(counts.attempted) || partition != Some(counts.attempted) {
        return Err("attempt partitions".to_owned());
    }
    let started = matches!(
        record.outcome,
        SummaryOutcome::Complete(_) | SummaryOutcome::Incomplete(_)
    );
    if started != record.pilot.is_some() {
        return Err(invalid("pilot presence"));
    }
    arithmetic_identity(record, position, &payload.raw)?;
    lc::identity(record)?;
    recipes::packing(record).map_err(invalid)?;
    summary_pilot(record)?;
    if let SummaryOutcome::Complete(analysis) = &record.outcome {
        summary_analysis(analysis, record.counts.rows, &payload.request.confidence)?;
        summary_numerators(record, analysis)?;
    }

    Ok(())
}

/// Validate fixed pilot evidence independently of case identity and analysis.
fn summary_pilot(record: &super::schema::SummaryCase) -> Result<(), String> {
    if let Some(pilot) = &record.pilot {
        let accepted = record.accepted_length.is_some();
        if accepted != (pilot.stop == super::schema::PilotStop::Complete)
            || accepted != pilot.accepted_median_seconds.is_some()
            || accepted != pilot.accepted_maximum_ns.is_some()
        {
            return Err(invalid("pilot acceptance evidence"));
        }
        if accepted
            && (pilot.control_median_seconds.is_none()
                || pilot.control_maximum_ns.is_none()
                || pilot.target_ns.is_none()
                || pilot.observed_tick_ns.is_none())
        {
            return Err(invalid("accepted pilot controls"));
        }
        if pilot.observed_tick_ns == Some(0)
            || pilot.declared_resolution_ns == super::schema::Evidence::Known(0)
        {
            return Err(invalid("pilot resolution"));
        }

        for text in [
            pilot.control_median_seconds.as_ref(),
            pilot.accepted_median_seconds.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            if !fixed(text, -10) || text.starts_with('-') {
                return Err(invalid("pilot median grid"));
            }
        }
    }
    Ok(())
}

/// Check exact arithmetic rate numerators from the public recipe, without reconstructing observations.
fn summary_numerators(
    record: &super::schema::SummaryCase,
    analysis: &super::schema::SummaryAnalysis,
) -> Result<(), String> {
    use super::schema::{Dependency, Operation, Scope, Width};
    if analysis.items_per_second.numerator != 1 {
        return Err(invalid("aggregate item numerator"));
    }
    if record.recipe.dependency == Dependency::PreparedTraversal {
        return lc::numerators(record, analysis);
    }
    let states = match record.recipe.dependency {
        Dependency::Dependent => 1_u64,
        Dependency::Independent => 8,
        _ => return recipes::numerators(record, analysis).map_err(invalid),
    };
    if !record.recipe.shape.is_empty() || record.recipe.scope != Scope::Prepared {
        return Err(invalid("arithmetic recipe shape or scope"));
    }
    let factor = match record.recipe.operation {
        Operation::Add | Operation::Multiply | Operation::Divide => 1,
        Operation::FmaAddend
        | Operation::FmaMultiplicand
        | Operation::MultiplyAdd
        | Operation::SquareRoot => 2,
        _ => return Err(invalid("arithmetic operation")),
    };
    let width = match record.recipe.width {
        Width::Scalar | Width::Simd1 => 1,
        Width::Simd2 => 2,
        Width::Simd4 => 4,
        Width::Simd8 => 8,
        Width::BackendSelectedBinary64 => return Err(invalid("arithmetic width")),
    };
    let operations = record
        .accepted_length
        .and_then(|length| length.checked_mul(states))
        .and_then(|n| n.checked_mul(record.recipe.workers))
        .ok_or_else(|| invalid("arithmetic count"))?;
    let elements = operations
        .checked_mul(width)
        .ok_or_else(|| invalid("arithmetic count"))?;
    let floating = elements
        .checked_mul(factor)
        .ok_or_else(|| invalid("arithmetic count"))?;
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
        return Err(invalid("arithmetic rate numerators"));
    }
    Ok(())
}

/// Check fixed statistic slots, directed interval order and reciprocal notation.
fn summary_analysis(
    analysis: &super::schema::SummaryAnalysis,
    rows: u64,
    confidence: &str,
) -> Result<(), String> {
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
                return Err(invalid("time or dispersion sign"));
            }
        }
        let values = [
            &estimate.point,
            &estimate.standard_error,
            &estimate.lower,
            &estimate.upper,
        ];
        if values
            .into_iter()
            .any(|text| !fixed(text, estimate.grid_power))
            || !ordered(&estimate.lower, &estimate.upper)
            || estimate.standard_error.starts_with('-')
        {
            return Err("estimate grid or ordering".to_owned());
        }
    }
    if !fixed(&analysis.confidence, -6)
        || analysis
            .fences
            .iter()
            .any(|text| !fixed(text, analysis.fence_grid))
    {
        return Err("confidence or fence grid".to_owned());
    }
    if analysis.outlier_counts.iter().sum::<u64>() != rows {
        return Err("outlier count".to_owned());
    }
    if analysis.confidence != confidence {
        return Err(invalid("confidence mismatch"));
    }
    for pair in analysis.fences.windows(2) {
        if !ordered(&pair[0], &pair[1]) {
            return Err(invalid("fence order"));
        }
    }
    summary_rates(analysis)?;

    Ok(())
}

/// Reciprocal intervals can be unavailable without removing the time analysis or point rate.
fn summary_rates(analysis: &super::schema::SummaryAnalysis) -> Result<(), String> {
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
                    return Err(invalid("rate sign or endpoint order"));
                }
                Some([lower, upper])
            }
            (None, None, Some(reason)) if reason == "NonpositiveTimeEndpoint" => None,
            _ => return Err(invalid("reciprocal interval availability")),
        };
        if rate.numerator == 0 || !positive_rate(&rate.point) {
            return Err(invalid("rate numerator or point"));
        }
        for text in std::iter::once(&rate.point).chain(interval.into_iter().flatten()) {
            if !significant(text) {
                return Err(invalid("rate representation"));
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
                if !source || !native || text.starts_with('-') || !fixed(text, power) {
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
    record: &super::schema::SummaryCase,
    position: u64,
    raw: &super::schema::RawContentId,
) -> Result<(), String> {
    let states = match record.recipe.dependency {
        super::schema::Dependency::Dependent => Some(1_u64),
        super::schema::Dependency::Independent => Some(8),
        _ => None,
    };
    let started = matches!(
        record.outcome,
        SummaryOutcome::Complete(_) | SummaryOutcome::Incomplete(_)
    );
    let prework_capacity = matches!(
        record.outcome,
        SummaryOutcome::Incomplete(super::schema::Reason::Capacity)
    ) && record.counts.attempted == 0
        && record
            .pilot
            .as_ref()
            .is_some_and(|pilot| pilot.stop == super::schema::PilotStop::Capacity);
    let missing_capacity_inputs = prework_capacity && record.arithmetic.is_none();
    if (started && states.is_some() && !missing_capacity_inputs) != record.arithmetic.is_some() {
        return Err(invalid("arithmetic evidence presence"));
    }
    if let Some(arithmetic) = &record.arithmetic {
        let width = match record.recipe.width {
            super::schema::Width::Scalar | super::schema::Width::Simd1 => 1,
            super::schema::Width::Simd2 => 2,
            super::schema::Width::Simd4 => 4,
            super::schema::Width::Simd8 => 8,
            super::schema::Width::BackendSelectedBinary64 => {
                return Err(invalid("arithmetic width"));
            }
        };
        let fields = if matches!(
            record.recipe.operation,
            super::schema::Operation::FmaAddend
                | super::schema::Operation::FmaMultiplicand
                | super::schema::Operation::MultiplyAdd
        ) {
            2
        } else {
            1
        };
        let bytes = states.ok_or_else(|| invalid("arithmetic dependency"))? * width * 8;
        if arithmetic.cycle != 2
            || arithmetic.state_bytes != bytes
            || arithmetic.coefficient_bytes != bytes * fields * 2
            || arithmetic.exact_inputs.raw != *raw
            || arithmetic.exact_inputs.pointer
                != format!("/payload/cases/{position}/arithmetic_inputs")
        {
            return Err(invalid("arithmetic byte identity or reference"));
        }
    }
    Ok(())
}

/// Worker evidence has stable worker ordering, bounded storage and no exported address.
fn worker_layouts(
    layouts: &[super::schema::WorkerLayout],
    workers: u64,
    complete: bool,
) -> Result<(), String> {
    let count = u64::try_from(layouts.len()).map_err(|_| String::from("worker layouts"))?;
    if count > workers || (complete && count != workers) {
        return Err(String::from("worker layout population"));
    }
    for (index, layout) in layouts.iter().enumerate() {
        if usize::try_from(layout.worker).ok() != Some(index)
            || layout.first_touch_worker != layout.worker
        {
            return Err(String::from("worker layout ownership"));
        }
        if complete && layout.buffers.iter().all(Option::is_none) {
            return Err(String::from("missing numerical storage"));
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
            return Err(String::from("worker buffer total"));
        }
        for buffer in layout.buffers.iter().flatten() {
            let numerical = buffer.role != super::schema::BufferRole::Scratch;
            let aligned = buffer.address_modulo_64 % 8 == 0;
            if numerical && !aligned {
                return Err(String::from("worker buffer alignment"));
            }
            if buffer.live_bytes > buffer.capacity_bytes
                || buffer.address_modulo_64 >= 64
                || buffer.row_stride_bytes == 0
            {
                return Err(String::from("worker buffer layout"));
            }
        }
    }
    Ok(())
}

/// Reviewed source links stay in a fixed repository partition and carry full blob identities.
fn source_attachment(attachment: &super::schema::SourceAttachment) -> bool {
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
    let digest = match attachment.blob.algorithm {
        ObjectFormat::Sha1 => hex(&attachment.blob.digest, 40),
        ObjectFormat::Sha256 => hex(&attachment.blob.digest, 64),
    };
    name && digest
}
