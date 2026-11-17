// SPDX-License-Identifier: AGPL-3.0-only
//! Reproduction policy is separate from native invocation allowances.

use super::{Observation, TestResult, compare_endpoint, limits, payload, reproduce};
use nrz_cas::limits::{BudgetKind, Limits};
use nrz_cas::{Exhaustion, Fault, FaultKind};
use serde_json::Value;
use std::io::Write as _;

type Trial = fn(&Value, &Value, Limits) -> TestResult<Observation>;

fn reproduction_search_limits() -> Limits {
    limits()
}

fn comparison_search_limits() -> Limits {
    Limits {
        component_bits: 1_048_576,
        intermediate_bits: 2_097_152,
        ..limits()
    }
}

fn set(limits: &mut Limits, kind: BudgetKind, value: u64) {
    match kind {
        BudgetKind::Work => limits.work = value,
        BudgetKind::ArithmeticSteps => limits.arithmetic_steps = value,
        BudgetKind::ComponentBits => limits.component_bits = value,
        BudgetKind::IntermediateBits => limits.intermediate_bits = value,
        BudgetKind::OutputBytes => limits.output_bytes = value,
        BudgetKind::WitnessBytes => limits.witness_bytes = value,
        _ => unreachable!("only the profiled resources are selected"),
    }
}

