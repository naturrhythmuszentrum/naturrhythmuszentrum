// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit collector/export orchestration; the library itself executes no commands.

#![expect(
    clippy::multiple_crate_versions,
    reason = "Pinned faer/nano-gemm, petgraph/indexmap and gemm-common/sysctl carry duplicate equator, hashbrown and thiserror versions."
)]

use nrz_compute::report::{ObjectFormat, RawContentId};
use nrz_compute::{CancellationToken, ExecutionBounds, RunContext, RunRequest};
use serde::Deserialize;
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// Complete explicit CLI request, including context and separate export caps.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectionRequest {
    /// Exact public run request.
    request: RunRequest,
    /// Host cooperative execution assumptions.
    bounds: Option<ExecutionBounds>,
    /// Opaque public alias.
    machine_alias: String,
    /// Raw Git content format.
    object_format: ObjectFormat,
    /// Independent raw export byte cap.
    export_bytes: u64,
    /// Independent cooperative export time allowance.
    export_ns: u64,
}

/// Temporary publication file, removed on every failure path.
#[derive(Debug)]
struct Publication {
    /// Hidden staging path beside destination.
    staging: PathBuf,
    /// Explicit final path.
    destination: PathBuf,
    /// Owned open staging file.
    file: File,
}

impl Publication {
    /// Reserve a unique sibling file without overwriting existing staging state.
    fn new(destination: &Path) -> io::Result<Self> {
        let parent = destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .canonicalize()?;
        let filename = destination
            .file_name()
            .ok_or_else(|| io::Error::other("output has no filename"))?;
        let mut name = std::ffi::OsString::from(".");
        name.push(filename);
        name.push(format!(".{}.staging", std::process::id()));
        let staging = parent.join(name);
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)?;
        Ok(Self {
            staging,
            destination: parent.join(filename),
            file,
        })
    }
    /// Sync before atomic publication and sync the containing directory afterward.
    fn publish(self, deadline: Option<Instant>) -> io::Result<()> {
        self.file.sync_all()?;
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(io::Error::other("export time cap before publication"));
        }
        fs::rename(&self.staging, &self.destination)?;
        if let Some(parent) = self.destination.parent() {
            File::open(parent)?.sync_all()?;
        }
        Ok(())
    }
}

impl Drop for Publication {
    fn drop(&mut self) {
        let _removed = fs::remove_file(&self.staging);
    }
}

/// Explicit export caps apply independently of the completed run's wall budget.
struct BoundedWriter<'a> {
    /// Caller-selected staging file.
    file: &'a mut File,
    /// Remaining encoded bytes.
    bytes: u64,
    /// Export clock epoch.
    started: Instant,
    /// Cooperative export allowance.
    budget: Duration,
}

impl Write for BoundedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.started.elapsed() >= self.budget {
            return Err(io::Error::other("export time cap"));
        }
        let length = u64::try_from(bytes.len()).map_err(io::Error::other)?;
        if length > self.bytes {
            return Err(io::Error::other("export byte cap"));
        }
        let written = self.file.write(bytes)?;
        self.bytes -= u64::try_from(written).map_err(io::Error::other)?;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

/// Reject any destination beneath a repository checkout through canonical parents.
fn external_destination(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()?;
    if parent.ancestors().any(|ancestor| {
        let checkout = ancestor.join(".git").exists();
        let bare = ancestor.join("HEAD").is_file()
            && ancestor.join("objects").is_dir()
            && ancestor.join("config").is_file();
        checkout || bare
    }) {
        return Err(
            "raw output must be outside every repository checkout or object directory".into(),
        );
    }
    Ok(())
}

/// Bound request input before JSON decoding, including streams without a known length.
fn read_request(reader: impl Read) -> Result<CollectionRequest, Box<dyn std::error::Error>> {
    const REQUEST_BYTES: u64 = 1_048_576;
    let mut bytes = Vec::new();
    reader.take(REQUEST_BYTES + 1).read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len())? > REQUEST_BYTES {
        return Err("RequestBytesExceeded".into());
    }
    Ok(serde_json::from_slice(&bytes)?)
}

/// Explicit collection calls the same public run and writer surfaces.
fn collect(
    input: &Path,
    output: &Path,
    arguments: Vec<String>,
) -> Result<u8, Box<dyn std::error::Error>> {
    external_destination(output)?;
    let request: CollectionRequest = read_request(File::open(input)?)?;
    if request.export_bytes == 0
        || request.export_ns == 0
        || request.export_ns > nrz_compute::decimal::MAX_INTEGER
        || request.export_bytes > nrz_compute::decimal::MAX_INTEGER
    {
        return Err("invalid explicit export caps".into());
    }
    let context = RunContext {
        bounds: request.bounds,
        cancellation: CancellationToken::default(),
        machine_alias: request.machine_alias,
        command: arguments,
        object_format: request.object_format,
    };
    let report = nrz_compute::run(&request.request, &context)?;
    let started = Instant::now();
    let budget = Duration::from_nanos(request.export_ns);
    let deadline = started
        .checked_add(budget)
        .ok_or("export deadline overflow")?;
    let mut publication = Publication::new(output)?;
    let writer = BoundedWriter {
        file: &mut publication.file,
        bytes: request.export_bytes,
        started,
        budget,
    };
    nrz_compute::write_report(&report, writer)?;
    publication.publish(Some(deadline))?;
    Ok(u8::from(!report.payload.execution.complete))
}

