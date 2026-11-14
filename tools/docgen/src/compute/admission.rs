// SPDX-License-Identifier: AGPL-3.0-only
//! Committed index admission, byte caps and receipt bindings.

mod size;

use super::{schema::SummaryDataset, validate};
use crate::Error;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path},
    process::{Command, Stdio},
};

/// Explicit metadata/storage policy; None is permitted only for an empty index.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    /// Encoded metadata string allowance.
    metadata_string_bytes: u64,
    /// Per-list metadata allowance.
    metadata_list_items: u64,
    /// Summary envelope source allowance.
    source_references: u64,
    /// Encoded bytes per admitted summary.
    summary_bytes: u64,
    /// Index plus all current summary bytes.
    aggregate_bytes: u64,
}

/// Review receipt binds sanitized bytes and every evidence revision.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    /// Full sanitized summary Git blob ID in the checkout's object format.
    summary_blob: String,
    /// Algorithm-qualified canonical raw digest.
    raw_blob: String,
    /// Raw schema revision.
    raw_version: [u64; 3],
    /// Summary schema revision.
    summary_version: [u64; 3],
    /// Workload recipe revision.
    recipe_revision: String,
    /// Host estimator revision.
    estimator_revision: String,
    /// Explicit extent of verification: summary-only, submitter-attested or raw-reanalyzed.
    verification: String,
    /// Nonpersonal reviewer role.
    reviewer_role: String,
    /// Non-reversible scrub description.
    scrubbing: String,
    /// Review-selected comparison eligibility.
    comparison_eligible: bool,
}

/// One current admitted summary; raw observations are never repository inputs.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    /// Unique caller-assigned run ID.
    run_id: String,
    /// Public opaque machine alias.
    machine_alias: String,
    /// Fixed family tags for grouping.
    families: Vec<String>,
    /// Repository-relative summary path.
    path: String,
    /// Review receipt bound to those exact bytes.
    receipt: Receipt,
}

/// Index payload deliberately keeps undecided policy explicit.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    /// Missing policy refuses admission of any run.
    policy: Option<Policy>,
    /// Admitted runs in stable input order.
    runs: Vec<Entry>,
}

/// Current strict index envelope.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Index {
    /// Current index namespace.
    namespace: String,
    /// Exact schema revision.
    version: [u64; 3],
    /// Estate owner.
    owner: String,
    /// Data license.
    license: String,
    /// Reviewed source references.
    sources: Vec<String>,
    /// Design labels.
    labels: Vec<String>,
    /// Explicit policy and current runs.
    payload: Payload,
}

/// Path-bearing schema refusal.
fn invalid(detail: impl Into<String>) -> Error {
    Error::Payload {
        path: "data/inputs/compute/index.json".to_owned(),
        detail: detail.into(),
    }
}

/// Enforce repository-relative paths and the dedicated current summary partition.
fn path(entry: &Entry) -> Result<(), Error> {
    let expected = format!("data/inputs/compute/runs/{}.json", entry.run_id);
    let components = Path::new(&entry.path)
        .components()
        .all(|part| matches!(part, Component::Normal(_)));
    let slug = !entry.run_id.is_empty()
        && entry
            .run_id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !components || !slug || entry.path != expected {
        return Err(invalid("invalid summary path or run ID"));
    }
    Ok(())
}

