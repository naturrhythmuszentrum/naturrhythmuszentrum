// SPDX-License-Identifier: AGPL-3.0-only
//! Exact projection of retained before/after load evidence.

use crate::{
    Evidence,
    decimal::{self, Exact, Rounding},
    telemetry::{CpuCounters, DecimalText, LoadSnapshot, Pressure},
};
use serde::{Deserialize, Serialize};

/// Fixed native-unit decimal telemetry with its retained exact source resolution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoadValue {
    /// Canonical decimal in the declared native unit.
    pub value: String,
    /// Decimal quantum exponent.
    pub grid_power: i32,
    /// Native source unit, never silently converted.
    pub unit: String,
    /// Exact source resolution numerator.
    pub resolution_numerator: u64,
    /// Positive source resolution denominator.
    pub resolution_denominator: u64,
}

/// Fixed PSI projection, without its source decimal coefficients.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryPressure {
    /// Native percent averages for ten, sixty and three hundred seconds.
    pub averages_percent: [LoadValue; 3],
    /// Native cumulative microseconds.
    pub total_usec: u64,
}

/// Fixed-size load snapshot; no source history or private membership path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryLoad {
    /// Named hwmon-zero sensor; no assumption that it measures the CPU package.
    pub thermal_sensor: Evidence<String>,
    /// Native signed millidegrees Celsius from the named sensor.
    pub temperature_millidegrees: Evidence<i64>,
    /// CPU-zero scaling frequency in native integer kHz, never wall-time cycles.
    pub scaling_frequency_khz: Evidence<u64>,
    /// CPU-zero frequency in native decimal MHz.
    pub cpu_mhz: Evidence<LoadValue>,

    /// Capture offset from run entry.
    pub offset_ns: u64,
    /// Native runnable-task averages.
    pub load_tasks: Evidence<[LoadValue; 3]>,
    /// Runnable task population.
    pub runnable_tasks: Evidence<u64>,
    /// Total task population.
    pub total_tasks: Evidence<u64>,
    /// System some pressure.
    pub system_some: Evidence<SummaryPressure>,
    /// System full pressure.
    pub system_full: Evidence<SummaryPressure>,
    /// Applicable native cgroup counters, retained for interval reconstruction.
    pub cpu: Evidence<CpuCounters>,
    /// Applicable some pressure.
    pub cgroup_some: Evidence<SummaryPressure>,
    /// Applicable full pressure.
    pub cgroup_full: Evidence<SummaryPressure>,
    /// Recorded allowed CPU population.
    pub allowed_cpus: Evidence<u64>,
    /// Capture method without private paths.
    pub method: String,
}

/// Exact interval fractions, unavailable when their retained sources do not suffice.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoadFractions {
    /// CPU usage divided by elapsed wall time and recorded allowed population.
    pub utilization: Evidence<LoadValue>,
    /// Throttled periods divided by scheduling periods.
    pub throttling: Evidence<LoadValue>,
}

/// Preserve the exact evidence state while transforming known numeric sources.
fn map<T, U>(
    value: &Evidence<T>,
    transform: impl FnOnce(&T) -> Result<U, decimal::DecimalError>,
) -> Result<Evidence<U>, decimal::DecimalError> {
    Ok(match value {
        Evidence::Known(value) => Evidence::Known(transform(value)?),
        Evidence::Unknown(reason) => Evidence::Unknown(reason.clone()),
        Evidence::Unavailable(reason) => Evidence::Unavailable(reason.clone()),
        Evidence::Redacted(reason) => Evidence::Redacted(reason.clone()),
    })
}

/// Round native decimal text once, from its retained coefficient and resolution.
fn value(source: &DecimalText, unit: &str) -> Result<LoadValue, decimal::DecimalError> {
    value_grid(source, unit, -2)
}

/// Native frequency and load fields retain their distinct display sensitivities.
fn value_grid(
    source: &DecimalText,
    unit: &str,
    nominal: i32,
) -> Result<LoadValue, decimal::DecimalError> {
    let exact = Exact::decimal(source.coefficient, source.scale)?;
    let resolution = Exact::ratio(source.resolution_numerator, source.resolution_denominator)?;
    let grid_power = exact.telemetry_grid(nominal, resolution)?;
    Ok(LoadValue {
        value: decimal::fixed(exact, grid_power, Rounding::Nearest)?,
        grid_power,
        unit: unit.to_owned(),
        resolution_numerator: source.resolution_numerator,
        resolution_denominator: source.resolution_denominator,
    })
}

/// Fixed three-value native telemetry shape.
fn averages(
    source: &[DecimalText; 3],
    unit: &str,
) -> Result<[LoadValue; 3], decimal::DecimalError> {
    let [a, b, c] = source.each_ref().map(|source| value(source, unit));
    Ok([a?, b?, c?])
}

/// PSI keeps its native percent and microsecond units.
fn pressure(source: &Pressure) -> Result<SummaryPressure, decimal::DecimalError> {
    Ok(SummaryPressure {
        averages_percent: averages(&source.averages_percent, "percent")?,
        total_usec: source.total_usec,
    })
}

