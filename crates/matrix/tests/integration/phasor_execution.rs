// SPDX-License-Identifier: AGPL-3.0-only
//! Checked Real block execution retains native operands, rounded copies and deterministic coverage.

use crate::phasor_testing as fixture;
use crate::reading_testing::{TestResult, ledger};
use nrz_matrix::{PhasorOperand, PhasorProduct};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Independent Complex corner products lie inside every corresponding block result entry.
/// ´test:integration:phasor-product-encloses-shared-rounded-copies´
#[test]
fn phasor_product_encloses_shared_rounded_copies() -> TestResult {
    containment::<f32>()?;
    containment::<f64>()
}

/// A scalar oracle forms Complex products directly, independently of the matrix contraction.
fn containment<P: Scalar + 'static>() -> TestResult {
    for rounded in [false, true] {
        let case = fixture::case::<P>(rounded)?;
        let plan = case.product(&mut ledger()?)?;
        let context = ArithmeticContext::new()?;
        let result = case
            .heap
            .evaluate_phasor_product(&plan, &context, &mut ledger()?)?;
        assert_eq!(result.correspondence().original(), case.law.original());
        let centers = result.centers(&case.heap, &context)?;
        let totals = result.totals(&case.heap, &context)?;
        let errors = result.errors(&case.heap, &context)?;
        for (scale, radii) in [(1, errors), (4, totals)] {
            for signs in 0..16 {
                let mut point = [16, 24, 40, 56];
                for (index, value) in point.iter_mut().enumerate() {
                    *value += if signs & (1 << index) == 0 {
                        -scale
                    } else {
                        scale
                    };
                }
                let [a, b, u, v] = point;
                let exact = [a * u - b * v, b * u + a * v, -b * u - a * v, a * u - b * v]
                    .map(|value| f64::from(value) / 64.0);
                for ((center, radius), expected) in centers.iter().zip(radii).zip(exact) {
                    assert!(center.to_f64() - radius.to_f64() <= expected);
                    assert!(center.to_f64() + radius.to_f64() >= expected);
                }
            }
        }
        assert!(totals.iter().all(|radius| radius.to_f64() < 22.0));
        if rounded {
            assert!(!centers[0].same_bits(centers[3]));
        }
    }
    Ok(())
}

/// Stored-copy identity and whole-operand order cannot be replaced by matching numerical shapes.
/// ´test:integration:phasor-product-refuses-copy-and-operand-substitution´
#[test]
fn phasor_product_refuses_copy_and_operand_substitution() -> TestResult {
    let case = fixture::case::<f64>(true)?;
    let copies = [
        case.bindings[4].clone(),
        case.bindings[2].clone(),
        case.bindings[3].clone(),
        case.bindings[1].clone(),
    ];
    assert!(PhasorOperand::new(case.bindings[0].clone(), copies, &mut ledger()?).is_err());
    let reversed = [
        case.operand(5, &mut ledger()?)?,
        case.operand(0, &mut ledger()?)?,
    ];
    assert!(PhasorProduct::new(case.law, reversed, &mut ledger()?).is_err());
    Ok(())
}

/// Equal-value writes invalidate both retained snapshots and repeated numerical execution.
/// ´test:integration:phasor-product-rechecks-all-source-versions´
#[test]
fn phasor_product_rechecks_all_source_versions() -> TestResult {
    for index in [0, 5] {
        let mut case = fixture::case::<f64>(false)?;
        let plan = case.product(&mut ledger()?)?;
        let context = ArithmeticContext::new()?;
        let result = case
            .heap
            .evaluate_phasor_product(&plan, &context, &mut ledger()?)?;
        let values = case.heap.reading_entries(&case.bindings[index])?;
        case.heap
            .replace_reading_entries(&case.bindings[index], &values, &mut ledger()?)?;
        assert!(result.centers(&case.heap, &context).is_err());
        assert!(result.errors(&case.heap, &context).is_err());
        assert!(
            case.heap
                .evaluate_phasor_product(&plan, &context, &mut ledger()?)
                .is_err()
        );
    }
    Ok(())
}

