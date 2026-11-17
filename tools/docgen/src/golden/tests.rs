// SPDX-License-Identifier: AGPL-3.0-only
//! Positive fixtures, independent evidence mutations and resource boundaries.

use nrz_cas::limits::{Budget, Limits as CasLimits};
use serde_json::{Value, json};

use super::{Cause, Error, Limits, read};

mod bounds;
mod constants;
mod power;

/// Read the small CC0 fixture, never a separately implemented validator.
///
/// # Panics
/// Panics only if the committed fixture is not JSON, which the fixture gate rejects.
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../tests/support/prepared-binary64-golden-seven.json"
    ))
    .expect("committed JSON fixture")
}

/// Give exact fixture checking explicit finite capacity.
///
/// # Panics
/// Panics only if the declared test limits cease to be valid CAS limits.
fn budget(work: u64) -> Budget {
    Budget::new(CasLimits {
        work,
        literal_bytes: 4096,
        ..CasLimits::default()
    })
    .expect("finite fixture limits")
}

/// Encode a fixture mutation and check the public reader's typed refusal.
///
/// # Panics
/// Panics if JSON encoding fails or the checker returns a different test outcome.
fn rejects(value: &Value, cause: Cause) {
    let bytes = serde_json::to_vec(value).expect("JSON encoding");
    let result = read(
        bytes.as_slice(),
        Limits::default(),
        &mut budget(100_000_000),
    );
    assert!(
        matches!(result, Err(Error::Invalid { cause: actual, .. }) if actual == cause),
        "expected {cause:?}, got {result:?}"
    );
}

/// The accepted sample supplies all five operations and endpoint evidence.
///
/// # Panics
/// Panics if fixture encoding or admission fails, or checked bytes change meaning.
/// ´claim:docgen:golden-admits-sample-evidence´
/// ´test:unit:golden-admits-sample-evidence´
#[test]
fn golden_admits_sample_evidence() {
    let value = fixture();
    let bytes = serde_json::to_vec(&value).expect("fixture serialization");
    let checked = read(
        bytes.as_slice(),
        Limits::default(),
        &mut budget(100_000_000),
    )
    .expect("accepted sample");
    assert_eq!(checked.value(), &value);
    let mut empty_sources = value;
    empty_sources["sources"] = json!([]);
    let bytes = serde_json::to_vec(&empty_sources).expect("empty source array encoding");
    assert!(
        read(
            bytes.as_slice(),
            Limits::default(),
            &mut budget(100_000_000)
        )
        .is_ok()
    );
}

/// Independent mutations distinguish all intersection and association failures.
///
/// # Panics
/// Panics if the fixture shape changes or a mutation has the wrong outcome.
/// ´claim:docgen:golden-refuses-evidence-mutations´
/// ´test:unit:golden-refuses-evidence-mutations´
#[test]
fn golden_refuses_evidence_mutations() {
    let mut value = fixture();
    value["entries"][0]["evidence"][0]["routes"][0]["interval"] = json!(["2", "3"]);
    rejects(&value, Cause::EmptyIntersection);
    let mut value = fixture();
    value["entries"][0]["golden"]["point"] = json!(["0", "1"]);
    rejects(&value, Cause::Intersection);
    let mut value = fixture();
    assert!(
        value["entries"][0]["evidence"][0]["routes"]
            .as_array_mut()
            .expect("route list")
            .pop()
            .is_some()
    );
    rejects(&value, Cause::Association);
    let mut value = fixture();
    value["entries"][0]["evidence"][0]["routes"][1] =
        value["entries"][0]["evidence"][0]["routes"][0].clone();
    rejects(&value, Cause::Association);
    let mut value = fixture();
    let mut stray = value["entries"][0]["evidence"][0].clone();
    stray["endpoint"] = json!("upper_endpoint");
    value["entries"][0]["evidence"]
        .as_array_mut()
        .expect("evidence list")
        .push(stray);
    rejects(&value, Cause::Association);
    let mut value = fixture();
    value["entries"][3]["evidence"]
        .as_array_mut()
        .expect("box evidence")
        .swap(0, 1);
    rejects(&value, Cause::Association);
    let mut value = fixture();
    let other = value["entries"][3]["evidence"][1]["input"].clone();
    value["entries"][3]["evidence"][0]["input"] = other;
    rejects(&value, Cause::Association);
    let mut value = fixture();
    value["routes"][0]["kind"] = json!("unreviewed-route");
    rejects(&value, Cause::RouteKind);
}

