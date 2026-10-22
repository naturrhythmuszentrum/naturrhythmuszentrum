// SPDX-License-Identifier: AGPL-3.0-only
//! Real-symbol construction preserves the existing mathematical declaration route.

use nrz_bindings::{Context, Descriptor, ExponentPolicy, Failure, Registry};
use nrz_cas::limits::{Budget, Limits};

use crate::testing::declarations::{TestResult, context, input};

/// The entire initial mathematical graph, including builtins, matches the CAS route.
/// ´test:integration:real-symbol-declarations-match-cas-route´
#[test]
fn real_symbol_declarations_match_cas_route() -> TestResult {
    for keys in [&[][..], &["origin", "reference", "x"][..]] {
        let explicit = context(keys)?;
        let real = Context::with_real_symbols(
            Registry::new(ExponentPolicy::Rational),
            Limits::default(),
            keys,
        )?;
        let expected = explicit
            .cas()?
            .canonical_snapshot(&mut Budget::new(Limits::default())?)?;
        let actual = real
            .cas()?
            .canonical_snapshot(&mut Budget::new(Limits::default())?)?;
        assert_eq!(actual, expected);
        assert_ne!(real.snapshot_id(), explicit.snapshot_id());
    }
    Ok(())
}

/// Missing mathematical keys refuse parsing and cannot become frozen declarations.
/// ´test:integration:real-symbol-unknown-key-refuses´
#[test]
fn real_symbol_unknown_key_refuses() -> TestResult {
    let mut builder = Context::with_real_symbols(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        &["known"],
    )?;
    assert_eq!(
        builder
            .parse("unknown", "missing")
            .err()
            .ok_or("unknown key parsed")?
            .failure,
        Failure::UnknownSymbol("missing".into()),
    );
    input(&mut builder, "missing", &Descriptor::ratio())?;
    assert_eq!(
        builder.freeze().err().ok_or("unknown key frozen")?.failure,
        Failure::UnknownSymbol("missing".into()),
    );
    Ok(())
}

/// Duplicate keys and builtin collisions use the ordinary constructor's refusal boundary.
/// ´test:integration:real-symbol-key-collisions-refuse´
#[test]
fn real_symbol_key_collisions_refuse() {
    for keys in [&["x", "x"][..], &["sqrt"][..]] {
        assert!(
            Context::with_real_symbols(
                Registry::new(ExponentPolicy::Rational),
                Limits::default(),
                keys,
            )
            .is_err()
        );
    }
}

/// Physical checking retains the caller's limits after mathematical admission.
/// ´test:integration:real-symbol-check-limits-are-retained´
#[test]
fn real_symbol_check_limits_are_retained() -> TestResult {
    let builder = Context::with_real_symbols(
        Registry::new(ExponentPolicy::Rational),
        Limits {
            depth: 1,
            ..Limits::default()
        },
        &[],
    )?;
    let original = builder.parse("deep", "1+1")?;
    assert!(matches!(
        builder
            .check(&original)
            .err()
            .ok_or("depth limit ignored")?
            .failure,
        Failure::Algebra(nrz_cas::Fault {
            kind: nrz_cas::FaultKind::Exhausted {
                budget: nrz_cas::limits::BudgetKind::Depth,
                ..
            },
            ..
        }),
    ));
    Ok(())
}
