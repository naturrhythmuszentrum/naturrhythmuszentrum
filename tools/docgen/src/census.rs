// SPDX-License-Identifier: AGPL-3.0-only

//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`census_across_workspace_roots`] | docgen | Use explicit members across all three roots and only mirrored README sources. |
//! | [`derives_member_order_and_optional_facts`] | docgen | Keep member order, inherit one description, and represent absent values distinctly. |
//! | [`refuses_invalid_member_paths`] | docgen | Reject globs, parent traversal, and duplicates before reading member manifests. |
//! | [`census_dependency_identities_and_refusals`] | docgen | Manifest edges retain aliases, workspace inheritance and dependency kinds while refusing conditional declarations. |

//! Workspace census derived directly from Cargo manifests.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::envelope;
use crate::error::{self, Error};
use crate::manifest;

/// Path of the generated census.
/// The census path fixes the generated manifest inventory consumed by templates (´rule:docgen:dataset-identities´).
/// ´const:docgen:census-dataset´ (´[ORCHESTRATION-alg:const:text]´)
/// ´const:docgen:census-dataset-text-xed59db91´
pub(crate) const DATASET: &str = "data/datasets/census.json";
/// Namespace of the generated census.
/// The census namespace identifies workspace membership facts independently of their storage path (´rule:docgen:dataset-identities´).
/// ´const:docgen:census-namespace´ (´[ORCHESTRATION-alg:const:text]´)
/// ´const:docgen:census-namespace-text-xcb052780´
const NAMESPACE: &str = "org.naturrhythmuszentrum.docs.dataset.census";
/// Path of the root Cargo manifest.
/// The root Cargo manifest declares the workspace members inspected by the census (´rule:docgen:input-layout´).
/// ´const:docgen:census-root-manifest´ (´[ORCHESTRATION-alg:const:text]´)
/// ´const:docgen:census-root-manifest-text-xe2a94b47´
const ROOT_MANIFEST: &str = "Cargo.toml";

/// One explicit workspace member.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Member {
    /// Cargo package name.
    name: String,
    /// Path exactly as declared in the workspace.
    path: String,
    /// Description or absence of one.
    description: Option<String>,
    /// Whether a regular mirrored README template exists.
    has_readme_template: bool,
    /// Direct unconditional normal dependency package names.
    normal_dependencies: Vec<String>,
    /// Direct development dependency package names.
    development_dependencies: Vec<String>,
    /// Direct build dependency package names.
    build_dependencies: Vec<String>,
}

/// Typed census payload.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Census {
    /// Members in workspace declaration order.
    members: Vec<Member>,
}

/// Derive member facts without running Cargo.
pub(crate) fn derive(root: &Path) -> Result<(Vec<u8>, Value), Error> {
    let manifest = envelope::read_toml(root, ROOT_MANIFEST)?;
    let workspace = manifest
        .get("workspace")
        .and_then(Value::as_object)
        .ok_or_else(|| membership(ROOT_MANIFEST, "missing workspace table"))?;
    if workspace.contains_key("exclude") {
        return Err(membership(
            ROOT_MANIFEST,
            "workspace exclude is not supported",
        ));
    }
    let paths = workspace
        .get("members")
        .and_then(Value::as_array)
        .ok_or_else(|| membership(ROOT_MANIFEST, "workspace members must be an array"))?;
    let mut seen = BTreeSet::new();
    let mut members = Vec::with_capacity(paths.len());
    let mut sources = vec![ROOT_MANIFEST.to_owned()];
    for entry in paths {
        let path = entry
            .as_str()
            .ok_or_else(|| membership(ROOT_MANIFEST, "member path must be a string"))?;
        if !envelope::normal_path(path) || path.chars().any(|part| matches!(part, '*' | '?' | '['))
        {
            return Err(membership(
                ROOT_MANIFEST,
                &format!("invalid member path {path}"),
            ));
        }
        if !seen.insert(path.to_owned()) {
            return Err(membership(
                ROOT_MANIFEST,
                &format!("duplicate member {path}"),
            ));
        }
        members.push(inspect_member(root, workspace, path)?);
        sources.push(format!("{path}/Cargo.toml"));
    }
    let payload = Census { members };
    let fields = BTreeMap::from([(
        "members".to_owned(),
        serde_json::to_value(payload.members).map_err(|problem| Error::Payload {
            path: DATASET.to_owned(),
            detail: problem.to_string(),
        })?,
    )]);
    let labels = vec!["guide:docgen:overview".to_owned()];
    let bytes = envelope::encode(DATASET, NAMESPACE, "NRZ", &sources, &labels, &fields)?;
    let dataset = serde_json::from_slice(&bytes).map_err(|problem| Error::JsonSyntax {
        path: DATASET.to_owned(),
        detail: problem.to_string(),
    })?;
    Ok((bytes, dataset))
}

