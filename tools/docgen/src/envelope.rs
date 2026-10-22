// SPDX-License-Identifier: AGPL-3.0-only

//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`accepts_supported_major_and_newer_minor`] | docgen | Accept major one with newer minor and patch versions. |
//! | [`checks_problem_schema_version_policy`] | docgen | Admit only the named v2 minor and reject legacy data with a migration diagnostic. |
//! | [`rejects_invalid_envelopes`] | docgen | Reject missing fields, namespace and version errors, owners, and labels. |
//! | [`envelope_error_precedes_payload_error`] | docgen | Reserve envelope validation ahead of an invalid typed payload. |
//! | [`serializer_orders_fields_and_preserves_arrays`] | docgen | Serialize envelope keys first and sort nested objects without reordering arrays. |

//! Envelope parsing and deterministic dataset encoding.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::error::{self, Error};

/// The six keys reserved by the documentation envelope.
/// The ordered envelope keys are reserved ahead of payload keys so serialization and decoding agree (´rule:docgen:envelope-order´).
/// ´const:docgen:envelope-keys´ (´[ORCHESTRATION-alg:const:form]´)
/// ´const:docgen:envelope-keys-form-xe8b94b6d´
const KEYS: [&str; 6] = [
    "namespace",
    "version",
    "owner",
    "license",
    "sources",
    "labels",
];

/// An envelope followed by a sorted payload map.
#[derive(Debug, Serialize)]
struct Dataset<'a> {
    /// Exact namespace of this dataset.
    namespace: &'a str,
    /// Version of the envelope and payload schema.
    version: [u32; 3],
    /// Declared estate owner.
    owner: &'a str,
    /// SPDX license identifier.
    license: &'a str,
    /// Derivation sources in order.
    sources: &'a [String],
    /// Labels supplied by the dataset.
    labels: &'a [String],
    /// Payload keys sorted after the envelope.
    #[serde(flatten)]
    payload: &'a BTreeMap<String, Value>,
}

/// Read a TOML document completely and convert it to a JSON value for validation.
pub(crate) fn read_toml(root: &Path, path: &str) -> Result<Value, Error> {
    let bytes = fs::read(root.join(path)).map_err(|source| error::io(path, source))?;
    let source = String::from_utf8(bytes).map_err(|problem| Error::TomlSyntax {
        path: path.to_owned(),
        detail: problem.to_string(),
    })?;
    let parsed: toml::Value = toml::from_str(&source).map_err(|problem| Error::TomlSyntax {
        path: path.to_owned(),
        detail: problem.to_string(),
    })?;
    serde_json::to_value(parsed).map_err(|problem| Error::TomlSyntax {
        path: path.to_owned(),
        detail: problem.to_string(),
    })
}

/// Read a JSON dataset completely before envelope inspection.
pub(crate) fn read_json(root: &Path, path: &str) -> Result<Value, Error> {
    let bytes = fs::read(root.join(path)).map_err(|source| error::io(path, source))?;
    serde_json::from_slice(&bytes).map_err(|problem| Error::JsonSyntax {
        path: path.to_owned(),
        detail: problem.to_string(),
    })
}

/// Read declared owner names from the linter snapshot.
pub(crate) fn owners(root: &Path) -> Result<BTreeSet<String>, Error> {
    /// Owner admission reads the declared linter owner inventory rather than a second hard-coded registry (´rule:docgen:owner-authority´).
    /// ´const:docgen:envelope-path´ (´[ORCHESTRATION-alg:const:text]´)
    const PATH: &str = ".linter/owners.toml";
    let value = read_toml(root, PATH)?;
    let entries = value
        .get("owners")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Envelope {
            path: PATH.to_owned(),
            detail: "owners must be an array".to_owned(),
        })?;
    entries
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| Error::Envelope {
                    path: PATH.to_owned(),
                    detail: "each owner must be a string".to_owned(),
                })
        })
        .collect()
}

