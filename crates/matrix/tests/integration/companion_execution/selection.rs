// SPDX-License-Identifier: AGPL-3.0-only
//! Affine selection executes the original value and both covariance axes together.

use super::fixture;
use crate::reading_testing::{TestResult, ledger};
use crate::reciprocal_testing::oracle::Rational;
use nrz_matrix::ReadingSelection;
use nrz_scalar::{ArithmeticContext, LiteralLimits, Scalar};

/// Nonidentity value maps and both covariance maps retain one original shared source.
/// ´test:integration:mapped-selection-retains-both-covariance-targets´
#[test]
fn mapped_selection_retains_both_covariance_targets() -> TestResult {
    covariance::<f32>()?;
    covariance::<f64>()
}

fn covariance<P: Scalar + 'static>() -> TestResult {
    let case =
        fixture::attached::<P>(true).map_err(|e| format!("mapped covariance fixture: {e:?}"))?;
    let execution = case
        .execution(&mut ledger()?)
        .map_err(|e| format!("mapped covariance execution: {e:?}"))?;
    let context = ArithmeticContext::new().map_err(|e| format!("mapped context: {e:?}"))?;
    for (index, expected) in [(1, [4, 9]), (2, [-8, 15])] {
        let selection = ReadingSelection::new(
            case.bindings[index].clone(),
            vec![execution.clone()],
            LiteralLimits::default(),
            &mut ledger()?,
        )
        .map_err(|e| format!("mapped covariance plan: {e:?}"))?;
        let evaluated = case
            .heap
            .select_mapped_reading(&selection, &context, &mut ledger()?)
            .map_err(|e| format!("mapped covariance evaluation: {e:?}"))?;
        let values = evaluated
            .values(&case.heap, &context)
            .map_err(|e| format!("mapped values: {e:?}"))?;
        let errors = evaluated
            .errors(&case.heap, &context)
            .map_err(|e| format!("mapped errors: {e:?}"))?;
        for ((value, error), expected) in values.iter().zip(errors).zip(expected) {
            assert!(Rational::new(expected, 1)?.covered(value.to_f64(), error.to_f64())?);
        }
        let companion = evaluated.companions().first().ok_or("mapped covariance")?;
        assert_eq!(
            companion.exact().target().node(),
            execution.exact().target().node()
        );
        let parameters = companion
            .values(&case.heap, &context)
            .map_err(|e| format!("mapped covariance parameters: {e:?}"))?;
        let errors = companion
            .errors(&case.heap, &context)
            .map_err(|e| format!("mapped covariance parameter errors: {e:?}"))?;
        for ((value, error), (top, bottom)) in
            parameters
                .iter()
                .zip(errors)
                .zip([(-2, 1), (-3, 2), (5, 4), (15, 1)])
        {
            assert!(Rational::new(top, bottom)?.covered(value.to_f64(), error.to_f64())?);
        }
        assert_eq!(
            companion.targets()[0].reading().family().root(),
            companion.targets()[1].reading().family().root()
        );
        assert_eq!(evaluated.source().reading(), case.bindings[index].reading());
    }
    Ok(())
}

/// The absolute map transports original radius and parameter error without applying it twice.
/// ´test:integration:mapped-selection-transports-radius-once´
#[test]
fn mapped_selection_transports_radius_once() -> TestResult {
    let case =
        fixture::attached::<f64>(false).map_err(|e| format!("mapped radius fixture: {e:?}"))?;
    let execution = case
        .execution(&mut ledger()?)
        .map_err(|e| format!("mapped radius execution: {e:?}"))?;
    let plan = ReadingSelection::new(
        case.bindings[1].clone(),
        vec![execution],
        LiteralLimits::default(),
        &mut ledger()?,
    )
    .map_err(|e| format!("mapped radius plan: {e:?}"))?;
    let context = ArithmeticContext::new().map_err(|e| format!("mapped radius context: {e:?}"))?;
    let evaluated = case
        .heap
        .select_mapped_reading(&plan, &context, &mut ledger()?)
        .map_err(|e| format!("mapped radius evaluation: {e:?}"))?;
    let totals = evaluated
        .totals(&case.heap, &context)
        .map_err(|e| format!("mapped radius totals: {e:?}"))?;
    let errors = evaluated
        .errors(&case.heap, &context)
        .map_err(|e| format!("mapped radius errors: {e:?}"))?;
    assert_eq!(evaluated.coverage(), nrz_matrix::Coverage::DeclaredBounds);
    for ((total, error), (top, bottom, upper)) in totals
        .iter()
        .zip(errors)
        .zip([(5, 8, 0.626), (27, 16, 1.689)])
    {
        assert!(Rational::new(top, bottom)?.at_most(&Rational::binary(*total)?)?);
        assert!(*total < upper);
        assert!(*error < 0.000_001);
    }
    Ok(())
}

