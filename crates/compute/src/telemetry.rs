// SPDX-License-Identifier: AGPL-3.0-only
//! Exact text/counter telemetry with explicit unavailable states.

use crate::{
    Evidence,
    decimal::{self, MAX_INTEGER},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    path::{Component, Path, PathBuf},
};

/// Decimal-text native source, preserving coefficient and trailing decimal scale.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecimalText {
    /// Exact signed native-unit coefficient.
    pub coefficient: i64,
    /// Decimal divisor exponent, at most nine.
    pub scale: u32,
    /// Exact native-unit source resolution numerator.
    pub resolution_numerator: u64,
    /// Exact positive native-unit source resolution denominator.
    pub resolution_denominator: u64,
}

/// One PSI line, with native percent averages and microsecond total.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pressure {
    /// Exact native percent avg10, avg60 and avg300 text.
    pub averages_percent: [DecimalText; 3],
    /// Native microsecond total.
    pub total_usec: u64,
}

/// Native cgroup CPU accounting, without cgroup path or container identifiers.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CpuCounters {
    /// Total CPU usage microseconds.
    pub usage_usec: u64,
    /// Scheduling periods.
    pub periods: Evidence<u64>,
    /// Throttled scheduling periods.
    pub throttled_periods: Evidence<u64>,
    /// Total throttled microseconds.
    pub throttled_usec: Evidence<u64>,
}

/// One bounded before/after snapshot; no background monitor is started.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoadSnapshot {
    /// Named hwmon-zero sensor; no assumption that it measures the CPU package.
    pub thermal_sensor: Evidence<String>,
    /// Native signed millidegrees Celsius from the named sensor.
    pub temperature_millidegrees: Evidence<i64>,
    /// CPU-zero scaling frequency in native integer kHz, never wall-time cycles.
    pub scaling_frequency_khz: Evidence<u64>,
    /// CPU-zero frequency in native decimal MHz.
    pub cpu_mhz: Evidence<DecimalText>,

    /// Capture offset from run entry in integer nanoseconds.
    pub offset_ns: u64,
    /// Load averages in native runnable-task units.
    pub load_tasks: Evidence<[DecimalText; 3]>,
    /// Runnable tasks at capture.
    pub runnable_tasks: Evidence<u64>,
    /// Total tasks at capture.
    pub total_tasks: Evidence<u64>,
    /// System-wide CPU some pressure.
    pub system_some: Evidence<Pressure>,
    /// System-wide CPU full pressure, when exposed.
    pub system_full: Evidence<Pressure>,
    /// Applicable cgroup CPU counters.
    pub cpu: Evidence<CpuCounters>,
    /// Applicable cgroup CPU some pressure.
    pub cgroup_some: Evidence<Pressure>,
    /// Applicable cgroup CPU full pressure.
    pub cgroup_full: Evidence<Pressure>,
    /// Recorded allowed CPU population, never inferred from `available_parallelism`.
    pub allowed_cpus: Evidence<u64>,
    /// Public method description without membership or mount identifiers.
    pub method: String,
}

/// Read bounded text without treating an incomplete prefix as a valid probe.
fn read(path: &Path, cap: u64) -> Result<String, &'static str> {
    let mut file = File::open(path).map_err(|_| "probe missing or denied")?;
    if path == Path::new("/proc/cpuinfo") {
        crate::capture::first_record(&mut file, cap)
    } else {
        crate::capture::bounded_read(&mut file, cap)
    }
}

/// Preserve unavailable probes without silently filling zeros.
fn evidence<T>(result: Result<T, &'static str>) -> Evidence<T> {
    result.map_or_else(
        |reason| Evidence::Unavailable(reason.to_owned()),
        Evidence::Known,
    )
}

/// Native nonnegative integer with the shared wire bound.
fn integer(text: &str) -> Result<u64, &'static str> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("invalid native counter");
    }
    text.parse::<u64>()
        .ok()
        .filter(|n| *n <= MAX_INTEGER)
        .ok_or("unrepresentable native counter")
}

/// Exact decimal source parsing never visits binary64.
fn decimal(text: &str) -> Result<DecimalText, &'static str> {
    let (coefficient, scale) =
        decimal::decimal_source(text).map_err(|_| "unrepresentable decimal source")?;
    Ok(DecimalText {
        coefficient,
        scale,
        resolution_numerator: 1,
        resolution_denominator: 10_u64.pow(scale),
    })
}

/// Parse only the three declared averages and task counts, omitting process identity.
fn load(text: &str) -> Result<([DecimalText; 3], u64, u64), &'static str> {
    let mut fields = text.split_whitespace();
    let a = decimal(fields.next().ok_or("missing load")?)?;
    let b = decimal(fields.next().ok_or("missing load")?)?;
    let c = decimal(fields.next().ok_or("missing load")?)?;
    let (runnable, total) = fields
        .next()
        .and_then(|counts| counts.split_once('/'))
        .ok_or("missing task counts")?;
    Ok(([a, b, c], integer(runnable)?, integer(total)?))
}

