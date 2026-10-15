// SPDX-License-Identifier: AGPL-3.0-only
#![doc = include_str!("../README.md")]
#![expect(
    clippy::multiple_crate_versions,
    reason = "nrz-cas uses petgraph 0.8.3 with hashbrown 0.15 while indexmap uses hashbrown 0.17"
)]

#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod census;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod check;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod envelope;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod error;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod manifest;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod problems;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod project;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod render;

#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies unreachable_pub"
)]
mod problem_schema;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde_json::Value;

pub use check::{Class, Finding};
pub use error::Error;

/// Dataset file bytes and parsed dataset values produced in one derivation.
type DatasetGeneration = (BTreeMap<String, Vec<u8>>, BTreeMap<String, Value>);

/// Derive and write all documentation datasets.
///
/// # Errors
///
/// Returns a path-bearing failure if an input, manifest, or write is invalid.
pub fn data(root: &Path) -> Result<(), Error> {
    let owners = envelope::owners(root)?;
    let (files, _) = datasets(root, &owners)?;
    write_files(root, &files)
}

/// Render all declared Markdown documents from committed datasets.
///
/// # Errors
///
/// Returns a path-bearing failure if a manifest, dataset, template, or write is invalid.
pub fn render(root: &Path) -> Result<(), Error> {
    let owners = envelope::owners(root)?;
    let documents = manifest::read(root, &owners)?;
    let mut datasets = BTreeMap::new();
    for document in &documents {
        for name in &document.requires {
            if !datasets.contains_key(name) {
                let path = dataset_path(name).ok_or_else(|| Error::DatasetCompatibility {
                    document: document.output.clone(),
                    dataset: name.clone(),
                    detail: "dataset is not supported".to_owned(),
                })?;
                let value = envelope::read_json(root, path)?;
                validate_dataset(name, &value, &owners)?;
                datasets.extend([(name.clone(), value)]);
            }
        }
    }
    let output = render::all(root, &documents, &datasets)?;
    write_files(root, &output)
}

/// Check dataset and render bytes, managed inventory, and two-generation stability without writing.
///
/// # Errors
///
/// Returns a path-bearing failure for invalid sources, manifests, templates, or I/O.
pub fn check(root: &Path) -> Result<Vec<Finding>, Error> {
    let owners = envelope::owners(root)?;
    let documents = manifest::read(root, &owners)?;
    let first = generation(root, &owners, &documents)?;
    // Generated paths are disjoint from every input path, member manifest, and template.
    // Laying this generation over the tree therefore leaves the second read set unchanged.
    let second = generation(root, &owners, &documents)?;
    let templates = documents
        .iter()
        .map(|document| manifest::template_path(&document.output))
        .collect();
    check::compare(root, &first, &second, &templates)
}

/// Derive dataset bytes and parsed values together.
fn datasets(root: &Path, owners: &BTreeSet<String>) -> Result<DatasetGeneration, Error> {
    let (project_bytes, project_value) = project::derive(root, owners)?;
    let (census_bytes, census_value) = census::derive(root)?;
    let (problems_bytes, problems_value) = problems::derive(root, owners)?;
    let files = BTreeMap::from([
        (project::DATASET.to_owned(), project_bytes),
        (census::DATASET.to_owned(), census_bytes),
        (problems::DATASET.to_owned(), problems_bytes),
    ]);
    let values = BTreeMap::from([
        ("project".to_owned(), project_value),
        ("census".to_owned(), census_value),
        ("problems".to_owned(), problems_value),
    ]);
    Ok((files, values))
}

/// Build every managed byte sequence entirely in memory.
fn generation(
    root: &Path,
    owners: &BTreeSet<String>,
    documents: &[manifest::Document],
) -> Result<BTreeMap<String, Vec<u8>>, Error> {
    let (mut files, datasets) = datasets(root, owners)?;
    files.extend(render::all(root, documents, &datasets)?);
    Ok(files)
}

/// Map a supported dataset name to its committed path.
fn dataset_path(name: &str) -> Option<&'static str> {
    match name {
        "project" => Some(project::DATASET),
        "census" => Some(census::DATASET),
        "problems" => Some(problems::DATASET),
        _ => None,
    }
}

/// Validate one committed dataset before it enters a template context.
fn validate_dataset(name: &str, value: &Value, owners: &BTreeSet<String>) -> Result<(), Error> {
    match name {
        "project" => project::validate_dataset(value, owners),
        "census" => census::validate_dataset(value, owners),
        "problems" => problems::validate_dataset(value, owners),
        _ => Err(Error::DatasetCompatibility {
            document: String::new(),
            dataset: name.to_owned(),
            detail: "dataset is not supported".to_owned(),
        }),
    }
}

/// Write a previously completed and validated output set.
fn write_files(root: &Path, files: &BTreeMap<String, Vec<u8>>) -> Result<(), Error> {
    for (path, bytes) in files {
        let target = root.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|source| error::io(path, source))?;
        }
        fs::write(target, bytes).map_err(|source| error::io(path, source))?;
    }
    Ok(())
}
