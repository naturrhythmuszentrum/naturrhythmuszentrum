// SPDX-License-Identifier: AGPL-3.0-only

//! Binary-level checks of the documentation fixed point and exit classes.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Build a minimal complete documentation tree.
fn fixture(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(root.join(".linter"))?;
    fs::create_dir_all(root.join("data/inputs"))?;
    fs::create_dir_all(root.join("templates/docs/design"))?;
    fs::write(root.join(".linter/owners.toml"), "owners = [\"NRZ\"]\n")?;
    fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n")?;
    fs::write(
        root.join("data/inputs/project.toml"),
        concat!(
            "# SPDX-License-Identifier: CC0-1.0\n",
            "namespace = \"org.naturrhythmuszentrum.docs.input.project\"\n",
            "version = [1, 0, 0]\n",
            "owner = \"NRZ\"\n",
            "license = \"CC0-1.0\"\n",
            "sources = []\n",
            "labels = [\"guide:docgen:overview\"]\n",
            "name = \"Fixture\"\n",
            "description = \"A fixture project.\"\n",
            "repository = \"https://example.invalid/fixture\"\n",
            "bites = [{ id = \"1\", title = \"Documentation generator\" }]\n",
        ),
    )?;
    fs::write(
        root.join("data/documents.toml"),
        concat!(
            "# SPDX-License-Identifier: CC0-1.0\n",
            "namespace = \"org.naturrhythmuszentrum.docs.manifest\"\n",
            "version = [1, 0, 0]\n",
            "owner = \"NRZ\"\n",
            "license = \"CC0-1.0\"\n",
            "sources = []\n",
            "labels = []\n",
            "[[document]]\n",
            "output = \"docs/design/example.md\"\n",
            "requires = [\"census\", \"project\", \"problems\"]\n",
        ),
    )?;
    fs::write(
        root.join("templates/docs/design/example.md.j2"),
        concat!(
            "{# SPDX-License-Identifier: CC0-1.0 -#}\n",
            "{# requires census v1 -#}\n",
            "{# requires project v1 -#}\n",
            "{# requires problems v2 -#}\n",
            "<!-- SPDX-License-Identifier: CC0-1.0 -->\n\n",
            "# {{ project.name }}\n",
        ),
    )?;
    write_problem(root, "alpha", "problem")?;
    Ok(())
}

/// Write one minimal valid problem-stage input under its declared kind.
fn write_problem(root: &Path, slug: &str, kind: &str) -> Result<(), std::io::Error> {
    let (directory, filename) = if kind == "fixture" {
        (format!("data/problems/fixtures/{slug}"), "fixture.toml")
    } else {
        (format!("data/problems/{slug}"), "problem.toml")
    };
    fs::create_dir_all(root.join(&directory))?;
    #[expect(
        clippy::literal_string_with_formatting_args,
        reason = "the fixture template uses brace placeholders filled by str::replace, not a format string"
    )]
    let source = include_str!("support/problem.toml")
        .replace("{kind}", kind)
        .replace("{slug}", slug);
    fs::write(root.join(directory).join(filename), source)
}

/// Run the built binary against one fixture root.
fn run(root: &Path, stage: &str) -> Result<Output, std::io::Error> {
    Command::new(env!("CARGO_BIN_EXE_nrz-docgen"))
        .arg(stage)
        .arg("--root")
        .arg(root)
        .output()
}

/// Snapshot all regular files to prove that check never writes.
fn snapshot(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>, std::io::Error> {
    let mut files = BTreeMap::new();
    collect(root, root, &mut files)?;
    Ok(files)
}

/// Collect regular files recursively for a byte and inventory comparison.
fn collect(
    root: &Path,
    directory: &Path,
    files: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, files)?;
        } else if path.is_file() {
            let relative = path.strip_prefix(root).map_err(std::io::Error::other)?;
            files.extend([(relative.to_owned(), fs::read(&path)?)]);
        }
    }
    Ok(())
}

