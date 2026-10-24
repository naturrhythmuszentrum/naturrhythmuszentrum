// SPDX-License-Identifier: AGPL-3.0-only
//! Source-bound parameter maps retain original symbols, two-sided covariance and live versions.

use crate::mapped_testing as fixture;
mod selection;

use crate::reading_testing::{TestResult, ledger};
use nrz_matrix::{CompanionExecution, CompanionExecutionError};
use nrz_scalar::{ArithmeticContext, LiteralLimits, Scalar};
use std::rc::Rc;

/// Both covariance target maps are applied once to the original four source parameters.
/// ´test:integration:companion-execution-restores-both-covariance-axes´
#[test]
fn companion_execution_restores_both_covariance_axes() -> TestResult {
    covariance::<f32>()?;
    covariance::<f64>()
}

/// Independent entrywise products give the exact covariance and propagated source error.
fn covariance<P: Scalar + 'static>() -> TestResult {
    let case = fixture::case::<P>(true)?;
    let execution = case.execution(&mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    let evaluated = case
        .heap
        .evaluate_companion(&execution, &context, &mut ledger()?)?;
    let values = evaluated.values(&case.heap, &context)?;
    let errors = evaluated.errors(&case.heap, &context)?;
    let expected = [-2.0, -1.5, 1.25, 15.0];
    let source_errors = [0.5, 0.75, 0.625, 0.9375];
    for ((value, error), (target, prior)) in values
        .iter()
        .zip(errors)
        .zip(expected.into_iter().zip(source_errors))
    {
        let center = value.to_f64();
        let radius = error.to_f64();
        assert!(center - radius <= target - prior && center + radius >= target + prior);
        assert!(radius < prior + 0.001);
    }
    assert_eq!(
        evaluated.targets()[0].reading().family().root(),
        evaluated.targets()[1].reading().family().root()
    );
    assert_eq!(
        evaluated.sources()[0].reading().family().root(),
        evaluated.exact().source_target()
    );
    // Independence would erase this cross term, outside the certified interval.
    assert!(values[0].to_f64() + errors[0].to_f64() < 0.0);
    Ok(())
}

