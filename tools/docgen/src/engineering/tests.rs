// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`engineering_record_order_and_fixed_point`] | docgen | Reviewed records serialize deterministically and sort observation identities. |
//! | [`engineering_source_drift_refuses`] | docgen | Any changed reviewed source refuses before the catalogue can render. |
//! | [`engineering_record_mutations_refuse`] | docgen | Invalid provenance, table structure, source ownership and unknown fields refuse explicitly. |
//! | [`engineering_render_rechecks_inputs`] | docgen | Rendering rejects forged stored rows even when their envelope remains valid. |

//! Engineering records reject stale source reviews and malformed observation provenance.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use super::{INPUT, derive, validate, validate_dataset};
use crate::Error;

/// Keep fixture construction fallible and independent of the checked-in catalogue.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Build a reviewed scalar fixture with an exact source-byte version.
fn fixture() -> TestResult<(tempfile::TempDir, Value)> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    fs::create_dir_all(root.join("data/inputs"))?;
    fs::create_dir_all(root.join("crates/scalar/src"))?;
    let source = "// reviewed source fixture\n";
    fs::write(root.join("crates/scalar/src/example.rs"), source)?;
    let value = json!({
        "namespace": "org.naturrhythmuszentrum.docs.input.engineering",
        "version": [1, 0, 0], "owner": "NRZ", "license": "CC0-1.0",
        "sources": [], "labels": [],
        "source_versions": [{"path": "crates/scalar/src/example.rs",
            "fingerprint": validate::fingerprint(source.as_bytes())}],
        "kernels": [{"id": "sample", "owner": "SCALAR", "operation": "sample",
            "law": "identity", "domain": "finite", "guarantee": "exact",
            "shape": "scalar", "refusals": "nonfinite", "source": "crates/scalar/src/example.rs",
            "claims": ["[SCALAR-claim:scalar:sample]"]}],
        "refusals": [], "measurements": [observation("z"), observation("a")]
    });
    write_input(root, &value)?;
    Ok((directory, value))
}

/// A recorded fixture observation has complete, separately validated provenance.
fn observation(id: &str) -> Value {
    json!({"id": id, "metric": "test wall", "value": "0.01", "unit": "s",
        "date": "2024-02-29", "commit": "0123456789012345678901234567890123456789",
        "command": "cargo test -p nrz-scalar", "source": "fixture observation"})
}

/// Serialize only the input; production derivation owns every dataset byte.
fn write_input(root: &Path, value: &Value) -> TestResult {
    fs::write(root.join(INPUT), toml::to_string(value)?)?;
    Ok(())
}

/// Reviewed records serialize deterministically and sort observation identities.
/// ´claim:docgen:engineering-record-order-and-fixed-point´
/// ´test:unit:engineering-record-order-and-fixed-point´
#[test]
fn engineering_record_order_and_fixed_point() -> TestResult {
    let (directory, _) = fixture()?;
    let owners = BTreeSet::from(["NRZ".to_owned()]);
    let (bytes, value) = derive(directory.path(), &owners)?;
    assert_eq!(value["measurements"][0]["id"], "a");
    assert_eq!(value["measurements"][1]["id"], "z");
    assert_eq!(derive(directory.path(), &owners)?.0, bytes);
    validate_dataset(directory.path(), &value, &owners)?;
    Ok(())
}

/// Any changed reviewed source refuses before the catalogue can render.
/// ´claim:docgen:engineering-source-drift-refuses´
/// ´test:unit:engineering-source-drift-refuses´
#[test]
fn engineering_source_drift_refuses() -> TestResult {
    let (directory, _) = fixture()?;
    fs::write(
        directory.path().join("crates/scalar/src/example.rs"),
        "// changed\n",
    )?;
    let result = derive(directory.path(), &BTreeSet::from(["NRZ".to_owned()]));
    let Err(Error::Payload { detail, .. }) = result else {
        return Err("expected a source review refusal".into());
    };
    for expected in [
        "source fingerprint changed",
        "crates/scalar/src/example.rs",
        "kernel:sample",
        "refresh",
        INPUT,
    ] {
        assert!(detail.contains(expected));
    }
    Ok(())
}

/// Invalid provenance, table structure, source ownership and unknown fields refuse explicitly.
/// ´claim:docgen:engineering-record-mutations-refuse´
/// ´test:unit:engineering-record-mutations-refuse´
#[test]
fn engineering_record_mutations_refuse() -> TestResult {
    let (directory, original) = fixture()?;
    let owners = BTreeSet::from(["NRZ".to_owned()]);
    for (pointer, replacement) in [
        ("/measurements/0/date", json!("2026-02-29")),
        ("/measurements/0/value", json!("NaN")),
        ("/measurements/0/commit", json!("short")),
        ("/measurements/0/id", json!("a")),
        ("/kernels/0/law", json!("left|right")),
        ("/kernels/0/owner", json!("CAS")),
        ("/kernels/0/source", json!("crates/scalar/src/missing.rs")),
        ("/kernels/0/claims", json!([])),
        ("/kernels", json!([])),
        ("/source_versions/0/path", json!("../escape.rs")),
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(pointer).ok_or("fixture pointer")? = replacement;
        write_input(directory.path(), &changed)?;
        assert!(derive(directory.path(), &owners).is_err());
    }
    let mut changed = original;
    changed["unknown"] = json!(true);
    write_input(directory.path(), &changed)?;
    assert!(derive(directory.path(), &owners).is_err());
    Ok(())
}

/// Rendering rejects forged stored rows even when their envelope remains valid.
/// ´claim:docgen:engineering-render-rechecks-inputs´
/// ´test:unit:engineering-render-rechecks-inputs´
#[test]
fn engineering_render_rechecks_inputs() -> TestResult {
    let (directory, _) = fixture()?;
    let owners = BTreeSet::from(["NRZ".to_owned()]);
    let (_, mut value) = derive(directory.path(), &owners)?;
    value["kernels"][0]["guarantee"] = json!("invented guarantee");
    assert!(validate_dataset(directory.path(), &value, &owners).is_err());
    Ok(())
}
