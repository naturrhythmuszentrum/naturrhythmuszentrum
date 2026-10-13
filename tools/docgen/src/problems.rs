// SPDX-License-Identifier: AGPL-3.0-only

//! Discovery, validation, and derivation of reference problems and fixtures.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use crate::envelope;
use crate::error::{self, Error};

/// Path of the derived exemplar dataset.
pub(crate) const DATASET: &str = "data/datasets/problems.json";
/// Dataset namespace.
const DATASET_NAMESPACE: &str = "org.naturrhythmuszentrum.docs.dataset.problems";
/// Problem input namespace.
const PROBLEM_NAMESPACE: &str = "org.naturrhythmuszentrum.problems.problem";
/// Fixture input namespace.
const FIXTURE_NAMESPACE: &str = "org.naturrhythmuszentrum.problems.fixture";

/// One input path and its identity inferred from the tree.
#[derive(Debug)]
struct Location {
    /// Root-relative input path.
    path: String,
    /// Directory name carrying the input.
    slug: String,
    /// Declared entry kind inferred from the location.
    kind: &'static str,
}

/// The dataset's only payload key.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DatasetPayload {
    /// Ordered problem and fixture payloads.
    entries: Vec<Value>,
}

/// Derive the dataset from all validated entry files in catalogue order.
pub(crate) fn derive(root: &Path, owners: &BTreeSet<String>) -> Result<(Vec<u8>, Value), Error> {
    let locations = layout(root)?;
    let mut sources = Vec::new();
    let mut entries = Vec::new();
    for location in &locations {
        let value = envelope::read_toml(root, &location.path)?;
        let namespace = if location.kind == "problem" {
            PROBLEM_NAMESPACE
        } else {
            FIXTURE_NAMESPACE
        };
        envelope::validate(&value, &location.path, namespace, owners)?;
        let object = value
            .as_object()
            .ok_or_else(|| invalid(&location.path, "envelope", "entry must be an object"))?;
        let sources_in = envelope::string_array(object, &location.path, "sources")?;
        let labels = envelope::string_array(object, &location.path, "labels")?;
        if !sources_in.is_empty() || labels.len() != 1 {
            return Err(Error::Envelope {
                path: location.path.clone(),
                detail: "sources must be empty and labels must contain exactly one label"
                    .to_owned(),
            });
        }
        let payload = envelope::payload(&value, &location.path)?;
        let entry = crate::problem_schema::validate(
            &payload,
            &location.path,
            &location.slug,
            location.kind,
            &labels[0],
        )?;
        entries.push(entry);
        sources.push(location.path.clone());
    }
    let payload = BTreeMap::from([("entries".to_owned(), Value::Array(entries))]);
    let bytes = envelope::encode_version(
        DATASET,
        DATASET_NAMESPACE,
        "NRZ",
        &sources,
        &[],
        &payload,
        [2, 0, 0],
    )?;
    let value = serde_json::from_slice(&bytes).map_err(|problem| Error::JsonSyntax {
        path: DATASET.to_owned(),
        detail: problem.to_string(),
    })?;
    Ok((bytes, value))
}

