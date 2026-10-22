// SPDX-License-Identifier: AGPL-3.0-only
//! Supported dependency edges are read from Cargo's resolved plain-text tree without a second runtime.

use std::{collections::BTreeSet, fmt::Write, path::Path, process::Command};

/// Fallible architecture checks never replace failed Cargo inspection with an empty graph.
type CheckResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Each existing workspace owner has an explicit normal and test-only permission inventory.
#[derive(Debug)]
struct Package {
    /// Resolved package name, independent of dependency aliases.
    name: &'static str,
    /// Manifest path relative to the workspace root.
    manifest: &'static str,
    /// Permitted normal edges; no package currently has a build edge.
    normal: &'static [&'static str],
    /// Permitted development edges, including independent oracle libraries.
    development: &'static [&'static str],
}

/// New workspace owners or direct edges require deliberate architecture review.
const PACKAGES: [Package; 6] = [
    Package {
        name: "nrz-cas",
        manifest: "crates/cas/Cargo.toml",
        normal: &[
            "petgraph",
            "num-bigint",
            "num-integer",
            "num-traits",
            "num-rational",
            "num-complex",
        ],
        development: &[],
    },
    Package {
        name: "nrz-scalar",
        manifest: "crates/scalar/Cargo.toml",
        normal: &["thiserror"],
        development: &["num-bigint", "num-rational", "num-traits"],
    },
    Package {
        name: "nrz-accuracy",
        manifest: "crates/accuracy/Cargo.toml",
        normal: &["nrz-scalar"],
        development: &["num-bigint", "num-rational", "num-traits"],
    },
    Package {
        name: "nrz-bindings",
        manifest: "crates/bindings/Cargo.toml",
        normal: &["nrz-cas", "thiserror"],
        development: &[],
    },
    Package {
        name: "nrz-matrix",
        manifest: "crates/matrix/Cargo.toml",
        normal: &[
            "faer",
            "nrz-scalar",
            "nrz-accuracy",
            "nrz-bindings",
            "thiserror",
        ],
        development: &[],
    },
    Package {
        name: "nrz-docgen",
        manifest: "tools/docgen/Cargo.toml",
        normal: &[
            "nrz-cas",
            "nrz-bindings",
            "minijinja",
            "serde",
            "serde_json",
            "toml",
            "clap",
            "thiserror",
        ],
        development: &["tempfile"],
    },
];

/// No workspace feature switches are currently admitted; all three public Cargo modes must agree.
const CONFIGURATIONS: [&[&str]; 3] = [&[], &["--no-default-features"], &["--all-features"]];

/// No feature or test-only route may reverse the public engine dependency graph.
/// ´test:integration:supported-feature-dependencies-match-the-public-graph´
#[test]
fn supported_feature_dependencies_match_the_public_graph() -> CheckResult {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let owners = tree(&root, &["--workspace", "--depth", "0", "-e", "normal"])?;
    let expected = PACKAGES
        .iter()
        .map(|package| package.name.to_owned())
        .collect::<BTreeSet<_>>();
    if packages(&owners)? != expected {
        return Err("workspace owner inventory changed".into());
    }
    for package in &PACKAGES {
        inspect(&root, package)?;
    }
    negative_twins()
}

/// Check each edge kind separately, across every target and supported feature configuration.
fn inspect(root: &Path, package: &Package) -> CheckResult {
    manifest_policy(&std::fs::read_to_string(root.join(package.manifest))?)?;
    for features in CONFIGURATIONS {
        for (kind, expected) in [
            ("normal", package.normal),
            ("build", &[]),
            ("dev", package.development),
        ] {
            let mut arguments = vec!["-p", package.name, "-e", kind, "--depth", "1"];
            arguments.extend_from_slice(features);
            let output = tree(root, &arguments)?;
            if let Err(cause) = edges(&output, package.name, expected) {
                return Err(format!("{} {kind} {features:?}: {cause}", package.name).into());
            }
        }
    }
    Ok(())
}

