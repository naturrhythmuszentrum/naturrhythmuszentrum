// SPDX-License-Identifier: AGPL-3.0-only
//! The new point consumer has a scoped cumulative threshold beside the unchanged invariant pins.

use super::{TestResult, ledger, setup};
use nrz_bindings::{Budget, Limits, readings::algebra::Location};
use nrz_scalar::Scalar;

/// Literal publication Work boundaries include a consumed prefix and preserve the retained root.
/// ´test:integration:lc-original-driven-publication-work-boundaries´
#[test]
fn lc_original_driven_publication_work_boundaries() -> TestResult {
    let work = [measured::<f32>()?, measured::<f64>()?];
    assert_eq!(work, [[9_602, 7_185_331], [9_602, 7_185_331]]);
    twins::<f32>(work[0][0])?;
    twins::<f64>(work[1][0])
}

/// Measure the actual point-varying consumer after all declarations and the invariant attempt.
fn measured<P: Scalar + 'static>() -> TestResult<[u64; 2]> {
    let mut case = setup::<P>(false)?;
    case.batch
        .execute(&case.heap, &case.context, &mut ledger()?)?;
    let root = case
        .batch
        .result(0, &case.heap, &case.context, &mut ledger()?)?;
    let mut budget = ledger()?;
    case.heap
        .publish_lc_impedance_reusing(&case.outputs[0], root, &case.context, &mut budget)?;
    Ok([budget.usage().work, case.rows[0].declaration_work])
}

/// A refusal exposes no prefix; the original root and exact predeclared slots remain usable.
fn twins<P: Scalar + 'static>(work: u64) -> TestResult {
    for (limit, prefix, success) in [(work, 0, true), (work - 1, 0, false), (work, 1, false)] {
        let mut case = setup::<P>(false)?;
        case.batch
            .execute(&case.heap, &case.context, &mut ledger()?)?;
        let root = case
            .batch
            .result(0, &case.heap, &case.context, &mut ledger()?)?;
        let before = case.heap.provenance_usage();
        let mut budget = Budget::new(Limits {
            work: limit,
            ..Limits::default()
        })?;
        budget.work(prefix, Location::new("driven point consumed prefix"))?;
        let result = case.heap.publish_lc_impedance_reusing(
            &case.outputs[0],
            root,
            &case.context,
            &mut budget,
        );
        assert_eq!(result.is_ok(), success);
        if !success {
            assert_eq!(case.heap.provenance_usage(), before);
            case.heap.publish_lc_impedance_reusing(
                &case.outputs[0],
                root,
                &case.context,
                &mut ledger()?,
            )?;
        }
        case.batch
            .result(0, &case.heap, &case.context, &mut ledger()?)?;
    }
    Ok(())
}