/// Validate a committed dataset before a template receives its entries.
pub(crate) fn validate_dataset(value: &Value, owners: &BTreeSet<String>) -> Result<(), Error> {
    envelope::validate(value, DATASET, DATASET_NAMESPACE, owners)?;
    let object = value
        .as_object()
        .ok_or_else(|| invalid(DATASET, "envelope", "dataset must be an object"))?;
    let sources = envelope::string_array(object, DATASET, "sources")?;
    let labels = envelope::string_array(object, DATASET, "labels")?;
    if value.get("owner").and_then(Value::as_str) != Some("NRZ") || !labels.is_empty() {
        return Err(Error::Envelope {
            path: DATASET.to_owned(),
            detail: "problems dataset owner or labels differ from its declaration".to_owned(),
        });
    }
    let payload = envelope::payload(value, DATASET)?;
    let dataset: DatasetPayload = serde_json::from_value(payload)
        .map_err(|problem| invalid(DATASET, "typed payload", &problem.to_string()))?;
    if sources.len() != dataset.entries.len() {
        return Err(invalid(
            DATASET,
            "identity",
            "sources and entries differ in length",
        ));
    }
    let mut previous: Option<(bool, String)> = None;
    for (source, entry) in sources.iter().zip(&dataset.entries) {
        let location = location_from_source(source)?;
        let order = (location.kind == "fixture", location.slug.clone());
        if previous.as_ref().is_some_and(|old| old >= &order) {
            return Err(invalid(
                DATASET,
                "identity",
                "entries are not in catalogue order",
            ));
        }
        previous = Some(order);
        crate::problem_schema::revalidate(
            entry,
            &format!("{DATASET}:{}", location.path),
            &location.slug,
            location.kind,
        )?;
    }
    Ok(())
}

/// Discover the permitted layout without following symbolic links.
fn layout(root: &Path) -> Result<Vec<Location>, Error> {
    crate::manifest::check_symlinks(root, "data/problems")?;
    let base = root.join("data/problems");
    let metadata = fs::symlink_metadata(&base).map_err(|source| {
        invalid(
            "data/problems",
            "layout",
            &format!("cannot read directory: {source}"),
        )
    })?;
    if !metadata.file_type().is_dir() {
        return Err(invalid(
            "data/problems",
            "layout",
            "expected a real directory",
        ));
    }
    let mut problems = Vec::new();
    let mut fixtures = Vec::new();
    for entry in fs::read_dir(&base).map_err(|source| error::io("data/problems", source))? {
        let entry = entry.map_err(|source| error::io("data/problems", source))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid("data/problems", "layout", "entry name is not UTF-8"))?;
        let path = format!("data/problems/{name}");
        let kind = entry
            .file_type()
            .map_err(|source| error::io(&path, source))?;
        if kind.is_symlink() {
            return Err(invalid(&path, "layout", "symbolic link is not allowed"));
        }
        if name == "fixtures" {
            if !kind.is_dir() {
                return Err(invalid(&path, "layout", "expected a real directory"));
            }
            collect_slugs(root, &path, "fixture", "fixture.toml", &mut fixtures)?;
        } else if valid_slug(&name) && kind.is_dir() {
            exact_file(root, &path, "problem.toml")?;
            problems.push(Location {
                path: format!("{path}/problem.toml"),
                slug: name,
                kind: "problem",
            });
        } else {
            return Err(invalid(
                &path,
                "layout",
                "unexpected entry or malformed slug",
            ));
        }
    }
    problems.sort_by(|left, right| left.slug.cmp(&right.slug));
    fixtures.sort_by(|left, right| left.slug.cmp(&right.slug));
    problems.extend(fixtures);
    Ok(problems)
}

/// Collect fixture slug directories under the fixture root.
fn collect_slugs(
    root: &Path,
    directory: &str,
    kind: &'static str,
    filename: &str,
    output: &mut Vec<Location>,
) -> Result<(), Error> {
    for entry in
        fs::read_dir(root.join(directory)).map_err(|source| error::io(directory, source))?
    {
        let entry = entry.map_err(|source| error::io(directory, source))?;
        let slug = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid(directory, "layout", "entry name is not UTF-8"))?;
        let path = format!("{directory}/{slug}");
        let file_type = entry
            .file_type()
            .map_err(|source| error::io(&path, source))?;
        if !valid_slug(&slug) || !file_type.is_dir() {
            return Err(invalid(&path, "layout", "expected a real slug directory"));
        }
        exact_file(root, &path, filename)?;
        output.push(Location {
            path: format!("{path}/{filename}"),
            slug,
            kind,
        });
    }
    Ok(())
}

