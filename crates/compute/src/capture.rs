// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded safe host probes; unavailable evidence remains explicit.

mod read;
#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) use read::{bounded as bounded_read, first_record};

use crate::{BuildRecord, EnvironmentRecord, Evidence, Setting};
use std::{
    fs::File,
    time::{SystemTime, UNIX_EPOCH},
};

/// Read at most the explicitly bounded probe text, refusing truncation.
fn probe(path: &str, cap: u64) -> Evidence<String> {
    let Ok(mut file) = File::open(path) else {
        return Evidence::Unavailable("probe missing or denied".to_owned());
    };
    let result = if path == "/proc/cpuinfo" {
        first_record(&mut file, cap)
    } else {
        bounded_read(&mut file, cap)
    };
    let mut text = match result {
        Ok(text) => text,
        Err(reason) => return Evidence::Unavailable(reason.to_owned()),
    };
    let start = text.len() - text.trim_start().len();
    let end = text.trim_end().len().max(start);
    text.truncate(end);
    drop(text.drain(..start));
    Evidence::Known(text)
}

/// One observed setting with separately tagged lack of requested control.
fn setting(path: &str, offset_ns: u64, cap: u64) -> Setting {
    Setting {
        requested: Evidence::Unknown("no control requested".to_owned()),
        observed: probe(path, cap),
        method: format!("safe bounded read of {path}"),
        offset_ns,
    }
}

/// One setting without a portable probe.
fn unknown(reason: &str, offset_ns: u64) -> Setting {
    Setting {
        requested: Evidence::Unknown("no control requested".to_owned()),
        observed: Evidence::Unknown(reason.to_owned()),
        method: "no available verified probe".to_owned(),
        offset_ns,
    }
}

/// Construct immutable embedded build evidence with checked capacities, never runtime Git state.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) fn build(
    budget: &mut crate::memory::Budget,
) -> Result<BuildRecord, crate::recipe::RecipeError> {
    budget.charge(size_of::<BuildRecord>())?;
    Ok(include!(concat!(env!("OUT_DIR"), "/compute-build.rs")))
}

/// Capture public CPU identity without retaining processor serials or hostnames.
fn cpu_model(cap: u64) -> Evidence<String> {
    match probe("/proc/cpuinfo", cap) {
        Evidence::Known(text) => text
            .lines()
            .find_map(|line| {
                let (key, value) = line.split_once(':')?;
                (key.trim() == "model name").then(|| Evidence::Known(value.trim().to_owned()))
            })
            .unwrap_or_else(|| Evidence::Unknown("cpu model field unavailable".to_owned())),
        Evidence::Unknown(reason) => Evidence::Unknown(reason),
        Evidence::Unavailable(reason) => Evidence::Unavailable(reason),
        Evidence::Redacted(reason) => Evidence::Redacted(reason),
    }
}

/// Revisioned feature inventory; build intent and runtime capability stay separate.
#[cfg(target_arch = "x86_64")]
fn features() -> (Vec<String>, Vec<String>) {
    let mut compile = Vec::new();
    let mut runtime = Vec::new();
    macro_rules! feature {
        ($name:tt) => {
            if cfg!(target_feature = $name) {
                compile.push($name.to_owned());
            }
            if std::is_x86_feature_detected!($name) {
                runtime.push($name.to_owned());
            }
        };
    }
    feature!("sse2");
    feature!("sse3");
    feature!("ssse3");
    feature!("sse4.1");
    feature!("sse4.2");
    feature!("avx");
    feature!("avx2");
    feature!("fma");
    feature!("avx512f");
    feature!("avx512dq");
    feature!("avx512vl");
    (compile, runtime)
}

/// Other targets are explicitly outside the instrument collection contract.
#[cfg(not(target_arch = "x86_64"))]
fn features() -> (Vec<String>, Vec<String>) {
    (Vec::new(), Vec::new())
}

