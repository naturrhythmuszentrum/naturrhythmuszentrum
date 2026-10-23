// SPDX-License-Identifier: AGPL-3.0-only
//! Atomic result publication and source dependency checks at the owning heap boundary.

use crate::phasor_testing as fixture;
use crate::reading_testing::{TestResult, ledger};
use nrz_bindings::{Budget, Limits};
use nrz_scalar::ArithmeticContext;

/// Late exact-budget refusal and descriptor exhaustion publish no result regions or views.
/// ´test:unit:phasor-publication-work-boundaries-are-atomic´
#[test]
fn phasor_publication_work_boundaries_are_atomic() -> TestResult {
    let mut case = fixture::publication_case::<f64>(true, &mut ledger()?)?;
    let publication = case.publication(&mut ledger()?)?;
    let mut budget = ledger()?;
    case.heap
        .publish_phasor_product(&publication, &ArithmeticContext::new()?, &mut budget)?;
    let needed = budget.usage().work;
    for (limit, prior, success) in [
        (needed, 0, true),
        (needed - 1, 0, false),
        (needed, 1, false),
    ] {
        let mut case = fixture::publication_case::<f64>(true, &mut ledger()?)?;
        let publication = case.publication(&mut ledger()?)?;
        let before = case.heap.descriptors.iter().count();
        let mut budget = Budget::new(Limits {
            work: limit,
            ..Limits::default()
        })?;
        budget.work(prior, super::AT)?;
        let result =
            case.heap
                .publish_phasor_product(&publication, &ArithmeticContext::new()?, &mut budget);
        assert_eq!(result.is_ok(), success);
        assert_eq!(
            case.heap.descriptors.iter().count(),
            before + if success { 3 } else { 0 }
        );
        assert!(case.heap.reading_entries(&case.bindings[0]).is_ok());
    }
    Ok(())
}

/// A failed allocation leaves both the declared result slot and every existing source unmodified.
/// ´test:unit:phasor-publication-capacity-refusal-preserves-sources´
#[test]
fn phasor_publication_capacity_refusal_preserves_sources() -> TestResult {
    let mut case = fixture::publication_case::<f64>(true, &mut ledger()?)?;
    let publication = case.publication(&mut ledger()?)?;
    let before = case.heap.descriptors.iter().count();
    case.heap.limits.descriptors = before;
    assert!(
        case.heap
            .publish_phasor_product(&publication, &ArithmeticContext::new()?, &mut ledger()?)
            .is_err()
    );
    assert_eq!(case.heap.descriptors.iter().count(), before);
    assert!(case.heap.reading_entries(&case.bindings[0]).is_ok());
    assert!(case.missing_vector().is_err());
    Ok(())
}

/// Every separately stored numerical-error parameter remains in a snapshot's dependency closure.
/// ´test:unit:phasor-product-rechecks-each-error-parameter-version´
#[test]
fn phasor_product_rechecks_each_error_parameter_version() -> TestResult {
    for position in [1, 3, 5, 7, 9, 11, 13, 15] {
        let case = fixture::case::<f64>(true)?;
        let plan = case.product(&mut ledger()?)?;
        let context = ArithmeticContext::new()?;
        let result = case
            .heap
            .evaluate_phasor_product(&plan, &context, &mut ledger()?)?;
        let ticket = &case.bindings[0].group.tickets[2];
        case.heap.symbols.cover(ticket.key)?.versions[position].set(2);
        assert!(result.centers(&case.heap, &context).is_err());
        assert!(
            case.heap
                .evaluate_phasor_product(&plan, &context, &mut ledger()?)
                .is_err()
        );
    }
    Ok(())
}

/// Generated deterministic radii retain the original represented-input assurance scope.
/// ´test:unit:phasor-publication-does-not-upgrade-input-assurance´
#[test]
fn phasor_publication_does_not_upgrade_input_assurance() -> TestResult {
    let mut case = fixture::represented_case::<f64>(&mut ledger()?)?;
    let publication = case.publication(&mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    let published = case
        .heap
        .publish_phasor_product(&publication, &context, &mut ledger()?)?;
    let readings = published.readings().bindings();
    assert_eq!(
        readings[0].phasor_input_coverage(),
        Some([crate::Coverage::RepresentedInputs; 2])
    );
    let copies = readings[1..5]
        .to_vec()
        .try_into()
        .map_err(|_| "four copies")?;
    let operand = super::PhasorOperand::new(readings[0].clone(), copies, &mut ledger()?)?;
    let (_, coverage) = super::input::capture(&case.heap, &operand, &mut ledger()?)?;
    assert_eq!(coverage, crate::Coverage::RepresentedInputs);
    Ok(())
}