/// Project captured load without reading a clock, counter or sensor.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) fn project(source: &LoadSnapshot) -> Result<SummaryLoad, decimal::DecimalError> {
    Ok(SummaryLoad {
        thermal_sensor: source.thermal_sensor.clone(),
        temperature_millidegrees: source.temperature_millidegrees.clone(),
        scaling_frequency_khz: source.scaling_frequency_khz.clone(),
        cpu_mhz: map(&source.cpu_mhz, |source| value_grid(source, "MHz", -6))?,
        offset_ns: source.offset_ns,
        load_tasks: map(&source.load_tasks, |values| {
            averages(values, "runnable tasks")
        })?,
        runnable_tasks: source.runnable_tasks.clone(),
        total_tasks: source.total_tasks.clone(),
        system_some: map(&source.system_some, pressure)?,
        system_full: map(&source.system_full, pressure)?,
        cpu: source.cpu.clone(),
        cgroup_some: map(&source.cgroup_some, pressure)?,
        cgroup_full: map(&source.cgroup_full, pressure)?,
        allowed_cpus: source.allowed_cpus.clone(),
        method: source.method.clone(),
    })
}

/// Derive exact fractions only from nondecreasing counters and a stable recorded CPU count.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) fn fractions(before: &LoadSnapshot, after: &LoadSnapshot) -> LoadFractions {
    let counters = match (&before.cpu, &after.cpu) {
        (Evidence::Known(a), Evidence::Known(b)) => Some((a, b)),
        _ => None,
    };
    let utilization = counters.and_then(|(a, b)| {
        let (Evidence::Known(n), Evidence::Known(m)) = (&before.allowed_cpus, &after.allowed_cpus)
        else {
            return None;
        };
        if n != m || *n == 0 {
            return None;
        }
        let elapsed = after.offset_ns.checked_sub(before.offset_ns)?;
        Some((
            Exact::utilization(b.usage_usec.checked_sub(a.usage_usec)?, elapsed, *n).ok()?,
            Exact::utilization(1, elapsed, *n).ok()?,
        ))
    });
    let throttling = counters.and_then(|(a, b)| {
        match (
            &a.periods,
            &b.periods,
            &a.throttled_periods,
            &b.throttled_periods,
        ) {
            (Evidence::Known(p), Evidence::Known(q), Evidence::Known(t), Evidence::Known(u)) => u
                .checked_sub(*t)
                .zip(q.checked_sub(*p))
                .and_then(|(n, d)| Some((Exact::ratio(n, d).ok()?, Exact::ratio(1, d).ok()?))),
            _ => None,
        }
    });
    LoadFractions {
        utilization: fraction(utilization.as_ref()),
        throttling: fraction(throttling.as_ref()),
    }
}

/// Nominal unit-one grid refines to preserve every nonzero derived fraction.
fn fraction(exact: Option<&(Exact, Exact)>) -> Evidence<LoadValue> {
    let projected = exact.and_then(|(exact, resolution)| {
        let (resolution_numerator, resolution_denominator) = resolution.bounded_ratio().ok()?;
        let grid_power = exact.telemetry_grid(-4, *resolution).ok()?;
        Some(LoadValue {
            value: decimal::fixed(*exact, grid_power, Rounding::Nearest).ok()?,
            grid_power,
            unit: "one".to_owned(),
            resolution_numerator,
            resolution_denominator,
        })
    });
    projected.map_or_else(
        || {
            Evidence::Unavailable(
                "missing, changed, zero or unencodable counter resolution".to_owned(),
            )
        },
        Evidence::Known,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_percent_ties_and_small_fractions_keep_their_units()
    -> Result<(), decimal::DecimalError> {
        let source = DecimalText {
            coefficient: 150,
            scale: 2,
            resolution_numerator: 1,
            resolution_denominator: 100,
        };
        let projected = value(&source, "percent")?;
        assert_eq!(projected.value, "1.50");
        assert_eq!(projected.unit, "percent");
        let small = DecimalText {
            coefficient: 1,
            scale: 2,
            resolution_numerator: 1,
            resolution_denominator: 100,
        };
        assert_eq!(value(&small, "percent")?.value, "0.0100");
        let Evidence::Known(fraction) = fraction(Some(&(
            Exact::ratio(1, 1_000_000)?,
            Exact::ratio(1, 1_000_000)?,
        ))) else {
            panic!("positive ratio is representable");
        };
        assert_eq!(fraction.value, "0.00000100");
        assert_eq!(fraction.grid_power, -8);
        Ok(())
    }

    #[test]
    fn unusable_fraction_denominators_remain_unavailable() {
        assert!(matches!(fraction(None), Evidence::Unavailable(_)));
        assert!(matches!(
            fraction(Exact::ratio(1, 0).ok().map(|value| (value, value)).as_ref()),
            Evidence::Unavailable(_)
        ));
    }
}
