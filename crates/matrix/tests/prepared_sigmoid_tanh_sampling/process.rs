// SPDX-License-Identifier: AGPL-3.0-only
//! Exclusive working areas and bounded child execution for the real writer.
use super::TestResult;
use std::{
    io::{self, Write},
    os::unix::process::ExitStatusExt,
    path::PathBuf,
    process::{Child, Command, ExitStatus},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

/// Distinct requests within one process never reuse a working-area name.
static NEXT: AtomicU64 = AtomicU64::new(0);
/// Only the intended writer control executes in the child.
const OUTPUT_TEST: &str = "harness::tests::prepared_sigmoid_tanh_sampling_output";
/// A broken child refuses rather than holding the test target indefinitely.
const CHILD_LIMIT: Duration = Duration::from_secs(30);
/// A stale path consumes a retry, never an unbounded search.
const AREA_ATTEMPTS: usize = 16;
/// The child control selects only itself in a fresh test process.
const CLEANUP_TEST: &str = "harness::tests::prepared_sigmoid_tanh_sampling_child_cleanup";

/// The creator alone owns removal, including on an early error return.
struct WorkingArea(PathBuf);
impl WorkingArea {
    /// Reserve a name without adopting any surviving path.
    fn name() -> io::Result<PathBuf> {
        let ordinal = NEXT
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| io::Error::other("working-area counter exhausted"))?;
        Ok(std::env::temp_dir().join(format!(
            "sentinel-working-area-{}-{ordinal}",
            std::process::id()
        )))
    }
    /// Existing paths are a refusal, never shared working storage.
    fn create(path: PathBuf) -> io::Result<Self> {
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
    /// Only existing paths consume retries; other filesystem failures refuse immediately.
    fn reserve(mut name: impl FnMut() -> io::Result<PathBuf>) -> io::Result<Self> {
        for _ in 0..AREA_ATTEMPTS {
            match Self::create(name()?) {
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                result => return result,
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "working-area attempts exhausted",
        ))
    }
    /// Fresh names are chosen at runtime, including after each collision.
    fn fresh() -> io::Result<Self> {
        Self::reserve(Self::name)
    }
}
impl Drop for WorkingArea {
    fn drop(&mut self) {
        drop(std::fs::remove_dir_all(&self.0));
    }
}

/// Reap the child before its working area is removed, including on timeout.
struct RunningChild<'a>(&'a mut Child);
impl Drop for RunningChild<'_> {
    fn drop(&mut self) {
        drop(self.0.kill());
        drop(self.0.wait());
    }
}

