// SPDX-License-Identifier: AGPL-3.0-only
//! In-memory raw evidence fixtures never write observation reports into the repository.

use super::*;
use crate::report::{CaseRecord, Execution, Measurement, PhaseDurations, RawContentId, RunPayload};
use crate::{
    Binary64, CaseRequest, Dependency, Operation, Placement, RunRequest, SamplingPolicy, Scope,
    Seed, Width,
};

/// Explicit fixed synthetic observations; these are not machine measurements.
fn request(summary: &SummaryDataset) -> RunRequest {
    RunRequest {
        version: [1, 0, 0],
        run_id: "fixture".to_owned(),
        seeds: vec![Seed(1)],
        cases: vec![CaseRequest {
            operation: Operation::Add,
            width: Width::Scalar,
            dependency: Dependency::Dependent,
            scope: Scope::Prepared,
            shape: Vec::new(),
            item_length: 1,
            workers: 1,
            placement: Placement::Unbound,
            allowed_cpus: Vec::new(),
            budget_ns: 1_000_000_000,
            diagonal_margin: None,
            inputs: Vec::new(),
            cycles: Vec::new(),
        }],
        sampling: SamplingPolicy {
            samples: 10,
            resamples: 1000,
            confidence: Binary64(0.95_f64.to_bits()),
            warm_up_ns: 1,
            measurement_ns: 1,
            analysis_report_bytes: 1_048_576,
            analysis_scratch_bytes: 4_194_304,
        },
        resources: summary.payload.request.resources,
    }
}

/// Build exact chronological row sums, independent of the collector.
fn observations() -> Result<Observations, Box<dyn std::error::Error>> {
    let mut pilot = crate::pilot::stopped(crate::pilot::PilotStop::Complete);
    pilot.observed_tick_ns = Some(1);
    pilot.empty_ns = vec![1; 256];
    pilot.target_ns = Some(100_000);
    pilot.batches = vec![crate::pilot::PilotBatch {
        length: 1,
        elapsed_ns: vec![100_000; 8],
    }];
    pilot.attempted = 264;
    pilot.accepted_length = Some(1);
    let mut item_ns = vec![Some(1)];
    let mut rows = Vec::new();
    for iterations in 1..=10 {
        let duration = 100_000 + iterations * 1000;
        item_ns.extend(std::iter::repeat_n(
            Some(duration),
            usize::try_from(iterations)?,
        ));
        let elapsed_ns = iterations * duration;
        rows.push(crate::report::RawRow {
            iterations,
            elapsed_ns,
            seconds: crate::statistics::RawValue::time(
                std::time::Duration::from_nanos(elapsed_ns).as_secs_f64(),
                -9,
                Rounding::Nearest,
            )?,
        });
    }
    Ok(Observations {
        worker_layouts: vec![crate::layout::WorkerLayout {
            worker: 0,
            numerical_capacity_bytes: 512,
            numerical_live_bytes: 8,
            first_touch_worker: 0,
            startup_cpu: None,
            allowed_cpus: crate::layout::Allowance::UnavailableProbe,
            allowed_nodes: crate::layout::Allowance::UnavailableProbe,
            numa: crate::layout::NumaEvidence::UnavailableSafePageQuery,
            buffers: [
                Some(crate::layout::BufferLayout {
                    role: crate::layout::BufferRole::State,
                    capacity_bytes: 512,
                    live_bytes: 8,
                    address_modulo_64: 0,
                    row_stride_bytes: 8,
                    column_stride_bytes: 64,
                }),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ],
        }],
        pilot,
        item_ns,
        exceptions: Vec::new(),
        rows,
        linear_step: Some(1),
        sampler_stop: crate::sampling_evidence::SamplingStop::Complete,
        counts: Counts {
            attempted: 320,
            measured: 55,
            discarded: 265,
            pilot: 264,
            warm_up: 1,
            measurement: 55,
            rows: 10,
            resamples: 1000,
        },
        phases: PhaseDurations {
            pilot_ns: 800_256,
            warm_up_ns: 1,
            measurement_ns: 5_885_000,
            ..PhaseDurations::default()
        },
    })
}

