// SPDX-License-Identifier: AGPL-3.0-only
//! Implicit recipes expose polynomial evidence and retain their numerical premises.

use nrz_bindings::{
    Descriptor, ExponentPolicy, Kind, Measure, Registry, Role, SensitivityError, SensitivityKernel,
    SensitivityObligation, SensitivityRecipe, SolveSymbols,
};
use nrz_cas::{
    Rational,
    limits::{Budget, Limits},
};

use crate::testing::declarations::{TestResult, input, length};
use crate::testing::sensitivity::{
    coefficient, context, context_with_registry, equal_fraction, request_limits,
};

/// Two positive roots retain their polynomial partials, order and unresolved branch conditions.
/// ´test:integration:positive-root-polynomial-partials´
#[test]
fn positive_root_polynomial_partials() -> TestResult {
    let mut builder = context(&["x", "y", "u", "v"])?;
    for key in ["x", "y", "u", "v"] {
        input(&mut builder, key, &Descriptor::ratio())?;
    }
    let originals = [
        builder.parse("first root", "sqrt(x)")?,
        builder.parse("second root", "sqrt(y)")?,
    ];
    let checked = builder.check_root_sensitivity(
        &originals,
        &["y".into(), "x".into()],
        &["u".into(), "v".into()],
        &request_limits(),
    )?;
    assert_eq!(checked.recipe(), SensitivityRecipe::PositiveRoot);
    assert_eq!(checked.auxiliary_symbols(), ["u", "v"]);
    assert_eq!(checked.jacobian().coordinates(), ["u", "v", "y", "x"]);
    assert_eq!(
        checked
            .entries()
            .iter()
            .map(nrz_bindings::SensitivityEntry::kernel)
            .collect::<Vec<_>>(),
        [
            SensitivityKernel::Zero,
            SensitivityKernel::PositiveRoot {
                coordinate: 0,
                output: 1
            },
            SensitivityKernel::PositiveRoot {
                coordinate: 1,
                output: 0
            },
            SensitivityKernel::Zero
        ]
    );
    assert_eq!(
        checked.obligations(),
        [
            SensitivityObligation::PositiveRoot {
                coordinate: 1,
                output: 0
            },
            SensitivityObligation::PositiveRoot {
                coordinate: 0,
                output: 1
            }
        ]
    );
    let cas = checked.declarations().cas()?;
    for (entry, expected) in checked
        .jacobian()
        .entries()
        .iter()
        .zip([6, 0, 0, 10, 0, -1, -1, 0])
    {
        equal_fraction(
            coefficient(
                &cas,
                entry.expression(),
                &[("u", 3), ("v", 5), ("x", 9), ("y", 25)],
            )?,
            (expected, 1),
        )?;
    }
    for witness in checked.jacobian().witnesses() {
        assert_eq!(witness.hypotheses(), []);
    }
    Ok(())
}

/// Physical root admission requires the explicit root and reverse-square semantic relations.
/// ´test:integration:positive-root-kind-admission-twins´
#[test]
fn positive_root_kind_admission_twins() -> TestResult {
    for registered in [false, true] {
        let distance = length()?;
        let mut ledger = Budget::new(Limits::default())?;
        let two = Rational::from_ratio(2, 1, &mut ledger)?;
        let half = Rational::from_ratio(1, 2, &mut ledger)?;
        let area = Kind::Named(
            "area".into(),
            Box::new(distance.dimension().power(&two, ExponentPolicy::Rational)?),
        );
        let mut registry = Registry::new(ExponentPolicy::Rational);
        registry.register_power(distance.kind().clone(), two, area.clone())?;
        if registered {
            registry.register_power(area.clone(), half, distance.kind().clone())?;
        }
        let mut builder = context_with_registry(registry, &["x", "t"])?;
        input(
            &mut builder,
            "x",
            &Descriptor::new(area, Measure::coherent(), Role::Difference)?,
        )?;
        input(&mut builder, "t", &distance)?;
        let original = builder.parse("root", "sqrt(x)")?;
        let result = builder.check_root_sensitivity(
            &[original],
            &["x".into()],
            &["t".into()],
            &request_limits(),
        );
        if registered {
            let checked = result?;
            assert_eq!(checked.outputs(), [distance]);
            assert_eq!(
                checked
                    .entries()
                    .first()
                    .ok_or("missing root quotient")?
                    .pairing()
                    .dimension(),
                &checked
                    .outputs()
                    .first()
                    .ok_or("missing output")?
                    .dimension()
                    .quotient(checked.inputs().first().ok_or("missing input")?.dimension())?
            );
        } else {
            assert!(matches!(result, Err(SensitivityError::Original(_))));
        }
    }
    Ok(())
}

