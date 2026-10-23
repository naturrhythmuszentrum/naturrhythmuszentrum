// SPDX-License-Identifier: AGPL-3.0-only
//! Payload limits preserve the complete immutable solve boundary.

use crate::phasor_solve_testing as fixture;
use crate::reading_testing::{TestResult, ledger};
use nrz_bindings::{Budget, Limits, readings::PhasorResidualNorm};
use nrz_scalar::ArithmeticContext;

/// Staged and retained output payload each have an exact success/refusal boundary.
/// ´test:unit:phasor-solve-payload-boundaries´
#[test]
fn phasor_solve_payload_boundaries() -> TestResult {
    let case = fixture::case(-75.0_f64, 0.0, false, &mut fixture::budget()?)?;
    let plan = case.solve(
        false,
        PhasorResidualNorm::ComponentEuclidean,
        &mut ledger()?,
    )?;
    let context = ArithmeticContext::new()?;
    let bounds = required(&case, &plan)?;
    for (staged, needed) in [(true, bounds[0]), (false, bounds[1])] {
        for (limit, success) in [(needed, true), (needed - 1, false)] {
            let mut budget = policy(staged, limit)?;
            assert_eq!(
                case.heap
                    .evaluate_phasor_solve(&plan, &context, &mut budget)
                    .is_ok(),
                success
            );
            assert!(case.heap.reading_entries(&case.bindings[0]).is_ok());
        }
    }
    Ok(())
}

/// Find each payload boundary without changing the operation or its default policy.
pub(super) fn required(
    case: &fixture::Case<f64>,
    plan: &super::super::PhasorSolve<f64>,
) -> TestResult<[u64; 2]> {
    let mut result = [0; 2];
    let context = ArithmeticContext::new()?;
    for (index, staged) in [true, false].into_iter().enumerate() {
        let mut lower = 0;
        let mut upper = if staged {
            Limits::default().staged_payload_bytes
        } else {
            Limits::default().output_bytes
        };
        case.heap
            .evaluate_phasor_solve(plan, &context, &mut policy(staged, upper)?)?;
        while lower + 1 < upper {
            let middle = lower + (upper - lower) / 2;
            if case
                .heap
                .evaluate_phasor_solve(plan, &context, &mut policy(staged, middle)?)
                .is_ok()
            {
                upper = middle;
            } else {
                lower = middle;
            }
        }
        result[index] = upper;
    }
    Ok(result)
}

/// Only the measured payload channel changes; Work and every other policy stay at defaults.
fn policy(staged: bool, limit: u64) -> TestResult<Budget> {
    let mut limits = Limits::default();
    if staged {
        limits.staged_payload_bytes = limit;
    } else {
        limits.output_bytes = limit;
    }
    Ok(Budget::new(limits)?)
}

/// Publication adds owned witnesses and metadata while retaining one shared solve allocation.
/// ´test:unit:phasor-solve-publication-payload-boundaries´
#[test]
fn phasor_solve_publication_payload_boundaries() -> TestResult {
    let case = fixture::case(-75.0_f64, 0.0, false, &mut fixture::budget()?)?;
    let plan = case.solve(
        false,
        PhasorResidualNorm::ComponentEuclidean,
        &mut ledger()?,
    )?;
    let publication = super::super::PhasorSolvePublication::new(
        plan.clone(),
        case.coverage.clone(),
        case.companions.clone(),
        &mut ledger()?,
    )?;
    let needed = publication.retained_payload_bytes();
    assert!(needed < plan.retained_payload_bytes());
    for staged in [true, false] {
        for (limit, success) in [(needed, true), (needed - 1, false)] {
            assert_eq!(
                super::super::PhasorSolvePublication::new(
                    plan.clone(),
                    case.coverage.clone(),
                    case.companions.clone(),
                    &mut policy(staged, limit)?
                )
                .is_ok(),
                success
            );
        }
    }
    Ok(())
}

/// Restored voltage owns output witnesses once and shares its original equation and operand groups.
/// ´test:unit:phasor-restoration-shares-evidence-and-bounds-output´
#[test]
fn phasor_restoration_shares_evidence_and_bounds_output() -> TestResult {
    let mut case = fixture::case(-75.0_f64, 0.0, false, &mut fixture::budget()?)?;
    let current = case.publication(&mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    let solved = case
        .heap
        .publish_phasor_solve(&current, &context, &mut ledger()?)?;
    let restored = case.voltage(&solved, &mut ledger()?)?;
    assert!(
        restored
            .definition()
            .shares_allocation(&case.restoration.definition)
    );
    assert!(
        restored
            .product()
            .correspondence()
            .shares_allocation(current.solve().definition().correspondence())
    );
    assert!(std::rc::Rc::ptr_eq(
        &restored.product().operands()[0].whole().group,
        &current.solve().operands()[0].whole().group
    ));
    assert!(std::rc::Rc::ptr_eq(
        &restored.product().operands()[1].whole().group,
        &solved.readings().bindings()[0].group
    ));
    let needed = restored.retained_payload_bytes();
    let witnesses = restored
        .coverage()
        .readings()
        .iter()
        .chain(
            restored
                .companions()
                .iter()
                .flat_map(crate::CompanionAdapter::targets),
        )
        .map(|adapter| adapter.representation().witness().exported_byte_len())
        .sum::<usize>();
    let sources =
        super::super::super::publication::sources(restored.product().operands(), &mut ledger()?)?;
    assert_eq!(
        needed,
        u64::try_from(
            8192 + witnesses
                + sources.len()
                    * std::mem::size_of::<crate::binding::live::selected::SelectedReading>()
        )?
    );
    for staged in [true, false] {
        for (limit, success) in [(needed, true), (needed - 1, false)] {
            assert_eq!(
                crate::PhasorPublication::new(
                    restored.product().clone(),
                    restored.definition().clone(),
                    restored.coverage().clone(),
                    restored.companions().to_vec(),
                    &mut policy(staged, limit)?
                )
                .is_ok(),
                success
            );
        }
    }
    Ok(())
}