/// Require exactly one named regular file inside a slug directory.
fn exact_file(root: &Path, directory: &str, filename: &str) -> Result<(), Error> {
    let entries =
        fs::read_dir(root.join(directory)).map_err(|source| error::io(directory, source))?;
    let mut count = 0;
    for entry in entries {
        let entry = entry.map_err(|source| error::io(directory, source))?;
        let path = format!("{directory}/{}", entry.file_name().to_string_lossy());
        let file_type = entry
            .file_type()
            .map_err(|source| error::io(&path, source))?;
        if entry.file_name() != filename || !file_type.is_file() {
            return Err(invalid(&path, "layout", "unexpected file or symbolic link"));
        }
        count += 1;
    }
    if count != 1 {
        return Err(invalid(
            directory,
            "layout",
            "expected exactly one entry file",
        ));
    }
    Ok(())
}

/// Parse a dataset source path into the same identity as a tree location.
fn location_from_source(source: &str) -> Result<Location, Error> {
    let parts: Vec<&str> = source.split('/').collect();
    let (kind, slug) = match parts.as_slice() {
        ["data", "problems", slug, "problem.toml"] => ("problem", *slug),
        ["data", "problems", "fixtures", slug, "fixture.toml"] => ("fixture", *slug),
        _ => return Err(invalid(source, "identity", "invalid entry source path")),
    };
    if !valid_slug(slug) {
        return Err(invalid(source, "identity", "malformed source slug"));
    }
    Ok(Location {
        path: source.to_owned(),
        slug: slug.to_owned(),
        kind,
    })
}

/// Recognize lowercase ASCII slugs with single internal hyphens.
fn valid_slug(slug: &str) -> bool {
    let mut bytes = slug.bytes();
    if !matches!(bytes.next(), Some(first) if first.is_ascii_lowercase()) {
        return false;
    }
    let mut hyphen = false;
    for byte in bytes {
        if byte == b'-' {
            if hyphen {
                return false;
            }
            hyphen = true;
        } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            hyphen = false;
        } else {
            return false;
        }
    }
    !hyphen
}

/// Construct a path-bearing problem-stage error with its rule name.
fn invalid(path: &str, rule: &str, detail: &str) -> Error {
    Error::Payload {
        path: path.to_owned(),
        detail: format!("{rule}: {detail}"),
    }
}