/// Every source and parameter version remains live authority for the whole selected result.
/// ´test:integration:mapped-selection-invalidates-on-source-or-parameter-write´
#[test]
fn mapped_selection_invalidates_on_source_or_parameter_write() -> TestResult {
    for index in [0, 3, 4] {
        let mut case =
            fixture::case::<f64>(true).map_err(|e| format!("mapped mutation fixture: {e:?}"))?;
        let execution = case
            .execution(&mut ledger()?)
            .map_err(|e| format!("mapped mutation execution: {e:?}"))?;
        let plan = ReadingSelection::new(
            case.bindings[1].clone(),
            vec![execution],
            LiteralLimits::default(),
            &mut ledger()?,
        )
        .map_err(|e| format!("mapped mutation plan: {e:?}"))?;
        let context =
            ArithmeticContext::new().map_err(|e| format!("mapped mutation context: {e:?}"))?;
        let evaluated = case
            .heap
            .select_mapped_reading(&plan, &context, &mut ledger()?)
            .map_err(|e| format!("mapped mutation evaluation: {e:?}"))?;
        let values = case
            .heap
            .reading_entries(&case.bindings[index])
            .map_err(|e| format!("mapped mutation values: {e:?}"))?;
        case.heap
            .replace_reading_entries(&case.bindings[index], &values, &mut ledger()?)
            .map_err(|e| format!("mapped mutation write: {e:?}"))?;
        assert!(evaluated.values(&case.heap, &context).is_err());
        assert!(evaluated.totals(&case.heap, &context).is_err());
        assert!(evaluated.errors(&case.heap, &context).is_err());
        assert!(
            case.heap
                .select_mapped_reading(&plan, &context, &mut ledger()?)
                .is_err()
        );
    }
    Ok(())
}

/// Equal units, another selected copy and duplicated covariance cannot replace exact target identities.
/// ´test:integration:mapped-selection-refuses-wrong-target-and-duplicate-companion´
#[test]
fn mapped_selection_refuses_wrong_target_and_duplicate_companion() -> TestResult {
    let case = fixture::case::<f64>(true).map_err(|e| format!("mapped refusal fixture: {e:?}"))?;
    let execution = case
        .execution(&mut ledger()?)
        .map_err(|e| format!("mapped refusal execution: {e:?}"))?;
    assert!(
        ReadingSelection::new(
            case.bindings[0].clone(),
            vec![execution.clone()],
            LiteralLimits::default(),
            &mut ledger()?
        )
        .is_err()
    );
    assert!(
        ReadingSelection::new(
            case.bindings[1].clone(),
            vec![execution.clone(), execution],
            LiteralLimits::default(),
            &mut ledger()?
        )
        .is_err()
    );
    Ok(())
}

/// Measure completed selection rather than subtracting incomplete refusal charges.
/// ´test:integration:mapped-selection-costs-and-boundaries´
#[test]
fn mapped_selection_costs_and_boundaries() -> TestResult {
    let profiles = [
        (false, [1726, 1200, 44_872, 91_520, 45_064]),
        (true, [2546, 1232, 63_912, 129_984, 64_136]),
    ];
    for (covariance, expected) in profiles {
        let case =
            fixture::case::<f64>(covariance).map_err(|e| format!("mapped cost fixture: {e:?}"))?;
        let execution = case
            .execution(&mut ledger()?)
            .map_err(|e| format!("mapped cost execution: {e:?}"))?;
        let plan = ReadingSelection::new(
            case.bindings[1].clone(),
            vec![execution],
            LiteralLimits::default(),
            &mut ledger()?,
        )
        .map_err(|e| format!("mapped cost plan: {e:?}"))?;
        let context =
            ArithmeticContext::new().map_err(|e| format!("mapped cost context: {e:?}"))?;
        let mut budget = ledger().map_err(|e| format!("mapped cost budget: {e:?}"))?;
        let evaluated = case
            .heap
            .select_mapped_reading(&plan, &context, &mut budget)
            .map_err(|e| format!("mapped cost evaluation: {e:?}"))?;
        assert_eq!(
            [
                evaluated.work(),
                evaluated.owned_bytes(),
                plan.retained_payload_bytes()
            ],
            [expected[0], expected[1], expected[2]]
        );
        boundaries(
            &case,
            &plan,
            &context,
            [expected[0], expected[3], expected[4]],
        )?;
    }
    Ok(())
}

