// SPDX-License-Identifier: AGPL-3.0-only

//! Read-only fixed-point and managed-inventory comparison.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use crate::error::{self, Error};

/// The four ordered drift classes emitted by a fixed-point check.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Class {
    /// A generated file is absent on disk.
    Missing,
    /// A generated file differs from its committed bytes.
    Changed,
    /// A managed file on disk has no declared output or template.
    Stale,
    /// A second generation differs from the first.
    Unstable,
}

impl Class {
    /// Return the stable output word for a class.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Changed => "changed",
            Self::Stale => "stale",
            Self::Unstable => "unstable",
        }
    }
}

/// One sorted path-specific drift finding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Finding {
    /// Drift class.
    pub class: Class,
    /// Root-relative managed path.
    pub path: String,
}

/// Compare both generations with each other and with the managed files on disk.
pub(crate) fn compare(
    root: &Path,
    first: &BTreeMap<String, Vec<u8>>,
    second: &BTreeMap<String, Vec<u8>>,
    templates: &BTreeSet<String>,
) -> Result<Vec<Finding>, Error> {
    let mut findings = BTreeSet::new();
    for (path, expected) in first {
        match fs::symlink_metadata(root.join(path)) {
            Ok(metadata) if metadata.file_type().is_file() => {
                let actual = fs::read(root.join(path)).map_err(|source| error::io(path, source))?;
                if actual != *expected {
                    findings.extend([(path.clone(), Class::Changed)]);
                }
            }
            Ok(_) => {
                findings.extend([(path.clone(), Class::Missing)]);
            }
            Err(problem) if problem.kind() == ErrorKind::NotFound => {
                findings.extend([(path.clone(), Class::Missing)]);
            }
            Err(source) => return Err(error::io(path, source)),
        }
    }
    for path in first.keys().chain(second.keys()) {
        if first.get(path) != second.get(path) {
            findings.extend([(path.clone(), Class::Unstable)]);
        }
    }
    for path in inventory(root)? {
        if !first.contains_key(&path) && !templates.contains(&path) {
            findings.extend([(path, Class::Stale)]);
        }
    }
    Ok(findings
        .into_iter()
        .map(|(path, class)| Finding { class, path })
        .collect())
}

/// Enumerate managed datasets, renders and templates without following links.
fn inventory(root: &Path) -> Result<BTreeSet<String>, Error> {
    let mut found = BTreeSet::new();
    walk(root, root, &mut found)?;
    Ok(found)
}

/// Walk one real directory in deterministic path order.
fn walk(root: &Path, directory: &Path, found: &mut BTreeSet<String>) -> Result<(), Error> {
    let relative = directory.strip_prefix(root).map_or_else(
        |_| ".".to_owned(),
        |path| path.to_string_lossy().into_owned(),
    );
    for entry in fs::read_dir(directory).map_err(|source| error::io(&relative, source))? {
        let entry = entry.map_err(|source| error::io(&relative, source))?;
        let path = entry.path();
        let name = entry.file_name();
        let metadata = fs::symlink_metadata(&path).map_err(|source| {
            let display = path.strip_prefix(root).map_or_else(
                |_| relative.clone(),
                |part| part.to_string_lossy().into_owned(),
            );
            error::io(&display, source)
        })?;
        if metadata.file_type().is_dir() {
            if name != "target" && name != ".git" && name != ".lane" {
                walk(root, &path, found)?;
            }
            continue;
        }
        if !metadata.file_type().is_file() && !metadata.file_type().is_symlink() {
            continue;
        }
        let relative = path.strip_prefix(root).map_or_else(
            |_| String::new(),
            |part| part.to_string_lossy().into_owned(),
        );
        let file = Path::new(&relative);
        let markdown = file.extension() == Some(OsStr::new("md"))
            || file.file_name() == Some(OsStr::new(".md"));
        let json = file.extension() == Some(OsStr::new("json"))
            || file.file_name() == Some(OsStr::new(".json"));
        let template = file.extension() == Some(OsStr::new("j2"))
            || file.file_name() == Some(OsStr::new(".j2"));
        if markdown || template || (relative.starts_with("data/datasets/") && json) {
            found.extend([relative]);
        }
    }
    Ok(())
}
