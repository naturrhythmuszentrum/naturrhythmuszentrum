// SPDX-License-Identifier: AGPL-3.0-only
//! Source intersections are reused across the flattened original operand closure.

use super::{Invalidations, source_touched};
use crate::phasor_solve_testing as fixture;
use crate::reading_testing::{TestResult, ledger};
use nrz_scalar::ArithmeticContext;
use std::rc::Rc;

/// Every original operand selection invalidates the complete solve independent of visitation order.
/// ´test:unit:phasor-operand-invalidation-is-order-independent´
#[test]
fn phasor_operand_invalidation_is_order_independent() -> TestResult {
    let mut case = fixture::case(-75.0_f64, 0.0, false, &mut fixture::budget()?)?;
    let publication = case.publication(&mut ledger()?)?;
    let solved =
        case.heap
            .publish_phasor_solve(&publication, &ArithmeticContext::new()?, &mut ledger()?)?;
    let output = &solved.readings().bindings()[0].group;
    let source = &case.bindings[0].group;
    for changed in 0..case.bindings.len() {
        for reverse in [false, true] {
            let mut prepared = vec![
                (Rc::clone(source), vec![changed]),
                (Rc::clone(output), Vec::new()),
            ];
            if reverse {
                prepared.reverse();
            }
            assert!(source_touched(output, &prepared, &mut ledger()?)?);
        }
    }
    let untouched = vec![
        (Rc::clone(source), Vec::new()),
        (Rc::clone(output), Vec::new()),
    ];
    assert!(!source_touched(output, &untouched, &mut ledger()?)?);
    Ok(())
}

/// Flattened ancestors remain authoritative when an intermediate result has no local intersection.
/// ´test:unit:phasor-publication-retains-transitive-operand-dependencies´
#[test]
fn phasor_publication_retains_transitive_operand_dependencies() -> TestResult {
    let mut case = fixture::case(-75.0_f64, 0.0, false, &mut fixture::budget()?)?;
    let publication = case.publication(&mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    let solved = case
        .heap
        .publish_phasor_solve(&publication, &context, &mut ledger()?)?;
    let restoration = case.voltage(&solved, &mut ledger()?)?;
    let restored = case
        .heap
        .publish_phasor_product(&restoration, &context, &mut ledger()?)?;
    let output = &restored.readings().bindings()[0].group;
    let source = &case.bindings[0].group;
    let intermediate = &solved.readings().bindings()[0].group;
    for changed in 0..case.bindings.len() {
        let prepared: Invalidations = vec![
            (Rc::clone(output), Vec::new()),
            (Rc::clone(intermediate), Vec::new()),
            (Rc::clone(source), vec![changed]),
        ];
        assert!(source_touched(output, &prepared, &mut ledger()?)?);
    }
    assert!(source_touched(output, &[], &mut ledger()?)?);
    Ok(())
}