/// Decode one selected PSI line with strict required source fields.
fn pressure(text: &str, kind: &str) -> Result<Pressure, &'static str> {
    let line = text
        .lines()
        .find(|line| line.split_whitespace().next() == Some(kind))
        .ok_or("pressure class unavailable")?;
    let field = |key| {
        line.split_whitespace()
            .skip(1)
            .find_map(|entry| {
                entry
                    .split_once('=')
                    .filter(|(name, _)| *name == key)
                    .map(|(_, value)| value)
            })
            .ok_or("pressure field unavailable")
    };
    Ok(Pressure {
        averages_percent: [
            decimal(field("avg10")?)?,
            decimal(field("avg60")?)?,
            decimal(field("avg300")?)?,
        ],
        total_usec: integer(field("total")?)?,
    })
}

/// Read one named cgroup counter without accepting duplicate native fields.
fn counter(text: &str, key: &str) -> Result<u64, &'static str> {
    let mut values = text.lines().filter_map(|line| {
        let mut fields = line.split_whitespace();
        (fields.next()? == key).then(|| fields.next()).flatten()
    });
    let value = integer(values.next().ok_or("counter unavailable")?)?;
    if values.next().is_some() {
        return Err("duplicate counter");
    }
    Ok(value)
}

/// Resolve unified cgroup membership against its matching mounted root.
fn cgroup(cap: u64) -> Result<PathBuf, &'static str> {
    let membership = read(Path::new("/proc/self/cgroup"), cap)?;
    let member = membership
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .ok_or("unified cgroup unavailable")?;
    let mounts = read(Path::new("/proc/self/mountinfo"), cap)?;
    let mut selected = None;
    for line in mounts.lines() {
        let Some((before, after)) = line.split_once(" - ") else {
            continue;
        };
        if after.split_whitespace().next() != Some("cgroup2") {
            continue;
        }
        let mut fields = before.split_whitespace().skip(3);
        let root = fields.next().ok_or("mount root unavailable")?;
        let mount = fields.next().ok_or("mount point unavailable")?;
        if root.contains('\\') || mount.contains('\\') {
            continue;
        }
        let Ok(relative) = Path::new(member).strip_prefix(root) else {
            continue;
        };
        if relative
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err("invalid membership path");
        }
        selected = Some(Path::new(mount).join(relative));
    }
    selected.ok_or("matching cgroup mount unavailable")
}

/// Public resource value from the resolved applicable cgroup, never its identifying path.
pub(crate) fn resource(name: &str, cap: u64) -> Evidence<String> {
    evidence(
        cgroup(cap)
            .and_then(|path| read(&path.join(name), cap))
            .map(|value| value.trim().to_owned()),
    )
}

/// Count an ascending disjoint CPU-list grammar without expanding ranges.
fn allowed(text: &str) -> Result<u64, &'static str> {
    let value = text
        .lines()
        .find_map(|line| line.strip_prefix("Cpus_allowed_list:"))
        .ok_or("allowed CPU list unavailable")?;
    let mut count = 0_u64;
    let mut previous = None;
    for range in value.trim().split(',') {
        let (first, last) = range.split_once('-').unwrap_or((range, range));
        let first = integer(first)?;
        let last = integer(last)?;
        if last < first || previous.is_some_and(|end| first <= end) {
            return Err("invalid CPU ranges");
        }
        count = count
            .checked_add(last - first + 1)
            .filter(|n| *n <= MAX_INTEGER)
            .ok_or("CPU count overflow")?;
        previous = Some(last);
    }
    if count == 0 {
        return Err("empty allowed CPU set");
    }
    Ok(count)
}

/// Signed native sensor integers preserve negative temperatures and the wire bound.
fn temperature(text: &str) -> Result<i64, &'static str> {
    text.trim()
        .parse::<i64>()
        .ok()
        .filter(|n| n.unsigned_abs() <= MAX_INTEGER)
        .ok_or("invalid native temperature")
}

/// The first CPU record is explicitly CPU zero; only its frequency text is retained.
fn frequency(text: &str) -> Result<DecimalText, &'static str> {
    let cpu = text.split("\n\n").next().ok_or("CPU record unavailable")?;
    let zero = cpu.lines().any(|line| {
        line.split_once(':')
            .is_some_and(|(key, value)| key.trim() == "processor" && value.trim() == "0")
    });
    if !zero {
        return Err("CPU-zero record unavailable");
    }
    let value = cpu
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            (key.trim() == "cpu MHz").then_some(value.trim())
        })
        .ok_or("CPU frequency unavailable")?;
    let value = decimal(value)?;
    if value.coefficient < 0 {
        return Err("negative CPU frequency");
    }
    Ok(value)
}