/// Exact Work boundaries preserve earlier caller charges on the same numerical execution ledger.
/// ´test:integration:phasor-product-cumulative-work-boundaries´
#[test]
fn phasor_product_cumulative_work_boundaries() -> TestResult {
    let case = fixture::case::<f64>(true)?;
    let plan = case.product(&mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    let mut budget = ledger()?;
    case.heap
        .evaluate_phasor_product(&plan, &context, &mut budget)?;
    let needed = budget.usage().work;
    for (limit, prior, success) in [
        (needed, 0, true),
        (needed - 1, 0, false),
        (needed, 1, false),
    ] {
        let mut budget = nrz_bindings::Budget::new(nrz_bindings::Limits {
            work: limit,
            ..nrz_bindings::Limits::default()
        })?;
        budget.work(
            prior,
            nrz_bindings::readings::algebra::Location::new("phasor earlier work"),
        )?;
        assert_eq!(
            case.heap
                .evaluate_phasor_product(&plan, &context, &mut budget)
                .is_ok(),
            success
        );
    }
    Ok(())
}

/// Whole-family replacement refuses uncanonical rounded payloads without invalidating old snapshots.
/// ´test:integration:phasor-product-refused-write-preserves-snapshot´
#[test]
fn phasor_product_refused_write_preserves_snapshot() -> TestResult {
    let mut case = fixture::case::<f64>(true)?;
    let plan = case.product(&mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    let result = case
        .heap
        .evaluate_phasor_product(&plan, &context, &mut ledger()?)?;
    let centers = *result.centers(&case.heap, &context)?;
    let values = case.heap.reading_entries(&case.bindings[0])?;
    assert!(
        case.heap
            .replace_reading_entries(&case.bindings[0], &values, &mut ledger()?)
            .is_err()
    );
    assert_eq!(
        result.centers(&case.heap, &context)?.map(f64::to_bits),
        centers.map(f64::to_bits)
    );
    Ok(())
}

/// Original correspondence, heap admission, preparation and execution retain one cumulative policy.
/// ´test:integration:phasor-product-complete-cumulative-boundaries´
#[test]
fn phasor_product_complete_cumulative_boundaries() -> TestResult {
    let needed = complete(nrz_bindings::Limits::default(), 0)?;
    for (limit, prior, success) in [
        (needed, 0, true),
        (needed - 1, 0, false),
        (needed, 1, false),
    ] {
        assert_eq!(
            complete(
                nrz_bindings::Limits {
                    work: limit,
                    ..nrz_bindings::Limits::default()
                },
                prior
            )
            .is_ok(),
            success
        );
    }
    Ok(())
}

/// Rebuild fresh source identities without resetting any charged operation phase.
fn complete(limits: nrz_bindings::Limits, prior: u64) -> TestResult<u64> {
    let mut budget = nrz_bindings::Budget::new(limits)?;
    budget.work(
        prior,
        nrz_bindings::readings::algebra::Location::new("earlier phasor work"),
    )?;
    let case = fixture::case_with_budget::<f64>(true, &mut budget)?;
    let plan = case.product(&mut budget)?;
    case.heap
        .evaluate_phasor_product(&plan, &ArithmeticContext::new()?, &mut budget)?;
    Ok(budget.usage().work)
}

/// Every result view and per-copy companion is declared before execution and published together.
/// ´test:integration:phasor-product-publishes-complete-source-bound-family´
#[test]
fn phasor_product_publishes_complete_source_bound_family() -> TestResult {
    let mut budget = ledger()?;
    let mut case = fixture::publication_case::<f64>(true, &mut budget)?;
    let publication = case.publication(&mut budget)?;
    assert!(case.missing_vector().is_err());
    let context = ArithmeticContext::new()?;
    let published = case
        .heap
        .publish_phasor_product(&publication, &context, &mut budget)?;
    let readings = published.readings().bindings();
    assert_eq!(readings.len(), 6);
    let centers = published.evaluation().centers(&case.heap, &context)?;
    assert_eq!(case.heap.reading_entries(&readings[5])?, centers[..2]);
    for (index, copy) in readings[1..5].iter().enumerate() {
        assert_eq!(copy.state(), nrz_matrix::SymbolState::Published);
        assert_eq!(
            case.heap.reading_entries(copy)?[0].to_bits(),
            centers[index].to_bits()
        );
        assert_eq!(
            copy.phasor_result().ok_or("result definition")?.operation(),
            case.law.original()
        );
        companions(&case.heap, copy, index, published.evaluation(), &context)?;
    }
    assert!(readings[0].copy_discrepancies().count() > 0);
    assert!(
        case.heap
            .publish_phasor_product(&publication, &context, &mut ledger()?)
            .is_err()
    );
    assert!(case.heap.reading_entries(&readings[0]).is_ok());
    case.heap.compact()?;
    let original = [2.0, 3.0, -3.0, 2.0];
    case.heap
        .replace_reading_entries(&case.bindings[0], &original, &mut ledger()?)?;
    for reading in readings {
        assert!(case.heap.reading_entries(reading).is_err());
    }
    Ok(())
}

/// Output parameter positions are checked against their actual produced coverage channels.
fn companions(
    heap: &nrz_matrix::Heap,
    copy: &nrz_matrix::ReadingBinding<f64>,
    index: usize,
    evaluation: &nrz_matrix::PhasorEvaluation<f64>,
    context: &ArithmeticContext,
) -> TestResult {
    for (position, companion) in copy.companions().iter().enumerate() {
        if companion.targets()[0].reading() != copy.reading() {
            continue;
        }
        let expected = match companion.companion().kind() {
            nrz_bindings::readings::CompanionReadingKind::Radius(
                nrz_bindings::CompanionKind::DeterministicBound,
            ) => evaluation.totals(heap, context)?[index],
            nrz_bindings::readings::CompanionReadingKind::Radius(
                nrz_bindings::CompanionKind::NumericalError,
            ) => evaluation.errors(heap, context)?[index],
            nrz_bindings::readings::CompanionReadingKind::Covariance => {
                return Err("unexpected covariance".into());
            }
        };
        assert_eq!(
            heap.reading_companion_entries(copy, position)?[0].to_bits(),
            expected.to_bits()
        );
    }
    Ok(())
}

/// Generated totals never turn represented coordinates into declared physical uncertainty.
/// ´test:integration:phasor-publication-preserves-represented-assurance´
#[test]
fn phasor_publication_preserves_represented_assurance() -> TestResult {
    let mut case = fixture::represented_case::<f64>(&mut ledger()?)?;
    let publication = case.publication(&mut ledger()?)?;
    let result = case.heap.publish_phasor_product(
        &publication,
        &ArithmeticContext::new()?,
        &mut ledger()?,
    )?;
    assert_eq!(
        result.readings().bindings()[0].phasor_input_coverage(),
        Some([nrz_matrix::Coverage::RepresentedInputs; 2])
    );
    Ok(())
}