/// Read one validated member manifest and its optional README template.
fn inspect_member(
    root: &Path,
    workspace: &Map<String, Value>,
    path: &str,
) -> Result<Member, Error> {
    ensure_directory(root, path)?;
    let member_manifest = format!("{path}/Cargo.toml");
    let metadata = fs::symlink_metadata(root.join(&member_manifest))
        .map_err(|source| error::io(&member_manifest, source))?;
    if !metadata.file_type().is_file() {
        return Err(membership(
            &member_manifest,
            "member manifest must be a regular file",
        ));
    }
    let member_value = envelope::read_toml(root, &member_manifest)?;
    let package = member_value
        .get("package")
        .and_then(Value::as_object)
        .ok_or_else(|| membership(&member_manifest, "missing package table"))?;
    let name = package
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| membership(&member_manifest, "package name must be a string"))?
        .to_owned();
    let description = member_description(workspace, package, &member_manifest)?;
    let template = manifest::template_path(&format!("{path}/README.md"));
    manifest::check_symlinks(root, &template)?;
    let has_readme_template = match fs::symlink_metadata(root.join(&template)) {
        Ok(metadata) if metadata.file_type().is_file() => true,
        Ok(_) => {
            return Err(membership(
                &template,
                "README template must be a regular file",
            ));
        }
        Err(problem) if problem.kind() == ErrorKind::NotFound => false,
        Err(source) => return Err(error::io(&template, source)),
    };
    Ok(Member {
        name,
        path: path.to_owned(),
        description,
        has_readme_template,
        normal_dependencies: dependencies(
            &member_value,
            workspace,
            &member_manifest,
            "dependencies",
        )?,
        development_dependencies: dependencies(
            &member_value,
            workspace,
            &member_manifest,
            "dev-dependencies",
        )?,
        build_dependencies: dependencies(
            &member_value,
            workspace,
            &member_manifest,
            "build-dependencies",
        )?,
    })
}

/// Resolve only the supported explicit or inherited package description forms.
fn member_description(
    workspace: &Map<String, Value>,
    package: &Map<String, Value>,
    path: &str,
) -> Result<Option<String>, Error> {
    let description = match package.get("description") {
        None => None,
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Object(table))
            if table.get("workspace").and_then(Value::as_bool) == Some(true)
                && table.len() == 1 =>
        {
            Some(
                workspace
                    .get("package")
                    .and_then(Value::as_object)
                    .and_then(|details| details.get("description"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        membership(ROOT_MANIFEST, "workspace package description is absent")
                    })?
                    .to_owned(),
            )
        }
        Some(_) => {
            return Err(membership(
                path,
                "description must be a string or description.workspace = true",
            ));
        }
    };
    Ok(description)
}

/// Read declared direct package identities, refusing conditional edges instead of guessing.
fn dependencies(
    value: &Value,
    workspace: &Map<String, Value>,
    path: &str,
    kind: &str,
) -> Result<Vec<String>, Error> {
    if value.get("target").is_some() {
        return Err(membership(
            path,
            "target-specific dependencies are not supported",
        ));
    }
    let Some(entries) = value.get(kind) else {
        return Ok(Vec::new());
    };
    let entries = entries
        .as_object()
        .ok_or_else(|| membership(path, "dependencies must be a table"))?;
    let mut names = BTreeSet::new();
    for (alias, declaration) in entries {
        if declaration.get("optional").and_then(Value::as_bool) == Some(true) {
            return Err(membership(path, "optional dependencies are not supported"));
        }
        let inherited = declaration.get("workspace").and_then(Value::as_bool) == Some(true);
        let declaration = if inherited {
            workspace
                .get("dependencies")
                .and_then(|table| table.get(alias))
                .ok_or_else(|| membership(path, "inherited workspace dependency is absent"))?
        } else {
            declaration
        };
        names.insert(dependency_name(path, alias, declaration)?.to_owned());
    }
    Ok(names.into_iter().collect())
}