fn exhausted(error: &(dyn std::error::Error + 'static), kind: BudgetKind) -> bool {
    matches!(error.downcast_ref::<Fault>().map(|fault| &fault.kind),
        Some(FaultKind::Exhausted { budget, .. }) if *budget == kind)
}

/// Manually bisect capacities; this ignored control does not run in the gate.
/// # Errors
/// Returns fixture, arithmetic, output or unexpected service errors.
/// # Panics
/// Panics if a capacity changes a retained outcome or interval, or no ceiling reproduces.
pub fn search() -> TestResult {
    let all = payload()?;
    for (name, trial, mut caps) in [
        (
            "reproduction",
            reproduce as Trial,
            reproduction_search_limits(),
        ),
        (
            "comparison",
            compare_endpoint as Trial,
            comparison_search_limits(),
        ),
    ] {
        let mut samples = Vec::new();
        for row in all["entries"].as_array().ok_or("entries")? {
            for endpoint in row["evidence"].as_array().ok_or("evidence")? {
                samples.push((row, endpoint, trial(row, endpoint, caps.clone())?));
            }
        }
        let mut calls = samples.len();
        for kind in [
            BudgetKind::ComponentBits,
            BudgetKind::IntermediateBits,
            BudgetKind::OutputBytes,
            BudgetKind::WitnessBytes,
        ] {
            let mut high = caps.get(kind);
            let mut low = match kind {
                BudgetKind::ComponentBits => 1,
                BudgetKind::IntermediateBits => caps.component_bits,
                _ => 0,
            };
            while low < high {
                let middle = low + (high - low) / 2;
                let mut probe = caps.clone();
                set(&mut probe, kind, middle);
                let mut passes = true;
                for (row, endpoint, baseline) in &samples {
                    calls += 1;
                    match trial(row, endpoint, probe.clone()) {
                        Ok(observed) => assert_eq!(
                            observed.signature, baseline.signature,
                            "capacity preserves outcome and retained interval"
                        ),
                        Err(error) if exhausted(error.as_ref(), kind) => {
                            passes = false;
                            break;
                        }
                        Err(error) => return Err(error),
                    }
                }
                if passes {
                    high = middle;
                } else {
                    low = middle + 1;
                }
            }
            set(&mut caps, kind, high);
            writeln!(
                std::io::stdout(),
                "CAPACITY {name} {kind:?} {high} calls {calls}"
            )?;
            // Name a witness for this coordinate's exact and one-below controls.
            if high > 0 && !(kind == BudgetKind::IntermediateBits && high == caps.component_bits) {
                calls += witness(name, trial, &caps, kind, &samples)?;
            }
        }
        let mut maximum = 0;
        for (row, endpoint, baseline) in &samples {
            calls += 1;
            let observed = trial(row, endpoint, caps.clone())?;
            assert_eq!(
                observed.signature, baseline.signature,
                "combined minima preserve reproduction"
            );
            if observed.work > maximum {
                maximum = observed.work;
                writeln!(
                    std::io::stdout(),
                    "WORK {name} {maximum} {} {} {}",
                    row["id"],
                    endpoint["endpoint"],
                    observed.phase
                )?;
            }
        }
        writeln!(
            std::io::stdout(),
            "PROFILE {name} {caps:?} work {maximum} calls {calls}"
        )?;
    }
    Ok(())
}

pub(super) fn reproduction_minimum() -> Limits {
    Limits {
        work: 151_316_399_978,
        arithmetic_steps: 351_478,
        component_bits: 42_586,
        intermediate_bits: 42_639,
        output_bytes: 1_110_440,
        witness_bytes: 2_768,
        ..limits()
    }
}

pub(super) fn reproduction_limits() -> Limits {
    Limits {
        work: 302_632_799_956,
        arithmetic_steps: 702_956,
        component_bits: 85_172,
        intermediate_bits: 85_278,
        output_bytes: 2_220_880,
        witness_bytes: 5_536,
        ..limits()
    }
}

/// Pin the observed counters and searched capacity boundaries on their limiting rows.
/// # Errors
/// Returns fixture, arithmetic, output or service errors.
/// # Panics
/// Panics if an exact boundary or its typed one-below refusal changes.
pub fn reproduction_boundaries() -> TestResult {
    boundaries(
        reproduce,
        &reproduction_minimum(),
        &reproduction_limits(),
        &[
            (BudgetKind::Work, "power-normal-next-near-one", "point"),
            (
                BudgetKind::ArithmeticSteps,
                "power-normal-next-near-one",
                "point",
            ),
            (BudgetKind::ComponentBits, "power-normal-near-one", "point"),
            (
                BudgetKind::IntermediateBits,
                "power-normal-near-one",
                "point",
            ),
            (
                BudgetKind::OutputBytes,
                "power-normal-next-near-one",
                "point",
            ),
            (
                BudgetKind::WitnessBytes,
                "power-normal-next-near-one",
                "point",
            ),
        ],
    )
}

pub(super) fn comparison_minimum() -> Limits {
    Limits {
        work: 163_753_305_695,
        arithmetic_steps: 437_034,
        component_bits: 294_401,
        intermediate_bits: 294_402,
        output_bytes: 0,
        witness_bytes: 0,
        ..limits()
    }
}

pub(super) fn comparison_limits() -> Limits {
    Limits {
        work: 327_506_611_390,
        arithmetic_steps: 874_068,
        component_bits: 588_802,
        intermediate_bits: 588_804,
        output_bytes: 0,
        witness_bytes: 0,
        ..limits()
    }
}

/// Pin the observed counters and searched capacity boundaries on their limiting rows.
/// # Errors
/// Returns fixture, arithmetic, output or service errors.
/// # Panics
/// Panics if an exact boundary or its typed one-below refusal changes.
pub fn comparison_boundaries() -> TestResult {
    boundaries(
        compare_endpoint,
        &comparison_minimum(),
        &comparison_limits(),
        &[
            (BudgetKind::Work, "power-normal-near-one", "point"),
            (
                BudgetKind::ArithmeticSteps,
                "power-normal-next-near-one",
                "point",
            ),
            (BudgetKind::ComponentBits, "power-normal-near-one", "point"),
            (
                BudgetKind::IntermediateBits,
                "power-normal-near-one",
                "point",
            ),
        ],
    )
}

fn boundaries(
    trial: Trial,
    minimum: &Limits,
    ceiling: &Limits,
    cases: &[(BudgetKind, &str, &str)],
) -> TestResult {
    let all = payload()?;
    for kind in [
        BudgetKind::Work,
        BudgetKind::ArithmeticSteps,
        BudgetKind::ComponentBits,
        BudgetKind::IntermediateBits,
        BudgetKind::OutputBytes,
        BudgetKind::WitnessBytes,
    ] {
        let figure = minimum.get(kind);
        let expected = figure * 2;
        assert_eq!(ceiling.get(kind), expected, "profile margin: {kind:?}");
    }
    for (kind, id, position) in cases {
        let row = super::row(&all, id)?;
        let endpoint = row["evidence"]
            .as_array()
            .ok_or("evidence")?
            .iter()
            .find(|endpoint| endpoint["endpoint"] == *position)
            .ok_or("endpoint")?;
        let baseline = trial(row, endpoint, ceiling.clone())?;
        let exact = trial(row, endpoint, minimum.clone())?;
        assert_eq!(
            exact.signature, baseline.signature,
            "exact capacity retains outcome and interval: {kind:?}"
        );
        assert_eq!(
            exact.work, baseline.work,
            "capacity preserves Work: {kind:?}"
        );
        assert_eq!(
            exact.arithmetic_steps, baseline.arithmetic_steps,
            "capacity preserves arithmetic steps: {kind:?}"
        );
        match kind {
            BudgetKind::Work => assert_eq!(exact.work, minimum.work, "heaviest ledger Work"),
            BudgetKind::ArithmeticSteps => assert_eq!(
                exact.arithmetic_steps, minimum.arithmetic_steps,
                "heaviest ledger arithmetic steps"
            ),
            _ => {}
        }
        let mut below = minimum.clone();
        set(&mut below, *kind, minimum.get(*kind) - 1);
        let error = trial(row, endpoint, below).expect_err("one-below capacity must refuse");
        let reason = match kind {
            BudgetKind::ComponentBits | BudgetKind::IntermediateBits => Exhaustion::ArithmeticRange,
            _ => Exhaustion::Limit,
        };
        assert!(
            matches!(error.downcast_ref::<Fault>().map(|fault| &fault.kind),
            Some(FaultKind::Exhausted { reason: found, budget, limit, needed })
                if *found == reason && *budget == *kind && *limit == minimum.get(*kind) - 1 && needed > limit),
            "one-below resource cause and boundary: {kind:?}: {error}"
        );
        writeln!(
            std::io::stdout(),
            "BOUNDARY {kind:?} {id} {position} exact {} one-below {error}",
            minimum.get(*kind)
        )?;
    }
    Ok(())
}

/// Name a limiting row without changing the coordinate search.
/// # Panics
/// Panics if the lower capacity refuses for a different resource.
fn witness(
    name: &str,
    trial: Trial,
    caps: &Limits,
    kind: BudgetKind,
    samples: &[(&Value, &Value, Observation)],
) -> TestResult<usize> {
    let high = caps.get(kind);
    let mut calls = 0;
    let mut below = caps.clone();
    set(&mut below, kind, high - 1);
    for (row, endpoint, _) in samples {
        calls += 1;
        if let Err(error) = trial(row, endpoint, below.clone()) {
            assert!(
                exhausted(error.as_ref(), kind),
                "capacity witness cause: {error}"
            );
            writeln!(
                std::io::stdout(),
                "WITNESS {name} {kind:?} {} {} {error}",
                row["id"],
                endpoint["endpoint"]
            )?;
            break;
        }
    }
    Ok(calls)
}
