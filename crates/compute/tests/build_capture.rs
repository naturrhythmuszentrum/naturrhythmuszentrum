// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`capture_failures_remain_unknown`] | compute | Missing repositories and failed commands retain unknown provenance. |
//! | [`capture_resolves_lockfile_and_watches_only_sources`] | compute | Crate-relative capture hashes the root lockfile and excludes recursive output watches. |
//! | [`capture_handles_packed_refs_and_ignored_trees`] | compute | Packed branch refs use existing parent watches and ignored trees are skipped. |
//! | [`capture_classifies_instruction_flags`] | compute | Linkage flags do not claim explicit instruction selection. |

#[path = "../build_capture.rs"]
mod build_capture;
use std::{fs, path::PathBuf, process::Command};

/// Isolated deterministic filesystem fixture.
#[derive(Debug)]
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _removed = fs::remove_dir_all(&self.0);
    }
}

/// Each test owns a separate path, with no live clock in its assertions.
fn root(name: &str) -> Result<Root, Box<dyn std::error::Error>> {
    let path = std::env::temp_dir().join(format!("compute-build-{name}-{}", std::process::id()));
    fs::create_dir(&path)?;
    Ok(Root(path.canonicalize()?))
}

/// Missing repositories and failed commands retain unknown provenance.
/// ´claim:compute:build-capture-failures-remain-unknown´
/// ´test:integration:build-capture-failures-remain-unknown´
#[test]
fn capture_failures_remain_unknown() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("COMPUTE_CAPTURE_ISOLATED").is_none() {
        let mut command = Command::new(std::env::current_exe()?);
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("GIT_") {
                command.env_remove(name);
            }
        }
        let status = command
            .args(["--exact", "capture_failures_remain_unknown"])
            .env("COMPUTE_CAPTURE_ISOLATED", "1")
            .env(
                "GIT_CEILING_DIRECTORIES",
                std::env::temp_dir().canonicalize()?,
            )
            .status()?;
        assert!(status.success());
        return Ok(());
    }
    let root = root("missing")?;
    let mut watches = Vec::new();
    assert!(build_capture::watch(&root.0, &[], &mut watches).is_err());
    assert_eq!(watches, [] as [u8; 0]);
    let unknown = build_capture::evidence(None);
    assert_eq!(
        build_capture::source(&root.0, &mut watches)?,
        (unknown.clone(), unknown.clone(), unknown)
    );
    assert_eq!(
        build_capture::capture("git", &["not-a-git-command"], &root.0),
        None
    );
    assert_eq!(
        build_capture::capture("/nonexistent/compute-probe", &[], &root.0),
        None
    );
    build_capture::environment(&mut watches)?;
    let watches = String::from_utf8(watches)?;
    assert!(watches.contains("cargo:rerun-if-env-changed=PATH\n"));
    assert!(watches.contains("cargo:rerun-if-env-changed=HOME\n"));
    assert!(watches.contains("cargo:rerun-if-env-changed=GIT_DIR\n"));
    Ok(())
}

/// Crate-relative capture hashes the root lockfile and excludes recursive output watches.
/// ´claim:compute:build-capture-root-lockfile-and-source-watches´
/// ´test:integration:build-capture-root-lockfile-and-source-watches´
#[test]
fn capture_resolves_lockfile_and_watches_only_sources() -> Result<(), Box<dyn std::error::Error>> {
    let root = root("checkout")?;
    let initialized = Command::new("git")
        .args(["init", "--template="])
        .current_dir(&root.0)
        .output()?;
    assert!(initialized.status.success());
    fs::write(
        root.0.join("Cargo.lock"),
        "[[package]]\nname = \"faer\"\nversion = \"0.24.4\"\n",
    )?;
    fs::create_dir_all(root.0.join("crates/compute"))?;
    fs::create_dir_all(root.0.join("target/debug"))?;
    let mut watches = Vec::new();
    let (_, _, lockfile) = build_capture::source(&root.0.join("crates/compute"), &mut watches)?;
    let digest = build_capture::capture(
        "git",
        &["hash-object", "--no-filters", "--", "Cargo.lock"],
        &root.0,
    )
    .ok_or("hash unavailable")?;
    let format = build_capture::capture("git", &["rev-parse", "--show-object-format"], &root.0)
        .ok_or("format unavailable")?;
    assert_eq!(
        lockfile,
        build_capture::evidence(Some(format!("{format}:{digest}")))
    );
    assert_eq!(
        build_capture::dependency_versions(&root.0),
        Some(vec!["faer@0.24.4".to_owned()])
    );
    let fields: [_; 3] = build_capture::source_with_probe(&root.0, &mut Vec::new(), |_, _, _| {
        Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
    })?
    .into();
    for unknown in fields {
        assert_eq!(unknown["state"], "Unknown");
        assert_eq!(
            unknown["value"],
            "source directory probe denied or unreadable"
        );
    }
    let watches = String::from_utf8(watches)?;
    assert!(
        !watches
            .lines()
            .any(|line| line == format!("cargo:rerun-if-changed={}", root.0.display()))
    );
    assert!(!watches.contains("/target"));
    assert!(watches.contains("/crates\n"));
    fs::create_dir_all(root.0.join("custom-build/debug"))?;
    fs::write(root.0.join("custom-build/debug/generated"), "output")?;
    let mut watches = Vec::new();
    build_capture::watch(
        &root.0,
        &[root.0.join("custom-build").canonicalize()?],
        &mut watches,
    )?;
    assert!(!String::from_utf8(watches)?.contains("custom-build"));
    Ok(())
}

