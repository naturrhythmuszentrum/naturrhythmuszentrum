// SPDX-License-Identifier: AGPL-3.0-only
//! A constants client needs only the bindings facade, with inferred default limits.

use nrz_bindings::{
    Context, Declaration, Descriptor, Evidence, ExponentPolicy, Kind, Limits, LimitsOverride,
    Measure, Registry, Role,
};

/// A bindings-only client declares exact references and origins, freezes and exports a map.
/// ´test:integration:real-symbol-bindings-only-constant-lifecycle´
#[test]
fn real_symbol_bindings_only_constant_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let point = Descriptor::new(Kind::Angle, Measure::coherent(), Role::Point)?;
    let mut builder = Context::with_real_symbols(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        &["reference", "origin"],
    )?;
    builder.declare(
        "reference".into(),
        Declaration::Constant {
            descriptor: point.difference(),
            value: point.measure().rational_scale()?.clone(),
            evidence: Evidence::default(),
        },
    )?;
    builder.declare(
        "origin".into(),
        Declaration::Constant {
            descriptor: point.clone(),
            value: point.measure().offset().clone(),
            evidence: Evidence::default(),
        },
    )?;
    let frozen = builder.freeze()?;
    assert_eq!(
        frozen.checked("reference")?.descriptor(),
        &point.difference()
    );
    assert_eq!(frozen.checked("origin")?.descriptor(), &point);
    let map = frozen.normalization_map(&point, "reference", Some("origin"))?;
    map.validate_snapshot(&frozen)?;
    assert_eq!(map.scale().export(&LimitsOverride::default())?, "1");
    assert_eq!(map.shift().export(&LimitsOverride::default())?, "0");
    Ok(())
}