/// Bind a receipt to exact current sanitized bytes through the existing Git route.
fn receipt(
    root: &Path,
    entry: &Entry,
    summary: &SummaryDataset,
    bytes: &[u8],
) -> Result<(), Error> {
    let digest = blob(root, bytes)?;
    let format = match summary.payload.raw.algorithm {
        super::schema::ObjectFormat::Sha1 => "sha1",
        super::schema::ObjectFormat::Sha256 => "sha256",
    };
    let expected_raw = format!("{format}:{}", summary.payload.raw.digest);
    let receipt = &entry.receipt;
    if digest.trim_end() != receipt.summary_blob
        || expected_raw != receipt.raw_blob
        || summary.version != receipt.summary_version
        || summary.payload.raw_version != receipt.raw_version
        || summary.payload.identity.recipe_revision != receipt.recipe_revision
        || summary.payload.identity.estimator_revision != receipt.estimator_revision
    {
        return Err(invalid("stale admission receipt"));
    }
    if !matches!(
        receipt.verification.as_str(),
        "summary-only" | "submitter-attested" | "raw-reanalyzed"
    ) || receipt.reviewer_role.is_empty()
        || receipt.scrubbing.is_empty()
    {
        return Err(invalid("missing verification scope or scrub description"));
    }
    if receipt.comparison_eligible && !comparison_evidence(summary) {
        return Err(invalid(
            "comparison eligibility lacks required public build evidence",
        ));
    }
    if entry.run_id != summary.payload.identity.run_id
        || entry.machine_alias != summary.payload.environment.machine_alias
    {
        return Err(invalid("index identity mismatch"));
    }
    Ok(())
}

/// Load only admitted bounded summaries; raw artifacts are never opened.
pub(super) fn read(root: &Path) -> Result<serde_json::Value, Error> {
    let name = "data/inputs/compute/index.json";
    crate::manifest::check_symlinks(root, name)?;
    let bytes = fs::read(root.join(name)).map_err(|error| invalid(error.to_string()))?;
    let index: Index =
        serde_json::from_slice(&bytes).map_err(|error| invalid(error.to_string()))?;
    layout(root, &index.payload.runs)?;
    if index.namespace != "org.naturrhythmuszentrum.compute.index"
        || index.version != [1, 0, 0]
        || index.owner != "NRZ"
        || index.license != "CC0-1.0"
    {
        return Err(invalid("index envelope"));
    }
    let mut total = u64::try_from(bytes.len()).map_err(|_| invalid("index size"))?;
    if let Some(policy) = index.payload.policy {
        adopted_policy(root)?;
        let caps = [
            policy.metadata_string_bytes,
            policy.metadata_list_items,
            policy.source_references,
            policy.summary_bytes,
            policy.aggregate_bytes,
        ];
        if caps.into_iter().any(|value| value > 9_007_199_254_740_991) {
            return Err(invalid("admission policy count domain"));
        }
        if total > policy.aggregate_bytes {
            return Err(invalid("index exceeds aggregate byte cap"));
        }
        metadata(
            &serde_json::to_value(&index).map_err(|error| invalid(error.to_string()))?,
            "",
            policy,
        )?;
        if u64::try_from(index.sources.len()).map_err(|_| invalid("index source count"))?
            > policy.source_references
        {
            return Err(invalid("index source reference cap"));
        }
    }
    let mut summaries = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for entry in &index.payload.runs {
        let policy = index
            .payload
            .policy
            .ok_or_else(|| invalid("compute admission policy is undecided"))?;
        path(entry)?;
        crate::manifest::check_symlinks(root, &entry.path)?;
        if !seen.insert(&entry.run_id) {
            return Err(invalid("duplicate admitted run"));
        }
        let size = fs::metadata(root.join(&entry.path))
            .map_err(|error| invalid(error.to_string()))?
            .len();
        total = total
            .checked_add(size)
            .ok_or_else(|| invalid("aggregate byte overflow"))?;
        if size > policy.summary_bytes || total > policy.aggregate_bytes {
            return Err(invalid("admission byte caps"));
        }
        let mut bytes = Vec::new();
        let limit = policy
            .summary_bytes
            .checked_add(1)
            .ok_or_else(|| invalid("summary byte overflow"))?;
        fs::File::open(root.join(&entry.path))
            .map_err(|error| invalid(error.to_string()))?
            .take(limit)
            .read_to_end(&mut bytes)
            .map_err(|error| invalid(error.to_string()))?;
        if u64::try_from(bytes.len()).ok() != Some(size) {
            return Err(invalid("summary changed during admission"));
        }
        let summary: SummaryDataset =
            serde_json::from_slice(&bytes).map_err(|error| invalid(error.to_string()))?;
        validate::summary(&summary).map_err(invalid)?;
        metadata(
            &serde_json::to_value(&summary).map_err(|error| invalid(error.to_string()))?,
            "",
            policy,
        )?;
        if u64::try_from(summary.sources.len()).map_err(|_| invalid("source reference overflow"))?
            > policy.source_references
        {
            return Err(invalid("source reference cap"));
        }
        let cases =
            u64::try_from(summary.payload.cases.len()).map_err(|_| invalid("case count"))?;
        if size > size::schema_size(policy)?.report(cases)? {
            return Err(invalid("schema-derived byte cap"));
        }
        total = attachment(root, &summary, total, policy.aggregate_bytes)?;
        receipt(root, entry, &summary, &bytes)?;
        summaries.push(serde_json::json!({"entry":entry,"summary":summary}));
    }
    Ok(serde_json::json!({"policy_adopted":index.payload.policy.is_some(),"runs":summaries}))
}

