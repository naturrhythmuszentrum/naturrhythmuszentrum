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
    Ok(())
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