/// Retain only named public capacity fields from the bounded memory inventory.
fn memory(offset_ns: u64, cap: u64) -> Setting {
    let observed = match probe("/proc/meminfo", cap) {
        Evidence::Known(text) => {
            let fields = text
                .lines()
                .filter(|line| line.starts_with("MemTotal:") || line.starts_with("SwapTotal:"))
                .collect::<Vec<_>>()
                .join("; ");
            if fields.is_empty() {
                Evidence::Unknown("memory capacity fields absent".to_owned())
            } else {
                Evidence::Known(fields)
            }
        }
        other => other,
    };
    Setting {
        requested: Evidence::Unknown("no control requested".to_owned()),
        observed,
        method:
            "bounded /proc/meminfo MemTotal and SwapTotal fields; hardware configuration unknown"
                .to_owned(),
        offset_ns,
    }
}

/// Fixed CPU-zero cache inventory; shared lists describe topology without asserting residency.
fn caches(offset_ns: u64, cap: u64) -> Setting {
    let mut entries = Vec::new();
    for index in 0..8 {
        let prefix = format!("/sys/devices/system/cpu/cpu0/cache/index{index}");
        let mut fields = Vec::new();
        for name in [
            "level",
            "type",
            "size",
            "coherency_line_size",
            "shared_cpu_list",
        ] {
            if let Evidence::Known(value) = probe(&format!("{prefix}/{name}"), cap) {
                if value.len() > 256 {
                    return unknown("cache descriptor exceeds bounded field capacity", offset_ns);
                }
                fields.push(format!("{name}={value}"));
            }
        }
        if !fields.is_empty() {
            entries.push(format!("index{index}: {}", fields.join(", ")));
        }
    }
    let observed = if entries.is_empty() {
        Evidence::Unavailable("CPU-zero cache inventory unavailable".to_owned())
    } else {
        Evidence::Known(entries.join("; "))
    };
    Setting {
        requested: Evidence::Unknown("no cache control requested".to_owned()),
        observed,
        method: "bounded CPU-zero sysfs cache indices 0 through 7; other CPU asymmetry unknown"
            .to_owned(),
        offset_ns,
    }
}

/// Select only a public allowance field from proc status; omit all process identity.
fn allowance(key: &str, offset_ns: u64, cap: u64) -> Setting {
    let observed = match probe("/proc/thread-self/status", cap) {
        Evidence::Known(text) => text
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .map_or_else(
                || Evidence::Unknown("allowance field unavailable".to_owned()),
                |value| Evidence::Known(value.trim().to_owned()),
            ),
        other => other,
    };
    Setting {
        requested: Evidence::Unknown("no control requested".to_owned()),
        observed,
        method: format!(
            "bounded thread-self status field {key}; allowance is not verified binding"
        ),
        offset_ns,
    }
}

/// Resolved cgroup settings retain only public values and a non-identifying method.
fn resource(name: &str, offset_ns: u64, cap: u64) -> Setting {
    Setting {
        requested: Evidence::Unknown("no control requested".to_owned()),
        observed: crate::telemetry::resource(name, cap),
        method: format!(
            "membership and mount resolved unified cgroup {name}; private path omitted"
        ),
        offset_ns,
    }
}

/// Reserve source text, cache field/entry joins and cgroup parsing under one scratch cap.
/// Forty cache fields can coexist with their formatted entries and joined projection;
/// 128 text slots cover those three populations plus the bounded proc/cgroup sources.
/// Each slot also reserves 128 fixed label/path bytes and one owned string header;
/// this bookkeeping is independent of the caller-selected probe text allowance.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) fn probe_allowance(scratch: u64) -> Result<u64, &'static str> {
    let header = u64::try_from(size_of::<String>()).map_err(|_| "capture scratch overflow")?;
    let overhead = 128_u64
        .checked_add(header)
        .ok_or("capture scratch overflow")?;
    scratch
        .checked_div(128)
        .and_then(|slot| slot.checked_sub(overhead))
        .and_then(|text| text.checked_sub(1))
        .map(|text| text.min(65_536))
        .ok_or("capture scratch capacity")
}