/// Validate all envelope fields before the caller decodes a payload.
pub(crate) fn validate(
    value: &Value,
    path: &str,
    namespace: &str,
    owners: &BTreeSet<String>,
) -> Result<u32, Error> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(path, "document must be an object"))?;
    for key in KEYS {
        if !object.contains_key(key) {
            return Err(invalid(path, &format!("missing {key}")));
        }
    }
    if object.get("namespace").and_then(Value::as_str) != Some(namespace) {
        return Err(invalid(path, &format!("namespace must be {namespace}")));
    }
    let version = object
        .get("version")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(path, "version must be an array of three unsigned integers"))?;
    let numbers: Vec<u32> = version
        .iter()
        .map(|part| {
            part.as_u64()
                .and_then(|number| u32::try_from(number).ok())
                .ok_or_else(|| invalid(path, "version must contain three unsigned 32-bit integers"))
        })
        .collect::<Result<_, _>>()?;
    if numbers.len() != 3 {
        return Err(invalid(path, "version must contain exactly three integers"));
    }
    let problem_schema = matches!(
        namespace,
        "org.naturrhythmuszentrum.problems.problem"
            | "org.naturrhythmuszentrum.problems.fixture"
            | "org.naturrhythmuszentrum.docs.dataset.problems"
    );
    let supported = if problem_schema {
        numbers[0] == 2 && numbers[1] == 0
    } else {
        numbers[0] == 1
    };
    if !supported {
        return Err(invalid(
            path,
            &format!(
                "found version {version:?}; supported versions are major 1 for documentation and 2.0 for problem declarations; migrate under req:vision:unit-data-migration and docs/design/problem-data-schema.md"
            ),
        ));
    }
    let owner = object
        .get("owner")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(path, "owner must be a string"))?;
    if !owners.contains(owner) {
        return Err(invalid(path, &format!("owner {owner} is not declared")));
    }
    if object.get("license").and_then(Value::as_str) != Some("CC0-1.0") {
        return Err(invalid(path, "license must be CC0-1.0"));
    }
    let sources = string_array(object, path, "sources")?;
    for source in &sources {
        if !normal_path(source) {
            return Err(invalid(path, &format!("invalid source path {source}")));
        }
    }
    let labels = string_array(object, path, "labels")?;
    for label in &labels {
        if !valid_label(label) {
            return Err(invalid(path, &format!("malformed label {label}")));
        }
    }
    Ok(numbers[0])
}

/// Return the payload after envelope validation.
pub(crate) fn payload(value: &Value, path: &str) -> Result<Value, Error> {
    let mut object = value
        .as_object()
        .cloned()
        .ok_or_else(|| invalid(path, "document must be an object"))?;
    object.retain(|key, _| !KEYS.contains(&key.as_str()));
    Ok(Value::Object(object))
}

/// Extract a validated string array from an envelope object.
pub(crate) fn string_array(
    object: &Map<String, Value>,
    path: &str,
    key: &str,
) -> Result<Vec<String>, Error> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(path, &format!("{key} must be an array")))?
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid(path, &format!("{key} must contain strings")))
        })
        .collect()
}

/// Encode a dataset with stable key order and one trailing newline.
pub(crate) fn encode(
    path: &str,
    namespace: &str,
    owner: &str,
    sources: &[String],
    labels: &[String],
    payload: &BTreeMap<String, Value>,
) -> Result<Vec<u8>, Error> {
    encode_version(path, namespace, owner, sources, labels, payload, [1, 0, 0])
}

/// Encode an explicitly admitted schema version with the same deterministic ordering.
pub(crate) fn encode_version(
    path: &str,
    namespace: &str,
    owner: &str,
    sources: &[String],
    labels: &[String],
    payload: &BTreeMap<String, Value>,
    version: [u32; 3],
) -> Result<Vec<u8>, Error> {
    if let Some(key) = payload.keys().find(|key| KEYS.contains(&key.as_str())) {
        return Err(Error::Payload {
            path: path.to_owned(),
            detail: format!("payload key {key} collides with the envelope"),
        });
    }
    let record = Dataset {
        namespace,
        version,
        owner,
        license: "CC0-1.0",
        sources,
        labels,
        payload,
    };
    let mut encoded = serde_json::to_string_pretty(&record).map_err(|problem| Error::Payload {
        path: path.to_owned(),
        detail: problem.to_string(),
    })?;
    encoded.push('\n');
    Ok(encoded.into_bytes())
}

/// Recognize a portable, root-relative path with normal slash-separated components.
pub(crate) fn normal_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// Build a path-specific envelope error.
fn invalid(path: &str, detail: &str) -> Error {
    Error::Envelope {
        path: path.to_owned(),
        detail: detail.to_owned(),
    }
}

/// Check the three-part estate label grammar.
pub(crate) fn valid_label(label: &str) -> bool {
    let parts: Vec<&str> = label.split(':').collect();
    parts.len() == 3
        && parts[0].bytes().all(lower_alnum)
        && parts[1].bytes().all(lower_alnum)
        && !parts[0].is_empty()
        && !parts[1].is_empty()
        && parts[2]
            .split('-')
            .all(|word| !word.is_empty() && word.bytes().all(lower_alnum))
}

/// Accept lowercase ASCII letters and digits.
const fn lower_alnum(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit()
}