/// Equal dimensions cannot supply an unrelated output kind or an overlapping implicit symbol.
/// ´test:integration:positive-root-placeholder-refusals´
#[test]
fn positive_root_placeholder_refusals() -> TestResult {
    let mut builder = context(&["x", "t"])?;
    input(&mut builder, "x", &Descriptor::ratio())?;
    let angle = Descriptor::new(Kind::Angle, Measure::coherent(), Role::Difference)?;
    input(&mut builder, "t", &angle)?;
    let original = builder.parse("root", "sqrt(x)")?;
    assert!(matches!(
        builder.check_root_sensitivity(
            &[original],
            &["x".into()],
            &["t".into()],
            &request_limits()
        ),
        Err(SensitivityError::Units(
            nrz_bindings::UnitError::KindMismatch
        ))
    ));
    let mut alias = context(&["x"])?;
    input(&mut alias, "x", &Descriptor::ratio())?;
    let original = alias.parse("root", "sqrt(x)")?;
    assert!(matches!(
        alias.check_root_sensitivity(&[original], &["x".into()], &["x".into()], &request_limits()),
        Err(SensitivityError::Coordinates)
    ));
    Ok(())
}

/// A two-by-two operator and two RHS columns have the independently derived 4-by-12 partial table.
/// ´test:integration:solve-entrywise-partials-and-common-operator´
#[test]
fn solve_entrywise_partials_and_common_operator() -> TestResult {
    let keys = [
        "a00", "a10", "a01", "a11", "b00", "b10", "b01", "b11", "x00", "x10", "x01", "x11",
    ];
    let mut builder = context(&keys)?;
    for key in keys {
        input(&mut builder, key, &Descriptor::ratio())?;
    }
    let symbols = SolveSymbols {
        order: 2,
        right_hand_sides: 2,
        operator: ["a00", "a10", "a01", "a11"].map(str::to_owned).to_vec(),
        right_hand_side: ["b00", "b10", "b01", "b11"].map(str::to_owned).to_vec(),
        solution: ["x00", "x10", "x01", "x11"].map(str::to_owned).to_vec(),
    };
    let checked = builder.check_solve_sensitivity(&symbols, &request_limits())?;
    assert_eq!(checked.coordinates().len(), 8);
    assert_eq!(checked.outputs().len(), 4);
    assert_eq!(checked.entries().len(), 32);
    assert_eq!(
        checked.obligations(),
        [
            SensitivityObligation::Invertible,
            SensitivityObligation::NominalSolution
        ]
    );
    let expected = [
        2, 3, 0, 0, 5, 7, 0, 0, 0, 0, 2, 3, 0, 0, 5, 7, 11, 0, 17, 0, 0, 11, 0, 17, 13, 0, 19, 0,
        0, 13, 0, 19, -1, 0, 0, 0, 0, -1, 0, 0, 0, 0, -1, 0, 0, 0, 0, -1,
    ];
    let point = [
        ("a00", 2),
        ("a10", 3),
        ("a01", 5),
        ("a11", 7),
        ("x00", 11),
        ("x10", 13),
        ("x01", 17),
        ("x11", 19),
    ];
    let cas = checked.declarations().cas()?;
    assert_eq!(checked.jacobian().entries().len(), expected.len());
    for (entry, value) in checked.jacobian().entries().iter().zip(expected) {
        equal_fraction(coefficient(&cas, entry.expression(), &point)?, (value, 1))?;
    }
    assert_eq!(
        checked
            .entries()
            .get(3)
            .ok_or("missing shared operator column")?
            .kernel(),
        SensitivityKernel::Solve {
            coordinate: 0,
            output: 3
        }
    );
    Ok(())
}

