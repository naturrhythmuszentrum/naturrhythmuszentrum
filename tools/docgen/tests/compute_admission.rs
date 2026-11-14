// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`compute_input_layout_refuses_unadmitted_artifacts`] | docgen | The compute input partition accepts its empty index and refuses undeclared artifacts. |
//! | [`compute_receipts_bind_bytes_revisions_and_raw_identity`] | docgen | Admission receipts bind summary bytes, raw identity and every declared revision. |
//! | [`compute_templates_preserve_separate_observations`] | docgen | Strict compute templates retain separate run identities, intervals and escaped provenance. |
//! | [`compute_source_attachments_bind_reviewed_repository_bytes`] | docgen | Source attachments bind exact repository patch bytes and refuse changed content. |
//! | [`compute_open_policy_refuses_data_activation`] | docgen | An index edit cannot adopt the repository's still-open policy. |

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Owned isolated test input partition.
#[derive(Debug)]
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _removed = fs::remove_dir_all(&self.0);
    }
}

/// Create a unique fixture tree without loading raw artifacts.
fn root(name: &str) -> Result<Root, Box<dyn std::error::Error>> {
    let path = std::env::temp_dir().join(format!(
        "nrz-compute-admission-{name}-{}",
        std::process::id()
    ));
    fs::create_dir(&path)?;
    fs::create_dir_all(path.join("data/inputs/compute"))?;
    fs::write(
        path.join("data/inputs/compute/index.json"),
        include_bytes!("../../../data/inputs/compute/index.json"),
    )?;
    Ok(Root(path))
}

/// Hash only the sanitized summary fixture through an isolated Git context.
fn git(root: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os().filter(|(key, _)| key.to_string_lossy().starts_with("GIT_"))
    {
        command.env_remove(key);
    }
    let output = command
        .args(args)
        .current_dir(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()?;
    if !output.status.success() {
        return Err("fixture Git command failed".into());
    }
    Ok(String::from_utf8(output.stdout)?.trim_end().to_owned())
}

/// The compute input partition accepts its empty index and refuses undeclared artifacts.
/// ´claim:docgen:compute-input-layout-refuses-unadmitted-artifacts´
/// ´test:integration:compute-input-layout-refuses-unadmitted-artifacts´
#[test]
fn compute_input_layout_refuses_unadmitted_artifacts() -> Result<(), Box<dyn std::error::Error>> {
    let root = root("layout")?;
    nrz_docgen::compute::validate_inputs(&root.0)?;
    fs::write(root.0.join("data/inputs/compute/unadmitted.json"), "{}")?;
    assert!(nrz_docgen::compute::validate_inputs(&root.0).is_err());
    Ok(())
}

/// Build one explicitly admitted sanitized summary and matching receipt.
fn admitted(root: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    fs::create_dir_all(root.join("templates/docs/design"))?;
    fs::write(
        root.join("templates/docs/design/compute-benchmarks.md.j2"),
        "**Decision (Synthetic fixture policy)** · `dec:nrz:compute-admission-size-policy`\n",
    )?;
    git(root, &["init", "--object-format=sha1", "--template="])?;
    fs::create_dir(root.join("data/inputs/compute/runs"))?;
    let path = "data/inputs/compute/runs/fixture.json";
    fs::write(
        root.join(path),
        include_bytes!("../../../data/fixtures/compute/valid-complete.json"),
    )?;
    let blob = git(root, &["hash-object", "--no-filters", path])?;
    let summary: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../data/fixtures/compute/valid-complete.json"
    ))?;
    let mut index: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../data/inputs/compute/index.json"))?;
    index["payload"]["policy"] = serde_json::json!({"metadata_string_bytes":4096,"metadata_list_items":64,
        "source_references":64,"summary_bytes":1_048_576,"aggregate_bytes":2_097_152});
    index["payload"]["runs"] = serde_json::json!([{"run_id":"fixture","machine_alias":"fixture-machine","families":["arithmetic"],"path":path,
        "receipt":{"summary_blob":blob,"raw_blob":format!("sha1:{}","0".repeat(40)),"raw_version":[1,1,0],"summary_version":[1,1,0],
        "recipe_revision":summary["payload"]["identity"]["recipe_revision"],"estimator_revision":summary["payload"]["identity"]["estimator_revision"],
        "verification":"summary-only","reviewer_role":"data maintainer","scrubbing":"synthetic public fixture","comparison_eligible":false}}]);
    Ok(index)
}