/// External placement supplies its own population without probing the process allowance.
fn population(
    case: &crate::CaseRequest,
    status: impl FnOnce() -> Result<String, &'static str>,
) -> Result<u64, &'static str> {
    if case.placement == crate::Placement::ExternallyConstrained {
        u64::try_from(case.allowed_cpus.len()).map_err(|_| "CPU population overflow")
    } else {
        allowed(&status()?)
    }
}

/// Capture every required before/after load source by bounded safe reads.
pub(crate) fn capture(offset_ns: u64, cap: u64, case: &crate::CaseRequest) -> LoadSnapshot {
    let cap = (cap / 16).min(65_536);
    let loaded = read(Path::new("/proc/loadavg"), cap).and_then(|text| load(&text));
    let load_tasks = evidence(
        loaded
            .as_ref()
            .map(|(averages, _, _)| averages.clone())
            .map_err(|reason| *reason),
    );
    let runnable_tasks = evidence(
        loaded
            .as_ref()
            .map(|(_, runnable, _)| *runnable)
            .map_err(|reason| *reason),
    );
    let total_tasks = evidence(loaded.map(|(_, _, total)| total));
    let system = read(Path::new("/proc/pressure/cpu"), cap);
    let group = cgroup(cap);
    let counters = group
        .as_ref()
        .map_err(|reason| *reason)
        .and_then(|path| read(&path.join("cpu.stat"), cap))
        .and_then(|text| {
            Ok(CpuCounters {
                usage_usec: counter(&text, "usage_usec")?,
                periods: evidence(counter(&text, "nr_periods")),
                throttled_periods: evidence(counter(&text, "nr_throttled")),
                throttled_usec: evidence(counter(&text, "throttled_usec")),
            })
        });
    let group_pressure = group.and_then(|path| read(&path.join("cpu.pressure"), cap));
    let selected_pressure = |source: &Result<String, &'static str>, kind| {
        evidence(
            source
                .as_ref()
                .map_err(|reason| *reason)
                .and_then(|text| pressure(text, kind)),
        )
    };
    let allowed_cpus = population(case, || read(Path::new("/proc/self/status"), cap));
    LoadSnapshot {
        thermal_sensor: evidence(read(Path::new("/sys/class/hwmon/hwmon0/name"), cap).map(|text| text.trim().to_owned())),
        temperature_millidegrees: evidence(read(Path::new("/sys/class/hwmon/hwmon0/temp1_input"), cap).and_then(|text| temperature(&text))),
        scaling_frequency_khz: evidence(read(Path::new("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq"), cap).and_then(|text| integer(text.trim()))),
        cpu_mhz: evidence(read(Path::new("/proc/cpuinfo"), cap).and_then(|text| frequency(&text))),
        offset_ns,
        load_tasks,
        runnable_tasks,
        total_tasks,
        system_some: selected_pressure(&system, "some"),
        system_full: selected_pressure(&system, "full"),
        cpu: evidence(counters),
        cgroup_some: selected_pressure(&group_pressure, "some"),
        cgroup_full: selected_pressure(&group_pressure, "full"),
        allowed_cpus: evidence(allowed_cpus),
        method: "bounded proc load/PSI, declared external CPU set or process CPU allowance, resolved unified cgroup, hwmon0 temp1 and CPU-zero frequency reads"
            .to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_sensor_sources_preserve_sign_scale_and_wire_bounds() -> Result<(), &'static str> {
        assert_eq!(temperature("-1250\n")?, -1250);
        assert!(temperature("1.25").is_err());
        assert!(temperature("9007199254740992").is_err());
        let source =
            frequency("processor : 0\ncpu MHz : 3000.125000\n\nprocessor : 1\ncpu MHz : 1.0")?;
        assert_eq!(source.coefficient, 3_000_125_000);
        assert_eq!(source.scale, 6);
        assert_eq!(source.resolution_denominator, 1_000_000);
        assert!(frequency("processor : 1\ncpu MHz : 3000").is_err());
        assert!(frequency("processor : 0\ncpu MHz : -1").is_err());
        Ok(())
    }
    #[test]
    fn external_population_overrides_process_allowance() {
        let mut case = crate::request::tests::request().cases.remove(0);
        let status = || Ok("Cpus_allowed_list:\t0-63\n".to_owned());
        assert_eq!(population(&case, status), Ok(64));
        case.placement = crate::Placement::ExternallyConstrained;
        case.allowed_cpus = vec![3, 7];
        assert_eq!(population(&case, status), Ok(2));
        assert_eq!(population(&case, || Err("probe denied")), Ok(2));
    }
}
