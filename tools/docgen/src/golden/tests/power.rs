// SPDX-License-Identifier: AGPL-3.0-only
//! Stored power rows isolate existing checker guards and their finite capacities.

use super::{Cause, Error, Limits, Value, budget, json, read, rejects};
use crate::golden::json::Audit;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Read the authored power corpus without adding another validator.
/// # Panics
/// Panics only if the committed dataset is not JSON.
fn payload() -> Value {
    serde_json::from_str(include_str!("../../../../../data/golden/dyadic-power.json"))
        .expect("power golden JSON")
}

/// Start each negative control with one admitted stored row.
/// # Panics
/// Panics if the named stored positive fixture disappears or ceases to admit.
fn single(id: &str) -> Value {
    let mut value = payload();
    let row = value["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|r| r["id"] == id)
        .expect("stored power row")
        .clone();
    value["entries"] = json!([row]);
    let bytes = serde_json::to_vec(&value).expect("positive encoding");
    assert!(
        read(
            bytes.as_slice(),
            Limits::default(),
            &mut budget(100_000_000)
        )
        .is_ok(),
        "stored positive row: {id}"
    );
    value
}

/// Stored power rows isolate original exponent and refusal compatibility guards.
/// # Panics
/// Panics if a changed exponent loses the exact Exponent refusal.
/// ´claim:docgen:power-golden-exponent-controls´
/// ´test:unit:power-golden-exponent-controls´
#[test]
fn power_golden_exponent_controls() {
    for q in ["0", "-1/4", "5/4", "1/3"] {
        let mut value = single("power-quarter");
        assert_eq!(
            value["entries"][0]["exponent"], "1/4",
            "stored quarter exponent"
        );
        value["entries"][0]["exponent"] = json!(q);
        rejects(&value, Cause::Exponent);
    }
    for (id, reason) in [
        ("power-non-dyadic", "outside-domain"),
        ("power-zero-exponent", "below-normal-operand"),
    ] {
        let mut value = single(id);
        value["entries"][0]["expected"]["refusal"] = json!(reason);
        rejects(&value, Cause::Exponent);
    }
    let mut value = single("power-non-dyadic");
    value["entries"][0]["exponent"] = json!("1/4");
    rejects(&value, Cause::Exponent);
}

/// Stored box evidence isolates endpoint input, order and ordered route associations.
/// # Panics
/// Panics if a changed association or route loses its named refusal.
/// ´claim:docgen:power-golden-association-controls´
/// ´test:unit:power-golden-association-controls´
#[test]
fn power_golden_association_controls() {
    let mut value = single("power-rounding-box");
    value["entries"][0]["evidence"][0]["endpoint"] = json!("upper_endpoint");
    rejects(&value, Cause::Association);
    let mut value = single("power-rounding-box");
    value["entries"][0]["evidence"][0]["input"] = value["entries"][0]["input"]["box"][1].clone();
    rejects(&value, Cause::Association);
    let mut value = single("power-rounding-box");
    value["entries"][0]["evidence"]
        .as_array_mut()
        .expect("evidence")
        .swap(0, 1);
    rejects(&value, Cause::Association);
    let mut value = single("power-rounding-box");
    value["entries"][0]["evidence"][0]["routes"]
        .as_array_mut()
        .expect("routes")
        .swap(0, 1);
    rejects(&value, Cause::Association);
    let mut value = single("power-rounding-box");
    value["entries"][0]["routes"]
        .as_array_mut()
        .expect("references")
        .swap(0, 1);
    rejects(&value, Cause::Routes);
    let mut value = single("power-rounding-box");
    value["routes"][0]["kind"] = json!("other");
    rejects(&value, Cause::RouteKind);
    let mut value = single("power-rounding-box");
    value["routes"][1]["argument"] = json!("");
    rejects(&value, Cause::Routes);
}

/// A published subset or merely overlapping interval cannot replace the exact intersection.
/// # Panics
/// Panics if one changed published endpoint or an altered exact promise admits.
/// ´claim:docgen:power-golden-intersection-controls´
/// ´test:unit:power-golden-intersection-controls´
#[test]
fn power_golden_intersection_controls() {
    for end in [0, 1] {
        let mut value = single("power-quarter");
        let pair = &value["entries"][0]["golden"]["point"];
        assert_ne!(pair[0], pair[1], "non-singleton power fixture");
        value["entries"][0]["golden"]["point"][end] = pair[1 - end].clone();
        rejects(&value, Cause::Intersection);
    }
    for id in ["power-one", "power-identity"] {
        let mut value = single(id);
        let wrong = if id == "power-one" { "2" } else { "1" };
        value["entries"][0]["golden"]["point"] = json!([wrong, wrong]);
        for route in value["entries"][0]["evidence"][0]["routes"]
            .as_array_mut()
            .expect("routes")
        {
            route["interval"] = json!([wrong, wrong]);
        }
        rejects(&value, Cause::Exact);
    }
    // A structural fixture checks selection from both routes; it makes no power claim.
    let denominator = 1_u128 << 123;
    let middle = format!("{}/{}", denominator + 1, denominator);
    let upper = format!("{}/{}", denominator / 2 + 1, denominator / 2);
    let outer = format!("{}/{}", denominator + 3, denominator);
    let mut value = single("power-quarter");
    value["entries"][0]["evidence"][0]["routes"][0]["interval"] = json!(["1", upper]);
    value["entries"][0]["evidence"][0]["routes"][1]["interval"] = json!([middle, outer]);
    value["entries"][0]["golden"]["point"] = json!([middle, upper]);
    let bytes = serde_json::to_vec(&value).expect("structural encoding");
    assert!(
        read(
            bytes.as_slice(),
            Limits::default(),
            &mut budget(100_000_000)
        )
        .is_ok(),
        "non-nested exact intersection"
    );
    for (end, wrong) in [(0, "1".to_owned()), (1, outer)] {
        let mut changed = value.clone();
        changed["entries"][0]["golden"]["point"][end] = json!(wrong);
        rejects(&changed, Cause::Intersection);
    }
}

/// Power exponent encoding exhausts as a resource before allocating the JSON tree.
/// # Panics
/// Panics if the exact literal boundary, typed refusal or preallocation boundary changes.
/// ´claim:docgen:power-golden-encoding-controls´
/// ´test:unit:power-golden-encoding-controls´
#[test]
fn power_golden_encoding_controls() -> TestResult {
    let mut value = single("power-one");
    let bytes = serde_json::to_vec(&value)?;
    for (ceiling, admitted) in [(3, true), (2, false)] {
        let limits = Limits {
            literal_bytes: ceiling,
            ..Limits::default()
        };
        let mut audit = Audit::default();
        let result = crate::golden::json::parse(&bytes, limits, &mut audit);
        if admitted {
            assert!(result.is_ok(), "exact exponent encoding bound");
            assert_eq!(audit.tree_decodes, 1);
        } else {
            assert!(
                matches!(
                    result,
                    Err(Error::Invalid {
                        cause: Cause::Literal,
                        ..
                    })
                ),
                "one-below exponent encoding bound"
            );
            assert_eq!(
                audit.tree_decodes, 0,
                "power preflight precedes tree allocation"
            );
        }
    }
    value["entries"][0]["exponent"] = json!("1/18446744073709551616");
    let bytes = serde_json::to_vec(&value)?;
    assert!(
        read(
            bytes.as_slice(),
            Limits::default(),
            &mut budget(100_000_000)
        )
        .is_ok(),
        "wide exact dyadic exponent"
    );
    value["entries"][0]["exponent"] = json!(format!("1/{}", "7".repeat(500_000)));
    let bytes = serde_json::to_vec(&value)?;
    let mut audit = Audit::default();
    let result = crate::golden::json::parse(&bytes, Limits::default(), &mut audit);
    assert!(
        matches!(
            result,
            Err(Error::Invalid {
                cause: Cause::Literal,
                ..
            })
        ),
        "oversized exponent cause"
    );
    assert_eq!(audit.tree_decodes, 0, "oversized exponent preallocation");
    Ok(())
}

/// Complete row admission derives cost maxima from the checker ledger and preflight inventory.
/// # Panics
/// Panics if an authored row exceeds the existing per-entry policy or a pinned maximum changes.
/// ´claim:docgen:power-golden-costs´
/// ´test:unit:power-golden-costs´
#[test]
fn power_golden_costs() -> TestResult {
    use std::io::Write;
    let all = payload();
    let mut maxima = [0_u64; 3];
    for row in all["entries"].as_array().ok_or("entries")? {
        let mut one = all.clone();
        one["entries"] = json!([row]);
        let bytes = serde_json::to_vec(&one)?;
        let mut ledger = nrz_cas::limits::Budget::new(crate::golden::checker_limits())?;
        let result = read(bytes.as_slice(), Limits::default(), &mut ledger);
        assert!(
            result.is_ok(),
            "power profile admission {}: {result:?}",
            row["id"]
        );
        let mut audit = Audit::default();
        drop(crate::golden::json::parse(
            &bytes,
            Limits::default(),
            &mut audit,
        )?);
        let costs = [
            ledger.usage().work,
            u64::try_from(audit.inventory.largest_literal)?,
            u64::try_from(audit.inventory.literal_bytes)?,
        ];
        for (maximum, cost) in maxima.iter_mut().zip(costs) {
            *maximum = (*maximum).max(cost);
        }
        writeln!(std::io::stdout(), "POWER COST {} {costs:?}", row["id"])?;
    }
    assert_eq!(maxima, [3_937_691, 388, 2_285], "power checker maxima");
    assert!(
        maxima[0] <= crate::golden::checker_limits().work,
        "power Work margin"
    );
    assert!(
        maxima[1] <= crate::golden::checker_limits().literal_bytes,
        "power literal margin"
    );
    writeln!(std::io::stdout(), "POWER MAXIMA {maxima:?}")?;
    Ok(())
}