/// Temporary isolated Git context; no raw blob is stored.
#[derive(Debug)]
struct GitContext(PathBuf);

impl Drop for GitContext {
    fn drop(&mut self) {
        let _removed = fs::remove_dir_all(&self.0);
    }
}

/// Remove inherited Git configuration and isolate all command configuration sources.
fn git(directory: &Path) -> Command {
    let mut command = Command::new("git");
    for (name, _) in env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("GIT_")) {
        command.env_remove(name);
    }
    command
        .current_dir(directory)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("HOME", directory.join("home"))
        .env("XDG_CONFIG_HOME", directory.join("xdg"));
    command
}

/// Hash canonical bytes through an isolated bare repository of the declared format.
fn content_id(raw: &nrz_compute::RunReport) -> Result<RawContentId, Box<dyn std::error::Error>> {
    let root = env::temp_dir().join(format!("nrz-compute-git-{}", std::process::id()));
    fs::create_dir(&root)?;
    let context = GitContext(root);
    fs::create_dir(context.0.join("home"))?;
    fs::create_dir(context.0.join("xdg"))?;
    let format = match raw.payload.identity.object_format {
        ObjectFormat::Sha1 => "sha1",
        ObjectFormat::Sha256 => "sha256",
    };
    let initialized = git(&context.0)
        .args([
            "init",
            "--bare",
            &format!("--object-format={format}"),
            "--template=",
            "objects.git",
        ])
        .output()?;
    if !initialized.status.success() {
        return Err("Git object-format initialization failed".into());
    }
    let mut child = git(&context.0)
        .env("GIT_DIR", context.0.join("objects.git"))
        .args(["hash-object", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdin = child.stdin.take().ok_or("Git hash input unavailable")?;
    let written = nrz_compute::write_report(raw, &mut stdin);
    drop(stdin);
    let output = child.wait_with_output()?;
    written?;
    if !output.status.success() {
        return Err("Git blob hash failed".into());
    }
    let digest = String::from_utf8(output.stdout)?.trim_end().to_owned();
    let id = RawContentId {
        algorithm: raw.payload.identity.object_format,
        digest,
    };
    if !id.is_valid() {
        return Err("Git returned malformed digest".into());
    }
    Ok(id)
}

/// Read and project raw evidence without acquiring replacement telemetry.
fn summarize(input: &Path, output: &Path) -> Result<u8, Box<dyn std::error::Error>> {
    let file = File::open(input)?;
    let byte_cap = file.metadata()?.len();
    let raw = nrz_compute::read_report(file, byte_cap)?;
    let raw_id = content_id(&raw)?;
    let summary = nrz_compute::summarize(&raw, &raw_id)?;
    let mut publication = Publication::new(output)?;
    nrz_compute::write_summary(&summary, &mut publication.file)?;
    publication.publish(None)?;
    Ok(0)
}

/// Parse the two explicit subcommands; no command starts a default suite.
fn execute() -> Result<u8, Box<dyn std::error::Error>> {
    let arguments: Vec<String> = env::args().collect();
    let [_, command, input_flag, input, output_flag, output] = arguments.as_slice() else {
        return Err("usage: nrz-compute collect --request PATH --output PATH | summarize --raw PATH --output PATH".into());
    };
    if output_flag != "--output" {
        return Err("expected --output".into());
    }
    match (command.as_str(), input_flag.as_str()) {
        ("collect", "--request") => collect(Path::new(input), Path::new(output), arguments.clone()),
        ("summarize", "--raw") => summarize(Path::new(input), Path::new(output)),
        _ => Err("unsupported subcommand or input flag".into()),
    }
}

/// Distinguish fully complete, valid partial report and request/export failure.
fn main() -> std::process::ExitCode {
    match execute() {
        Ok(code) => std::process::ExitCode::from(code),
        Err(error) => {
            let _written = writeln!(io::stderr().lock(), "{error}");
            std::process::ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_request_has_named_refusal_before_json_decoding()
    -> Result<(), Box<dyn std::error::Error>> {
        let error = read_request(std::io::repeat(b' ').take(1_048_577))
            .err()
            .ok_or("oversized request accepted")?;
        assert_eq!(error.to_string(), "RequestBytesExceeded");
        let error = read_request(std::io::repeat(b' ').take(1_048_576))
            .err()
            .ok_or("empty JSON accepted")?;
        assert_ne!(error.to_string(), "RequestBytesExceeded");
        Ok(())
    }
}