/// Capture bounded environment facts; never start background monitoring.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) fn environment(alias: String, offset_ns: u64, cap: u64) -> EnvironmentRecord {
    let (compile_isa, runtime_isa) = features();
    EnvironmentRecord {
        machine_alias: alias,
        cpu_model: cpu_model(cap),
        runtime_isa,
        compile_isa,
        topology: setting("/sys/devices/system/cpu/online", offset_ns, cap),
        caches: caches(offset_ns, cap),
        memory: memory(offset_ns, cap),
        smt: setting("/sys/devices/system/cpu/smt/active", offset_ns, cap),
        placement: unknown("per-worker observed CPU unavailable", offset_ns),
        numa: unknown("NUMA placement not verified", offset_ns),
        kernel: setting("/proc/sys/kernel/osrelease", offset_ns, cap),
        distribution: setting("/etc/os-release", offset_ns, cap),
        resource_limits: unknown(
            "restrictions beyond the captured process and unified-cgroup controls are not verified",
            offset_ns,
        ),
        power: setting(
            "/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor",
            offset_ns,
            cap,
        ),
        turbo: setting(
            "/sys/devices/system/cpu/intel_pstate/no_turbo",
            offset_ns,
            cap,
        ),
        tracing: unknown("caller-scoped tracing is not inspected", offset_ns),
        concurrent_load: unknown("concurrent workloads not declared", offset_ns),
        core_classes: setting(
            "/sys/devices/system/cpu/cpu0/topology/core_type",
            offset_ns,
            cap,
        ),
        coordinator_placement: allowance("Cpus_allowed_list:", offset_ns, cap),
        worker_placement: unknown(
            "worker CPU residency is not established by process allowance",
            offset_ns,
        ),
        numa_allowed_nodes: allowance("Mems_allowed_list:", offset_ns, cap),
        process_limits: setting("/proc/self/limits", offset_ns, cap),
        cgroup_cpu_limit: resource("cpu.max", offset_ns, cap),
        cgroup_memory_limit: resource("memory.max", offset_ns, cap),
        cgroup_cpuset: resource("cpuset.cpus.effective", offset_ns, cap),
        virtual_machine: unknown(
            "virtualization and host isolation are not verified",
            offset_ns,
        ),
        frequency_min: setting(
            "/sys/devices/system/cpu/cpu0/cpufreq/scaling_min_freq",
            offset_ns,
            cap,
        ),
        frequency_max: setting(
            "/sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq",
            offset_ns,
            cap,
        ),
    }
}

/// UTC timestamp from system time without subprocesses or locale state.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) fn timestamp() -> Result<String, &'static str> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "clock before Unix epoch")?
        .as_secs();
    timestamp_seconds(seconds)
}

/// Gregorian civil date using bounded whole-day arithmetic.
fn timestamp_seconds(seconds: u64) -> Result<String, &'static str> {
    let days = i64::try_from(seconds / 86_400).map_err(|_| "UTC date overflow")?;
    let shifted = days.checked_add(719_468).ok_or("UTC date overflow")?;
    let era = shifted / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = month_index + if month_index < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    if year > 9999 {
        return Err("UTC date outside four-digit year domain");
    }
    let within = seconds % 86_400;
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        within / 3600,
        within / 60 % 60,
        within % 60
    ))
}

#[cfg(test)]
mod tests {
    use super::build;

    #[test]
    fn embedded_build_uses_only_its_actual_shared_ownership_allowance()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut budget = crate::memory::Budget::new(1);
        assert!(build(&mut budget).is_err());
        let mut budget = crate::memory::Budget::new(u64::MAX);
        let captured = build(&mut budget)?;
        assert_eq!(u64::MAX - budget.remaining(), captured.owned_bytes()?);
        Ok(())
    }

    #[test]
    fn probe_text_never_reuses_fixed_metadata_reservations()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixed = 128 * (128 + u64::try_from(size_of::<String>())?);
        assert_eq!(
            super::probe_allowance(fixed),
            Err("capture scratch capacity")
        );
        assert_eq!(super::probe_allowance(fixed + 128), Ok(0));
        assert_eq!(super::probe_allowance(fixed + 128 * 1025), Ok(1024));
        assert_eq!(super::probe_allowance(u64::MAX), Ok(65_536));
        Ok(())
    }
}
