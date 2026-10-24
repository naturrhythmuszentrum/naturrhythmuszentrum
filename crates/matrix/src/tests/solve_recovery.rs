// SPDX-License-Identifier: AGPL-3.0-only
//! Private correction candidates still need the independent public solve theorem.

use crate::kernel_oracle::Rational;
use crate::solve_testing::{TestResult, draft};
use crate::{Binding, ExecutionLimits, Extent, Heap, HeapLimits};
use nrz_bindings::Descriptor;

/// A deliberately poor f32 iterate is corrected using widened residual arithmetic and the original LU.
/// ´test:crate:mixed-residual-correction-reuses-the-certified-factor´
#[test]
fn mixed_residual_correction_reuses_the_certified_factor() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let operator = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[3.0_f32])?;
    let rhs = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[1.0_f32])?;
    let limits = ExecutionLimits::default();
    let bound = Binding::new(
        draft(1, 1)?
            .with_solve_policy(crate::recovery_testing::policy(
                1.0,
                1,
                nrz_accuracy::Promotion::Never,
                nrz_accuracy::Limits::new(2, 1_000_000),
            )?)?
            .check(limits)?,
        vec![operator.value(), rhs.value()],
        limits,
    )
    .validate(&mut heap)?;
    let factor = bound.factor()?;
    let views = bound.normalized_inputs()?;
    let crate::binding::NativeRecipe::Solve(contract) = &bound.operation.recipe else {
        return Err("solve contract".into());
    };
    let budget = nrz_bindings::Budget::new(nrz_bindings::Limits::default())?;
    let inventory = crate::solve::scratch::Run::new(&bound, 0, &budget)?;
    inventory.certified(0)?;
    let attempt = crate::solve::scratch::Attempt {
        run: &inventory,
        index: 1,
    };
    let corrected =
        crate::solve::recovery::correction(contract, views.views(), &factor, &[0.25], attempt)?;
    let mut retained = attempt.buffer(crate::solve::scratch::Stage::RetainedCenters)?;
    for &value in &corrected {
        retained.push(value)?;
    }
    let mut evidence = Vec::new();
    crate::solve::evaluate::certify(
        contract,
        views.views(),
        &factor,
        retained,
        *bound.output_reservations().first().ok_or("reservation")?,
        &mut evidence,
        attempt,
    )?;
    let radius = crate::recovery_testing::proof(&evidence)?.numerical_radius();
    assert!(
        Rational::new(1, 3)?.covered(f64::from(*corrected.first().ok_or("candidate")?), radius)?
    );
    assert!(radius < 0.01);
    Ok(())
}