/// Pure host analysis uses fixed rows and a fixed recorded bootstrap seed.
fn measurement(request: &RunRequest) -> Result<Measurement, Box<dyn std::error::Error>> {
    let observations = observations()?;
    let rows: Vec<_> = observations
        .rows
        .iter()
        .map(|row| nrz_matrix::sampling::SampleRow {
            iterations: row.iterations,
            elapsed_seconds: f64::from_bits(row.seconds.source_bits.0),
        })
        .collect();
    let settings = nrz_matrix::sampling::AnalysisSettings::new(
        10,
        1000,
        0.95,
        1,
        nrz_matrix::sampling::AnalysisCaps {
            report_bytes: 1_048_576,
            scratch_bytes: 4_194_304,
        },
    )?;
    let host = nrz_matrix::sampling::analyze(&rows, settings, Some(1), |_| false)?;
    let analysis = crate::statistics::RawAnalysis::from_host(&host)?;
    let numerators = crate::recipe::arithmetic_counts(&request.cases[0], 1)?;
    let rate = crate::statistics::RawRate::from_time(&analysis.estimates[4], 1)?;
    Ok(Measurement {
        observations,
        analysis,
        numerators,
        rates: [
            Some(rate.clone()),
            Some(rate.clone()),
            None,
            Some(rate),
            None,
        ],
    })
}

/// Shared public fixture supplies tagged provenance, while raw evidence is assembled in memory.
fn raw() -> Result<RunReport, Box<dyn std::error::Error>> {
    let summary: SummaryDataset = serde_json::from_slice(include_bytes!(
        "../../../../data/fixtures/compute/valid-complete.json"
    ))?;
    let request = request(&summary);
    let measurement = measurement(&request)?;
    let arithmetic_inputs = crate::kernels::arithmetic_inputs(&request.cases[0], Seed(1))?;
    let publication = crate::PublicProjection::captured(
        &summary.payload.build,
        &summary.payload.environment,
        &[],
        &mut crate::memory::Budget::new(request.resources.report_bytes),
    )?;
    Ok(RunReport {
        namespace: "org.naturrhythmuszentrum.compute.raw".to_owned(),
        version: [1, 1, 0],
        owner: "NRZ".to_owned(),
        license: "CC0-1.0".to_owned(),
        sources: Vec::new(),
        labels: Vec::new(),
        payload: RunPayload {
            identity: summary.payload.identity,
            build: summary.payload.build,
            environment: summary.payload.environment,
            publication,
            request,
            execution: Execution {
                complete: true,
                elapsed_ns: Some(6_685_257),
                ..summary.payload.execution
            },
            cases: vec![CaseRecord {
                case_index: 0,
                repetition: 0,
                seed: Seed(1),
                effective_offset: 1,
                accepted_length: Some(1),
                arithmetic_inputs,
                lc_ledger: None,
                packing: None,
                load_before: None,
                load_after: None,
                outcome: CaseOutcome::Complete(Box::new(measurement)),
            }],
        },
    })
}

#[test]
fn raw_roundtrip_and_summary_bytes_preserve_fixed_source_evidence()
-> Result<(), Box<dyn std::error::Error>> {
    let raw = raw()?;
    let mut bytes = Vec::new();
    write_report(&raw, &mut bytes)?;
    let decoded = read_report(bytes.as_slice(), u64::try_from(bytes.len())?)?;
    assert_eq!(decoded, raw);
    let id = RawContentId {
        algorithm: crate::report::ObjectFormat::Sha1,
        digest: "0".repeat(40),
    };
    let a = crate::summarize(&raw, &id)?;
    let b = crate::summarize(&decoded, &id)?;
    let mut first = Vec::new();
    let mut second = Vec::new();
    write_summary(&a, &mut first)?;
    write_summary(&b, &mut second)?;
    assert_eq!(first, second);
    let mut alternate = vec![b' '];
    alternate.extend(bytes);
    assert!(read_report(alternate.as_slice(), u64::try_from(alternate.len())?).is_err());
    Ok(())
}

#[test]
fn altered_sources_and_failed_items_cannot_enter_complete_rows()
-> Result<(), Box<dyn std::error::Error>> {
    let mut raw = raw()?;
    let CaseOutcome::Complete(measurement) = &mut raw.payload.cases[0].outcome else {
        return Err("complete fixture required".into());
    };
    measurement
        .observations
        .exceptions
        .push(crate::report::AttemptException {
            attempt: 1,
            outcome: crate::sampling_evidence::IterationOutcome::Returned(Err(
                crate::recipe::RecipeError::Domain,
            )),
            finalization: [None, None],
        });
    assert!(validate_report(&raw).is_err());
    Ok(())
}

#[test]
fn public_redaction_changes_projection_without_inventing_a_known_fact()
-> Result<(), Box<dyn std::error::Error>> {
    let mut raw = raw()?;
    raw.payload.publication.environment.cpu_model =
        crate::Evidence::Redacted("reviewed removal".to_owned());
    let id = RawContentId {
        algorithm: crate::report::ObjectFormat::Sha1,
        digest: "1".repeat(40),
    };
    let summary = crate::summarize(&raw, &id)?;
    assert_eq!(
        summary.payload.environment.cpu_model,
        raw.payload.publication.environment.cpu_model
    );
    raw.payload.publication.environment.cpu_model = crate::Evidence::Known("invented".to_owned());
    assert!(validate_report(&raw).is_err());
    let wrong = RawContentId {
        algorithm: crate::report::ObjectFormat::Sha256,
        digest: "0".repeat(64),
    };
    assert!(crate::summarize(&raw, &wrong).is_err());
    Ok(())
}

