// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`preserves_every_reference_payload_field`] | docgen | Every original value, magnitude, identity, citation and documentary string survives exactly. Additional entries must pass authoritative v2 validation and the generated fixed point. |
//! | [`preservation_rejects_missing_or_changed_originals`] | docgen | Neither removing an original source nor changing its documentary payload can pass preservation. |

//! Exact conservation of the independent v1 reference payload during schema migration.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};

/// Reconstruct only the old unit fields, reading each coordinate declaration independently.
fn legacy_unit(value: &Value, original: &Value) -> Value {
    if value["role"] == "metadata" {
        assert!(value.get("descriptor").is_none());
        return json!({"display": "1"});
    }
    let descriptor = &value["descriptor"];
    let mut unit = json!({"display": descriptor["measure"]["display"]});
    assert_eq!(descriptor["measure"]["scale"], "1");
    for (index, axis) in ["m", "kg", "s", "A", "K", "mol", "cd"].iter().enumerate() {
        let exponent = descriptor["dimensions"][index]
            .as_str()
            .expect("exponent")
            .parse::<i64>()
            .expect("original integer exponent");
        if original.get(*axis).is_some() {
            unit[*axis] = json!(exponent);
        } else {
            assert_eq!(exponent, 0);
        }
    }
    if original.get("offset").is_some() {
        unit["offset"] = descriptor["measure"]["offset"].clone();
    } else {
        assert_eq!(descriptor["measure"]["offset"], "0");
    }
    unit
}

/// Project a migrated payload back to every original data and documentary field.
fn project(value: &Value, original: &Value) -> Value {
    let mut result = value.clone();
    let object = result.as_object_mut().expect("entry");
    for key in [
        "exponent_policy",
        "kinds",
        "kind_rules",
        "symbols",
        "equations",
        "validation",
    ] {
        object.remove(key);
    }
    for group in ["parameters", "observables"] {
        let old = original[group].as_array().expect("old declarations");
        let entries = result[group].as_array_mut().expect("new declarations");
        assert_eq!(old.len(), entries.len());
        for (entry, old) in entries.iter_mut().zip(old) {
            let unit = legacy_unit(entry, &old["unit"]);
            let record = entry.as_object_mut().expect("declaration");
            for key in [
                "role",
                "descriptor",
                "nonzero",
                "metadata",
                "expression",
                "companion",
            ] {
                record.remove(key);
            }
            record.insert("unit".to_owned(), unit);
            if group == "parameters" && record["uncertainty"]["kind"] == "components" {
                let components = record["uncertainty"]["components"]
                    .as_array()
                    .expect("components");
                assert_eq!(components.len(), 1);
                let component = &components[0];
                assert_eq!(component["target"], record["name"]);
                assert_eq!(component["class"], "measurement");
                let mut uncertainty = component["magnitude"].clone();
                uncertainty["kind"] = json!(match component["mode"].as_str().expect("mode") {
                    "standard" => "standard",
                    "radius" => "enclosure",
                    mode => panic!("unexpected migrated mode {mode}"),
                });
                uncertainty["source"] = component["source"].clone();
                record.insert("uncertainty".to_owned(), uncertainty);
            }
        }
    }
    result
}

/// Index entries by their source identity without dropping duplicates or unmatched rows.
fn entries_by_source(dataset: &Value) -> BTreeMap<&str, &Value> {
    let sources = dataset["sources"].as_array().expect("sources");
    let entries = dataset["entries"].as_array().expect("entries");
    assert_eq!(sources.len(), entries.len());
    let mut indexed = BTreeMap::new();
    for (source, entry) in sources.iter().zip(entries) {
        let path = source.as_str().expect("source path");
        assert!(indexed.insert(path, entry).is_none(), "duplicate {path}");
    }
    indexed
}

