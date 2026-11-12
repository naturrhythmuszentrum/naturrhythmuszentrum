// SPDX-License-Identifier: AGPL-3.0-only
//! Required process provenance uses the engineering record's existing source field.
use super::support::TestResult;

/// Supplied protocol declarations are checked for consistency, not authenticated.
#[derive(Debug)]
pub(super) struct Protocol {
    /// Admitted input population.
    pub(super) count: u32,
    /// Observation mode; enabled runs are not comparable native samples.
    pub(super) enabled: bool,
    /// Exactly one nonzero logical CPU.
    cpu: u32,
    /// Zero denotes the discarded fresh warm-up; one through five denote measurements.
    ordinal: u32,
    /// Previous discarded process, distinct from the measured process.
    warm_up_pid: u32,
    /// Ordered builds, alternated forward and reverse on successive rounds.
    builds: Vec<String>,
    /// This process's position within its interleaved round.
    build_index: usize,
    /// Original environment conditions, not a promise of isolation.
    environment: String,
    /// UTC date for the live engineering record.
    pub(super) date: String,
    /// Executed source identity, supplied by the runner.
    pub(super) commit: String,
    /// Exact command retained by the runner.
    pub(super) command: String,
}
impl Protocol {
    /// Require every parameter rather than filling missing provenance with defaults.
    pub(super) fn parse(args: &[String]) -> TestResult<Self> {
        if args.len() != 12 {
            return Err("expected count mode cpu ordinal processes warm-up-pid builds build-index date commit command environment".into());
        }
        let count = args[0].parse()?;
        let enabled = match args[1].as_str() {
            "enabled" => true,
            "disabled" => false,
            _ => return Err("observation mode".into()),
        };
        let cpu = args[2].parse()?;
        let ordinal = args[3].parse()?;
        let processes: u32 = args[4].parse()?;
        let warm_up_pid = args[5].parse()?;
        let builds = args[6].split(',').map(str::to_owned).collect::<Vec<_>>();
        let build_index = args[7].parse()?;
        if cpu == 0
            || processes != 5
            || ordinal > 5
            || build_index >= builds.len()
            || builds.is_empty()
            || builds.len() > 2
            || builds.iter().any(|build| !cell(build))
            || builds
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != builds.len()
        {
            return Err("process protocol".into());
        }
        if ordinal > 0 && (warm_up_pid == 0 || warm_up_pid == std::process::id()) {
            return Err("discarded fresh warm-up required".into());
        }
        if !date(&args[8])
            || args[9].len() != 40
            || !args[9]
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || !cell(&args[10])
            || !cell(&args[11])
        {
            return Err("run identity".into());
        }
        Ok(Self {
            count,
            enabled,
            cpu,
            ordinal,
            warm_up_pid,
            builds,
            build_index,
            date: args[8].clone(),
            commit: args[9].clone(),
            command: args[10].clone(),
            environment: args[11].clone(),
        })
    }
    /// Refuse a declaration that differs from the kernel's actual one-CPU affinity.
    pub(super) fn check_affinity(&self, status: &str) -> TestResult {
        let cpu = status
            .lines()
            .find_map(|s| s.strip_prefix("Cpus_allowed_list:"))
            .ok_or("affinity unavailable")?
            .trim()
            .parse::<u32>()?;
        if cpu != self.cpu {
            return Err("pinned CPU mismatch".into());
        }
        Ok(())
    }
    /// Warm-up output is retained as provenance, never as a measured estimate.
    pub(super) const fn measured(&self) -> bool {
        self.ordinal != 0
    }
    /// With two builds each occupies either position in two or three of the five rounds.
    pub(super) fn order(&self) -> Vec<&str> {
        let mut order = self.builds.iter().map(String::as_str).collect::<Vec<_>>();
        if self.ordinal % 2 == 0 {
            order.reverse();
        }
        order
    }
    /// Complete protocol travels with each row through the existing provenance field.
    pub(super) fn source(&self) -> String {
        format!(
            "count={}; pid={}; cpu={}; ordinal={}; processes=5; warm_up_pid={}; measured={}; builds={:?}; round_order={:?}; build={}; tracing={}; comparable={}; within_process_intervals=not-collected; seed=none; resamples=0; environment={}",
            self.count,
            std::process::id(),
            self.cpu,
            self.ordinal,
            self.warm_up_pid,
            self.measured(),
            self.builds,
            self.order(),
            self.builds[self.build_index],
            self.enabled,
            !self.enabled,
            self.environment
        )
    }
}

/// Match the engineering validator's nonblank, single-line, pipe-free cell contract.
fn cell(value: &str) -> bool {
    !value.trim().is_empty() && !value.contains(['\n', '\r', '|'])
}
/// Match the engineering validator's Gregorian date contract, including century leap years.
fn date(value: &str) -> bool {
    let parts = value.split('-').collect::<Vec<_>>();
    if parts.len() != 3
        || parts[0].len() != 4
        || parts[1].len() != 2
        || parts[2].len() != 2
        || !parts
            .iter()
            .all(|part| part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return false;
    }
    let Ok(year) = parts[0].parse::<u32>() else {
        return false;
    };
    let Ok(month) = parts[1].parse::<u32>() else {
        return false;
    };
    let Ok(day) = parts[2].parse::<u32>() else {
        return false;
    };
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => return false,
    };
    year != 0 && day != 0 && day <= days
}
