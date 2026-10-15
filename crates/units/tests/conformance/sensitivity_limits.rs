// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit symbolic service allowances remain bounded independently of store ceilings.

use nrz_cas::{
    limits::{Limits, LimitsOverride},
    syntax::{AtomicSort, Declaration as Mathematical, Environment, Field, Sort},
};
use nrz_units::{
    CheckedSensitivity, Context, Descriptor, ExponentPolicy, Registry, SensitivityError,
    SensitivityRecipe,
};

use crate::testing::declarations::{TestResult, input};
use crate::testing::sensitivity::{context, replay_boundary, request_limits};

/// The same ordered native fixture is reconstructed for each independent request allowance.
fn native(recipe: SensitivityRecipe, limits: &LimitsOverride) -> TestResult<CheckedSensitivity> {
    let mut builder = context(&["x", "y", "z"])?;
    for key in ["x", "y", "z"] {
        input(&mut builder, key, &Descriptor::ratio())?;
    }
    let [first, second] = if recipe == SensitivityRecipe::Square {
        ["x^2", "y^2"]
    } else {
        ["1/x", "1/y"]
    };
    let originals = [
        builder.parse("first", first)?,
        builder.parse("second", second)?,
    ];
    Ok(builder.check_sensitivity(
        recipe,
        &originals,
        &["y".into(), "z".into(), "x".into()],
        limits,
    )?)
}

/// Square and reciprocal replay succeed at the measured work limit and refuse one unit below it.
/// ´test:integration:native-sensitivity-replay-boundary´
#[test]
fn native_sensitivity_replay_boundary() -> TestResult {
    for recipe in [SensitivityRecipe::Square, SensitivityRecipe::Reciprocal] {
        replay_boundary(|limits| native(recipe, limits))?;
    }
    Ok(())
}

/// A larger request override cannot bypass the explicitly smaller permanent store ceiling.
/// ´test:integration:sensitivity-request-obeys-store-ceiling´
#[test]
fn sensitivity_request_obeys_store_ceiling() -> TestResult {
    let environment = Environment {
        declarations: vec![Mathematical {
            key: "x".into(),
            display_name: "x".into(),
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
        }],
        ..Environment::default()
    };
    let ceilings = Limits {
        replay_work: 1,
        ..Limits::default()
    };
    let mut builder = Context::with_cas_limits(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        environment,
        ceilings,
    )?;
    input(&mut builder, "x", &Descriptor::ratio())?;
    let original = builder.parse("square", "x^2")?;
    let error = builder
        .check_sensitivity(
            SensitivityRecipe::Square,
            &[original],
            &["x".into()],
            &request_limits(),
        )
        .err()
        .ok_or("store ceiling bypassed")?;
    let SensitivityError::Evidence(outcome) = error else {
        return Err("missing exhausted request".into());
    };
    assert!(matches!(
        *outcome,
        nrz_cas::request::Outcome::Exhausted {
            fault: nrz_cas::Fault {
                kind: nrz_cas::FaultKind::Exhausted { limit: 1, .. },
                ..
            },
            ..
        }
    ));
    Ok(())
}