/// Every schema category has an isolated typed negative control.
///
/// # Panics
/// Panics if a mutation path disappears or the checker has the wrong outcome.
/// ´claim:docgen:golden-refuses-malformed-forms´
/// ´test:unit:golden-refuses-malformed-forms´
#[test]
fn golden_refuses_malformed_forms() {
    for (pointer, replacement, cause) in [
        ("/namespace", json!("other"), Cause::Namespace),
        ("/version", json!([1, 0, 1]), Cause::Version),
        ("/owner", json!("OTHER"), Cause::Envelope),
        ("/license", json!("AGPL-3.0-only"), Cause::Envelope),
        ("/sources", json!(["../escape"]), Cause::Envelope),
        ("/labels", json!(["not-a-label"]), Cause::Envelope),
        ("/routes/0/argument", json!(""), Cause::Routes),
        (
            "/entries/0/routes",
            json!(["mpmath-iv", "nrz-cas"]),
            Cause::Routes,
        ),
        ("/entries/1/id", json!("E1"), Cause::EntryId),
        ("/entries/0/operation", json!("log"), Cause::Operation),
        ("/entries/0/target/kind", json!("relative"), Cause::Target),
        (
            "/entries/6/expected/refusal",
            json!("resource"),
            Cause::Expected,
        ),
        ("/entries/3/input/box", json!(["2", "1"]), Cause::Ordering),
        (
            "/entries/0/golden/point",
            json!(["1", "0"]),
            Cause::Ordering,
        ),
    ] {
        let mut value = fixture();
        *value.pointer_mut(pointer).expect("mutation location") = replacement;
        rejects(&value, cause);
    }
    for spelling in [
        "-0", "+1", "01", "1/0", "2/4", "1/1", "0/2", "1.0", "1e0", "1/-2",
    ] {
        let mut value = fixture();
        value["entries"][0]["input"]["point"] = json!(spelling);
        rejects(&value, Cause::Rational);
    }
    let mut value = fixture();
    value["entries"][0]["extra"] = json!("unversioned extension");
    rejects(&value, Cause::Fields);
    let mut value = fixture();
    value["sources"] = json!(["same", "same"]);
    rejects(&value, Cause::Envelope);
}

/// Build one exact-value row to isolate width, zero and capacity controls.
///
/// # Panics
/// Panics only if the fixture no longer has the declared route array.
fn singleton(operation: &str, input: &str, output: &str) -> Value {
    let mut value = fixture();
    let mut row = value["entries"][2].clone();
    row["operation"] = json!(operation);
    row["input"]["point"] = json!(input);
    row["evidence"][0]["input"] = json!(input);
    row["golden"]["point"] = json!([output, output]);
    for route in row["evidence"][0]["routes"]
        .as_array_mut()
        .expect("route list")
    {
        route["interval"] = json!([output, output]);
    }
    if matches!(operation, "exp" | "sigma") {
        row["target"]["kind"] = json!("absolute");
    }
    if operation == "power" {
        row["target"] = json!({"kind":"outward"});
        row["exponent"] = json!("1/4");
    }
    value["entries"] = json!([row]);
    value
}

/// Exact promises and the inclusive width ceiling are checked independently.
///
/// # Panics
/// Panics if fixture encoding, shape or a declared checker outcome changes.
/// ´claim:docgen:golden-checks-exact-results-and-width´
/// ´test:unit:golden-checks-exact-results-and-width´
#[test]
fn golden_checks_exact_results_and_width() {
    let mut identity = singleton("power", "1/2", "1/2");
    identity["entries"][0]["exponent"] = json!("1");
    let bytes = serde_json::to_vec(&identity).expect("identity encoding");
    assert!(
        read(
            bytes.as_slice(),
            Limits::default(),
            &mut budget(100_000_000)
        )
        .is_ok()
    );
    for (operation, input, output) in [
        ("exp", "0", "1"),
        ("expm1", "0", "0"),
        ("tanh", "0", "0"),
        ("sigma", "0", "1/2"),
        ("power", "1", "1"),
    ] {
        let value = singleton(operation, input, output);
        let bytes = serde_json::to_vec(&value).expect("encoding");
        assert!(
            read(
                bytes.as_slice(),
                Limits::default(),
                &mut budget(100_000_000)
            )
            .is_ok()
        );
        rejects(&singleton(operation, input, "2"), Cause::Exact);
    }
    for (upper, accepted) in [
        ("1/1329227995784915872903807060280344576", true),
        ("1/664613997892457936451903530140172288", false),
    ] {
        let mut value = singleton("exp", "-1", "0");
        value["entries"][0]["golden"]["point"] = json!(["0", upper]);
        for route in value["entries"][0]["evidence"][0]["routes"]
            .as_array_mut()
            .expect("routes")
        {
            route["interval"] = json!(["0", upper]);
        }
        if accepted {
            let bytes = serde_json::to_vec(&value).expect("encoding");
            assert!(
                read(
                    bytes.as_slice(),
                    Limits::default(),
                    &mut budget(100_000_000)
                )
                .is_ok()
            );
        } else {
            rejects(&value, Cause::Width);
        }
    }
}