/// Rendering is idempotent, drift classes are distinct, and check is read-only.
///
/// TODO ´todo:test:verify-migrated-binding-tables-preserve-point´: verify migrated binding tables preserve point order and shared source references under conv:nrz:batch-order-and-replay.
///
/// ´test:integration:binary-fixed-point-and-exit-classes´
#[test]
fn binary_fixed_point_and_exit_classes() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    fixture(root)?;
    assert_eq!(run(root, "data")?.status.code(), Some(0));
    assert_eq!(run(root, "render")?.status.code(), Some(0));
    let original = fs::read(root.join("docs/design/example.md"))?;
    assert_eq!(run(root, "render")?.status.code(), Some(0));
    assert_eq!(fs::read(root.join("docs/design/example.md"))?, original);
    let before = snapshot(root)?;
    assert_eq!(run(root, "check")?.status.code(), Some(0));
    assert_eq!(snapshot(root)?, before);

    fs::write(root.join("docs/design/example.md"), "changed\n")?;
    let changed = run(root, "check")?;
    assert_eq!(changed.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&changed.stdout).contains("changed docs/design/example.md"));
    assert_eq!(run(root, "render")?.status.code(), Some(0));

    fs::remove_file(root.join("docs/design/example.md"))?;
    let missing = run(root, "check")?;
    assert_eq!(missing.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&missing.stdout).contains("missing docs/design/example.md"));
    assert_eq!(run(root, "render")?.status.code(), Some(0));

    fs::write(root.join("docs/design/extra.md"), "extra\n")?;
    let stale = run(root, "check")?;
    assert_eq!(stale.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&stale.stdout).contains("stale docs/design/extra.md"));
    fs::remove_file(root.join("docs/design/extra.md"))?;

    fs::remove_file(root.join("data/datasets/project.json"))?;
    assert_eq!(run(root, "render")?.status.code(), Some(1));
    let absent_dataset = run(root, "check")?;
    assert_eq!(absent_dataset.status.code(), Some(3));
    assert!(
        String::from_utf8_lossy(&absent_dataset.stdout)
            .contains("missing data/datasets/project.json")
    );
    assert_eq!(run(root, "data")?.status.code(), Some(0));
    assert_eq!(run(root, "unknown")?.status.code(), Some(2));
    Ok(())
}

/// Data writes problems before fixtures, with bytewise slug order in each group.
/// ´test:integration:orders-problem-dataset-entries´
#[test]
fn orders_problem_dataset_entries() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    fixture(root)?;
    write_problem(root, "aardvark", "problem")?;
    write_problem(root, "beta", "fixture")?;
    assert_eq!(run(root, "data")?.status.code(), Some(0));
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("data/datasets/problems.json"))?)?;
    assert_eq!(
        value["sources"],
        serde_json::json!([
            "data/problems/aardvark/problem.toml",
            "data/problems/alpha/problem.toml",
            "data/problems/fixtures/beta/fixture.toml",
        ])
    );
    assert_eq!(value["entries"][0]["problem"]["slug"], "aardvark");
    assert_eq!(value["entries"][2]["problem"]["slug"], "beta");
    Ok(())
}

/// A changed input value makes check report the derived dataset as changed.
///
/// ´test:integration:detects-changed-problem-value´
#[test]
fn detects_changed_problem_value() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    fixture(root)?;
    assert_eq!(run(root, "data")?.status.code(), Some(0));
    let path = root.join("data/problems/alpha/problem.toml");
    let source = fs::read_to_string(&path)?;
    fs::write(&path, source.replacen("value = \"1\"", "value = \"2\"", 1))?;
    let checked = run(root, "check")?;
    assert_eq!(checked.status.code(), Some(3));
    assert!(
        String::from_utf8_lossy(&checked.stdout).contains("changed data/datasets/problems.json")
    );
    Ok(())
}

/// Data rejects a root from which the entire problems tree was removed.
/// ´test:integration:refuses-missing-problems-tree´
#[test]
fn refuses_missing_problems_tree() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    fixture(root)?;
    fs::remove_dir_all(root.join("data/problems"))?;
    assert_eq!(run(root, "data")?.status.code(), Some(1));
    Ok(())
}

