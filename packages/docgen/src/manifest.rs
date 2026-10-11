// SPDX-License-Identifier: AGPL-3.0-only

//! The manifest of adjacent templates and renders.

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
const PATH: &str = "docs/documents.toml";
/// Manifest namespace.
const NAMESPACE: &str = "org.naturrhythmuszentrum.docs.manifest";

/// One document output and its complete input declaration.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Document {
    /// Adjacent template path.
    pub(crate) template: String,
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
        if document.template != format!("{}.j2", document.output) {
            return Err(invalid(
                &document.template,
                "template must equal output plus .j2",
            ));
        }
        if !outputs.insert(document.output.clone()) {
            return Err(invalid(&document.output, "duplicate output"));
        }
        check_symlinks(root, &document.output)?;
        check_symlinks(root, &document.template)?;
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
fn check_symlinks(root: &Path, path: &str) -> Result<(), Error> {
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
        template: &str,
        output: &str,
        duplicate: bool,
    ) -> Result<(), std::io::Error> {
        fs::create_dir_all(root.join("docs"))?;
        let mut text = format!(
            "namespace = \"org.naturrhythmuszentrum.docs.manifest\"\nversion = [1, 0, 0]\nowner = \"NRZ\"\nlicense = \"CC0-1.0\"\nsources = []\nlabels = []\n[[document]]\ntemplate = \"{template}\"\noutput = \"{output}\"\nrequires = []\n"
        );
        if duplicate {
            write!(
                text,
                "[[document]]\ntemplate = \"{template}\"\noutput = \"{output}\"\nrequires = []\n"
            )
            .map_err(std::io::Error::other)?;
        }
        fs::write(root.join("docs/documents.toml"), text)
    }

    /// Refuse nonadjacent templates, escaping paths, and repeated outputs.
    /// ´test:unit:refuses-invalid-document-rows´
    #[test]
    fn refuses_invalid_document_rows() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        let owners = BTreeSet::from(["NRZ".to_owned()]);
        for (template, output, duplicate) in [
            ("docs/other.md.j2", "docs/example.md", false),
            ("../escape.md.j2", "../escape.md", false),
            ("docs/example.md.j2", "docs/example.md", true),
        ] {
            write_manifest(root, template, output, duplicate)?;
            assert!(matches!(read(root, &owners), Err(Error::Manifest { .. })));
        }
        Ok(())
    }
}
