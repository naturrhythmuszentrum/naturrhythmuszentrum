// SPDX-License-Identifier: AGPL-3.0-only

//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`refuses_invalid_document_rows`] | docgen | Refuse source-directory outputs, escaping paths, and repeated outputs. |
//! | [`refuses_explicit_template_path`] | docgen | Refuse even a correctly mirrored template override in the manifest. |

//! The manifest of renders and their mirrored template sources.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use crate::envelope;
use crate::error::{self, Error};

/// Path of the document manifest.
/// The document manifest path fixes the complete declared output inventory (´rule:docgen:input-layout´).
/// ´const:docgen:manifest-path´ (´[ORCHESTRATION-alg:const:text]´)
/// ´const:docgen:manifest-path-text-xc31b15ce´
const PATH: &str = "data/documents.toml";
/// Manifest namespace.
/// The manifest namespace separates output declarations from input and dataset payloads (´rule:docgen:dataset-identities´).
/// ´const:docgen:manifest-namespace´ (´[ORCHESTRATION-alg:const:text]´)
/// ´const:docgen:manifest-namespace-text-x0b00b544´
const NAMESPACE: &str = "org.naturrhythmuszentrum.docs.manifest";

/// One document output and its complete input declaration.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Document {
    /// Markdown output path.
    pub(crate) output: String,
    /// Dataset names in header and context order.
    pub(crate) requires: Vec<String>,
}

/// Typed manifest payload.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    /// Documents in render order.
    document: Vec<Document>,
}

/// Derive the sole template location for a root-relative output path.
pub(crate) fn template_path(output: &str) -> String {
    format!("templates/{output}.j2")
}

/// Parse and validate the entire manifest before writing any output.
pub(crate) fn read(root: &Path, owners: &BTreeSet<String>) -> Result<Vec<Document>, Error> {
    let value = envelope::read_toml(root, PATH)?;
    envelope::validate(&value, PATH, NAMESPACE, owners)?;
    let object = value
        .as_object()
        .ok_or_else(|| invalid(PATH, "manifest must be an object"))?;
    if value.get("owner").and_then(Value::as_str) != Some("NRZ")
        || !envelope::string_array(object, PATH, "sources")?.is_empty()
        || !envelope::string_array(object, PATH, "labels")?.is_empty()
    {
        return Err(Error::Envelope {
            path: PATH.to_owned(),
            detail: "manifest owner, sources, or labels differ from its declaration".to_owned(),
        });
    }
    let payload = envelope::payload(&value, PATH)?;
    let manifest: Manifest = serde_json::from_value(payload).map_err(|problem| Error::Payload {
        path: PATH.to_owned(),
        detail: problem.to_string(),
    })?;
    let mut outputs = BTreeSet::new();
    for document in &manifest.document {
        let output = Path::new(&document.output);
        let markdown = output.extension() == Some(OsStr::new("md"))
            || output.file_name() == Some(OsStr::new(".md"));
        if !envelope::normal_path(&document.output) || !markdown {
            return Err(invalid(
                &document.output,
                "output must be a root-relative .md path",
            ));
        }
        if document.output.starts_with("templates/")
            || document.output.starts_with("data/")
            || document
                .output
                .split('/')
                .any(|part| matches!(part, ".git" | "target" | ".lane"))
        {
            return Err(invalid(
                &document.output,
                "output must be outside source and ignored directories",
            ));
        }
        if !outputs.insert(document.output.clone()) {
            return Err(invalid(&document.output, "duplicate output"));
        }
        check_symlinks(root, &document.output)?;
        check_symlinks(root, &template_path(&document.output))?;
        let mut requirements = BTreeSet::new();
        for name in &document.requires {
            if name != "project" && name != "census" && name != "problems" {
                return Err(invalid(
                    &document.output,
                    &format!("unknown dataset {name}"),
                ));
            }
            if !requirements.insert(name) {
                return Err(invalid(
                    &document.output,
                    &format!("duplicate dataset {name}"),
                ));
            }
        }
    }
    Ok(manifest.document)
}

/// Reject a symbolic link at any existing path component.
pub(crate) fn check_symlinks(root: &Path, path: &str) -> Result<(), Error> {
    let mut current = String::new();
    for component in path.split('/') {
        if !current.is_empty() {
            current.push('/');
        }
        current.push_str(component);
        match fs::symlink_metadata(root.join(&current)) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(invalid(&current, "symbolic link in document path"));
            }
            Ok(_) => {}
            Err(problem) if problem.kind() == ErrorKind::NotFound => break,
            Err(source) => return Err(error::io(&current, source)),
        }
    }
    Ok(())
}

/// Construct a path-bearing manifest error.
fn invalid(path: &str, detail: &str) -> Error {
    Error::Manifest {
        path: path.to_owned(),
        detail: detail.to_owned(),
    }
}

/// Tests for whole-manifest path validation.
#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fmt::Write;
    use std::fs;

    use super::read;
    use crate::error::Error;

    /// Write a manifest with one configurable document row.
    fn write_manifest(
        root: &std::path::Path,
        output: &str,
        duplicate: bool,
    ) -> Result<(), std::io::Error> {
        fs::create_dir_all(root.join("data"))?;
        let mut text = format!(
            "namespace = \"org.naturrhythmuszentrum.docs.manifest\"\nversion = [1, 0, 0]\nowner = \"NRZ\"\nlicense = \"CC0-1.0\"\nsources = []\nlabels = []\n[[document]]\noutput = \"{output}\"\nrequires = []\n"
        );
        if duplicate {
            write!(text, "[[document]]\noutput = \"{output}\"\nrequires = []\n")
                .map_err(std::io::Error::other)?;
        }
        fs::write(root.join("data/documents.toml"), text)
    }

    /// Refuse source-directory outputs, escaping paths, and repeated outputs.
    /// ´claim:docgen:refuses-invalid-document-rows´
    /// ´test:unit:refuses-invalid-document-rows´
    #[test]
    fn refuses_invalid_document_rows() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        let owners = BTreeSet::from(["NRZ".to_owned()]);
        for (output, duplicate) in [
            ("templates/docs/design/example.md", false),
            ("data/example.md", false),
            ("../escape.md", false),
            ("/absolute.md", false),
            ("docs/design/../escape.md", false),
            ("docs/target/example.md", false),
            (".git/example.md", false),
            (".lane/example.md", false),
            ("docs/design/example.md", true),
        ] {
            write_manifest(root, output, duplicate)?;
            assert!(matches!(read(root, &owners), Err(Error::Manifest { .. })));
        }
        Ok(())
    }

    /// Refuse even a correctly mirrored template override in the manifest.
    /// ´claim:docgen:refuses-explicit-template-path´
    /// ´test:unit:refuses-explicit-template-path´
    #[test]
    fn refuses_explicit_template_path() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        write_manifest(root, "docs/design/example.md", false)?;
        let path = root.join("data/documents.toml");
        let mut source = fs::read_to_string(&path)?;
        source.push_str("template = \"templates/docs/design/example.md.j2\"\n");
        fs::write(path, source)?;
        let owners = BTreeSet::from(["NRZ".to_owned()]);
        assert!(matches!(read(root, &owners), Err(Error::Payload { .. })));
        Ok(())
    }
}