/// Dependency aliases resolve only through an explicit package field or their own key.
fn dependency_name<'a>(path: &str, alias: &'a str, value: &'a Value) -> Result<&'a str, Error> {
    if value.is_string() {
        return Ok(alias);
    }
    let table = value
        .as_object()
        .ok_or_else(|| membership(path, "dependency must be a version or table"))?;
    if table.get("optional").and_then(Value::as_bool) == Some(true) {
        return Err(membership(path, "optional dependencies are not supported"));
    }
    match table.get("package") {
        None => Ok(alias),
        Some(Value::String(name)) if !name.is_empty() => Ok(name),
        Some(_) => Err(membership(
            path,
            "dependency package must be a nonempty string",
        )),
    }
}

/// Validate a committed census dataset before rendering uses it.
pub(crate) fn validate_dataset(value: &Value, owners: &BTreeSet<String>) -> Result<(), Error> {
    envelope::validate(value, DATASET, NAMESPACE, owners)?;
    let payload = envelope::payload(value, DATASET)?;
    let census: Census = serde_json::from_value(payload).map_err(|problem| Error::Payload {
        path: DATASET.to_owned(),
        detail: problem.to_string(),
    })?;
    let object = value.as_object().ok_or_else(|| Error::Envelope {
        path: DATASET.to_owned(),
        detail: "dataset must be an object".to_owned(),
    })?;
    let sources = envelope::string_array(object, DATASET, "sources")?;
    let labels = envelope::string_array(object, DATASET, "labels")?;
    let expected_sources: Vec<String> = std::iter::once("Cargo.toml".to_owned())
        .chain(
            census
                .members
                .iter()
                .map(|member| format!("{}/Cargo.toml", member.path)),
        )
        .collect();
    if value.get("owner").and_then(Value::as_str) != Some("NRZ")
        || sources != expected_sources
        || labels.len() != 1
        || labels[0] != "guide:docgen:overview"
    {
        return Err(Error::Envelope {
            path: DATASET.to_owned(),
            detail: "census dataset owner, sources, or labels differ from its declaration"
                .to_owned(),
        });
    }
    Ok(())
}

/// Reject symbolic links and non-directories in a member's path.
fn ensure_directory(root: &Path, path: &str) -> Result<(), Error> {
    let mut current = String::new();
    for component in path.split('/') {
        if !current.is_empty() {
            current.push('/');
        }
        current.push_str(component);
        let metadata = fs::symlink_metadata(root.join(&current))
            .map_err(|source| error::io(&current, source))?;
        if !metadata.file_type().is_dir() {
            return Err(membership(
                &current,
                "member path must use real directories",
            ));
        }
    }
    Ok(())
}

/// Construct a path-bearing membership error.
fn membership(path: &str, detail: &str) -> Error {
    Error::Membership {
        path: path.to_owned(),
        detail: detail.to_owned(),
    }
}