#[test]
fn arithmetic_bits_and_accepted_length_are_bound_to_raw_recipe()
-> Result<(), Box<dyn std::error::Error>> {
    let mut report = raw()?;
    let inputs = report.payload.cases[0]
        .arithmetic_inputs
        .as_mut()
        .ok_or("arithmetic input fixture")?;
    inputs.pair[0][0].0 ^= 1;
    assert!(validate_report(&report).is_err());
    let mut report = raw()?;
    report.payload.cases[0].accepted_length = Some(2);
    assert!(validate_report(&report).is_err());
    Ok(())
}

#[test]
fn explicit_reanalysis_reconstructs_all_raw_statistics_and_honors_cancellation()
-> Result<(), Box<dyn std::error::Error>> {
    let mut report = raw()?;
    report.payload.identity.source_revision = nrz_matrix::sampling::SOURCE_REVISION.to_owned();
    report.payload.identity.estimator_revision =
        nrz_matrix::sampling::ESTIMATOR_REVISION.to_owned();
    crate::replay::verify_raw_analysis(&report, |_| false)?;
    assert!(matches!(
        crate::replay::verify_raw_analysis(&report, |_| true),
        Err(crate::replay::ReplayError::Analysis(_))
    ));
    if let CaseOutcome::Complete(measurement) = &mut report.payload.cases[0].outcome {
        measurement.analysis.estimates[0].point.source_bits.0 += 1;
    }
    validate_report(&report)?;
    assert!(matches!(
        crate::replay::verify_raw_analysis(&report, |_| false),
        Err(crate::replay::ReplayError::Mismatch)
    ));
    report.payload.identity.estimator_revision = "unavailable-estimator".to_owned();
    assert!(matches!(
        crate::replay::verify_raw_analysis(&report, |_| false),
        Err(crate::replay::ReplayError::Revision)
    ));
    Ok(())
}

#[test]
fn nonpositive_slope_endpoint_keeps_raw_report_and_summary()
-> Result<(), Box<dyn std::error::Error>> {
    for lower in [0.0, -0.000_001] {
        let mut report = raw()?;
        let CaseOutcome::Complete(measurement) = &mut report.payload.cases[0].outcome else {
            return Err("complete fixture required".into());
        };
        let slope = crate::statistics::RawEstimate::from_host(nrz_matrix::sampling::Estimate {
            point_estimate: 0.000_001,
            standard_error: 0.000_001,
            confidence_interval: nrz_matrix::sampling::ConfidenceInterval {
                confidence_level: 0.95,
                lower_bound: lower,
                upper_bound: 0.000_003,
            },
        })?;
        measurement.analysis.estimates[4] = slope.clone();
        for rate in measurement.rates.iter_mut().flatten() {
            *rate = crate::statistics::RawRate::from_time(&slope, rate.decimal.numerator)?;
            assert_eq!(rate.source_bits[1..], [None, None]);
            assert_eq!(
                rate.decimal.interval_unavailable.as_deref(),
                Some("NonpositiveTimeEndpoint")
            );
        }
        validate_report(&report)?;
        let id = RawContentId {
            algorithm: crate::report::ObjectFormat::Sha1,
            digest: "0".repeat(40),
        };
        let summary = crate::summarize(&report, &id)?;
        let mut bytes = Vec::new();
        write_summary(&summary, &mut bytes)?;
        assert_eq!(
            read_summary(bytes.as_slice(), u64::try_from(bytes.len())?)?,
            summary
        );
    }
    Ok(())
}

#[test]
fn clock_stop_keeps_complete_case_evidence_exportable() -> Result<(), Box<dyn std::error::Error>> {
    let mut report = raw()?;
    let cases = report.payload.cases.clone();
    let execution = &mut report.payload.execution;
    execution.elapsed_ns = None;
    execution.overshoot_ns = None;
    execution.phases = None;
    execution.stop_reason = Some(crate::report::Reason::Clock);
    execution.complete = false;
    let mut bytes = Vec::new();
    write_report(&report, &mut bytes)?;
    let decoded = read_report(bytes.as_slice(), u64::try_from(bytes.len())?)?;
    assert_eq!(decoded.payload.cases, cases);
    let id = RawContentId {
        algorithm: crate::report::ObjectFormat::Sha1,
        digest: "0".repeat(40),
    };
    let summary = crate::summarize(&decoded, &id)?;
    assert!(matches!(
        summary.payload.cases[0].outcome,
        SummaryOutcome::Complete(_)
    ));
    assert!(!summary.payload.execution.complete);
    Ok(())
}