/// Boundary tests for the ordered problem-stage rules.
///
#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;

    use serde_json::{Value, json};

    use super::{derive, layout};
    use crate::error::Error;

    /// Build a complete minimal entry for direct rule tests.
    fn sample() -> Value {
        json!({
            "problem": {
                "slug": "alpha", "title": "Alpha", "register": "prob:exemplars:alpha",
                "kind": "problem", "model": "Model", "instance": "Instance"
            },
            "exponent_policy": "rational",
            "kinds": [{
                "name": "length",
                "type": "named",
                "dimensions": ["1", "0", "0", "0", "0", "0", "0"]
            }],
            "kind_rules": [],
            "parameters": [{
                "name": "p", "description": "Input", "value": "1/2", "role": "quantity", "nonzero": false,
                "descriptor": {
                    "dimensions": ["1", "0", "0", "0", "0", "0", "0"],
                    "kind": "length",
                    "measure": {"display": "m", "scale": "1", "offset": "0"},
                    "affine_role": "difference"
                }, "uncertainty": {"kind": "none"}
            }],
            "observables": [{
                "name": "answer", "description": "Output", "descriptor": {
                    "dimensions": ["1", "0", "0", "0", "0", "0", "0"],
                    "kind": "length",
                    "measure": {"display": "m", "scale": "1", "offset": "0"},
                    "affine_role": "difference"
                },
                "expression": {
                    "kind": "algebra",
                    "text": "p"
                }, "closed_form": "p", "value": "0.500000000000", "value_kind": "decimal-12",
                "derivation": "Direct", "second_route": "none"
            }],
            "variants": [{
                "name": "base", "description": "Base", "overrides": {"p": "1/2"},
                "expected": "Half"
            }],
            "errors": {"bounded": [], "estimated": [], "excluded": []},
            "stress": {
                "claims": ["rule:vision:example"],
                "bites": ["step:nrz:bite-exemplar-register"]
            }
        })
    }

    /// Assert that a located modified source sample is refused.
    fn invalid_sample(value: &Value) {
        assert!(
            crate::problem_schema::validate(
                value,
                "data/problems/alpha/problem.toml",
                "alpha",
                "problem",
                "prob:exemplars:alpha",
            )
            .is_err()
        );
    }

    /// Write one minimal valid entry under its inferred location.
    fn write_entry(root: &Path, kind: &str, slug: &str) -> Result<(), std::io::Error> {
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
        let source = include_str!("../tests/support/problem.toml")
            .replace("{kind}", kind)
            .replace("{slug}", slug);
        fs::write(root.join(directory).join(filename), source)
    }

    /// Return the one declared owner for an isolated fixture tree.
    fn owners() -> BTreeSet<String> {
        BTreeSet::from(["NRZ".to_owned()])
    }

    /// Decode one valid problem and one valid fixture into the ordered dataset.
    ///
    /// ´test:unit:decodes-valid-problem-and-fixture´
    #[test]
    fn decodes_valid_problem_and_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_entry(directory.path(), "fixture", "beta")?;
        write_entry(directory.path(), "problem", "alpha")?;
        let (_, value) = derive(directory.path(), &owners())?;
        assert_eq!(value["entries"][0]["problem"]["slug"], "alpha");
        assert_eq!(value["entries"][1]["problem"]["slug"], "beta");
        Ok(())
    }

    /// Reject a stray file at the top of the problems tree.
    /// ´test:unit:layout-refuses-stray-file´
    #[test]
    fn layout_refuses_stray_file() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_entry(directory.path(), "problem", "alpha")?;
        fs::write(directory.path().join("data/problems/stray.txt"), "stray")?;
        assert!(matches!(
            layout(directory.path()),
            Err(Error::Payload { .. })
        ));
        Ok(())
    }

    /// Reject a second file inside an otherwise valid slug directory.
    /// ´test:unit:layout-refuses-second-file´
    #[test]
    fn layout_refuses_second_file() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_entry(directory.path(), "problem", "alpha")?;
        fs::write(directory.path().join("data/problems/alpha/extra"), "extra")?;
        assert!(matches!(
            layout(directory.path()),
            Err(Error::Payload { .. })
        ));
        Ok(())
    }

    /// Reject a slug with a repeated hyphen.
    /// ´test:unit:layout-refuses-malformed-slug´
    #[test]
    fn layout_refuses_malformed_slug() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_entry(directory.path(), "problem", "bad--slug")?;
        assert!(matches!(
            layout(directory.path()),
            Err(Error::Payload { .. })
        ));
        Ok(())
    }

    /// Reject an input namespace that disagrees with its tree location.
    /// ´test:unit:envelope-refuses-wrong-namespace´
    #[test]
    fn envelope_refuses_wrong_namespace() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_entry(directory.path(), "problem", "alpha")?;
        let path = directory.path().join("data/problems/alpha/problem.toml");
        let source = fs::read_to_string(&path)?;
        fs::write(
            &path,
            source.replace("problems.problem", "problems.fixture"),
        )?;
        assert!(matches!(
            derive(directory.path(), &owners()),
            Err(Error::Envelope { .. })
        ));
        Ok(())
    }

    /// Reject a second envelope label even when both labels are well formed.
    /// ´test:unit:envelope-refuses-second-label´
    #[test]
    fn envelope_refuses_second_label() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_entry(directory.path(), "problem", "alpha")?;
        let path = directory.path().join("data/problems/alpha/problem.toml");
        let source = fs::read_to_string(&path)?;
        fs::write(
            &path,
            source.replace(
                "labels = [\"prob:exemplars:alpha\"]",
                "labels = [\"prob:exemplars:alpha\", \"prob:exemplars:beta\"]",
            ),
        )?;
        assert!(matches!(
            derive(directory.path(), &owners()),
            Err(Error::Envelope { .. })
        ));
        Ok(())
    }

    /// Catch a register that differs from the envelope label.
    /// ´test:unit:identity-refuses-register-mismatch´
    #[test]
    fn identity_refuses_register_mismatch() {
        let mut value = sample();
        value["problem"]["register"] = json!("prob:exemplars:other");
        invalid_sample(&value);
    }

    /// Reject a value string outside the exact grammar.
    ///
    /// ´test:unit:values-refuse-invalid-number´
    #[test]
    fn values_refuse_invalid_number() {
        let mut value = sample();
        value["parameters"][0]["value"] = json!("1+2");
        invalid_sample(&value);
    }

    /// Reject an undeclared descriptor field.
    ///
    /// ´test:unit:units-refuse-unknown-key´
    #[test]
    fn units_refuse_unknown_key() {
        let mut value = sample();
        value["parameters"][0]["descriptor"]["gram"] = json!(1);
        invalid_sample(&value);
    }

    /// Reject an enclosure that omits its source.
    ///
    /// TODO ´todo:code:reject-missing-correlations-under-the-future´: reject missing correlations under the future shared-source contract without treating standard uncertainty as a radius under rule:vision:uncertainty-unit-algebra.
    ///
    /// ´test:unit:uncertainty-refuses-incomplete-shape´
    #[test]
    fn uncertainty_refuses_incomplete_shape() {
        let mut value = sample();
        value["kinds"].as_array_mut().expect("kinds").push(json!({
            "name": "radius_length", "type": "uncertainty", "mode": "radius",
            "value_kind": "length", "dimensions": ["1", "0", "0", "0", "0", "0", "0"],
        }));
        let mut descriptor = value["parameters"][0]["descriptor"].clone();
        descriptor["kind"] = json!("radius_length");
        value["parameters"][0]["uncertainty"] = json!({
            "kind": "components", "components": [{
                "name": "radius_p", "class": "measurement", "mode": "radius", "target": "p",
                "descriptor": descriptor, "assurance": "Assumed deterministic bound",
                "magnitude": {"half_width": "1/10"},
            }],
        });
        invalid_sample(&value);
    }

    /// Reject a decimal with fewer significant digits than declared.
    ///
    /// ´test:unit:exactness-refuses-wrong-precision´
    #[test]
    fn exactness_refuses_wrong_precision() {
        let mut value = sample();
        value["observables"][0]["value"] = json!("0.5");
        invalid_sample(&value);
    }

    /// Reject a bite label outside the step and NRZ area.
    /// ´test:unit:labels-refuse-wrong-bite-kind´
    #[test]
    fn labels_refuse_wrong_bite_kind() {
        let mut value = sample();
        value["stress"]["bites"][0] = json!("rule:vision:example");
        invalid_sample(&value);
    }

    /// Report an invalid envelope before attempting to decode an invalid payload.
    /// ´test:unit:envelope-precedes-payload-error´
    #[test]
    fn envelope_precedes_payload_error() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_entry(directory.path(), "problem", "alpha")?;
        let path = directory.path().join("data/problems/alpha/problem.toml");
        let source = fs::read_to_string(&path)?;
        fs::write(
            &path,
            source
                .replace("version = [2, 0, 0]", "version = [3, 0, 0]")
                .replace("title = \"Sample\"", "bad_field = \"Value\""),
        )?;
        assert!(matches!(
            derive(directory.path(), &owners()),
            Err(Error::Envelope { .. })
        ));
        Ok(())
    }
}