/// Keep the complete original inventory and exact payload checks independent of additions.
fn assert_originals_preserved(current: &Value, baseline: &Value) {
    assert_eq!(current["version"], json!([2, 0, 0]));
    let originals = entries_by_source(baseline);
    assert_eq!(originals.len(), 10);
    let entries = entries_by_source(current);
    let mut checked = 0_u64;
    let mut unsupported = 0_u64;
    for (path, original) in originals {
        let entry = entries.get(path).expect("missing original source");
        assert_eq!(project(entry, original), *original, "{path}");
        checked += entry["validation"]["checked_observables"]
            .as_u64()
            .expect("checked count");
        unsupported += entry["validation"]["unsupported_observables"]
            .as_u64()
            .expect("unsupported count");
    }
    assert_eq!(checked, 78);
    assert_eq!(unsupported, 12);
}

/// Read the v2 source envelope separately from its generated dataset entry.
fn source_payload(
    root: &Path,
    path: &str,
    entry: &Value,
) -> Result<Value, Box<dyn std::error::Error>> {
    let source: toml::Value = toml::from_str(&std::fs::read_to_string(root.join(path))?)?;
    let mut source = serde_json::to_value(source)?;
    let object = source.as_object_mut().expect("source object");
    assert_eq!(object["version"], json!([2, 0, 0]));
    assert_eq!(object["labels"], json!([entry["problem"]["register"]]));
    for key in [
        "namespace",
        "version",
        "owner",
        "license",
        "sources",
        "labels",
    ] {
        object.remove(key);
    }
    Ok(source)
}

/// Every original value, magnitude, identity, citation and documentary string survives exactly.
/// Additional entries must pass authoritative v2 validation and the generated fixed point.
/// ´claim:docgen:preserves-every-reference-payload-field´
/// ´test:integration:preserves-every-reference-payload-field´
#[test]
fn preserves_every_reference_payload_field() -> Result<(), Box<dyn std::error::Error>> {
    let baseline: Value = serde_json::from_str(include_str!("support/reference-v1.json"))?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let current: Value =
        serde_json::from_slice(&std::fs::read(root.join("data/datasets/problems.json"))?)?;
    assert_originals_preserved(&current, &baseline);
    let originals = entries_by_source(&baseline);
    for (path, entry) in entries_by_source(&current) {
        let source = source_payload(&root, path, entry)?;
        if let Some(original) = originals.get(path) {
            assert_eq!(project(&source, original), **original, "{path}");
        }
    }
    // Re-derive every source through the schema, including all additions, then compare bytes.
    // This read-only public API rejects malformed additions and stale or forged dataset rows.
    assert_eq!(nrz_docgen::check(&root)?.len(), 0);
    let mut reordered = current;
    reordered["sources"]
        .as_array_mut()
        .expect("sources")
        .reverse();
    reordered["entries"]
        .as_array_mut()
        .expect("entries")
        .reverse();
    assert_originals_preserved(&reordered, &baseline);
    Ok(())
}

/// Neither removing an original source nor changing its documentary payload can pass preservation.
/// ´claim:docgen:preservation-rejects-missing-or-changed-originals´
/// ´test:integration:preservation-rejects-missing-or-changed-originals´
#[test]
fn preservation_rejects_missing_or_changed_originals() -> Result<(), Box<dyn std::error::Error>> {
    let baseline: Value = serde_json::from_str(include_str!("support/reference-v1.json"))?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let current: Value =
        serde_json::from_slice(&std::fs::read(root.join("data/datasets/problems.json"))?)?;
    let index = current["sources"]
        .as_array()
        .expect("sources")
        .iter()
        .position(|source| source == &baseline["sources"][0])
        .expect("original source");
    let mut missing = current.clone();
    missing["sources"]
        .as_array_mut()
        .expect("sources")
        .remove(index);
    missing["entries"]
        .as_array_mut()
        .expect("entries")
        .remove(index);
    assert!(std::panic::catch_unwind(|| assert_originals_preserved(&missing, &baseline)).is_err());
    let mut changed = current;
    changed["entries"][index]["problem"]["model"] = json!("altered reference model");
    assert!(std::panic::catch_unwind(|| assert_originals_preserved(&changed, &baseline)).is_err());
    Ok(())
}