/// Boundary tests for envelope precedence and deterministic encoding.
#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use serde_json::{Value, json};

    use super::{encode, validate};
    use crate::error::Error;

    /// Build a complete valid envelope with an irrelevant payload.
    fn fixture() -> Value {
        json!({
            "namespace": "org.example.input",
            "version": [1, 0, 0],
            "owner": "NRZ",
            "license": "CC0-1.0",
            "sources": [],
            "labels": ["guide:docgen:overview"],
            "payload": 7
        })
    }

    /// Build the one declared owner used by these cases.
    fn owners() -> BTreeSet<String> {
        BTreeSet::from(["NRZ".to_owned()])
    }

    /// Accept major one with newer minor and patch versions.
    ///
    ///
    /// ´claim:docgen:accepts-supported-major-and-newer-minor´
    /// ´test:unit:accepts-supported-major-and-newer-minor´
    #[test]
    fn accepts_supported_major_and_newer_minor() {
        let mut value = fixture();
        assert_eq!(
            validate(&value, "fixture.toml", "org.example.input", &owners()).ok(),
            Some(1)
        );
        value["version"] = json!([1, 9, 4]);
        assert_eq!(
            validate(&value, "fixture.toml", "org.example.input", &owners()).ok(),
            Some(1)
        );
    }

    /// Admit only the named v2 minor and reject legacy data with a migration diagnostic.
    /// ´claim:docgen:checks-problem-schema-version-policy´
    /// ´test:unit:checks-problem-schema-version-policy´
    #[test]
    fn checks_problem_schema_version_policy() {
        let namespace = "org.naturrhythmuszentrum.problems.problem";
        let mut value = fixture();
        value["namespace"] = json!(namespace);
        for version in [[2, 0, 0], [2, 0, 8]] {
            value["version"] = json!(version);
            assert!(validate(&value, "problem.toml", namespace, &owners()).is_ok());
        }
        for version in [[1, 0, 0], [2, 1, 0], [3, 0, 0]] {
            value["version"] = json!(version);
            let error = validate(&value, "problem.toml", namespace, &owners())
                .expect_err("unsupported schema")
                .to_string();
            assert!(error.contains("unit-data-migration"));
        }
    }

    /// Reject missing fields, namespace and version errors, owners, and labels.
    /// ´claim:docgen:rejects-invalid-envelopes´
    /// ´test:unit:rejects-invalid-envelopes´
    #[test]
    fn rejects_invalid_envelopes() {
        let mut cases = Vec::new();
        let mut missing = fixture();
        if let Some(object) = missing.as_object_mut() {
            assert!(object.remove("license").is_some());
        }
        cases.push(missing);
        let mut namespace = fixture();
        namespace["namespace"] = json!("org.example.other");
        cases.push(namespace);
        let mut length = fixture();
        length["version"] = json!([1, 0]);
        cases.push(length);
        let mut major = fixture();
        major["version"] = json!([2, 0, 0]);
        cases.push(major);
        let mut owner = fixture();
        owner["owner"] = json!("UNKNOWN");
        cases.push(owner);
        let mut label = fixture();
        label["labels"] = json!(["not a label"]);
        cases.push(label);
        for value in cases {
            assert!(matches!(
                validate(&value, "fixture.toml", "org.example.input", &owners()),
                Err(Error::Envelope { .. })
            ));
        }
    }

    /// Reserve envelope validation ahead of an invalid typed payload.
    /// ´claim:docgen:envelope-error-precedes-payload-error´
    /// ´test:unit:envelope-error-precedes-payload-error´
    #[test]
    fn envelope_error_precedes_payload_error() {
        let mut value = fixture();
        value["version"] = json!([3, 0, 0]);
        value["payload"] = json!({ "unexpected": true });
        assert!(matches!(
            validate(&value, "fixture.toml", "org.example.input", &owners()),
            Err(Error::Envelope { .. })
        ));
    }

    /// Serialize envelope keys first and sort nested objects without reordering arrays.
    /// ´claim:docgen:serializer-orders-fields-and-preserves-arrays´
    /// ´test:unit:serializer-orders-fields-and-preserves-arrays´
    #[test]
    fn serializer_orders_fields_and_preserves_arrays() -> Result<(), Error> {
        let payload = BTreeMap::from([
            ("aaa".to_owned(), json!({ "z": 1, "a": 2 })),
            ("items".to_owned(), json!([2, 1])),
        ]);
        let bytes = encode(
            "fixture.json",
            "org.example.dataset",
            "NRZ",
            &[],
            &[],
            &payload,
        )?;
        let output = String::from_utf8(bytes).map_err(|problem| Error::Payload {
            path: "fixture.json".to_owned(),
            detail: problem.to_string(),
        })?;
        assert!(output.starts_with("{\n  \"namespace\": \"org.example.dataset\""));
        assert!(output.find("\"labels\"") < output.find("\"aaa\""));
        assert!(output.find("\"a\"") < output.find("\"z\""));
        assert!(output.contains("\"items\": [\n    2,\n    1\n  ]"));
        assert!(output.ends_with('\n'));
        Ok(())
    }
}
