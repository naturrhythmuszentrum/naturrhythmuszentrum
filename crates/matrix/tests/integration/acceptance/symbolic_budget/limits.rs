// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit caller policy brackets the same full ordered symbolic solve inventory.

use super::super::support::ResultOf;
use nrz_bindings::readings::algebra::{
    Fault, FaultKind,
    limits::BudgetKind,
    request::Outcome,
    syntax::{AtomicSort, Declaration as Mathematical, Environment, Field, Sort},
};
use nrz_bindings::{
    CheckedSensitivity, Context, Declaration, Descriptor, Evidence, ExponentPolicy, Limits,
    LimitsOverride, Registry, SensitivityError, SolveSymbols,
};

/// The full symbolic ladder obeys exact construction/generation and replay boundaries.
/// ´test:integration:full-ladder-symbolic-work-has-explicit-boundary´
#[test]
fn full_ladder_symbolic_work_has_explicit_boundary() -> ResultOf {
    let baseline = lower(16_000_000, 16_000_000)?.jacobian().work();
    let needed = 4_916_585;
    assert_eq!(
        lower(needed, 16_000_000)?.jacobian().work().requests,
        baseline.requests
    );
    refusal(
        lower(needed - 1, 16_000_000)
            .err()
            .ok_or("one-below Work admitted")?,
        needed - 1,
    )?;
    assert_eq!(
        lower(16_000_000, baseline.replay)?.jacobian().work(),
        baseline
    );
    refusal(
        lower(16_000_000, baseline.replay - 1)
            .err()
            .ok_or("one-below replay admitted")?,
        baseline.replay - 1,
    )?;
    assert_eq!(
        (
            needed,
            baseline.construction,
            baseline.generation,
            baseline.replay
        ),
        (4_916_585, 1_703_795, 4_921_637, 2_116_155)
    );
    Ok(())
}

/// Semantic failures and a different limit cannot masquerade as the measured boundary.
fn refusal(error: Box<dyn std::error::Error>, cap: u64) -> ResultOf<Fault> {
    let fault = match error.downcast_ref::<SensitivityError>() {
        Some(SensitivityError::Algebra(fault)) => fault,
        Some(SensitivityError::Evidence(outcome)) => {
            let Outcome::Exhausted { fault, .. } = outcome.as_ref() else {
                return Err("expected symbolic exhaustion".into());
            };
            fault
        }
        _ => return Err(error),
    };
    assert!(
        matches!(fault.kind, FaultKind::Exhausted { budget: BudgetKind::Work, limit, .. } if limit == cap)
    );
    Ok(*fault)
}

/// Rebuild all 35 ordered symbols through public declarations without cross-crate fixtures.
fn lower(work: u64, replay: u64) -> ResultOf<CheckedSensitivity> {
    let symbols = SolveSymbols {
        order: 5,
        right_hand_sides: 1,
        operator: (0..25).map(|index| format!("a{index}")).collect(),
        right_hand_side: (0..5).map(|index| format!("b{index}")).collect(),
        solution: (0..5).map(|index| format!("x{index}")).collect(),
    };
    let keys = symbols
        .operator
        .iter()
        .chain(&symbols.right_hand_side)
        .chain(&symbols.solution);
    let environment = Environment {
        declarations: keys
            .clone()
            .map(|key| Mathematical {
                key: key.clone(),
                display_name: key.clone(),
                sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            })
            .collect(),
        ..Environment::default()
    };
    let ceilings = Limits {
        work: 16_000_000,
        replay_work: 16_000_000,
        ..Limits::default()
    };
    let mut builder = Context::with_cas_limits(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        environment,
        ceilings,
    )?;
    for key in keys {
        builder.declare(
            key.clone(),
            Declaration::Input {
                descriptor: Descriptor::ratio(),
                nonzero: false,
                evidence: Evidence {
                    source: key.clone(),
                    assurance: "declared".into(),
                },
            },
        )?;
    }
    Ok(builder.check_solve_sensitivity(
        &symbols,
        &LimitsOverride {
            work: Some(work),
            replay_work: Some(replay),
            ..LimitsOverride::default()
        },
    )?)
}
