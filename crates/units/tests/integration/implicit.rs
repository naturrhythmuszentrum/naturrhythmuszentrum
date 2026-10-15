// SPDX-License-Identifier: AGPL-3.0-only
//! Public implicit recipes retain their defining relations and exact nominal output symbols.

use nrz_cas::limits::{Budget, Limits, LimitsOverride};
use nrz_units::{Descriptor, SensitivityObligation, SolveSymbols};

use crate::testing::declarations::{TestResult, context, input};
use crate::testing::sensitivity::{coefficient, equal_fraction};

/// The shared scalar operator example retains one input column across both output columns.
/// ´test:integration:implicit-solve-shared-rhs-lifecycle´
#[test]
fn implicit_solve_shared_rhs_lifecycle() -> TestResult {
    let mut builder = context(&["a", "b0", "b1", "x0", "x1"])?;
    for key in ["a", "b0", "b1", "x0", "x1"] {
        input(&mut builder, key, &Descriptor::ratio())?;
    }
    let symbols = SolveSymbols {
        order: 1,
        right_hand_sides: 2,
        operator: vec!["a".into()],
        right_hand_side: vec!["b0".into(), "b1".into()],
        solution: vec!["x0".into(), "x1".into()],
    };
    let sensitivity = builder.check_solve_sensitivity(&symbols, &LimitsOverride::default())?;
    assert_eq!(sensitivity.coordinates(), ["a", "b0", "b1"]);
    assert_eq!(sensitivity.auxiliary_symbols(), ["x0", "x1"]);
    assert_eq!(
        sensitivity.jacobian().coordinates(),
        ["x0", "x1", "a", "b0", "b1"]
    );
    let cas = sensitivity.declarations().cas()?;
    let expected = [2, 0, 0, 2, 1, 2, -1, 0, 0, -1];
    for (entry, value) in sensitivity.jacobian().entries().iter().zip(expected) {
        equal_fraction(
            coefficient(&cas, entry.expression(), &[("a", 2), ("x0", 1), ("x1", 2)])?,
            (value, 1),
        )?;
    }
    for relation in sensitivity.originals() {
        sensitivity.declarations().validate_checked(relation)?;
        cas.walk_original(
            &relation.original().root,
            &mut Budget::new(Limits::default())?,
        )?;
    }
    assert!(
        sensitivity
            .obligations()
            .contains(&SensitivityObligation::NominalSolution)
    );
    Ok(())
}

/// A root at zero has no certified derivative merely because its polynomial partials exist there.
/// ´test:integration:positive-root-zero-remains-a-domain-obligation´
#[test]
fn positive_root_zero_remains_a_domain_obligation() -> TestResult {
    let mut builder = context(&["x", "t"])?;
    input(&mut builder, "x", &Descriptor::ratio())?;
    input(&mut builder, "t", &Descriptor::ratio())?;
    let original = builder.parse("root", "sqrt(x)")?;
    let sensitivity = builder.check_root_sensitivity(
        &[original],
        &["x".into()],
        &["t".into()],
        &LimitsOverride::default(),
    )?;
    let cas = sensitivity.declarations().cas()?;
    let partial = sensitivity
        .jacobian()
        .entries()
        .first()
        .ok_or("missing root partial")?;
    equal_fraction(
        coefficient(&cas, partial.expression(), &[("t", 0), ("x", 0)])?,
        (0, 1),
    )?;
    assert_eq!(
        sensitivity.obligations(),
        [SensitivityObligation::PositiveRoot {
            coordinate: 0,
            output: 0
        }]
    );
    assert_eq!(sensitivity.originals().len(), 1);
    assert_eq!(sensitivity.jacobian().originals().len(), 1);
    assert_ne!(
        sensitivity
            .originals()
            .first()
            .ok_or("missing original")?
            .original()
            .syntax,
        sensitivity
            .jacobian()
            .originals()
            .first()
            .ok_or("missing relation")?
            .original()
            .syntax
    );
    Ok(())
}
