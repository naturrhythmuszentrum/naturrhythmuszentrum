// SPDX-License-Identifier: AGPL-3.0-only
//! Refused current publication leaves every value/companion allocation and input unchanged.

mod payload;

use crate::phasor_solve_testing as fixture;
use crate::reading_testing::{TestResult, ledger};
use nrz_bindings::{Budget, Limits};
use nrz_scalar::ArithmeticContext;

/// Late cumulative Work refusal never publishes a prefix of the current family.
/// ´test:unit:phasor-solve-publication-work-refusal-is-atomic´
#[test]
fn phasor_solve_publication_work_refusal_is_atomic() -> TestResult {
    let context = ArithmeticContext::new()?;
    let mut case = fixture::case(-75.0_f64, 0.0, false, &mut fixture::budget()?)?;
    let publication = case.publication(&mut ledger()?)?;
    let mut budget = ledger()?;
    case.heap
        .publish_phasor_solve(&publication, &context, &mut budget)?;
    let needed = budget.usage().work;
    for (limit, prior, success) in [
        (needed, 0, true),
        (needed - 1, 0, false),
        (needed, 1, false),
    ] {
        let mut case = fixture::case(-75.0_f64, 0.0, false, &mut fixture::budget()?)?;
        let publication = case.publication(&mut ledger()?)?;
        let before = case.heap.descriptors.iter().count();
        let mut budget = Budget::new(Limits {
            work: limit,
            ..Limits::default()
        })?;
        budget.work(prior, super::AT)?;
        let result = case
            .heap
            .publish_phasor_solve(&publication, &context, &mut budget);
        assert_eq!(result.is_ok(), success);
        assert_eq!(
            case.heap.descriptors.iter().count(),
            before + if success { 3 } else { 0 }
        );
        assert!(case.heap.reading_entries(&case.bindings[0]).is_ok());
    }
    Ok(())
}

/// Descriptor exhaustion leaves all existing readers available and installs no output.
/// ´test:unit:phasor-solve-publication-capacity-refusal-is-atomic´
#[test]
fn phasor_solve_publication_capacity_refusal_is_atomic() -> TestResult {
    let mut case = fixture::case(-75.0_f64, 0.0, false, &mut fixture::budget()?)?;
    let publication = case.publication(&mut ledger()?)?;
    let before = case.heap.descriptors.iter().count();
    case.heap.limits.descriptors = before;
    assert!(
        case.heap
            .publish_phasor_solve(&publication, &ArithmeticContext::new()?, &mut ledger()?)
            .is_err()
    );
    assert_eq!(case.heap.descriptors.iter().count(), before);
    assert!(case.heap.reading_entries(&case.bindings[5]).is_ok());
    Ok(())
}

/// Independent companion revisions invalidate both snapshots and every restored descendant.
/// ´test:unit:phasor-solve-rechecks-every-companion-version´
#[test]
fn phasor_solve_rechecks_every_companion_version() -> TestResult {
    for index in 0..16 {
        let mut case = fixture::case(-75.0_f64, 0.0, false, &mut fixture::budget()?)?;
        let publication = case.publication(&mut ledger()?)?;
        let context = ArithmeticContext::new()?;
        let result = case
            .heap
            .publish_phasor_solve(&publication, &context, &mut ledger()?)?;
        let ticket = &case.bindings[0].group.tickets[2];
        case.heap.symbols.cover(ticket.key)?.versions[index].set(2);
        assert!(result.evaluation().centers(&case.heap, &context).is_err());
        for reading in result.readings().bindings() {
            assert!(case.heap.reading_entries(reading).is_err());
        }
    }
    Ok(())
}

