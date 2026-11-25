// SPDX-License-Identifier: AGPL-3.0-only

//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`joined_process_identifiers_refuse`] | docgen | Joined scheduling nouns with identifiers refuse and preserve spelling. |
//! | [`numbered_records_and_relative_pointers_refuse`] | docgen | Numbered process records, diagnostic references and relative pointers refuse. |
//! | [`product_and_measurement_names_pass`] | docgen | Product codes, physical notation, dates and generic nouns pass. |

//! Boundaries for names that identify the working process.

use super::find;

/// Joined scheduling nouns with identifiers refuse and preserve spelling.
/// ´claim:docgen:joined-process-identifiers-refuse´
/// ´test:unit:joined-process-identifiers-refuse´
#[test]
fn joined_process_identifiers_refuse() {
    for noun in ["lane", "run", "round", "step", "gate", "candidate"] {
        for join in ["-", "_"] {
            let code = format!("{noun}{join}42-alpha");
            assert_eq!(find(&format!("work on {code} ended")), Some(code.as_str()));
        }
    }
    let code = format!("{}-{}", "Lane", "Blue7");
    assert_eq!(find(&format!("{code}.txt")), Some(code.as_str()));
}

/// Numbered process records, diagnostic references and relative pointers refuse.
/// ´claim:docgen:numbered-records-and-relative-pointers-refuse´
/// ´test:unit:numbered-records-and-relative-pointers-refuse´
#[test]
fn numbered_records_and_relative_pointers_refuse() {
    for noun in ["review finding", "diagnostic run"] {
        let code = format!("{noun} #B42");
        assert_eq!(find(&format!("see {code}.")), Some(code.as_str()));
    }
    for (noun, separator) in [("diagnostic", " "), ("diag", "-")] {
        let code = format!("{noun}{separator}5");
        assert_eq!(find(&format!("from {code}.")), Some(code.as_str()));
    }
    let range = format!("{} {}–{}", "Diagnostics", "2", "3");
    assert_eq!(
        find(&format!("{range} measured costs")),
        Some(range.as_str())
    );
    let pointer = format!("{} {}", "previous", "sweep");
    assert_eq!(
        find(&format!("after the {pointer}.")),
        Some(pointer.as_str())
    );
}

/// Product codes, physical notation, dates and generic nouns pass.
/// ´claim:docgen:product-and-measurement-names-pass´
/// ´test:unit:product-and-measurement-names-pass´
#[test]
fn product_and_measurement_names_pass() {
    for text in [
        "diagnostic code E42 and fixture resonance-42",
        "round_trip_f64 run_with_policy_v2 candidate_root_1",
        "round_to_f32 dry-run-v2 step-response-2nd-order",
        "lane_0 lane_1 run_3d candidate_2x2 gate_1",
        "run-v2 step-v2",
        "diagnostic 2.5 is a decimal measurement",
        "previous round in the algorithm; previous lane in SIMD prose",
        "candidate 56 bytes, gate 3 samples, run 2026-11-25",
        "f64 u32 v1.2.3 R1 C1 x_0 10 µF 2000 rad/s",
        "phase 12 and step 53 of the algorithm",
        "a previous measurement at commit abc123",
        "review finding explains an error without a numbered record",
    ] {
        assert_eq!(find(text), None, "{text}");
    }
    let coordinate_step = format!("{}_2d", "step");
    assert_eq!(find(&coordinate_step), None);
}