/// A declared output companion cannot disappear or become a second source-map application.
/// ´test:integration:mapped-selection-requires-declared-companion-inventory´
#[test]
fn mapped_selection_requires_declared_companion_inventory() -> TestResult {
    for covariance in [false, true] {
        let mut case = fixture::attached::<f64>(covariance)
            .map_err(|e| format!("attached selection fixture: {e:?}"))?;
        let source = case.bindings[1].clone();
        assert!(
            ReadingSelection::new(
                source.clone(),
                Vec::new(),
                LiteralLimits::default(),
                &mut ledger()?
            )
            .is_err()
        );
        let execution = case
            .execution(&mut ledger()?)
            .map_err(|e| format!("attached source execution: {e:?}"))?;
        let selection = ReadingSelection::new(
            source.clone(),
            vec![execution],
            LiteralLimits::default(),
            &mut ledger()?,
        )
        .map_err(|e| format!("attached selection plan: {e:?}"))?;
        let context =
            ArithmeticContext::new().map_err(|e| format!("attached selection context: {e:?}"))?;
        let evaluated = case
            .heap
            .select_mapped_reading(&selection, &context, &mut ledger()?)
            .map_err(|e| format!("attached selection evaluation: {e:?}"))?;
        assert_eq!(evaluated.companions().len(), 1);
        let values = case
            .heap
            .reading_companion_entries(&source, 0)
            .map_err(|e| format!("attached companion values: {e:?}"))?;
        case.heap
            .replace_reading_companion_entries(&source, 0, &values, &mut ledger()?)
            .map_err(|e| format!("attached companion replacement: {e:?}"))?;
        assert!(evaluated.values(&case.heap, &context).is_err());
    }
    Ok(())
}

/// Total and numerical-only source radii pass through the absolute map as distinct channels.
/// ´test:integration:mapped-selection-keeps-numerical-errors-separate´
#[test]
fn mapped_selection_keeps_numerical_errors_separate() -> TestResult {
    let case = fixture::channels().map_err(|e| format!("mapped channels fixture: {e:?}"))?;
    assert!(
        ReadingSelection::new(
            case.source.clone(),
            vec![case.executions[1].clone()],
            LiteralLimits::default(),
            &mut ledger()?
        )
        .is_err()
    );
    let plan = ReadingSelection::new(
        case.source,
        case.executions,
        LiteralLimits::default(),
        &mut ledger()?,
    )
    .map_err(|e| format!("mapped channels plan: {e:?}"))?;
    let context =
        ArithmeticContext::new().map_err(|e| format!("mapped channels context: {e:?}"))?;
    let evaluated = case
        .heap
        .select_mapped_reading(&plan, &context, &mut ledger()?)
        .map_err(|e| format!("mapped channels evaluation: {e:?}"))?;
    let totals = evaluated
        .totals(&case.heap, &context)
        .map_err(|e| format!("mapped channel totals: {e:?}"))?;
    let errors = evaluated
        .errors(&case.heap, &context)
        .map_err(|e| format!("mapped channel errors: {e:?}"))?;
    for ((total, error), (top, bottom, upper)) in totals
        .iter()
        .zip(errors)
        .zip([(5, 32, 0.157), (27, 64, 0.423)])
    {
        assert!(Rational::new(top, bottom)?.at_most(&Rational::binary(*error)?)?);
        assert!(*error < upper);
        assert!(*total > 3.9 * *error && *total < 4.1 * *error);
    }
    Ok(())
}