/// Tests of explicit workspace membership and member facts.
#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::derive;
    use crate::error::Error;

    /// Create three member manifests with distinct description and template states.
    fn fixture(root: &Path) -> Result<(), std::io::Error> {
        fs::create_dir_all(root.join("crates/first"))?;
        fs::create_dir_all(root.join("templates/crates/first"))?;
        fs::create_dir_all(root.join("tools/second"))?;
        fs::create_dir_all(root.join("prototypes/third"))?;
        fs::write(
            root.join("Cargo.toml"),
            concat!(
                "[workspace]\n",
                "members = [\"crates/first\", \"tools/second\", \"prototypes/third\"]\n",
                "[workspace.package]\n",
                "description = \"Shared description\"\n",
            ),
        )?;
        fs::write(
            root.join("crates/first/Cargo.toml"),
            "[package]\nname = \"first\"\ndescription = { workspace = true }\n",
        )?;
        fs::write(
            root.join("templates/crates/first/README.md.j2"),
            "template\n",
        )?;
        fs::write(
            root.join("tools/second/Cargo.toml"),
            "[package]\nname = \"second\"\n",
        )?;
        fs::write(
            root.join("prototypes/third/Cargo.toml"),
            "[package]\nname = \"third\"\n",
        )?;
        Ok(())
    }

    /// Use explicit members across all three roots and only mirrored README sources.
    /// ´claim:docgen:census-across-workspace-roots´
    /// ´test:unit:census-across-workspace-roots´
    #[test]
    fn census_across_workspace_roots() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        fixture(root)?;
        fs::write(root.join("tools/second/README.md.j2"), "adjacent\n")?;
        fs::create_dir_all(root.join("templates/prototypes/third"))?;
        fs::write(
            root.join("templates/prototypes/third/README.md.j2"),
            "mirrored\n",
        )?;
        let (_, census) = derive(root)?;
        assert_eq!(
            census["sources"],
            serde_json::json!([
                "Cargo.toml",
                "crates/first/Cargo.toml",
                "tools/second/Cargo.toml",
                "prototypes/third/Cargo.toml",
            ])
        );
        assert_eq!(census["members"][0]["path"], "crates/first");
        assert_eq!(census["members"][1]["path"], "tools/second");
        assert_eq!(census["members"][2]["path"], "prototypes/third");
        assert_eq!(census["members"][1]["has_readme_template"], false);
        assert_eq!(census["members"][2]["has_readme_template"], true);
        Ok(())
    }

    /// Keep member order, inherit one description, and represent absent values distinctly.
    /// ´claim:docgen:derives-member-order-and-optional-facts´
    /// ´test:unit:derives-member-order-and-optional-facts´
    #[test]
    fn derives_member_order_and_optional_facts() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        fixture(directory.path())?;
        let (_, census) = derive(directory.path())?;
        let members = census["members"]
            .as_array()
            .ok_or_else(|| std::io::Error::other("members array absent"))?;
        assert_eq!(members[0]["name"], "first");
        assert_eq!(members[1]["name"], "second");
        assert_eq!(members[0]["description"], "Shared description");
        assert!(members[1]["description"].is_null());
        assert_eq!(members[0]["has_readme_template"], true);
        assert_eq!(members[1]["has_readme_template"], false);
        Ok(())
    }

    /// Reject globs, parent traversal, and duplicates before reading member manifests.
    /// ´claim:docgen:refuses-invalid-member-paths´
    /// ´test:unit:refuses-invalid-member-paths´
    #[test]
    fn refuses_invalid_member_paths() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        fixture(directory.path())?;
        for members in [
            "[\"crates/*\"]",
            "[\"crates/../first\"]",
            "[\"crates/first\", \"crates/first\"]",
        ] {
            fs::write(
                directory.path().join("Cargo.toml"),
                format!("[workspace]\nmembers = {members}\n"),
            )?;
            assert!(matches!(
                derive(directory.path()),
                Err(Error::Membership { .. })
            ));
        }
        Ok(())
    }
    /// Manifest edges retain aliases, workspace inheritance and dependency kinds while refusing conditional declarations.
    /// ´claim:docgen:census-dependency-identities-and-refusals´
    /// ´test:unit:census-dependency-identities-and-refusals´
    #[test]
    fn census_dependency_identities_and_refusals() -> Result<(), Box<dyn std::error::Error>> {
        let workspace =
            serde_json::json!({"dependencies": {"shared": {"package": "actual", "version": "1"}}});
        let workspace = workspace.as_object().ok_or("workspace object")?;
        let value = serde_json::json!({"dependencies": {"shared": {"workspace": true}, "plain": "1"},
            "dev-dependencies": {"oracle": {"package": "exact-oracle", "version": "1"}}});
        assert_eq!(
            super::dependencies(&value, workspace, "Cargo.toml", "dependencies")?,
            ["actual", "plain"]
        );
        assert_eq!(
            super::dependencies(&value, workspace, "Cargo.toml", "dev-dependencies")?,
            ["exact-oracle"]
        );
        assert_eq!(
            super::dependencies(&value, workspace, "Cargo.toml", "build-dependencies")?,
            [] as [String; 0]
        );
        for bad in [
            serde_json::json!({"target": {}, "dependencies": {}}),
            serde_json::json!({"dependencies": {"optional": {"optional": true}}}),
            serde_json::json!({"dependencies": {"missing": {"workspace": true}}}),
            serde_json::json!({"dependencies": {"shared": {"workspace": true, "optional": true}}}),
            serde_json::json!({"dependencies": {"invalid": false}}),
        ] {
            assert!(super::dependencies(&bad, workspace, "Cargo.toml", "dependencies").is_err());
        }
        Ok(())
    }
}
