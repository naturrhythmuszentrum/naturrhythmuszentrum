// SPDX-License-Identifier: AGPL-3.0-only
//! Implicit partial schedules retain the same exact finite replay boundary as native recipes.

use nrz_bindings::{CheckedSensitivity, Descriptor, Kind, Measure, Role, Scale, SolveSymbols};
use nrz_cas::{
    Rational,
    limits::{Budget, Limits, LimitsOverride},
};

use crate::testing::declarations::{TestResult, input};
use crate::testing::sensitivity::{context, generation_boundary, replay_boundary};

/// A scaled scalar relation exercises all three exact chain factors under independent replay limits.
fn scaled_solve(limits: &LimitsOverride) -> TestResult<CheckedSensitivity> {
    let mut builder = context(&["a", "b", "x"])?;
    for (key, denominator) in [("a", 2), ("b", 4), ("x", 10)] {
        let scale = Rational::from_ratio(1, denominator, &mut Budget::new(Limits::default())?)?;
        let descriptor = Descriptor::new(
            Kind::Ratio,
            Measure::new(key, Scale::Exact(scale), Rational::zero())?,
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
    Ok(builder.check_solve_sensitivity(&symbols, limits)?)
}

/// A two-by-two shared operator exercises the full 4-by-12 implicit partial schedule.
fn shared_solve(limits: &LimitsOverride) -> TestResult<CheckedSensitivity> {
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
    Ok(builder.check_solve_sensitivity(&symbols, limits)?)
}

/// The scaled implicit solve succeeds at its replay peak and refuses one unit below.
/// ´test:integration:implicit-sensitivity-replay-boundary´
#[test]
fn implicit_sensitivity_replay_boundary() -> TestResult {
    replay_boundary(scaled_solve)?;
    Ok(())
}

/// The shared table tests its binding generation phase at the exact success/refusal boundary.
/// ´test:integration:implicit-sensitivity-generation-boundary´
#[test]
fn implicit_sensitivity_generation_boundary() -> TestResult {
    generation_boundary(shared_solve)
}
