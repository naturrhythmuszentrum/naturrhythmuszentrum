// SPDX-License-Identifier: AGPL-3.0-only
//! Testable read-only build provenance capture.

#![expect(
    clippy::redundant_pub_crate,
    reason = "Shared build capture remains crate-visible inside private build-script and test modules because workspace unreachable_pub forbids unrestricted visibility."
)]

use serde_json::{Value, json};
use std::{
    collections::HashSet,
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};

/// Capture one successful UTF-8 command without exposing command stderr.
pub(crate) fn capture(program: &str, arguments: &[&str], directory: &Path) -> Option<String> {
    let output = Command::new(program)
        .args(arguments)
        .current_dir(directory)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|text| text.trim_end().to_owned())
}

/// Serialize an explicit known/unknown field in the runtime tagged grammar.
pub(crate) fn evidence(value: Option<String>) -> Value {
    value.map_or_else(
        || json!({"state":"Unknown","value":"build capture unavailable"}),
        |value| json!({"state":"Known","value":value}),
    )
}

/// Read an environment input and register its dependency before use.
pub(crate) fn variable(name: &str, output: &mut impl Write) -> io::Result<Option<String>> {
    writeln!(output, "cargo:rerun-if-env-changed={name}")?;
    Ok(env::var(name).ok())
}

/// Watch source directories recursively unless they contain an excluded subtree.
pub(crate) fn watch(path: &Path, excluded: &[PathBuf], output: &mut impl Write) -> io::Result<()> {
    let ignored = ignored_paths(path)?;
    watch_directory(path, path, excluded, &ignored, output).map(|_| ())
}

/// List ignored root-relative paths once, before walking any source directory.
fn ignored_paths(root: &Path) -> io::Result<HashSet<PathBuf>> {
    let output = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
        ])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other("source ignore listing failed"));
    }
    let paths = String::from_utf8(output.stdout)
        .map_err(|_| io::Error::other("source ignore listing is not UTF-8"))?;
    Ok(paths
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .collect())
}

/// Return whether a recursive watch would include excluded or ignored content.
fn watch_directory(
    path: &Path,
    root: &Path,
    excluded: &[PathBuf],
    ignored: &HashSet<PathBuf>,
    output: &mut impl Write,
) -> io::Result<bool> {
    let mut children = Vec::new();
    let mut barrier = excluded.iter().any(|directory| directory.starts_with(path));
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        let name = entry.file_name();
        let omitted = name == ".git" || name == "target";
        if omitted || excluded.iter().any(|output| child.starts_with(output)) {
            barrier = true;
            continue;
        }
        let kind = entry.file_type()?;
        let relative = child.strip_prefix(root).map_err(io::Error::other)?;
        if ignored.contains(relative) {
            barrier |= kind.is_dir() || kind.is_symlink();
            continue;
        }
        if kind.is_symlink() {
            barrier = true;
        } else if kind.is_dir() {
            barrier |= watch_directory(&child, root, excluded, ignored, &mut children)?;
        } else {
            writeln!(children, "cargo:rerun-if-changed={}", child.display())?;
        }
    }
    if path != root && !barrier {
        writeln!(output, "cargo:rerun-if-changed={}", path.display())?;
    } else {
        output.write_all(&children)?;
    }
    Ok(barrier)
}

/// Missing watches are perpetually stale in Cargo; use an existing ref ancestor.
fn watch_git_path(
    root: &Path,
    name: &str,
    parent_if_missing: bool,
    output: &mut impl Write,
) -> io::Result<()> {
    if let Some(resolved) = capture("git", &["rev-parse", "--git-path", name], root) {
        let mut path = root.join(resolved);
        if parent_if_missing {
            while !path.exists() {
                if !path.pop() {
                    return Ok(());
                }
            }
        }
        if path.exists() {
            writeln!(output, "cargo:rerun-if-changed={}", path.display())?;
        }
    }
    Ok(())
}

/// Register process configuration before any compiler or Git command is captured.
pub(crate) fn environment(output: &mut impl Write) -> io::Result<()> {
    for name in [
        "PATH",
        "HOME",
        "XDG_CONFIG_HOME",
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_CONFIG",
        "GIT_CONFIG_GLOBAL",
        "GIT_CONFIG_SYSTEM",
        "GIT_CONFIG_NOSYSTEM",
        "GIT_CONFIG_COUNT",
        "GIT_CEILING_DIRECTORIES",
        "GIT_DISCOVERY_ACROSS_FILESYSTEM",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_CONFIG_PARAMETERS",
        "GIT_OPTIONAL_LOCKS",
        "GIT_NAMESPACE",
        "GIT_SHALLOW_FILE",
        "GIT_REPLACE_REF_BASE",
        "GIT_NO_REPLACE_OBJECTS",
        "GIT_ATTR_NOSYSTEM",
        "CARGO_TARGET_DIR",
        "OUT_DIR",
    ] {
        let _value = variable(name, output)?;
    }
    for (name, _) in env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("GIT_")) {
        if let Some(name) = name.to_str() {
            let _value = variable(name, output)?;
        }
    }
    Ok(())
}

