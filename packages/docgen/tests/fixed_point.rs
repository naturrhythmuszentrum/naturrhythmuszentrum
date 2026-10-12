// SPDX-License-Identifier: AGPL-3.0-only

//! Binary-level checks of the documentation fixed point and exit classes.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Build a minimal complete documentation tree.
fn fixture(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(root.join(".linter"))?;
    fs::create_dir_all(root.join("docs/inputs"))?;
    fs::write(root.join(".linter/owners.toml"), "owners = [\"NRZ\"]\n")?;
    fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n")?;
    fs::write(
        root.join("docs/inputs/project.toml"),
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
        root.join("docs/documents.toml"),
        concat!(
            "# SPDX-License-Identifier: CC0-1.0\n",
            "namespace = \"org.naturrhythmuszentrum.docs.manifest\"\n",
            "version = [1, 0, 0]\n",
            "owner = \"NRZ\"\n",
            "license = \"CC0-1.0\"\n",
            "sources = []\n",
            "labels = []\n",
            "[[document]]\n",
            "template = \"docs/example.md.j2\"\n",
            "output = \"docs/example.md\"\n",
            "requires = [\"census\", \"project\"]\n",
        ),
    )?;
    fs::write(
        root.join("docs/example.md.j2"),
        concat!(
            "{# SPDX-License-Identifier: CC0-1.0 -#}\n",
            "{# requires census v1 -#}\n",
            "{# requires project v1 -#}\n",
            "<!-- SPDX-License-Identifier: CC0-1.0 -->\n\n",
            "# {{ project.name }}\n",
        ),
    )?;
    write_problem(root, "alpha", "problem")?;
    Ok(())
}

/// Write one minimal valid problem-stage input under its declared kind.
///
/// TODO ´todo:test:seed-explicit-quantity-and-metadata-roles´: seed explicit quantity and metadata roles with checkable observable expressions for fixed-point fixtures under req:vision:unit-data-migration.
fn write_problem(root: &Path, slug: &str, kind: &str) -> Result<(), std::io::Error> {
    let (directory, filename) = if kind == "fixture" {
        (format!("problems/fixtures/{slug}"), "fixture.toml")
    } else {
        (format!("problems/{slug}"), "problem.toml")
    };
    fs::create_dir_all(root.join(&directory))?;
    let source = format!(
        "# SPDX-License-Identifier: CC0-1.0\nnamespace = \"org.naturrhythmuszentrum.problems.{kind}\"\nversion = [1, 0, 0]\nowner = \"NRZ\"\nlicense = \"CC0-1.0\"\nsources = []\nlabels = [\"prob:exemplars:{slug}\"]\n[problem]\nslug = \"{slug}\"\ntitle = \"Sample\"\nregister = \"prob:exemplars:{slug}\"\nkind = \"{kind}\"\nmodel = \"Model\"\ninstance = \"Instance\"\n[[parameters]]\nname = \"p\"\ndescription = \"Input\"\nvalue = \"1\"\nunit = {{ display = \"1\" }}\nuncertainty = {{ kind = \"none\" }}\n[[observables]]\nname = \"answer\"\ndescription = \"Output\"\nunit = {{ display = \"1\" }}\nclosed_form = \"p\"\nvalue = \"1\"\nvalue_kind = \"exact\"\nderivation = \"Direct\"\nsecond_route = \"none\"\n[errors]\nbounded = []\nestimated = []\nexcluded = []\n[stress]\nclaims = []\nbites = [\"step:nrz:bite-exemplar-register\"]\n"
    );
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
/// TODO ´todo:test:verify-migrated-unit-and-uncertainty-data´: verify migrated unit and uncertainty data reach a fixed point while invalid expressions fail validation under req:vision:unit-data-migration.
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
    let original = fs::read(root.join("docs/example.md"))?;
    assert_eq!(run(root, "render")?.status.code(), Some(0));
    assert_eq!(fs::read(root.join("docs/example.md"))?, original);
    let before = snapshot(root)?;
    assert_eq!(run(root, "check")?.status.code(), Some(0));
    assert_eq!(snapshot(root)?, before);

    fs::write(root.join("docs/example.md"), "changed\n")?;
    let changed = run(root, "check")?;
    assert_eq!(changed.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&changed.stdout).contains("changed docs/example.md"));
    assert_eq!(run(root, "render")?.status.code(), Some(0));

    fs::remove_file(root.join("docs/example.md"))?;
    let missing = run(root, "check")?;
    assert_eq!(missing.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&missing.stdout).contains("missing docs/example.md"));
    assert_eq!(run(root, "render")?.status.code(), Some(0));

    fs::write(root.join("docs/extra.md"), "extra\n")?;
    let stale = run(root, "check")?;
    assert_eq!(stale.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&stale.stdout).contains("stale docs/extra.md"));
    fs::remove_file(root.join("docs/extra.md"))?;

    fs::remove_file(root.join("docs/datasets/project.json"))?;
    assert_eq!(run(root, "render")?.status.code(), Some(1));
    let absent_dataset = run(root, "check")?;
    assert_eq!(absent_dataset.status.code(), Some(3));
    assert!(
        String::from_utf8_lossy(&absent_dataset.stdout)
            .contains("missing docs/datasets/project.json")
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
        serde_json::from_slice(&fs::read(root.join("docs/datasets/problems.json"))?)?;
    assert_eq!(
        value["sources"],
        serde_json::json!([
            "problems/aardvark/problem.toml",
            "problems/alpha/problem.toml",
            "problems/fixtures/beta/fixture.toml"
        ])
    );
    assert_eq!(value["entries"][0]["problem"]["slug"], "aardvark");
    assert_eq!(value["entries"][2]["problem"]["slug"], "beta");
    Ok(())
}

/// A changed input value makes check report the derived dataset as changed.
///
/// TODO ´todo:test:detect-drift-from-changed-measures-kinds´: detect drift from changed measures, kinds and observable expressions as well as numeric values under req:vision:unit-data-migration.
///
/// ´test:integration:detects-changed-problem-value´
#[test]
fn detects_changed_problem_value() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    fixture(root)?;
    assert_eq!(run(root, "data")?.status.code(), Some(0));
    let path = root.join("problems/alpha/problem.toml");
    let source = fs::read_to_string(&path)?;
    fs::write(&path, source.replacen("value = \"1\"", "value = \"2\"", 1))?;
    let checked = run(root, "check")?;
    assert_eq!(checked.status.code(), Some(3));
    assert!(
        String::from_utf8_lossy(&checked.stdout).contains("changed docs/datasets/problems.json")
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
    fs::remove_dir_all(root.join("problems"))?;
    assert_eq!(run(root, "data")?.status.code(), Some(1));
    Ok(())
}