/// A second complete generation leaves every byte and the inventory unchanged.
/// ´test:integration:repeats-complete-generation´
#[test]
fn repeats_complete_generation() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    fixture(root)?;
    assert_eq!(run(root, "data")?.status.code(), Some(0));
    assert_eq!(run(root, "render")?.status.code(), Some(0));
    let before = snapshot(root)?;
    assert_eq!(run(root, "data")?.status.code(), Some(0));
    assert_eq!(run(root, "render")?.status.code(), Some(0));
    assert_eq!(run(root, "check")?.status.code(), Some(0));
    assert_eq!(snapshot(root)?, before);
    Ok(())
}

/// Changed templates produce render drift; absent templates fail without writes.
/// ´test:integration:checks-mirrored-template-sources´
#[test]
fn checks_mirrored_template_sources() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    fixture(root)?;
    assert_eq!(run(root, "data")?.status.code(), Some(0));
    assert_eq!(run(root, "render")?.status.code(), Some(0));
    let path = root.join("templates/docs/design/example.md.j2");
    let source = fs::read_to_string(&path)?;
    fs::write(
        &path,
        source.replace("# {{ project.name }}", "# Updated {{ project.name }}"),
    )?;
    let before = snapshot(root)?;
    let checked = run(root, "check")?;
    assert_eq!(checked.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&checked.stdout).contains("changed docs/design/example.md"));
    assert_eq!(snapshot(root)?, before);
    assert_eq!(run(root, "render")?.status.code(), Some(0));
    assert_eq!(run(root, "check")?.status.code(), Some(0));
    fs::remove_file(path)?;
    let before = snapshot(root)?;
    for stage in ["render", "check"] {
        let absent = run(root, stage)?;
        assert_eq!(absent.status.code(), Some(1));
        assert!(
            String::from_utf8_lossy(&absent.stderr).contains("templates/docs/design/example.md.j2")
        );
        assert_eq!(snapshot(root)?, before);
    }
    Ok(())
}

/// Find undeclared sources, renders and datasets throughout the live tree.
/// ´test:integration:checks-complete-managed-inventory´
#[test]
fn checks_complete_managed_inventory() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    fixture(root)?;
    assert_eq!(run(root, "data")?.status.code(), Some(0));
    assert_eq!(run(root, "render")?.status.code(), Some(0));
    let stale = [
        "templates/unused.md.j2",
        "loose.md.j2",
        "crates/unused/README.md",
        "tools/unused/README.md",
        "prototypes/unused/README.md",
        "data/datasets/nested/unused.json",
    ];
    for path in stale {
        let path = root.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, "unused\n")?;
    }
    for ignored in [".git", "target", ".lane", "nested/target"] {
        fs::create_dir_all(root.join(ignored))?;
        for name in ["ignored.md", "ignored.md.j2"] {
            fs::write(root.join(ignored).join(name), "ignored\n")?;
        }
    }
    let before = snapshot(root)?;
    let checked = run(root, "check")?;
    assert_eq!(checked.status.code(), Some(3));
    let output = String::from_utf8(checked.stdout)?;
    assert_eq!(output.lines().count(), stale.len());
    assert_eq!(snapshot(root)?, before);
    for path in stale {
        let expected = format!("stale {path}");
        assert!(output.lines().any(|line| line == expected));
        fs::remove_file(root.join(path))?;
    }
    assert_eq!(run(root, "check")?.status.code(), Some(0));
    assert_eq!(before.len(), snapshot(root)?.len() + stale.len());
    Ok(())
}

/// Problem data accepts neither catalogue renders nor catalogue templates.
/// ´test:integration:refuses-catalogue-in-problem-data´
#[test]
fn refuses_catalogue_in_problem_data() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    fixture(root)?;
    for name in ["README.md", "README.md.j2"] {
        let path = root.join("data/problems").join(name);
        fs::write(&path, "catalogue\n")?;
        assert_eq!(run(root, "data")?.status.code(), Some(1));
        assert_eq!(run(root, "check")?.status.code(), Some(1));
        fs::remove_file(path)?;
    }
    assert_eq!(run(root, "data")?.status.code(), Some(0));
    Ok(())
}