/// Negative value coefficients preserve signs in values and use absolute factors for radii.
/// ´test:integration:mapped-selection-uses-absolute-negative-radius-map´
#[test]
fn mapped_selection_uses_absolute_negative_radius_map() -> TestResult {
    let case = fixture::negative_radius().map_err(|e| format!("negative mapped fixture: {e:?}"))?;
    let execution = case
        .execution(&mut ledger()?)
        .map_err(|e| format!("negative mapped execution: {e:?}"))?;
    let selection = ReadingSelection::new(
        case.bindings[2].clone(),
        vec![execution],
        LiteralLimits::default(),
        &mut ledger()?,
    )
    .map_err(|e| format!("negative mapped plan: {e:?}"))?;
    let context =
        ArithmeticContext::new().map_err(|e| format!("negative mapped context: {e:?}"))?;
    let evaluated = case
        .heap
        .select_mapped_reading(&selection, &context, &mut ledger()?)
        .map_err(|e| format!("negative mapped evaluation: {e:?}"))?;
    let totals = evaluated
        .totals(&case.heap, &context)
        .map_err(|e| format!("negative mapped totals: {e:?}"))?;
    let values = evaluated
        .values(&case.heap, &context)
        .map_err(|e| format!("negative mapped values: {e:?}"))?;
    for ((value, total), (center, top, bottom, upper)) in values
        .iter()
        .zip(totals)
        .zip([(-8, 17, 4, 4.251), (15, 65, 16, 4.064)])
    {
        assert_eq!(value.to_bits(), f64::from(center).to_bits());
        assert!(Rational::new(top, bottom)?.at_most(&Rational::binary(*total)?)?);
        assert!(*total < upper);
    }
    Ok(())
}

/// Inconsistent transformed parameter storage cannot replace the original bound or covariance source.
/// ´test:integration:mapped-selection-refuses-inconsistent-output-parameters´
#[test]
fn mapped_selection_refuses_inconsistent_output_parameters() -> TestResult {
    for covariance in [false, true] {
        let case = fixture::inconsistent(covariance)
            .map_err(|e| format!("inconsistent mapped fixture: {e:?}"))?;
        let execution = case
            .execution(&mut ledger()?)
            .map_err(|e| format!("inconsistent source execution: {e:?}"))?;
        let selection = ReadingSelection::new(
            case.bindings[1].clone(),
            vec![execution],
            LiteralLimits::default(),
            &mut ledger()?,
        )
        .map_err(|e| format!("inconsistent mapped plan: {e:?}"))?;
        let context =
            ArithmeticContext::new().map_err(|e| format!("inconsistent mapped context: {e:?}"))?;
        assert!(matches!(
            case.heap
                .select_mapped_reading(&selection, &context, &mut ledger()?),
            Err(nrz_matrix::CompanionExecutionError::Identity)
        ));
    }
    Ok(())
}

/// Every measured ceiling succeeds at N and refuses at N-minus-one without resetting prior Work.
fn boundaries(
    case: &fixture::Case<f64>,
    plan: &ReadingSelection<f64>,
    context: &ArithmeticContext,
    expected: [u64; 3],
) -> TestResult {
    for (kind, needed) in expected.into_iter().enumerate() {
        for (limit, success) in [(needed, true), (needed - 1, false)] {
            let mut budget = selection_policy(kind, limit, 0)?;
            assert_eq!(
                case.heap
                    .select_mapped_reading(plan, context, &mut budget)
                    .is_ok(),
                success
            );
        }
    }
    let mut budget = selection_policy(0, expected[0], 1)?;
    assert!(
        case.heap
            .select_mapped_reading(plan, context, &mut budget)
            .is_err()
    );
    for (kind, needed) in [(0, 1), (2, plan.retained_payload_bytes())] {
        for (limit, success) in [(needed, true), (needed - 1, false)] {
            let mut budget = selection_policy(kind, limit, 0)?;
            assert_eq!(
                ReadingSelection::new(
                    plan.source().clone(),
                    plan.companions().to_vec(),
                    LiteralLimits::default(),
                    &mut budget
                )
                .is_ok(),
                success
            );
        }
    }
    Ok(())
}

/// A changed caller ceiling never changes a production default or clears earlier usage.
fn selection_policy(kind: usize, limit: u64, prior: u64) -> TestResult<nrz_bindings::Budget> {
    let mut limits = nrz_bindings::Limits::default();
    match kind {
        0 => limits.work = limit,
        1 => limits.staged_payload_bytes = limit,
        2 => limits.output_bytes = limit,
        _ => return Err("selection policy kind".into()),
    }
    let mut budget = nrz_bindings::Budget::new(limits)
        .map_err(|e| format!("selection boundary budget: {e:?}"))?;
    budget
        .work(
            prior,
            nrz_bindings::readings::algebra::Location::new("selection prior work"),
        )
        .map_err(|e| format!("selection prior work: {e:?}"))?;
    Ok(budget)
}
