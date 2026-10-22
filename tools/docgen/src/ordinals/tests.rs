// SPDX-License-Identifier: AGPL-3.0-only

//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`ordinal_ordering_forms_refuse`] | docgen | Ordering vocabulary catches prose, slug and identifier forms with exact spans. |
//! | [`ordinal_heads_and_slices_refuse`] | docgen | Numeric heads, contextual LC ranges and CAS slice identifiers refuse. |
//! | [`ordinal_mathematical_lookalikes_pass`] | docgen | Physical symbols, literals, types and mathematical step indices stay valid. |

//! Positive and negative boundaries for the schedule-code grammar.

use super::find;

/// Ordering vocabulary catches prose, slug and identifier forms with exact spans.
/// ´claim:docgen:ordinal-ordering-forms-refuse´
/// ´test:unit:ordinal-ordering-forms-refuse´
#[test]
fn ordinal_ordering_forms_refuse() {
    for prefix in [
        "stage", "Stages", "Bite", "bites", "wave", "phase", "tranche", "round",
    ] {
        for (number, suffix) in [
            ("0", ""),
            ("1", "b"),
            ("3", "a"),
            ("3", "h"),
            ("5", ""),
            ("8", ""),
        ] {
            let ordinal = format!("{number}{suffix}");
            for separator in [" ", "-", "_", "–", "\t"] {
                let code = format!("{prefix}{separator}{ordinal}");
                let text = format!("planned {code} follows");
                assert_eq!(find(&text), Some(code.as_str()));
            }
        }
    }
    let suffix = "4";
    let code = format!("bite_{suffix}");
    assert_eq!(find(&format!("fn {code}_admission()")), Some(code.as_str()));
}

/// Numeric heads, contextual LC ranges and CAS slice identifiers refuse.
/// ´claim:docgen:ordinal-heads-and-slices-refuse´
/// ´test:unit:ordinal-heads-and-slices-refuse´
#[test]
fn ordinal_heads_and_slices_refuse() {
    for ordinal in [format!("3{}", "d"), "5".to_owned()] {
        let code = format!("Step ({ordinal}");
        assert_eq!(
            find(&format!("**{code}: Named work)**")),
            Some(code.as_str())
        );
    }
    let endpoints = [format!("3{}", "b"), format!("3{}", "h")];
    for separator in ["–", "-", "—"] {
        let code = format!("{}{}{}", endpoints[0], separator, endpoints[1]);
        assert_eq!(find(&format!("after {code}")), Some(code.as_str()));
        assert_eq!(
            find(&format!("- {code}: implementation")),
            Some(code.as_str())
        );
    }
    for ordinal in 0..=4 {
        let uppercase = format!("S{ordinal}");
        let identifier = format!("s{ordinal}");
        assert_eq!(find(&uppercase), Some(uppercase.as_str()));
        assert_eq!(
            find(&format!("{identifier}_check")),
            Some(identifier.as_str())
        );
    }
    for suffix in ["c", "h"] {
        let ordinal = format!("3{suffix}");
        assert_eq!(find(&format!("then {ordinal}")), Some(ordinal.as_str()));
    }
}

/// Physical symbols, literals, types and mathematical step indices stay valid.
/// ´claim:docgen:ordinal-mathematical-lookalikes-pass´
/// ´test:unit:ordinal-mathematical-lookalikes-pass´
#[test]
fn ordinal_mathematical_lookalikes_pass() {
    for text in [
        "R1 V3 U0 x0 v1 s0 C1 smoothness S=diag(5 V,1/10 A)",
        "1e-12 5e3 5.3e-11 f64 u32 binary64 rad/s v1.2.3",
        "2026-10-22 f44b812 1,534,599,168",
        "x=t-b/(3a); (3h²-2βh+γ)y; x_k=-b/(3a)+u",
        "s_p = a_p s0; U0 = J u²(s0) Jᵀ",
        "f=x^3-3x+1, derivative=3x^2-3, s2=2x-1, s3=9/4",
        "At elimination stage 53, row 54 becomes 2^53+1.",
        "LC phase is -pi/2 rad and positive roots have C1 regularity.",
        "stage threefold; phase 2.5; stage 12; round 2026",
        "two steps: (1) assert copyright; steps: 0; steps.first(); phase, zero",
        "Stage one proposed transition; a failed candidate never replaces the current store.",
        "A typical bite first establishes a usable contract.",
        "**Step (First Complex failure diagnosis)**; **Step (One checked answer)**",
        "version S42; source_s3; binary32; 895b18e4f66aac8d296ee2bf500bcb23245a7f25",
    ] {
        assert_eq!(find(text), None, "{text}");
    }
}