/// Reject linked sources and outputs without following directory aliases.
/// ´test:integration:refuses-linked-document-paths´
#[cfg(unix)]
#[test]
fn refuses_linked_document_paths() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir()?;
    let root = directory.path();
    fixture(root)?;
    assert_eq!(run(root, "data")?.status.code(), Some(0));
    assert_eq!(run(root, "render")?.status.code(), Some(0));
    symlink(root, root.join("cycle"))?;
    assert_eq!(run(root, "check")?.status.code(), Some(0));
    symlink("absent", root.join("orphan.md.j2"))?;
    let orphan = run(root, "check")?;
    assert_eq!(orphan.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&orphan.stdout).contains("stale orphan.md.j2"));
    fs::remove_file(root.join("orphan.md.j2"))?;
    for path in [
        "docs/design/example.md",
        "templates/docs/design/example.md.j2",
    ] {
        let path = root.join(path);
        let bytes = fs::read(&path)?;
        fs::remove_file(&path)?;
        symlink("absent", &path)?;
        assert_eq!(run(root, "check")?.status.code(), Some(1));
        assert_eq!(run(root, "render")?.status.code(), Some(1));
        fs::remove_file(&path)?;
        fs::write(path, bytes)?;
    }
    fs::rename(root.join("templates"), root.join("saved-templates"))?;
    symlink("saved-templates", root.join("templates"))?;
    assert_eq!(run(root, "check")?.status.code(), Some(1));
    assert_eq!(run(root, "render")?.status.code(), Some(1));
    Ok(())
}

/// Changes to measures, semantic identities and original formulas invalidate generated bytes.
/// ´test:integration:detects-declaration-and-expression-drift´
#[test]
fn detects_declaration_and_expression_drift() -> Result<(), Box<dyn std::error::Error>> {
    for (before, after) in [
        ("scale = \"1\"", "scale = \"2\""),
        ("type = \"ratio\"", "type = \"named\""),
        ("text = \"p\"", "text = \"p+p\""),
    ] {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        fixture(root)?;
        assert_eq!(run(root, "data")?.status.code(), Some(0));
        assert_eq!(run(root, "render")?.status.code(), Some(0));
        let path = root.join("data/problems/alpha/problem.toml");
        fs::write(&path, fs::read_to_string(&path)?.replace(before, after))?;
        let unchanged = snapshot(root)?;
        let result = run(root, "check")?;
        assert_eq!(result.status.code(), Some(3));
        assert_eq!(snapshot(root)?, unchanged);
    }
    Ok(())
}

/// Reject legacy envelopes, invalid original formulas and forged committed evidence.
/// ´test:integration:refuses-legacy-formulas-and-forged-evidence´
#[test]
fn refuses_legacy_formulas_and_forged_evidence() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    fixture(root)?;
    assert_eq!(run(root, "data")?.status.code(), Some(0));
    assert_eq!(run(root, "render")?.status.code(), Some(0));
    let path = root.join("data/problems/alpha/problem.toml");
    let original = fs::read_to_string(&path)?;
    fs::write(
        &path,
        original.replace("version = [2, 0, 0]", "version = [1, 0, 0]"),
    )?;
    let legacy = run(root, "data")?;
    assert_eq!(legacy.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&legacy.stderr).contains("unit-data-migration"));
    fs::write(
        &path,
        original.replace("text = \"p\"", "text = \"selector*p\""),
    )?;
    assert_eq!(run(root, "data")?.status.code(), Some(1));
    fs::write(&path, original)?;
    let dataset = root.join("data/datasets/problems.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&dataset)?)?;
    value["entries"][0]["validation"]["checked_observables"] = serde_json::json!(9000);
    fs::write(&dataset, serde_json::to_vec_pretty(&value)?)?;
    let before = snapshot(root)?;
    assert_eq!(run(root, "render")?.status.code(), Some(1));
    assert_eq!(snapshot(root)?, before);
    Ok(())
}
