// SPDX-License-Identifier: AGPL-3.0-only
//! Independent physical LC current oracles, reference transport and atomic restored views.

mod companions;

use crate::kernel_oracle::Rational;
use crate::phasor_solve_testing as fixture;
use crate::reading_testing::{TestResult, ledger};
use nrz_bindings::readings::PhasorResidualNorm;
use nrz_matrix::{PhasorError, SymbolState};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Exact LC reactance and current answers hold below and above resonance in both formats.
/// ´test:integration:lc-phasor-solve-restores-independent-rational-currents´
#[test]
fn lc_phasor_solve_restores_independent_rational_currents() -> TestResult {
    containment::<f32>()?;
    containment::<f64>()
}

/// An independent rational circuit equation supplies each oracle, without the block implementation.
fn containment<P: Scalar + 'static>() -> TestResult {
    for (frequency, sign) in [(1000, -1), (4000, 1)] {
        let reactance =
            Rational::new(frequency, 40)?.subtract(Rational::new(100_000, frequency)?)?;
        let current = Rational::new(-3, 1)?.divide(reactance)?;
        for rounded in [false, true] {
            let mut budget = fixture::budget()?;
            let nominal = if sign < 0 {
                -P::from_u8(75)
            } else {
                P::from_u8(75)
            };
            let mut case = fixture::case(nominal, P::ZERO, rounded, &mut budget)?;
            assert!(reactance.covered(
                case.heap.reading_entries(&case.bindings[2])?[0].to_f64(),
                0.0
            )?);
            let publication = case.publication(&mut budget)?;
            let context = ArithmeticContext::new()?;
            let result = case
                .heap
                .publish_phasor_solve(&publication, &context, &mut budget)?;
            check_current(&case, &result, current, rounded, &context)?;
            restore_voltage(&mut case, &result, &context, &mut budget)?;
        }
    }
    Ok(())
}

/// All current views and overlapping numerical channels retain the same physical target.
fn check_current<P: Scalar + 'static>(
    case: &fixture::Case<P>,
    result: &nrz_matrix::PublishedPhasorSolve<P>,
    current: Rational,
    rounded: bool,
    context: &ArithmeticContext,
) -> TestResult {
    let centers = result.evaluation().centers(&case.heap, context)?;
    let totals = result.evaluation().totals(&case.heap, context)?;
    let errors = result.evaluation().errors(&case.heap, context)?;
    for (index, expected) in [
        Rational::new(0, 1)?,
        current,
        Rational::new(0, 1)?.subtract(current)?,
        Rational::new(0, 1)?,
    ]
    .into_iter()
    .enumerate()
    {
        assert!(expected.covered(centers[index].to_f64(), errors[index].to_f64())?);
        assert!(errors[index] <= totals[index]);
    }
    assert!(current.covered(
        centers[1].to_f64(),
        result.evaluation().transferred_residual().currents[0].to_f64()
    )?);
    companions::check(
        &case.heap,
        result.readings().bindings(),
        centers,
        totals,
        errors,
    )?;
    assert_eq!(result.readings().bindings().len(), 6);
    let vector = case
        .heap
        .reading_entries(&result.readings().bindings()[5])?;
    assert!(
        vector
            .iter()
            .zip(centers)
            .all(|(left, right)| left.same_bits(*right))
    );
    for reading in result.readings().bindings() {
        assert_eq!(reading.state(), SymbolState::Published);
        assert_eq!(
            reading
                .phasor_solve()
                .ok_or("solve definition")?
                .output()
                .reading(),
            case.definition.output().reading()
        );
    }
    assert!(
        result
            .evaluation()
            .certificate()
            .margins
            .iter()
            .all(|margin| *margin > P::ZERO)
    );
    if rounded {
        assert!(!centers[1].same_bits(-centers[2]));
    }
    Ok(())
}