/// Bound encoded metadata rather than silently truncating required provenance.
fn metadata(value: &serde_json::Value, field: &str, policy: Policy) -> Result<(), Error> {
    match value {
        serde_json::Value::String(text) => {
            let decimal = matches!(
                field,
                "point"
                    | "standard_error"
                    | "lower"
                    | "upper"
                    | "confidence"
                    | "fences"
                    | "control_median_seconds"
                    | "accepted_median_seconds"
            );
            if !decimal {
                let bytes = serde_json::to_vec(text).map_err(|error| invalid(error.to_string()))?;
                if u64::try_from(bytes.len()).map_err(|_| invalid("metadata size"))?
                    > policy.metadata_string_bytes
                {
                    return Err(invalid("metadata string cap"));
                }
            }
        }
        serde_json::Value::Array(values) => {
            let fixed = matches!(
                field,
                "version" | "raw_version" | "estimates" | "fences" | "outlier_counts" | "cases"
            );
            if !fixed
                && u64::try_from(values.len()).map_err(|_| invalid("metadata list size"))?
                    > policy.metadata_list_items
            {
                return Err(invalid("metadata list cap"));
            }
            for value in values {
                metadata(value, field, policy)?;
            }
        }
        serde_json::Value::Object(fields) => {
            for (field, value) in fields {
                metadata(value, field, policy)?;
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
    Ok(())
}

/// Reject stray/raw artifacts and symlinks instead of silently admitting an alternate input tree.
fn layout(root: &Path, entries: &[Entry]) -> Result<(), Error> {
    let directory = root.join("data/inputs/compute");
    if fs::symlink_metadata(&directory)
        .map_err(|error| invalid(error.to_string()))?
        .file_type()
        .is_symlink()
    {
        return Err(invalid("linked compute input directory"));
    }
    for child in fs::read_dir(&directory).map_err(|error| invalid(error.to_string()))? {
        let child = child.map_err(|error| invalid(error.to_string()))?;
        let kind = child
            .file_type()
            .map_err(|error| invalid(error.to_string()))?;
        let name = child.file_name();
        if name == "index.json" && kind.is_file() {
            continue;
        }
        if name != "runs" || !kind.is_dir() {
            return Err(invalid("undeclared compute input"));
        }
        for file in fs::read_dir(child.path()).map_err(|error| invalid(error.to_string()))? {
            let file = file.map_err(|error| invalid(error.to_string()))?;
            if !file
                .file_type()
                .map_err(|error| invalid(error.to_string()))?
                .is_file()
            {
                return Err(invalid("linked or nested compute summary"));
            }
            let expected = entries
                .iter()
                .any(|entry| file.path() == root.join(&entry.path));
            if !expected {
                return Err(invalid("unadmitted compute artifact"));
            }
        }
    }
    Ok(())
}

/// Data cannot adopt a policy whose owning design question remains open.
fn adopted_policy(root: &Path) -> Result<(), Error> {
    let name = "templates/docs/design/compute-benchmarks.md.j2";
    crate::manifest::check_symlinks(root, name)?;
    let design = fs::read_to_string(root.join(name))
        .map_err(|_| invalid("admission policy design unavailable"))?;
    let open = design.lines().any(|line| {
        line.starts_with("**Open Question")
            && line.ends_with("`open:nrz:compute-admission-size-policy`")
    });
    let adopted = design.lines().any(|line| {
        line.starts_with("**Decision") && line.ends_with("`dec:nrz:compute-admission-size-policy`")
    });
    if open || !adopted {
        return Err(invalid("compute admission policy is undecided"));
    }
    Ok(())
}

/// Restricted reproducibility fields cannot support an unrestricted comparison receipt.
fn comparison_evidence(summary: &SummaryDataset) -> bool {
    use super::schema::Evidence;
    let build = &summary.payload.build;
    let required = [
        &build.compiler,
        &build.source_commit,
        &build.lockfile,
        &build.target,
        &build.profile,
        &build.flags,
        &build.backend,
    ];
    let known = required
        .into_iter()
        .all(|value| matches!(value, Evidence::Known(_)));
    let source = match &build.dirty {
        Evidence::Known(false) => true,
        Evidence::Known(true) => matches!(
            summary.payload.identity.source_attachment,
            Evidence::Known(_)
        ),
        Evidence::Unknown(_) | Evidence::Unavailable(_) | Evidence::Redacted(_) => false,
    };
    let dependencies = matches!(build.dependencies, Evidence::Known(_));
    let intent = matches!(build.intent, Evidence::Known(_));
    known && source && dependencies && intent
}

/// Hash the exact bounded bytes that were decoded, without writing an object or following a second path read.
fn blob(root: &Path, bytes: &[u8]) -> Result<String, Error> {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os().filter(|(key, _)| key.to_string_lossy().starts_with("GIT_"))
    {
        command.env_remove(key);
    }
    let mut child = command
        .args(["hash-object", "--stdin", "--no-filters"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| invalid(error.to_string()))?;
    let mut input = child
        .stdin
        .take()
        .ok_or_else(|| invalid("summary hash input unavailable"))?;
    let written = input.write_all(bytes);
    drop(input);
    let output = child
        .wait_with_output()
        .map_err(|error| invalid(error.to_string()))?;
    written.map_err(|error| invalid(error.to_string()))?;
    if !output.status.success() {
        return Err(invalid("summary blob hashing failed"));
    }
    String::from_utf8(output.stdout).map_err(|error| invalid(error.to_string()))
}

/// The summary receipt binds the attachment path and blob; admission checks its actual bytes.
fn attachment(root: &Path, summary: &SummaryDataset, total: u64, cap: u64) -> Result<u64, Error> {
    let super::schema::Evidence::Known(source) = &summary.payload.identity.source_attachment else {
        return Ok(total);
    };
    crate::manifest::check_symlinks(root, &source.path)?;
    let path = root.join(&source.path);
    let info = fs::metadata(&path).map_err(|error| invalid(error.to_string()))?;
    if !info.is_file() {
        return Err(invalid("source attachment is not a regular file"));
    }
    let updated = total
        .checked_add(info.len())
        .filter(|n| *n <= cap)
        .ok_or_else(|| invalid("source attachment aggregate bytes"))?;
    let limit = info
        .len()
        .checked_add(1)
        .ok_or_else(|| invalid("source attachment size"))?;
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|error| invalid(error.to_string()))?
        .take(limit)
        .read_to_end(&mut bytes)
        .map_err(|error| invalid(error.to_string()))?;
    if u64::try_from(bytes.len()).ok() != Some(info.len())
        || blob(root, &bytes)?.trim_end() != source.blob.digest
    {
        return Err(invalid("source attachment content identity"));
    }
    Ok(updated)
}
