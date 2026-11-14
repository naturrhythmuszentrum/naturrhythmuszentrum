// SPDX-License-Identifier: AGPL-3.0-only
//! Recipe conformance across every promised source specialization, without timing or machine-code claims.

use super::*;

/// Explicit arithmetic-only recipe, not an instrument default.
fn case(operation: Operation, width: Width, dependency: Dependency) -> CaseRequest {
    CaseRequest {
        operation,
        width,
        dependency,
        scope: crate::Scope::Prepared,
        shape: Vec::new(),
        item_length: 257,
        workers: 1,
        placement: crate::Placement::Unbound,
        allowed_cpus: Vec::new(),
        budget_ns: 1_000_000,
        diagonal_margin: None,
        inputs: Vec::new(),
        cycles: Vec::new(),
    }
}

#[test]
fn every_arithmetic_specialization_accepts_the_declared_finite_cycle() -> Result<(), RecipeError> {
    for operation in [
        Operation::Add,
        Operation::Multiply,
        Operation::Divide,
        Operation::FmaAddend,
        Operation::FmaMultiplicand,
        Operation::MultiplyAdd,
        Operation::SquareRoot,
    ] {
        for width in [
            Width::Scalar,
            Width::Simd1,
            Width::Simd2,
            Width::Simd4,
            Width::Simd8,
        ] {
            for dependency in [Dependency::Dependent, Dependency::Independent] {
                let kernel = Arithmetic::prepare(&case(operation, width, dependency), Seed(1))?;
                kernel.validate(256)?;
                kernel.validate(257)?;
            }
        }
    }
    Ok(())
}

#[test]
fn active_seed_positions_are_accumulator_major_without_padding_gaps() -> Result<(), RecipeError> {
    let kernel = Arithmetic::prepare(
        &case(Operation::Add, Width::Simd2, Dependency::Independent),
        Seed(0),
    )?;
    let retained = kernel.inputs()?;
    assert_eq!(retained.states.len(), 16);
    assert_eq!(retained.pair[0].len(), 16);
    assert_eq!(retained.pair[1].len(), 16);
    assert_eq!(retained.tuple_fields, 1);
    // x_0=1, x_4=1+32764/2^20, x_8=1+65528/2^20 from the design generator.
    assert_eq!(retained.states[0].0, 1.0_f64.to_bits());
    assert_eq!(
        retained.states[1].0,
        (1.0_f64 + 32764.0 / 1_048_576.0).to_bits()
    );
    assert_eq!(
        retained.states[2].0,
        (1.0_f64 + 65528.0 / 1_048_576.0).to_bits()
    );
    for (positive, negative) in retained.pair[0].iter().zip(&retained.pair[1]) {
        assert_eq!(positive.0 ^ (1_u64 << 63), negative.0);
    }
    Ok(())
}