/// Admission receipts bind summary bytes, raw identity and every declared revision.
/// ´claim:docgen:compute-receipts-bind-bytes-revisions-and-raw-identity´
/// ´test:integration:compute-receipts-bind-bytes-revisions-and-raw-identity´
#[test]
fn compute_receipts_bind_bytes_revisions_and_raw_identity() -> Result<(), Box<dyn std::error::Error>>
{
    let root = root("receipt")?;
    let index = admitted(&root.0)?;
    let path = root.0.join("data/inputs/compute/index.json");
    fs::write(&path, serde_json::to_vec(&index)?)?;
    nrz_docgen::compute::validate_inputs(&root.0)?;
    for field in [
        "summary_blob",
        "raw_blob",
        "recipe_revision",
        "estimator_revision",
        "verification",
    ] {
        let mut changed = index.clone();
        changed["payload"]["runs"][0]["receipt"][field] = "wrong".into();
        fs::write(&path, serde_json::to_vec(&changed)?)?;
        assert!(
            nrz_docgen::compute::validate_inputs(&root.0).is_err(),
            "{field}"
        );
    }
    for field in ["summary_version", "raw_version"] {
        let mut changed = index.clone();
        changed["payload"]["runs"][0]["receipt"][field] = serde_json::json!([1, 0, 1]);
        fs::write(&path, serde_json::to_vec(&changed)?)?;
        assert!(
            nrz_docgen::compute::validate_inputs(&root.0).is_err(),
            "{field}"
        );
    }
    fs::write(&path, serde_json::to_vec(&index)?)?;
    let summary = root.0.join("data/inputs/compute/runs/fixture.json");
    let mut bytes = fs::read(&summary)?;
    bytes.push(b' ');
    fs::write(summary, bytes)?;
    assert!(nrz_docgen::compute::validate_inputs(&root.0).is_err());
    Ok(())
}

/// Two synthetic observations remain separate even when their estimates are equal.
fn render_context(root: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let index = admitted(root)?;
    let mut summary: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../data/fixtures/compute/valid-complete.json"
    ))?;
    summary["payload"]["build"]["compiler"] =
        serde_json::json!({"state":"Known","value":"compiler|field\nnext line"});
    nrz_docgen::compute::validate_summary_bytes(&serde_json::to_vec(&summary)?)?;
    let first = serde_json::json!({"entry":index["payload"]["runs"][0],"summary":summary});
    let mut second = first.clone();
    second["entry"]["run_id"] = "fixture-two".into();
    second["summary"]["payload"]["build"]["profile"] =
        serde_json::json!({"state":"Known","value":"changed-profile"});
    second["entry"]["machine_alias"] = "another-machine".into();
    second["summary"]["payload"]["identity"]["run_id"] = "fixture-two".into();
    second["summary"]["payload"]["environment"]["machine_alias"] = "another-machine".into();
    let mut partial = first.clone();
    partial["entry"]["run_id"] = "fixture-partial".into();
    partial["summary"]["payload"]["identity"]["run_id"] = "fixture-partial".into();
    partial["summary"]["payload"]["execution"]["complete"] = false.into();
    partial["summary"]["payload"]["cases"][0]["counts"]["resamples"] = 0.into();
    partial["summary"]["payload"]["cases"][0]["outcome"] =
        serde_json::json!({"status":"Incomplete","value":"Capacity"});
    nrz_docgen::compute::validate_summary_bytes(&serde_json::to_vec(&partial["summary"])?)?;
    let unavailable: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../data/fixtures/compute/valid-unsupported.json"
    ))?;
    let mut unsupported = first.clone();
    unsupported["entry"]["run_id"] = "fixture-unsupported".into();
    unsupported["summary"] = unavailable;
    unsupported["summary"]["payload"]["identity"]["run_id"] = "fixture-unsupported".into();
    let context = serde_json::json!({"compute":{"policy_adopted":true,"runs":[first,second,partial,unsupported]}});
    Ok(context)
}

/// Evaluate the real templates with the production strictness and escaping policy.
fn render_environment() -> Result<minijinja::Environment<'static>, minijinja::Error> {
    let mut environment = minijinja::Environment::new();
    environment.set_keep_trailing_newline(true);
    environment.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
    environment.set_auto_escape_callback(|_| minijinja::AutoEscape::None);
    for (name, source) in [
        (
            "index",
            include_str!("../../../templates/docs/compute/index.md.j2"),
        ),
        (
            "arithmetic",
            include_str!("../../../templates/docs/compute/arithmetic.md.j2"),
        ),
        (
            "memory",
            include_str!("../../../templates/docs/compute/memory.md.j2"),
        ),
        (
            "matrix",
            include_str!("../../../templates/docs/compute/matrix.md.j2"),
        ),
        (
            "lc",
            include_str!("../../../templates/docs/compute/lc.md.j2"),
        ),
        (
            "provenance",
            include_str!("../../../templates/docs/compute/provenance.md.j2"),
        ),
    ] {
        environment.add_template(name, source)?;
    }
    Ok(environment)
}