/// Recover Cargo's actual target root from its absolute build output location.
/// This also handles a relative target directory supplied from a crate subdirectory.
pub(crate) fn output_root(output: &Path, target: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let build = output.parent()?.parent()?;
    if build.file_name()? != "build" {
        return None;
    }
    let root = build.parent()?.parent()?;
    let root = if root.file_name() == target {
        root.parent()?
    } else {
        root
    };
    Some(root.to_path_buf())
}

/// Resolve output exclusions before traversal; no output ancestor is recursively watched.
fn excluded(root: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<_> = ["CARGO_TARGET_DIR", "OUT_DIR"]
        .into_iter()
        .filter_map(env::var_os)
        .map(|path| root.join(path))
        .map(|path| path.canonicalize().unwrap_or(path))
        .collect();
    if let Some(output) = env::var_os("OUT_DIR")
        && let Some(actual) = output_root(Path::new(&output), env::var_os("TARGET").as_deref())
    {
        paths.push(actual.canonicalize().unwrap_or(actual));
    }
    paths
}

/// Capture source facts against the resolved checkout, including worktree Git paths.
pub(crate) fn source(
    crate_root: &Path,
    output: &mut impl Write,
) -> io::Result<(Value, Value, Value)> {
    source_with_probe(crate_root, output, watch)
}

/// Inject filesystem refusal without relying on host account privileges in tests.
pub(crate) fn source_with_probe<W: Write>(
    crate_root: &Path,
    output: &mut W,
    probe: impl FnOnce(&Path, &[PathBuf], &mut W) -> io::Result<()>,
) -> io::Result<(Value, Value, Value)> {
    let root = capture("git", &["rev-parse", "--show-toplevel"], crate_root).map(PathBuf::from);
    let Some(root) = root else {
        return Ok((evidence(None), evidence(None), evidence(None)));
    };
    if probe(&root, &excluded(&root), output).is_err() {
        let unknown =
            json!({"state":"Unknown","value":"source directory probe denied or unreadable"});
        return Ok((unknown.clone(), unknown.clone(), unknown));
    }
    for path in ["HEAD", "index", "packed-refs"] {
        watch_git_path(&root, path, false, output)?;
    }
    if let Some(reference) = capture("git", &["symbolic-ref", "-q", "HEAD"], &root) {
        watch_git_path(&root, &reference, true, output)?;
    }
    let commit = evidence(capture("git", &["rev-parse", "HEAD"], &root));
    let dirty = capture(
        "git",
        &[
            "--no-optional-locks",
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
        ],
        &root,
    )
    .map_or_else(
        || evidence(None),
        |status| json!({"state":"Known","value":!status.is_empty()}),
    );
    let format = capture("git", &["rev-parse", "--show-object-format"], &root);
    let digest = capture(
        "git",
        &["hash-object", "--no-filters", "--", "Cargo.lock"],
        &root,
    );
    let lockfile = evidence(
        format
            .zip(digest)
            .map(|(format, digest)| format!("{format}:{digest}")),
    );
    Ok((commit, dirty, lockfile))
}

/// Capture only package names and versions, never registry credentials or source URLs.
pub(crate) fn dependency_versions(root: &Path) -> Option<Vec<String>> {
    let text = fs::read_to_string(root.join("Cargo.lock")).ok()?;
    let mut versions = Vec::new();
    for package in text.split("[[package]]").skip(1) {
        let field = |name: &str| {
            package
                .lines()
                .find_map(|line| line.strip_prefix(name))
                .and_then(|value| value.trim().strip_prefix('"'))
                .and_then(|value| value.strip_suffix('"'))
        };
        let name = field("name = ")?;
        if !matches!(
            name,
            "nrz-compute"
                | "nrz-matrix"
                | "faer"
                | "faer-traits"
                | "pulp"
                | "serde"
                | "serde_json"
                | "thiserror"
        ) {
            continue;
        }
        let version = field("version = ")?;
        versions.push(format!("{name}@{version}"));
    }
    versions.sort();
    Some(versions)
}

/// Classify effective target-selection options without inferring instructions from runtime ISA.
pub(crate) fn build_intent(flags: &str) -> String {
    let mut cpu = None;
    let mut explicit_features = false;
    for option in flags.split('\u{1f}') {
        let option = option.strip_prefix("-C").unwrap_or(option);
        if let Some(value) = option.strip_prefix("target-cpu=") {
            cpu = Some(value);
        }
        if let Some(features) = option.strip_prefix("target-feature=") {
            explicit_features |= features
                .split(',')
                .any(|feature| !matches!(feature, "+crt-static" | "-crt-static" | ""));
        }
    }
    match cpu {
        Some("native") => "NativeRequested",
        Some(_) => "ExplicitTarget",
        None if explicit_features => "ExplicitTarget",
        None => "Baseline",
    }
    .to_owned()
}