/// Radius maps use selected-copy parameters and propagate numerical error in those parameters.
/// ´test:integration:companion-execution-preserves-radius-parameter-errors´
#[test]
fn companion_execution_preserves_radius_parameter_errors() -> TestResult {
    let case = fixture::case::<f64>(false)?;
    let execution = case.execution(&mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    let evaluated = case
        .heap
        .evaluate_companion(&execution, &context, &mut ledger()?)?;
    for ((value, error), (target, prior)) in evaluated
        .values(&case.heap, &context)?
        .iter()
        .zip(evaluated.errors(&case.heap, &context)?)
        .zip([(0.5, 0.125), (1.5, 0.1875)])
    {
        assert!(value - error <= target - prior && value + error >= target + prior);
        assert!(*error < prior + 0.000_001);
    }
    Ok(())
}

/// Equal shapes and units cannot substitute output coordinates for original source symbols.
/// ´test:integration:companion-execution-refuses-substituted-sources-and-targets´
#[test]
fn companion_execution_refuses_substituted_sources_and_targets() -> TestResult {
    let case = fixture::case::<f64>(true)?;
    let source = [case.bindings[3].clone(), case.bindings[4].clone()];
    let swapped = vec![case.bindings[2].clone(), case.bindings[1].clone()];
    assert!(matches!(
        CompanionExecution::new(
            Rc::clone(&case.exact),
            source.clone(),
            swapped,
            LiteralLimits::default(),
            &mut ledger()?
        ),
        Err(CompanionExecutionError::Identity)
    ));
    let wrong = [source[1].clone(), source[0].clone()];
    assert!(matches!(
        CompanionExecution::new(
            Rc::clone(&case.exact),
            wrong,
            case.bindings[1..3].to_vec(),
            LiteralLimits::default(),
            &mut ledger()?
        ),
        Err(CompanionExecutionError::Definition(_))
    ));
    let foreign = fixture::case::<f64>(true)?;
    assert!(
        CompanionExecution::new(
            Rc::clone(&case.exact),
            [foreign.bindings[3].clone(), foreign.bindings[4].clone()],
            case.bindings[1..3].to_vec(),
            LiteralLimits::default(),
            &mut ledger()?
        )
        .is_err()
    );
    Ok(())
}

/// Writes to either covariance target or either parameter channel stale the complete result.
/// ´test:integration:companion-execution-rechecks-every-source-and-target´
#[test]
fn companion_execution_rechecks_every_source_and_target() -> TestResult {
    for index in [0, 3, 4] {
        let mut case = fixture::case::<f64>(true)?;
        let execution = case.execution(&mut ledger()?)?;
        let context = ArithmeticContext::new()?;
        let evaluated = case
            .heap
            .evaluate_companion(&execution, &context, &mut ledger()?)?;
        let values = case.heap.reading_entries(&case.bindings[index])?;
        case.heap
            .replace_reading_entries(&case.bindings[index], &values, &mut ledger()?)?;
        assert!(evaluated.values(&case.heap, &context).is_err());
        assert!(evaluated.errors(&case.heap, &context).is_err());
        assert!(
            case.heap
                .evaluate_companion(&execution, &context, &mut ledger()?)
                .is_err()
        );
    }
    Ok(())
}

/// Exact N and N-minus-one bounds include earlier caller work and retained metadata.
/// ´test:integration:companion-execution-keeps-cumulative-work-boundaries´
#[test]
fn companion_execution_keeps_cumulative_work_boundaries() -> TestResult {
    let case = fixture::case::<f64>(true)?;
    let execution = case.execution(&mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    let mut budget = ledger()?;
    case.heap
        .evaluate_companion(&execution, &context, &mut budget)?;
    let needed = budget.usage().work;
    assert!(needed > 0);
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
            nrz_bindings::readings::algebra::Location::new("companion precharge"),
        )?;
        assert_eq!(
            case.heap
                .evaluate_companion(&execution, &context, &mut budget)
                .is_ok(),
            success
        );
    }
    Ok(())
}

/// Every source radius and parameter error is checked even when its exact coefficient is zero.
/// ´test:integration:companion-execution-refuses-negative-unused-parameters´
#[test]
fn companion_execution_refuses_negative_unused_parameters() -> TestResult {
    for (covariance, error) in [(false, false), (false, true), (true, true)] {
        let case = fixture::invalid::<f64>(covariance, error)?;
        let plan = case.execution(&mut ledger()?)?;
        assert!(matches!(
            case.heap
                .evaluate_companion(&plan, &ArithmeticContext::new()?, &mut ledger()?),
            Err(CompanionExecutionError::Identity)
        ));
    }
    Ok(())
}

/// Preparation itself consumes the caller ledger and reserves retained exact metadata.
/// ´test:integration:companion-preparation-work-and-payload-boundaries´
#[test]
fn companion_preparation_work_and_payload_boundaries() -> TestResult {
    let case = fixture::case::<f64>(true)?;
    let mut budget = ledger()?;
    let plan = case.execution(&mut budget)?;
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
            nrz_bindings::readings::algebra::Location::new("companion preparation precharge"),
        )?;
        assert_eq!(case.execution(&mut budget).is_ok(), success);
    }
    let bytes = plan.retained_payload_bytes();
    let mut budget = nrz_bindings::Budget::new(nrz_bindings::Limits {
        output_bytes: bytes,
        ..nrz_bindings::Limits::default()
    })?;
    case.execution(&mut budget)?;
    let mut budget = nrz_bindings::Budget::new(nrz_bindings::Limits {
        output_bytes: bytes - 1,
        ..nrz_bindings::Limits::default()
    })?;
    assert!(case.execution(&mut budget).is_err());
    Ok(())
}