/// Offline and locked inspection cannot silently update manifests or the package resolution.
fn tree(root: &Path, arguments: &[&str]) -> CheckResult<String> {
    let output = Command::new(env!("CARGO"))
        .current_dir(root)
        .args([
            "tree",
            "--offline",
            "--locked",
            "--prefix",
            "none",
            "--target",
            "all",
        ])
        .args(arguments)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "cargo tree failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

/// Cargo prints resolved package names before versions; paths and duplicate annotations are not identities.
fn packages(output: &str) -> CheckResult<BTreeSet<String>> {
    let mut result = BTreeSet::new();
    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if matches!(line, "[build-dependencies]" | "[dev-dependencies]") {
            continue;
        }
        let mut words = line.split_whitespace();
        let name = words.next().ok_or("missing resolved package")?;
        let version = words.next().ok_or("missing resolved version")?;
        if !version.starts_with('v') || version.len() == 1 {
            return Err(format!("unrecognized Cargo tree line: {line}").into());
        }
        result.insert(name.to_owned());
    }
    Ok(result)
}

/// Compare direct package identities for one normal, build or development edge kind.
fn edges(output: &str, owner: &str, allowed: &[&str]) -> CheckResult {
    let mut actual = packages(output)?;
    if !actual.remove(owner) {
        return Err("Cargo tree omitted its requested root".into());
    }
    let expected = allowed
        .iter()
        .copied()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!("resolved edges {actual:?} differ from permitted {expected:?}").into());
    }
    Ok(())
}

/// The current manifests admit no feature switches, optional edges or target-specific declarations.
fn manifest_policy(manifest: &str) -> CheckResult {
    let mut section = String::new();
    for line in manifest.lines() {
        let content = line.split('#').next().ok_or("manifest line")?.trim();
        let compact = content
            .chars()
            .filter(|value| !value.is_whitespace())
            .collect::<String>();
        if compact.is_empty() {
            continue;
        }
        let optional_dependency =
            section.contains("dependencies") && compact.contains("optional=true");
        let target_key = section.is_empty() && compact.starts_with("target=");
        let feature_key = section.is_empty() && compact.starts_with("features=");
        let unsupported_features =
            section == "features" || (feature_key && compact != "features={}");
        if compact.starts_with('[') {
            section = compact.trim_matches(['[', ']']).replace(['\'', '"'], "");
            if section.starts_with("target.") || section == "target" {
                return Err("unsupported target-specific dependency declaration".into());
            }
        } else if unsupported_features {
            return Err("unsupported workspace feature switch".into());
        } else if optional_dependency || target_key {
            return Err("unsupported optional or target dependency".into());
        }
    }
    Ok(())
}

/// Negative fixtures exercise unsupported features and forbidden edges without editing any manifest.
fn negative_twins() -> CheckResult {
    for manifest in [
        "[features]\nunchecked-assurance = []\n",
        "[dependencies]\nnrz-bindings = { path = '../bindings', optional = true }\n",
        "[target.'cfg(windows)'.dependencies]\nnrz-matrix = { path = '../matrix' }\n",
    ] {
        if manifest_policy(manifest).is_ok() {
            return Err("manifest policy accepted its negative twin".into());
        }
    }
    for allowed in [PACKAGES[1].normal, &[], PACKAGES[1].development] {
        let mut output = String::from("nrz-scalar v0.1.0\n");
        for name in allowed {
            writeln!(output, "{name} v1.0.0")?;
        }
        output.push_str("nrz-bindings v0.1.0 (/a/misleading/thiserror/path)\n");
        if edges(&output, "nrz-scalar", allowed).is_ok() {
            return Err("edge policy accepted a forbidden resolved target".into());
        }
    }
    manifest_policy("[features]\n# no feature switches\n[dependencies]\nthiserror = '2'\n")?;
    edges(
        "nrz-scalar v0.1.0 (/workspace/scalar)\nthiserror v2.0.0 (*)\n",
        "nrz-scalar",
        &["thiserror"],
    )
}