/// A second checked producer restores voltage from the published current, with full companions.
fn restore_voltage<P: Scalar + 'static>(
    case: &mut fixture::Case<P>,
    current: &nrz_matrix::PublishedPhasorSolve<P>,
    context: &ArithmeticContext,
    budget: &mut nrz_bindings::Budget,
) -> TestResult {
    let restoration = case.voltage(current, budget)?;
    let voltage = case
        .heap
        .publish_phasor_product(&restoration, context, budget)?;
    let centers = voltage.evaluation().centers(&case.heap, context)?;
    let errors = voltage.evaluation().errors(&case.heap, context)?;
    for (index, expected) in [3, 0, 0, 3].into_iter().enumerate() {
        assert!(
            Rational::new(expected, 1)?.covered(centers[index].to_f64(), errors[index].to_f64())?
        );
    }
    let totals = voltage.evaluation().totals(&case.heap, context)?;
    companions::check(
        &case.heap,
        voltage.readings().bindings(),
        centers,
        totals,
        errors,
    )?;
    assert_eq!(voltage.readings().bindings().len(), 6);
    Ok(())
}

/// Resonance is a successful zero impedance reading and a named refused solve, without a phase value.
/// ´test:integration:lc-phasor-solve-refuses-zero-and-straddling-reactance´
#[test]
fn lc_phasor_solve_refuses_zero_and_straddling_reactance() -> TestResult {
    let resonance = Rational::new(2000, 40)?.subtract(Rational::new(100_000, 2000)?)?;
    for (reactance, radius, pole) in [
        (0.0, 0.0, true),
        (0.0, 0.125, false),
        (0.125, 0.25, false),
        (-0.125, 0.25, false),
        (0.125, 0.125, false),
    ] {
        let mut case = fixture::case(reactance, radius, false, &mut fixture::budget()?)?;
        let publication = case.publication(&mut ledger()?)?;
        let values = case.heap.reading_entries(&case.bindings[0])?;
        if pole {
            for value in &values {
                assert!(resonance.covered(*value, 0.0)?);
            }
        }
        let result = case.heap.publish_phasor_solve(
            &publication,
            &ArithmeticContext::new()?,
            &mut ledger()?,
        );
        assert!(matches!(
            (&result, pole),
            (Err(PhasorError::ZeroImpedance), true)
                | (Err(PhasorError::ReactanceNotSeparated), false)
        ));
        assert!(case.heap.reading_entries(&case.bindings[0]).is_ok());
    }
    Ok(())
}

/// The retained reference quotient and norm weight change the transported residual factor.
/// ´test:integration:lc-phasor-transfer-distinguishes-metrics-and-block-norms´
#[test]
fn lc_phasor_transfer_distinguishes_metrics_and_block_norms() -> TestResult {
    let case = fixture::case(75.0_f64, 0.0, false, &mut fixture::budget()?)?;
    let context = ArithmeticContext::new()?;
    let mut factors = Vec::new();
    for (changed, norm) in [
        (false, PhasorResidualNorm::ComponentEuclidean),
        (true, PhasorResidualNorm::ComponentEuclidean),
        (false, PhasorResidualNorm::BlockFrobenius),
    ] {
        let plan = case.solve(changed, norm, &mut ledger()?)?;
        let result = case
            .heap
            .evaluate_phasor_solve(&plan, &context, &mut ledger()?)?;
        let bounds = result.transfer();
        let expected = if changed { 37.5 } else { 75.0 };
        assert!(
            bounds.operator_two.lower().get() <= expected
                && bounds.operator_two.upper().get() >= expected
        );
        assert!(bounds.block_frobenius.lower().get() > bounds.operator_two.upper().get());
        squared_norm(bounds.block_frobenius, expected)?;
        factors.push(bounds.residual_factor);
    }
    assert!(factors[1].lower().get() > factors[0].upper().get());
    assert!(factors[2].upper().get() < factors[0].lower().get());
    Ok(())
}