/// Polling shares a bounded refusal path with the deterministic no-completion control.
fn wait_for(
    mut poll: impl FnMut() -> io::Result<Option<ExitStatus>>,
    limit: Duration,
) -> io::Result<ExitStatus> {
    let start = Instant::now();
    loop {
        if let Some(status) = poll()? {
            return Ok(status);
        }
        if start.elapsed() >= limit {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "writer sentinel child did not end",
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Existing paths, early returns and absent child completion each have a control.
///
/// # Errors
/// Returns working-area creation failures.
///
/// # Panics
/// Panics if an exclusive creation, cleanup or bounded-wait assertion fails.
pub(super) fn controls() -> TestResult {
    collision_controls()?;
    let area = WorkingArea::create(WorkingArea::name()?)?;
    assert!(
        WorkingArea::create(area.0.clone()).is_err(),
        "pre-existing working area is refused"
    );
    let path = WorkingArea::name()?;
    assert_ne!(area.0, path, "working-area names are unique");
    let failed = (|| -> TestResult {
        let _area = WorkingArea::create(path.clone())?;
        Err("deliberate early return".into())
    })();
    assert!(failed.is_err(), "control reaches its early return");
    assert!(!path.exists(), "early exit removes its working area");
    let failure = wait_for(|| Ok(None), Duration::ZERO);
    assert!(
        failure.is_err_and(|e| e.kind() == io::ErrorKind::TimedOut
            && e.to_string() == "writer sentinel child did not end"),
        "unfinished child has a bounded explicit refusal"
    );
    Ok(())
}

/// A bounded collision inventory retains every pre-existing path.
fn collision_controls() -> TestResult {
    let occupied = WorkingArea::fresh()?;
    let next = WorkingArea::name()?;
    let mut attempts = 0;
    let area = WorkingArea::reserve(|| {
        attempts += 1;
        Ok(if attempts == 1 {
            occupied.0.clone()
        } else {
            next.clone()
        })
    })?;
    assert_eq!(attempts, 2, "collision retries the next runtime name");
    assert_eq!(area.0, next, "retry owns the fresh path only");
    attempts = 0;
    let refused = WorkingArea::reserve(|| {
        attempts += 1;
        Ok(occupied.0.clone())
    });
    assert!(
        refused.is_err_and(|error| error.kind() == io::ErrorKind::AlreadyExists),
        "exhausted collisions refuse"
    );
    assert_eq!(attempts, AREA_ATTEMPTS, "collision retries are bounded");
    assert!(
        occupied.0.is_dir(),
        "collisions never adopt or remove an existing directory"
    );
    Ok(())
}

/// Independent cleanup remains active when the guard under test is mutated.
struct EmergencyChild(Child);
impl Drop for EmergencyChild {
    fn drop(&mut self) {
        drop(self.0.kill());
        drop(self.0.wait());
    }
}

/// A real blocking child witnesses kill and reap before working-area removal.
/// # Errors
/// Returns creation, readiness or process errors.
/// # Panics
/// Panics if the guard fails to kill or reap before directory removal.
pub(super) fn blocking_child() -> TestResult {
    if std::env::var_os("SAMPLING_BLOCKING_CHILD").is_some() {
        std::fs::write("ready", b"ready")?;
        // This ceiling also bounds a mutant that waits without first killing.
        std::thread::sleep(CHILD_LIMIT);
        return Ok(());
    }
    let area = WorkingArea::fresh()?;
    let directory = area.0.clone();
    let mut emergency = EmergencyChild(
        Command::new(std::env::current_exe()?)
            .args(["--exact", CLEANUP_TEST, "--nocapture"])
            .current_dir(&area.0)
            .env("SAMPLING_BLOCKING_CHILD", "blocking")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?,
    );
    let process_path = PathBuf::from(format!("/proc/{}", emergency.0.id()));
    let start = Instant::now();
    while !area.0.join("ready").exists() {
        if start.elapsed() >= CHILD_LIMIT {
            return Err("blocking child did not become ready".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        emergency.0.try_wait()?.is_none(),
        "control child is still blocked"
    );
    drop(RunningChild(&mut emergency.0));
    assert!(
        area.0.is_dir(),
        "directory survives until child cleanup completes"
    );
    assert!(
        !process_path.exists(),
        "guard reaps the child before directory removal"
    );
    let status = emergency
        .0
        .try_wait()?
        .ok_or("guard left the child running")?;
    assert_eq!(status.signal(), Some(9), "guard kills the blocking child");
    drop(emergency);
    drop(area);
    assert!(
        !directory.exists(),
        "directory is removed after child cleanup"
    );
    Ok(())
}

/// Run the writer under real ambient sentinels and exactly one child test.
///
/// # Errors
/// Returns working-area, child execution, timeout or captured-output failures.
///
/// # Panics
/// Panics if the child selector, completion or executed-test assertion fails.
pub(super) fn sentinel_child() -> TestResult<bool> {
    if std::env::var_os("SAMPLING_OUTPUT_CHILD").is_some() {
        let args: Vec<_> = std::env::args().skip(1).collect();
        assert_eq!(
            args,
            ["--exact", OUTPUT_TEST, "--nocapture"],
            "writer child observes the exact full test selector"
        );
        return Ok(false);
    }
    let area = WorkingArea::fresh()?;
    let stdout = area.0.join("stdout");
    let stderr = area.0.join("stderr");
    let start = Instant::now();
    let mut process = Command::new(std::env::current_exe()?)
        .args(["--exact", OUTPUT_TEST, "--nocapture"])
        .current_dir(&area.0)
        .env("SAMPLING_OUTPUT_CHILD", "sentinel-environment-value")
        .env("USER", "sentinel-account-value")
        .env("LOGNAME", "sentinel-login-value")
        .stdout(std::fs::File::create_new(&stdout)?)
        .stderr(std::fs::File::create_new(&stderr)?)
        .spawn()?;
    let child = RunningChild(&mut process);
    let status = wait_for(|| child.0.try_wait(), CHILD_LIMIT)?;
    writeln!(
        io::stdout().lock(),
        "writer-child-wall-ns={}",
        start.elapsed().as_nanos()
    )?;
    assert!(
        status.success(),
        "writer sentinel child {status}: {}",
        String::from_utf8_lossy(&std::fs::read(stderr)?)
    );
    let output = std::fs::read_to_string(stdout)?;
    assert!(
        output.contains(&format!("test {OUTPUT_TEST} ... ok")) && output.contains("1 passed"),
        "writer child actually executed its one named control"
    );
    Ok(true)
}