/// Exact-limit and one-below controls cover parsing and cumulative CAS Work.
///
/// # Panics
/// Panics if encoding or a declared resource-boundary outcome changes.
/// ´claim:docgen:golden-enforces-resource-boundaries´
/// ´test:unit:golden-enforces-resource-boundaries´
#[test]
fn golden_enforces_resource_boundaries() {
    let mut value = singleton("expm1", "0", "0");
    value["routes"][0]["argument"] = json!(r#"[[[{\"}]]]"#);
    let bytes = serde_json::to_vec(&value).expect("encoding");
    let limits = Limits {
        bytes: bytes.len(),
        entries: 1,
        literal_bytes: 1,
        target_bits: 40,
        depth: 8,
    };
    let mut ledger = budget(100_000_000);
    assert!(read(bytes.as_slice(), limits, &mut ledger).is_ok());
    let used = ledger.usage().work;
    assert!(read(bytes.as_slice(), limits, &mut budget(used)).is_ok());
    let mut short = budget(used - 1);
    bounds::work_refusal(&read(bytes.as_slice(), limits, &mut short), used - 1, used);
    let spent = short.usage().work;
    assert!(spent > 0 && spent < used);
    for (limits, cause) in [
        (Limits { depth: 7, ..limits }, Cause::Depth),
        (
            Limits {
                depth: 65,
                ..limits
            },
            Cause::Depth,
        ),
        (
            Limits {
                bytes: bytes.len() - 1,
                ..limits
            },
            Cause::Bytes,
        ),
        (
            Limits {
                entries: 0,
                ..limits
            },
            Cause::Entries,
        ),
        (
            Limits {
                literal_bytes: 0,
                ..limits
            },
            Cause::Literal,
        ),
        (
            Limits {
                target_bits: 39,
                ..limits
            },
            Cause::TargetBits,
        ),
    ] {
        assert!(
            matches!(read(bytes.as_slice(), limits, &mut budget(100_000_000)), Err(Error::Invalid { cause: actual, .. }) if actual == cause)
        );
    }
    let mut prior = budget(used);
    assert!(read(bytes.as_slice(), limits, &mut prior).is_ok());
    bounds::work_refusal(&read(bytes.as_slice(), limits, &mut prior), used, used + 1);
    assert_eq!(prior.usage().work, used);
}

/// Duplicate object keys, trailing data and excessive nesting never overwrite evidence.
///
/// # Panics
/// Panics if the checker admits one of these malformed JSON controls.
/// ´claim:docgen:golden-refuses-json-ambiguity´
/// ´test:unit:golden-refuses-json-ambiguity´
#[test]
fn golden_refuses_json_ambiguity() {
    for source in ["{\"x\":1,\"x\":2}".to_owned(), "{} {}".to_owned()] {
        assert!(matches!(
            read(
                source.as_bytes(),
                Limits::default(),
                &mut budget(100_000_000)
            ),
            Err(Error::Invalid {
                cause: Cause::Json,
                ..
            })
        ));
    }
    let deep = "[".repeat(65);
    assert!(matches!(
        read(deep.as_bytes(), Limits::default(), &mut budget(100_000_000)),
        Err(Error::Invalid {
            cause: Cause::Depth,
            ..
        })
    ));
}

/// Refusal fixtures carry the closed vocabulary without fabricated golden evidence.
///
/// # Panics
/// Panics if the fixture cannot be encoded or a declared refusal form is rejected.
/// ´claim:docgen:golden-admits-closed-refusal-forms´
/// ´test:unit:golden-admits-closed-refusal-forms´
#[test]
fn golden_admits_closed_refusal_forms() {
    for reason in [
        "outside-domain",
        "below-normal-operand",
        "non-dyadic-exponent",
        "target-not-certifiable",
    ] {
        let mut value = fixture();
        let mut row = value["entries"][6].clone();
        row["expected"]["refusal"] = json!(reason);
        if reason == "non-dyadic-exponent" {
            row["operation"] = json!("power");
            row["exponent"] = json!("1/3");
            row["target"] = json!({"kind":"outward"});
        }
        value["entries"] = json!([row]);
        let bytes = serde_json::to_vec(&value).expect("refusal encoding");
        assert!(
            read(
                bytes.as_slice(),
                Limits::default(),
                &mut budget(100_000_000)
            )
            .is_ok()
        );
        value["entries"][0]["golden"] = json!({"point":["0", "0"]});
        rejects(&value, Cause::Fields);
    }
}
