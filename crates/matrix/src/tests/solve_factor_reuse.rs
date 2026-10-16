// SPDX-License-Identifier: AGPL-3.0-only
//! Private factor eligibility excludes result-only evidence without weakening operator identity.

use crate::solve_testing::{TestResult, draft};
use crate::{Binding, Coverage, ExecutionLimits, Extent, Heap, HeapLimits};
use nrz_units::Descriptor;

/// Complete result coverage can change independently of the numerical factor's dependency key.
/// ´test:crate:factor-key-excludes-result-only-coverage´
#[test]
fn factor_key_excludes_result_only_coverage() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
    let limits = ExecutionLimits::default();
    let bound = Binding::new(draft(1, 1)?.check(limits)?, vec![source.value(); 2], limits)
        .validate(&mut heap)?;
    let factor = bound.factor()?;
    let mut dependency = bound.dependencies().first().ok_or("dependency")?.clone();
    dependency.coverage = Coverage::DeclaredBounds;
    let crate::binding::NativeRecipe::Solve(contract) = &bound.operation.recipe else {
        return Err("contract".into());
    };
    factor.validate(&dependency, contract)?;
    assert_eq!(factor.dependency().stamp(), dependency.stamp());
    Ok(())
}