/// Every operand version remains in the published descendants, including equal-value replacements.
/// ´test:integration:lc-phasor-solve-invalidates-all-restored-readers´
#[test]
fn lc_phasor_solve_invalidates_all_restored_readers() -> TestResult {
    for index in [0, 5] {
        let mut case = fixture::case(-75.0_f64, 0.0, false, &mut fixture::budget()?)?;
        let publication = case.publication(&mut ledger()?)?;
        let context = ArithmeticContext::new()?;
        let published = case
            .heap
            .publish_phasor_solve(&publication, &context, &mut ledger()?)?;
        assert!(
            case.heap
                .publish_phasor_solve(&publication, &context, &mut ledger()?)
                .is_err()
        );
        let restoration = case.voltage(&published, &mut ledger()?)?;
        let voltage = case
            .heap
            .publish_phasor_product(&restoration, &context, &mut ledger()?)?;
        let original = case.heap.reading_entries(&case.bindings[index])?;
        case.heap
            .replace_reading_entries(&case.bindings[index], &original, &mut ledger()?)?;
        assert!(
            published
                .evaluation()
                .centers(&case.heap, &context)
                .is_err()
        );
        for reading in published.readings().bindings() {
            assert!(case.heap.reading_entries(reading).is_err());
        }
        assert!(voltage.evaluation().centers(&case.heap, &context).is_err());
        for reading in voltage.readings().bindings() {
            assert!(case.heap.reading_entries(reading).is_err());
        }
    }
    Ok(())
}

/// Lowering, solve and storage use one explicitly authorized caller ledger with exact boundaries.
/// ´test:integration:lc-phasor-solve-cumulative-work-boundaries´
#[test]
fn lc_phasor_solve_cumulative_work_boundaries() -> TestResult {
    let needed = complete(fixture::policy(), 0)?;
    for (limit, prior, success) in [
        (needed, 0, true),
        (needed - 1, 0, false),
        (needed, 1, false),
    ] {
        let limits = nrz_bindings::Limits {
            work: limit,
            ..fixture::policy()
        };
        assert_eq!(complete(limits, prior).is_ok(), success);
    }
    Ok(())
}

/// Recreate exact identities before each charged original-to-publication replay.
fn complete(limits: nrz_bindings::Limits, prior: u64) -> TestResult<u64> {
    let mut budget = nrz_bindings::Budget::new(limits)?;
    budget.work(
        prior,
        nrz_bindings::readings::algebra::Location::new("earlier LC operation"),
    )?;
    let mut case = fixture::case(-75.0_f64, 0.0, false, &mut budget)?;
    assert!(case.construction.windows(2).all(|pair| pair[0] <= pair[1]));
    let publication = case.publication(&mut budget)?;
    case.heap
        .publish_phasor_solve(&publication, &ArithmeticContext::new()?, &mut budget)?;
    Ok(budget.usage().work)
}