/// Measured construction, lowering, solve and restoration costs preserve the complete cumulative ledger.
/// ´test:unit:phasor-solve-costs-are-pinned´
#[test]
fn phasor_solve_costs_are_pinned() -> TestResult {
    let mut budget = fixture::budget()?;
    let mut case = fixture::case(-75.0_f64, 0.0, false, &mut budget)?;
    let prepared = budget.usage().work;
    let solve = case.solve(
        false,
        nrz_bindings::readings::PhasorResidualNorm::ComponentEuclidean,
        &mut budget,
    )?;
    let plan_work = budget.usage().work - prepared;
    let publication = super::PhasorSolvePublication::new(
        solve.clone(),
        case.coverage.clone(),
        case.companions.clone(),
        &mut budget,
    )?;
    let publication_work = budget.usage().work - prepared - plan_work;
    let before = budget.usage().work;
    let context = ArithmeticContext::new()?;
    let result = case
        .heap
        .publish_phasor_solve(&publication, &context, &mut budget)?;
    let usage = result.evaluation().usage();
    let lowering = usage[1].work - usage[0].work;
    let numerical = usage[2].work - usage[1].work;
    let storage = budget.usage().work - before - lowering - numerical;
    let start = budget.usage().work;
    let restoration = case.voltage(&result, &mut budget)?;
    let planning = budget.usage().work - start;
    let restored = case
        .heap
        .publish_phasor_product(&restoration, &context, &mut budget)?;
    let phases = restored.evaluation().usage();
    let evaluation = phases[1].work - phases[0].work;
    let restoration_cost = [
        planning,
        evaluation,
        budget.usage().work - start - planning - evaluation,
        restoration.retained_payload_bytes(),
    ];
    assert_eq!(
        (
            case.construction,
            plan_work,
            publication_work,
            lowering,
            numerical,
            storage,
            solve.retained_payload_bytes(),
            publication.retained_payload_bytes(),
            budget.usage().work,
            payload::required(&case, &solve)?,
            restoration_cost
        ),
        (
            [153_006, 220_493, 1_772_635, 1_781_742],
            10_840,
            9603,
            336,
            2600,
            5520,
            752_808,
            95_335,
            1_842_545,
            [756_904, 756_904],
            [26_002, 376, 5526, 95_499]
        )
    );
    Ok(())
}

/// A proved fixed component needs its own numerical coverage, even when repeated centers agree.
/// ´test:unit:phasor-solve-refuses-uncovered-fixed-components´
#[test]
fn phasor_solve_refuses_uncovered_fixed_components() -> TestResult {
    let context = ArithmeticContext::new()?;
    let zero = nrz_scalar::Enclosure::singleton(nrz_scalar::Finite::new(0.0_f64)?);
    let fixed = [zero, zero];
    let present = [[true, false], [false, true]];
    let voltage = super::super::numeric::Values {
        centers: [3.0, 0.0, 0.0, 3.0],
        totals: [0.0; 4],
        errors: [0.0; 4],
    };
    let mut impedance = super::super::numeric::Values {
        centers: [1.0, 75.0, -75.0, 1.0],
        totals: [1.0; 4],
        errors: [0.0; 4],
    };
    assert!(matches!(
        super::constants::check(
            &[&impedance, &voltage],
            &fixed,
            present,
            &context,
            &mut ledger()?
        ),
        Err(crate::PhasorError::Live(
            crate::LiveBindingError::UncoveredCopy
        ))
    ));
    impedance.errors = [2.0, 0.0, 0.0, 2.0];
    impedance.totals = [2.0; 4];
    assert!(
        super::constants::check(
            &[&impedance, &voltage],
            &fixed,
            present,
            &context,
            &mut ledger()?
        )
        .is_ok()
    );
    let constant = nrz_scalar::Enclosure::singleton(nrz_scalar::Finite::new(76.0_f64)?);
    let fixed = [constant];
    let present = [[false, true], [false, false]];
    assert!(
        super::constants::check(
            &[&impedance, &voltage],
            &fixed,
            present,
            &context,
            &mut ledger()?
        )
        .is_err()
    );
    Ok(())
}
