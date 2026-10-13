// SPDX-License-Identifier: AGPL-3.0-only
//! Exact conservation of the independent v1 reference payload during schema migration.

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

/// Every original value, magnitude, identity, citation and documentary string survives exactly.
/// ´test:integration:preserves-every-reference-payload-field´
#[test]
fn preserves_every_reference_payload_field() -> Result<(), Box<dyn std::error::Error>> {
    let baseline: Value = serde_json::from_str(include_str!("support/reference-v1.json"))?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let current: Value =
        serde_json::from_slice(&std::fs::read(root.join("data/datasets/problems.json"))?)?;
    assert_eq!(current["version"], json!([2, 0, 0]));
    assert_eq!(current["sources"], baseline["sources"]);
    let originals = baseline["entries"].as_array().expect("original entries");
    let entries = current["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 10);
    assert_eq!(entries.len(), originals.len());
    for (index, (entry, original)) in entries.iter().zip(originals).enumerate() {
        assert_eq!(project(entry, original), *original);
        let path = current["sources"][index].as_str().expect("source");
        let source: toml::Value = toml::from_str(&std::fs::read_to_string(root.join(path))?)?;
        let mut source = serde_json::to_value(source)?;
        let object = source.as_object_mut().expect("source object");
        assert_eq!(object["version"], json!([2, 0, 0]));
        assert_eq!(object["labels"], json!([original["problem"]["register"]]));
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
        assert_eq!(project(&source, original), *original);
    }
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry["validation"]["checked_observables"]
                .as_u64()
                .expect("count"))
            .sum::<u64>(),
        78
    );
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry["validation"]["unsupported_observables"]
                .as_u64()
                .expect("count"))
            .sum::<u64>(),
        12
    );
    Ok(())
}