/// Every shared reactance endpoint is covered without inferring a sign from a central sample.
/// ´test:integration:lc-phasor-solve-encloses-a-declared-reactance-family´
#[test]
fn lc_phasor_solve_encloses_a_declared_reactance_family() -> TestResult {
    let case = fixture::case(-75.0_f64, 0.125, true, &mut fixture::budget()?)?;
    let plan = case.solve(false, PhasorResidualNorm::BlockFrobenius, &mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    let result = case
        .heap
        .evaluate_phasor_solve(&plan, &context, &mut ledger()?)?;
    let centers = result.centers(&case.heap, &context)?;
    let totals = result.totals(&case.heap, &context)?;
    for reactance in [-601, -599] {
        let current = Rational::new(-24, 1)?.divide(Rational::new(reactance, 1)?)?;
        assert!(current.covered(centers[1], totals[1])?);
        assert!(current.covered(centers[1], result.transferred_residual().currents[1])?);
    }
    Ok(())
}

/// Physical metrics and source order cannot be substituted even when all numerical shapes agree.
/// ´test:integration:lc-phasor-solve-refuses-swapped-sources-and-metrics´
#[test]
fn lc_phasor_solve_refuses_swapped_sources_and_metrics() -> TestResult {
    let case = fixture::case(75.0_f64, 0.0, false, &mut fixture::budget()?)?;
    assert!(
        nrz_bindings::readings::PhasorResidualTransfer::new(
            case.definition.clone(),
            [case.frames[1].clone(), case.frames[0].clone()],
            PhasorResidualNorm::ComponentEuclidean,
            &mut ledger()?
        )
        .is_err()
    );
    let plan = case.solve(
        false,
        PhasorResidualNorm::ComponentEuclidean,
        &mut ledger()?,
    )?;
    let [impedance, voltage] = case.operands(&mut ledger()?)?;
    assert!(
        nrz_matrix::PhasorSolve::new(
            plan.transfer().clone(),
            [voltage, impedance],
            nrz_scalar::LiteralLimits::default(),
            &mut ledger()?
        )
        .is_err()
    );
    let coverage = nrz_matrix::ReadingCoverage::new(
        case.coverage.entries().to_vec(),
        case.coverage.readings()[..5].to_vec(),
        case.coverage.overlaps()[..4].to_vec(),
        &mut ledger()?,
    )?;
    assert!(
        nrz_matrix::PhasorSolvePublication::new(plan, coverage, case.companions, &mut ledger()?)
            .is_err()
    );
    Ok(())
}

/// Exact squared endpoints distinguish the block norm without using a floating square root as truth.
fn squared_norm(enclosure: nrz_scalar::Enclosure<f64>, operator: f64) -> TestResult {
    let expected = Rational::binary(operator)?;
    let squared = expected
        .multiply(expected)?
        .multiply(Rational::new(2, 1)?)?;
    let lower = Rational::binary(enclosure.lower().get())?;
    let upper = Rational::binary(enclosure.upper().get())?;
    assert!(lower.multiply(lower)?.at_most(squared)?);
    assert!(squared.at_most(upper.multiply(upper)?)?);
    Ok(())
}

/// Public receipts retain the original ordered physical sources and share meanings across clones.
/// ´test:integration:lc-phasor-inventory-retains-original-selections´
#[test]
fn lc_phasor_inventory_retains_original_selections() -> TestResult {
    inventory_sources::<f32>()?;
    inventory_sources::<f64>()
}

/// Source roles distinguish repeated and negated storage copies and their own-target parameters.
fn inventory_sources<P: Scalar + 'static>() -> TestResult {
    let mut case = fixture::case(-P::from_u8(75), P::ZERO, false, &mut fixture::budget()?)?;
    let publication = case.publication(&mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    let result = case
        .heap
        .publish_phasor_solve(&publication, &context, &mut ledger()?)?;
    let inventory = result.evaluation().inventory();
    assert!(inventory.includes_publication());
    assert_eq!(inventory.precision(), P::PRECISION);
    let clone = inventory.clone();
    let mut copies = 0;
    for (original, shared) in inventory.allocations().zip(clone.allocations()) {
        assert!(original.same_binding(&shared));
        assert_eq!(original.ticket(), shared.ticket());
        assert!(
            original
                .plan()
                .definition()
                .shares_allocation(&case.definition)
        );
        if let nrz_matrix::PhasorAllocationStage::InputCopy { operand, copy, .. } =
            original.stage()?
        {
            assert_eq!(
                original.plan().operands()[operand].copies()[copy].reading(),
                case.bindings[operand * 5 + copy + 1].reading()
            );
            copies += 1;
        }
    }
    assert_eq!(copies, 24);
    let fresh = case
        .heap
        .evaluate_phasor_solve(publication.solve(), &context, &mut ledger()?)?;
    let first = inventory
        .allocations()
        .next()
        .ok_or("first original allocation")?;
    let second = fresh
        .inventory()
        .allocations()
        .next()
        .ok_or("first fresh allocation")?;
    assert!(!first.same_binding(&second));
    assert_ne!(first.ticket().execution(), second.ticket().execution());
    Ok(())
}
