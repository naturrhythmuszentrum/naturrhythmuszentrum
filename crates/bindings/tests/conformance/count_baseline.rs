// SPDX-License-Identifier: AGPL-3.0-only
//! Existing exact count declarations retain their physical domain without a decision.

use crate::testing::declarations::{TestResult, context};
use nrz_bindings::{
    CountDomain, Declaration, Descriptor, Evidence, Kind, MapError, Measure, ReferenceDefinition,
    Role, UnitError,
};
use nrz_cas::{
    Rational,
    limits::{Budget, Limits},
};

/// A data-declared cycle count is exact and checked without any numerical selection.
/// ´test:integration:bindings-declared-cycle-count-baseline´
#[test]
fn bindings_declared_cycle_count_baseline() -> TestResult {
    let count = Descriptor::new(
        Kind::Count("complete_lc_cycles".into(), CountDomain::NONNEGATIVE),
        Measure::coherent(),
        Role::Difference,
    )?;
    let mut builder = context(&["cycles"])?;
    let value = Rational::from_ratio(3, 1, &mut Budget::new(Limits::default())?)?;
    builder.declare(
        "cycles".into(),
        Declaration::Constant {
            descriptor: count.clone(),
            value: value.clone(),
            evidence: Evidence {
                source: "declared cycle inventory".into(),
                assurance: "exact data".into(),
            },
        },
    )?;
    let owner = builder.freeze()?;
    assert_eq!(owner.checked("cycles")?.descriptor(), &count);
    let Some(Declaration::Constant {
        value: actual,
        evidence,
        ..
    }) = owner.declarations().get("cycles")
    else {
        return Err("missing exact count declaration".into());
    };
    assert_eq!(actual, &value);
    assert_eq!(evidence.source, "declared cycle inventory");
    assert_eq!(
        count.compatible_kind(&Descriptor::ratio()),
        Err(UnitError::KindMismatch)
    );
    assert_eq!(
        ReferenceDefinition::unit("cycle reference", &count),
        Err(MapError::Units(UnitError::CountDomain))
    );
    Ok(())
}

/// Fractional and out-of-domain counts refuse before changing declaration identity.
/// ´test:integration:bindings-cycle-count-domain-baseline´
#[test]
fn bindings_cycle_count_domain_baseline() -> TestResult {
    let count = Descriptor::new(
        Kind::Count(
            "complete_lc_cycles".into(),
            CountDomain::new(Some(0), Some(3))?,
        ),
        Measure::coherent(),
        Role::Difference,
    )?;
    let mut builder = context(&["cycles"])?;
    let original = builder.snapshot_id();
    for spelling in ["51/16", "-1", "4"] {
        let value = Rational::parse(spelling, &mut Budget::new(Limits::default())?)?;
        let fault = builder
            .declare(
                "cycles".into(),
                Declaration::Constant {
                    descriptor: count.clone(),
                    value,
                    evidence: Evidence::default(),
                },
            )
            .err()
            .ok_or("invalid count declared")?;
        assert_eq!(fault.failure, UnitError::CountValue.into());
        assert_eq!(builder.snapshot_id(), original);
    }
    Ok(())
}