/// Strict compute templates retain separate run identities, intervals and escaped provenance.
/// ´claim:docgen:compute-templates-preserve-separate-observations´
/// ´test:integration:compute-templates-preserve-separate-observations´
#[test]
fn compute_templates_preserve_separate_observations() -> Result<(), Box<dyn std::error::Error>> {
    let root = root("render")?;
    let context = render_context(&root.0)?;
    let environment = render_environment()?;
    for name in [
        "index",
        "arithmetic",
        "memory",
        "matrix",
        "lc",
        "provenance",
    ] {
        let output = environment.get_template(name)?.render(&context)?;
        assert_eq!(output, environment.get_template(name)?.render(&context)?);
        assert!(output.contains("never") || output.contains("cannot"));
        if matches!(name, "index" | "arithmetic" | "provenance") {
            assert!(output.contains("fixture-two"));
            assert!(output.contains("fixture"));
        }
        if name == "arithmetic" {
            assert_eq!(
                output
                    .matches("|Slope|0.125000000|0.125000000|0.125976563|")
                    .count(),
                2
            );
            assert!(output.contains("Conditional lower"));
            assert!(output.contains("Incomplete"));
            assert!(output.contains("Unsupported"));
            assert!(output.contains("No complete estimate is available."));
        }
        if name == "provenance" {
            assert!(output.contains("compiler&#124;field next line"));
            assert!(!output.contains("compiler|field"));
            assert!(output.contains("Changed build field"));
            assert!(output.contains("Changed environment field"));
            assert!(output.contains("changed-profile"));
        }
    }
    Ok(())
}

/// Source attachments bind exact repository patch bytes and refuse changed content.
/// ´claim:docgen:compute-source-attachments-bind-reviewed-repository-bytes´
/// ´test:integration:compute-source-attachments-bind-reviewed-repository-bytes´
#[test]
fn compute_source_attachments_bind_reviewed_repository_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    let root = root("source-attachment")?;
    let mut index = admitted(&root.0)?;
    let source = "docs/attachments/compute/fixture.patch";
    fs::create_dir_all(root.0.join("docs/attachments/compute"))?;
    fs::write(root.0.join(source), "synthetic reviewed source patch\n")?;
    let source_blob = git(&root.0, &["hash-object", "--no-filters", source])?;
    let path = "data/inputs/compute/runs/fixture.json";
    let mut summary: serde_json::Value = serde_json::from_slice(&fs::read(root.0.join(path))?)?;
    summary["payload"]["identity"]["source_attachment"] = serde_json::json!({"state":"Known", "value":{"path":source,"blob":{"algorithm":"sha1","digest":source_blob}}});
    fs::write(root.0.join(path), serde_json::to_vec(&summary)?)?;
    index["payload"]["runs"][0]["receipt"]["summary_blob"] =
        git(&root.0, &["hash-object", "--no-filters", path])?.into();
    fs::write(
        root.0.join("data/inputs/compute/index.json"),
        serde_json::to_vec(&index)?,
    )?;
    nrz_docgen::compute::validate_inputs(&root.0)?;
    fs::write(root.0.join(source), "changed source bytes\n")?;
    assert!(nrz_docgen::compute::validate_inputs(&root.0).is_err());
    fs::remove_file(root.0.join(source))?;
    assert!(nrz_docgen::compute::validate_inputs(&root.0).is_err());
    Ok(())
}

/// An index edit cannot adopt the repository's still-open policy.
/// ´claim:docgen:compute-open-policy-refuses-data-activation´
/// ´test:integration:compute-open-policy-refuses-data-activation´
#[test]
fn compute_open_policy_refuses_data_activation() -> Result<(), Box<dyn std::error::Error>> {
    let root = root("open-policy")?;
    let index = admitted(&root.0)?;
    fs::write(
        root.0.join("data/inputs/compute/index.json"),
        serde_json::to_vec(&index)?,
    )?;
    fs::write(
        root.0
            .join("templates/docs/design/compute-benchmarks.md.j2"),
        include_bytes!("../../../templates/docs/design/compute-benchmarks.md.j2"),
    )?;
    let error = nrz_docgen::compute::validate_inputs(&root.0)
        .err()
        .ok_or("open policy accepted")?;
    assert!(error.contains("policy is undecided"));
    fs::remove_dir_all(root.0.join("data/inputs/compute/runs"))?;
    let mut empty = index;
    empty["payload"]["runs"] = serde_json::json!([]);
    fs::write(
        root.0.join("data/inputs/compute/index.json"),
        serde_json::to_vec(&empty)?,
    )?;
    let error = nrz_docgen::compute::validate_inputs(&root.0)
        .err()
        .ok_or("data-only policy accepted")?;
    assert!(error.contains("policy is undecided"));
    Ok(())
}