/// Packed branch refs use existing parent watches and ignored trees are skipped.
/// ´claim:compute:build-capture-packed-refs-and-ignored-trees´
/// ´test:integration:build-capture-packed-refs-and-ignored-trees´
#[test]
fn capture_handles_packed_refs_and_ignored_trees() -> Result<(), Box<dyn std::error::Error>> {
    let root = root("packed")?;
    for arguments in [
        vec!["init", "--template="],
        vec!["symbolic-ref", "HEAD", "refs/heads/capture-test"],
    ] {
        assert!(build_capture::capture("git", &arguments, &root.0).is_some());
    }
    fs::write(root.0.join("Cargo.lock"), "lockfile")?;
    fs::write(root.0.join(".gitignore"), "cache/\n*.swp\n")?;
    fs::create_dir_all(root.0.join("src/nested"))?;
    fs::write(root.0.join("src/editor.swp"), "ignored file")?;
    fs::create_dir_all(root.0.join("cache/unreadable"))?;
    fs::write(root.0.join("cache/unreadable/ignored"), "ignored")?;
    for arguments in [
        vec!["add", "Cargo.lock", ".gitignore"],
        vec![
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            "fixture",
        ],
        vec!["pack-refs", "--all"],
    ] {
        assert!(build_capture::capture("git", &arguments, &root.0).is_some());
    }
    let loose = root.0.join(".git/refs/heads/capture-test");
    assert!(!loose.exists());
    let mut watches = Vec::new();
    let (commit, _, _) = build_capture::source(&root.0, &mut watches)?;
    assert_eq!(commit["state"], "Known");
    let watches = String::from_utf8(watches)?;
    assert!(watches.contains("/.git/refs/heads\n"));
    assert!(watches.contains("/.git/packed-refs\n"));
    assert!(watches.contains("/src\n"));
    assert!(!watches.contains("/cache"));
    for line in watches.lines() {
        let path = line
            .strip_prefix("cargo:rerun-if-changed=")
            .ok_or("unexpected watch")?;
        assert!(std::path::Path::new(path).exists());
    }
    Ok(())
}

/// Linkage flags do not claim explicit instruction selection.
/// ´claim:compute:build-capture-instruction-intent´
/// ´test:integration:build-capture-instruction-intent´
#[test]
fn capture_classifies_instruction_flags() {
    use std::path::Path;
    assert_eq!(
        build_capture::output_root(Path::new("/checkout/custom/debug/build/pkg/out"), None),
        Some(PathBuf::from("/checkout/custom"))
    );
    assert_eq!(
        build_capture::output_root(
            Path::new("/checkout/custom/x86_64-unknown-linux-gnu/debug/build/pkg/out"),
            Some(std::ffi::OsStr::new("x86_64-unknown-linux-gnu"))
        ),
        Some(PathBuf::from("/checkout/custom"))
    );
    for flags in [
        "",
        "-Ctarget-feature=-crt-static",
        "-Ctarget-feature=+crt-static",
    ] {
        assert_eq!(build_capture::build_intent(flags), "Baseline");
    }
    assert_eq!(
        build_capture::build_intent("-Ctarget-feature=+avx2,-crt-static"),
        "ExplicitTarget"
    );
    assert_eq!(
        build_capture::build_intent("-Ctarget-cpu=native"),
        "NativeRequested"
    );
    assert_eq!(
        build_capture::build_intent("-Ctarget-cpu=x86-64-v3"),
        "ExplicitTarget"
    );
}
