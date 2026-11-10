// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit JSON export of the sampler's public evidence, never a new estimator.
use super::sampling::{Error, Work};
use nrz_matrix::sampling::{AnalysisReport, ConfidenceInterval, Estimate, SamplingReport};
use std::io::{self, Write};

/// Debug formatting is confined to enum diagnostics and correctly quoted strings.
fn quoted(out: &mut impl Write, value: &impl std::fmt::Debug) -> io::Result<()> {
    let text = format!("{value:?}");
    string(out, &text)
}
/// Escape JSON text without depending on a serialization framework.
fn string(out: &mut impl Write, text: &str) -> io::Result<()> {
    write!(out, "\"")?;
    for c in text.chars() {
        match c {
            '"' => write!(out, "\\\"")?,
            '\\' => write!(out, "\\\\")?,
            '\n' => write!(out, "\\n")?,
            '\r' => write!(out, "\\r")?,
            '\t' => write!(out, "\\t")?,
            c if c.is_control() => write!(out, "\\u{:04x}", u32::from(c))?,
            c => write!(out, "{c}")?,
        }
    }
    write!(out, "\"")
}
/// Missing evidence remains JSON null.
fn optional(out: &mut impl Write, value: Option<u64>) -> io::Result<()> {
    match value {
        Some(value) => write!(out, "{value}"),
        None => write!(out, "null"),
    }
}
/// Missing evidence is null; platform non-participation is explicitly named.
fn work(out: &mut impl Write, value: Option<Work>) -> io::Result<()> {
    match value {
        None => write!(out, "null"),
        Some(Work::Engine(value)) => write!(out, "{{\"engine_work\":{value}}}"),
        Some(Work::PlatformNoEngineWork) => string(out, "platform-no-engine-work"),
    }
}
/// Public confidence endpoints retain the sampler's estimator semantics.
fn interval(out: &mut impl Write, ci: ConfidenceInterval) -> io::Result<()> {
    write!(
        out,
        "{{\"confidence_level\":{},\"lower_bound\":{},\"upper_bound\":{}}}",
        ci.confidence_level, ci.lower_bound, ci.upper_bound
    )
}
/// Every estimate retains its standard error as well as its interval.
fn estimate(out: &mut impl Write, value: Estimate) -> io::Result<()> {
    write!(
        out,
        "{{\"point_estimate\":{},\"standard_error\":{},\"confidence_interval\":",
        value.point_estimate, value.standard_error
    )?;
    interval(out, value.confidence_interval)?;
    write!(out, "}}")
}
/// Complete analysis includes unpruned observations, rates and all five estimates.
fn analysis(out: &mut impl Write, value: &AnalysisReport) -> io::Result<()> {
    let estimates = value.estimates();
    write!(out, "{{\"estimates\":{{")?;
    for (i, (name, e)) in [
        ("mean", estimates.mean),
        ("median", estimates.median),
        ("standard_deviation", estimates.standard_deviation),
        (
            "median_absolute_deviation",
            estimates.median_absolute_deviation,
        ),
        ("slope", estimates.slope),
    ]
    .into_iter()
    .enumerate()
    {
        if i != 0 {
            write!(out, ",")?;
        }
        string(out, name)?;
        write!(out, ":")?;
        estimate(out, e)?;
    }
    write!(out, "}},\"throughput\":{{")?;
    for (i, (name, rate)) in [
        (
            "items_per_second",
            Some(value.throughput().items_per_second),
        ),
        (
            "elements_per_second",
            value.throughput().elements_per_second,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        if i != 0 {
            write!(out, ",")?;
        }
        string(out, name)?;
        write!(out, ":")?;
        if let Some(rate) = rate {
            write!(
                out,
                "{{\"point_estimate\":{},\"confidence_interval\":",
                rate.point_estimate
            )?;
            interval(out, rate.confidence_interval)?;
            write!(out, "}}")?;
        } else {
            write!(out, "null")?;
        }
    }
    let f = value.fences();
    write!(
        out,
        concat!(
            "}},\"fences\":{{\"lower_outer\":{},\"lower_inner\":{},\"upper_inner\":{},",
            "\"upper_outer\":{}}},\"label_counts\":{:?},\"observations\":[",
        ),
        f.lower_outer,
        f.lower_inner,
        f.upper_inner,
        f.upper_outer,
        value.label_counts()
    )?;
    for (i, row) in value.observations().iter().enumerate() {
        if i != 0 {
            write!(out, ",")?;
        }
        write!(
            out,
            "{{\"iterations\":{},\"elapsed_seconds\":{},\"seconds_per_item\":{},\"label\":",
            row.row.iterations, row.row.elapsed_seconds, row.seconds_per_item
        )?;
        quoted(out, &row.label)?;
        write!(out, "}}")?;
    }
    write!(out, "]}}")
}

/// Every public collection field is exported, including failures and partial rows.
pub(super) fn write(
    out: &mut impl Write,
    report: &SamplingReport<'_, Work, Error>,
) -> io::Result<()> {
    settings(out, report)?;
    let b = report.adapter_bounds();
    let id = report.identity();
    write!(out, "\"identity\":{{\"representative\":")?;
    string(out, id.representative)?;
    write!(out, ",\"shape\":{:?},\"operation_revision\":", id.shape)?;
    string(out, id.operation_revision)?;
    write!(out, ",\"workload\":")?;
    string(out, id.workload)?;
    write!(out, ",\"scope\":")?;
    string(out, id.scope)?;
    write!(out, ",\"trace_configuration\":")?;
    string(out, id.trace_configuration)?;
    write!(out, "}},\"elements_per_item\":")?;
    optional(out, report.elements_per_item())?;
    write!(
        out,
        concat!(
            ",\"adapter_bounds_ns\":{{\"uninterrupted\":{},\"receipt\":{},",
            "\"disposal\":{},\"cleanup\":{}}},\"uninterrupted_bound_ns\":{},",
            "\"finalization_bound_ns\":{},\"warm_up_iterations\":{},",
            "\"warm_up_elapsed_ns\":{},\"measured_elapsed_ns\":{},",
            "\"total_elapsed_ns\":{},\"overshoot_ns\":{},\"achieved_resamples\":{},",
            "\"linear_step\":",
        ),
        b.uninterrupted.as_nanos(),
        b.receipt.as_nanos(),
        b.disposal.as_nanos(),
        b.cleanup.as_nanos(),
        report.uninterrupted_bound().as_nanos(),
        report.finalization_bound().as_nanos(),
        report.warm_up_iterations(),
        report.warm_up_elapsed().as_nanos(),
        report.measured_elapsed().as_nanos(),
        report.total_elapsed().as_nanos(),
        report.overshoot().as_nanos(),
        report.achieved_resamples()
    )?;
    optional(out, report.linear_step())?;
    write!(out, ",\"source_revision\":")?;
    string(out, report.source_revision())?;
    write!(out, ",\"estimator_revision\":")?;
    string(out, report.estimator_revision())?;
    write!(out, ",\"stop\":")?;
    quoted(out, report.stop())?;
    write!(out, ",\"iterations\":[")?;
    for (i, r) in report.iterations().iter().enumerate() {
        if i != 0 {
            write!(out, ",")?;
        }
        write!(out, "{{\"position\":")?;
        quoted(out, &r.position)?;
        write!(out, ",\"outcome\":")?;
        quoted(out, &r.outcome)?;
        write!(out, ",\"elapsed_seconds\":")?;
        if let Some(t) = r.elapsed {
            write!(out, "{}", t.as_secs_f64())?;
        } else {
            write!(out, "null")?;
        }
        write!(out, ",\"entry\":")?;
        work(out, r.entry)?;
        write!(out, ",\"receipt\":")?;
        if let Some(w) = r.receipt {
            write!(out, "{{\"exit\":")?;
            work(out, w.exit)?;
            write!(out, ",\"operation\":")?;
            work(out, w.operation)?;
            write!(out, ",\"status\":")?;
            quoted(out, &w.status)?;
            write!(out, "}}")?;
        } else {
            write!(out, "null")?;
        }
        write!(out, ",\"disposal\":")?;
        quoted(out, &r.disposal)?;
        write!(out, ",\"cleanup\":")?;
        quoted(out, &r.cleanup)?;
        write!(out, "}}")?;
    }
    write!(out, "],\"rows\":[")?;
    for (i, row) in report.rows().iter().enumerate() {
        if i != 0 {
            write!(out, ",")?;
        }
        write!(
            out,
            "{{\"iterations\":{},\"elapsed_seconds\":{}}}",
            row.iterations, row.elapsed_seconds
        )?;
    }
    write!(out, "],\"analysis\":")?;
    if let Some(a) = report.analysis() {
        analysis(out, a)?;
    } else {
        write!(out, "null")?;
    }
    write!(out, "}}")
}

/// Preserve every explicit collection and analysis setting.
fn settings(out: &mut impl Write, report: &SamplingReport<'_, Work, Error>) -> io::Result<()> {
    let s = report.settings();
    let a = s.analysis;
    write!(
        out,
        concat!(
            "{{\"settings\":{{\"analysis\":{{\"samples\":{},\"resamples\":{},",
            "\"confidence\":{},\"seed\":{},\"report_bytes\":{},\"scratch_bytes\":{}}},",
            "\"warm_up_ns\":{},\"measurement_ns\":{},\"wall_budget_ns\":{},",
            "\"max_total_iterations\":{},\"max_iterations_per_sample\":{},",
            "\"report_bytes\":{},\"scratch_bytes\":{},\"sampler_uninterrupted_ns\":{},",
            "\"sampler_finalization_ns\":{}}},",
        ),
        a.samples(),
        a.resamples(),
        a.confidence(),
        a.seed(),
        a.caps().report_bytes,
        a.caps().scratch_bytes,
        s.warm_up.as_nanos(),
        s.measurement.as_nanos(),
        s.wall_budget.as_nanos(),
        s.max_total_iterations,
        s.max_iterations_per_sample,
        s.caps.report_bytes,
        s.caps.scratch_bytes,
        s.bounds.uninterrupted.as_nanos(),
        s.bounds.finalization.as_nanos()
    )?;
    Ok(())
}
