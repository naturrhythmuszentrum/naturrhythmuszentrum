// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`criterion_is_exclusive_to_benchmark_development`] | docgen | Only the unpublished benchmark host can resolve Criterion, through its pinned development dependency. |

use serde_json::Value;
use std::{collections::BTreeSet, path::Path, process::Command};

/// Borrow one required Cargo metadata array without guessing at a changed schema.
fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], Box<dyn std::error::Error>> {
    value[key]
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("Cargo metadata lacks array {key}").into())
}

/// Check package identity rather than dependency aliases, including transitive edges.
fn boundary(metadata: &Value) -> Result<(), Box<dyn std::error::Error>> {
    let packages = array(metadata, "packages")?;
    let bench = packages
        .iter()
        .find(|package| package["name"] == "nrz-bench")
        .ok_or("missing benchmark package")?;
    let criterion = packages
        .iter()
        .filter(|package| package["name"] == "criterion")
        .collect::<Vec<_>>();
    if criterion.len() != 1 {
        return Err("expected exactly one resolved Criterion version".into());
    }
    let criterion = criterion.first().ok_or("missing Criterion")?;
    if criterion["version"] != "0.8.2" || bench["publish"] != serde_json::json!([]) {
        return Err("Criterion version or unpublished benchmark boundary changed".into());
    }
    let members = array(metadata, "workspace_members")?;
    if !members.contains(&bench["id"]) {
        return Err("benchmark host must be a workspace member".into());
    }
    for package in packages
        .iter()
        .filter(|package| members.contains(&package["id"]))
    {
        for dependency in array(package, "dependencies")? {
            if dependency["name"] == "criterion" {
                let owner = package["id"] == bench["id"];
                let development = dependency["kind"] == "dev";
                let pinned = dependency["req"] == "=0.8.2";
                let minimal = dependency["uses_default_features"] == false;
                let features = dependency["features"] == serde_json::json!(["cargo_bench_support"]);
                if !(owner && development && pinned && minimal && features) {
                    return Err(format!(
                        "Criterion declaration outside benchmark policy: {}",
                        package["name"]
                    )
                    .into());
                }
            }
        }
    }
    let nodes = array(&metadata["resolve"], "nodes")?;
    for node in nodes {
        for dependency in array(node, "deps")? {
            if dependency["pkg"] == bench["id"] {
                return Err("another package depends on the benchmark host".into());
            }
            if dependency["pkg"] == criterion["id"] {
                let owner = node["id"] == bench["id"];
                let development = array(dependency, "dep_kinds")?
                    .iter()
                    .all(|kind| kind["kind"] == "dev");
                if !(owner && development) {
                    return Err("Criterion resolved outside benchmark development".into());
                }
            }
        }
    }
    // Follow all resolved normal/build/dev edges for each other workspace root.
    for member in members.iter().filter(|member| **member != bench["id"]) {
        let mut pending = vec![member];
        let mut seen = BTreeSet::new();
        while let Some(id) = pending.pop() {
            let identity = id.as_str().ok_or("non-string Cargo package identity")?;
            if !seen.insert(identity) {
                continue;
            }
            if id == &criterion["id"] {
                return Err(format!("Criterion reachable from {member}").into());
            }
            let node = nodes
                .iter()
                .find(|node| node["id"] == *id)
                .ok_or("missing resolve node")?;
            for dependency in array(node, "deps")? {
                pending.push(&dependency["pkg"]);
            }
        }
    }
    Ok(())
}

/// Only the unpublished benchmark host can resolve Criterion, through its pinned development dependency.
/// ´claim:docgen:criterion-is-exclusive-to-benchmark-development´
/// ´test:integration:criterion-is-exclusive-to-benchmark-development´
#[test]
fn criterion_is_exclusive_to_benchmark_development() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--locked",
            "--all-features",
            "--format-version",
            "1",
        ])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)?;
    boundary(&metadata)
}