/// Empty, overflowing, malformed and aliased solve inventories never publish partial proofs.
/// ´test:integration:solve-sensitivity-shape-refusals´
#[test]
fn solve_sensitivity_shape_refusals() -> TestResult {
    for (order, columns, duplicate) in [
        (0, 1, false),
        (1, 0, false),
        (usize::MAX, 2, false),
        (2, 1, false),
        (1, 1, true),
    ] {
        let mut builder = context(&["a", "b", "x"])?;
        for key in ["a", "b", "x"] {
            input(&mut builder, key, &Descriptor::ratio())?;
        }
        let symbols = SolveSymbols {
            order,
            right_hand_sides: columns,
            operator: vec!["a".into()],
            right_hand_side: vec!["b".into()],
            solution: vec![if duplicate { "a".into() } else { "x".into() }],
        };
        assert!(
            builder
                .check_solve_sensitivity(&symbols, &request_limits())
                .is_err()
        );
    }
    Ok(())
}

/// Native implicit recipes have dedicated constructors, never a fallback to explicit scalar lowering.
/// ´test:integration:implicit-recipe-dispatch-refusal´
#[test]
fn implicit_recipe_dispatch_refusal() -> TestResult {
    for recipe in [
        SensitivityRecipe::PositiveRoot,
        SensitivityRecipe::Solve {
            order: 1,
            right_hand_sides: 1,
        },
    ] {
        let mut builder = context(&["x"])?;
        input(&mut builder, "x", &Descriptor::ratio())?;
        let original = builder.parse("square", "x^2")?;
        assert!(matches!(
            builder.check_sensitivity(recipe, &[original], &["x".into()], &request_limits()),
            Err(SensitivityError::UnsupportedRecipe)
        ));
    }
    Ok(())
}

/// Noncoherent placeholder measures cancel only through the checked implicit relation.
/// ´test:integration:implicit-sensitivity-measure-chain-factors´
#[test]
fn implicit_sensitivity_measure_chain_factors() -> TestResult {
    let mut builder = context(&["a", "b", "x"])?;
    for (key, denominator) in [("a", 2), ("b", 4), ("x", 10)] {
        let descriptor = Descriptor::new(
            Kind::Ratio,
            Measure::new(
                key,
                nrz_bindings::Scale::Exact(Rational::from_ratio(
                    1,
                    denominator,
                    &mut Budget::new(Limits::default())?,
                )?),
                Rational::zero(),
            )?,
            Role::Difference,
        )?;
        input(&mut builder, key, &descriptor)?;
    }
    let symbols = SolveSymbols {
        order: 1,
        right_hand_sides: 1,
        operator: vec!["a".into()],
        right_hand_side: vec!["b".into()],
        solution: vec!["x".into()],
    };
    let checked = builder.check_solve_sensitivity(&symbols, &request_limits())?;
    let cas = checked.declarations().cas()?;
    for (entry, expected) in checked
        .jacobian()
        .entries()
        .iter()
        .zip([(1, 5), (1, 1), (-1, 4)])
    {
        equal_fraction(
            coefficient(&cas, entry.expression(), &[("a", 4), ("b", 16), ("x", 20)])?,
            expected,
        )?;
    }
    assert_eq!(checked.outputs(), [Descriptor::ratio()]);
    Ok(())
}
