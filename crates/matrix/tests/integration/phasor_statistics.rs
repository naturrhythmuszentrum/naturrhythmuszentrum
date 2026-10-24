// SPDX-License-Identifier: AGPL-3.0-only
//! Shared-voltage finite-moment propagation retains signed cross-output covariance and source identity.

mod blocks;
mod costs;
mod fixture;
mod lc_acceptance;
mod parameters;
mod refusals;
use crate::reading_testing::{TestResult, ledger};
use crate::reciprocal_testing::oracle::Rational;
use nrz_scalar::{ArithmeticContext, Scalar};

/// One source mean/variance gives exact opposite currents and a negative cross-covariance.
/// ´test:integration:shared-voltage-phasor-statistics-match-rational-oracles´
#[test]
fn shared_voltage_phasor_statistics_match_rational_oracles() -> TestResult {
    containment::<f32>()?;
    containment::<f64>()
}

/// Numerical covariance error is compared independently from the physical covariance magnitude.
fn containment<P: Scalar + 'static>() -> TestResult {
    let mut case = fixture::case::<P>()?;
    let means = case.means(&mut ledger()?)?;
    let executions = case.executions(&means, &mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    let result = case
        .heap
        .evaluate_phasor_statistics(&executions, &context, &mut ledger()?)?;
    let values = result.means(&case.heap, &context)?;
    for (index, numerator) in [1, -1].into_iter().enumerate() {
        let radius = means[index].evaluation().errors(&case.heap, &context)?[1];
        assert!(Rational::new(numerator, 25)?.covered(values[index].to_f64(), radius.to_f64())?);
    }
    let moments = result.moments(&case.heap, &context)?;
    let errors = result.parameter_errors(&case.heap, &context)?;
    for (index, numerator) in [1, -1, 1].into_iter().enumerate() {
        assert!(
            Rational::new(numerator, 22_500)?
                .covered(moments[index].to_f64(), errors[index].to_f64())?
        );
    }
    assert!(moments[1] < P::ZERO);
    assert!(!Rational::new(0, 1)?.covered(moments[1].to_f64(), errors[1].to_f64())?);
    for evaluation in result.evaluations() {
        assert_eq!(
            evaluation.exact().source_target(),
            result.evaluations()[0].exact().source_target()
        );
        let law = evaluation
            .exact()
            .target()
            .phasor_law()
            .ok_or("exact affine law")?;
        assert_eq!(
            law.equations()[0].voltage().reading().family().root(),
            law.equations()[1].voltage().reading().family().root()
        );
    }
    assert_eq!(
        case.bindings[11].reading().family().root(),
        case.bindings[14].reading().family().root()
    );
    assert!(
        case.heap.reading_entries(&case.bindings[11])?[0]
            .same_bits(case.heap.reading_entries(&case.bindings[14])?[0])
    );
    Ok(())
}

/// A changed shared source or either original parameter channel invalidates the whole immutable family.
/// ´test:integration:shared-voltage-phasor-statistics-recheck-all-source-versions´
#[test]
fn shared_voltage_phasor_statistics_recheck_all_source_versions() -> TestResult {
    for index in [10, 15, 16] {
        let mut case = fixture::case::<f64>()?;
        let means = case.means(&mut ledger()?)?;
        let executions = case.executions(&means, &mut ledger()?)?;
        let context = ArithmeticContext::new()?;
        let result = case
            .heap
            .evaluate_phasor_statistics(&executions, &context, &mut ledger()?)?;
        let values = case.heap.reading_entries(&case.bindings[index])?;
        case.heap
            .replace_reading_entries(&case.bindings[index], &values, &mut ledger()?)?;
        assert!(result.means(&case.heap, &context).is_err());
        assert!(result.moments(&case.heap, &context).is_err());
        assert!(result.parameter_errors(&case.heap, &context).is_err());
    }
    Ok(())
}

/// Replacing the cross block with another marginal cannot silently assert independence.
/// ´test:integration:shared-voltage-phasor-statistics-refuse-wrong-cross-targets´
#[test]
fn shared_voltage_phasor_statistics_refuse_wrong_cross_targets() -> TestResult {
    let mut case = fixture::case::<f64>()?;
    let means = case.means(&mut ledger()?)?;
    let executions = case.executions(&means, &mut ledger()?)?;
    let wrong = [
        executions[0].clone(),
        executions[0].clone(),
        executions[2].clone(),
    ];
    assert!(
        case.heap
            .evaluate_phasor_statistics(&wrong, &ArithmeticContext::new()?, &mut ledger()?)
            .is_err()
    );
    Ok(())
}

/// Complete family evaluation preserves earlier caller charges and refuses one Work below its boundary.
/// ´test:integration:shared-voltage-phasor-statistics-have-cumulative-boundaries´
#[test]
fn shared_voltage_phasor_statistics_have_cumulative_boundaries() -> TestResult {
    let mut case = fixture::case::<f64>()?;
    let means = case.means(&mut ledger()?)?;
    let executions = case.executions(&means, &mut ledger()?)?;
    let context = ArithmeticContext::new()?;
    let mut budget = ledger()?;
    case.heap
        .evaluate_phasor_statistics(&executions, &context, &mut budget)?;
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
            nrz_bindings::readings::algebra::Location::new("earlier shared-source work"),
        )?;
        assert_eq!(
            case.heap
                .evaluate_phasor_statistics(&executions, &context, &mut budget)
                .is_ok(),
            success
        );
    }
    Ok(())
}

/// The two physical solves and every shared-source covariance operation retain one caller budget.
/// ´test:integration:shared-voltage-phasor-construction-has-cumulative-boundaries´
#[test]
fn shared_voltage_phasor_construction_has_cumulative_boundaries() -> TestResult {
    let needed = complete(crate::phasor_solve_testing::policy(), 0)?;
    for (limit, prior, success) in [
        (needed, 0, true),
        (needed - 1, 0, false),
        (needed, 1, false),
    ] {
        let policy = nrz_bindings::Limits {
            work: limit,
            ..crate::phasor_solve_testing::policy()
        };
        assert_eq!(complete(policy, prior).is_ok(), success);
    }
    Ok(())
}

/// Rebuild exact common-source identities and publish both means before the atomic statistical snapshot.
fn complete(policy: nrz_bindings::Limits, prior: u64) -> TestResult<u64> {
    let mut budget = nrz_bindings::Budget::new(policy)?;
    budget.work(
        prior,
        nrz_bindings::readings::algebra::Location::new("earlier covariance work"),
    )?;
    let mut case = fixture::case_with_budget::<f64>(&mut budget)?;
    assert!(case.construction.windows(2).all(|pair| pair[0] <= pair[1]));
    let means = case.means(&mut budget)?;
    let executions = case.executions(&means, &mut budget)?;
    case.heap
        .evaluate_phasor_statistics(&executions, &ArithmeticContext::new()?, &mut budget)?;
    Ok(budget.usage().work)
}

/// Either restored mean is a required live target even when its replacement has identical bits.
/// ´test:integration:shared-voltage-phasor-statistics-recheck-both-mean-versions´
#[test]
fn shared_voltage_phasor_statistics_recheck_both_mean_versions() -> TestResult {
    for index in 0..2 {
        let mut case = fixture::case::<f64>()?;
        let means = case.means(&mut ledger()?)?;
        let executions = case.executions(&means, &mut ledger()?)?;
        let context = ArithmeticContext::new()?;
        let result = case
            .heap
            .evaluate_phasor_statistics(&executions, &context, &mut ledger()?)?;
        let target = &means[index].readings().bindings()[0];
        let values = case.heap.reading_entries(target)?;
        case.heap
            .replace_reading_entries(target, &values, &mut ledger()?)?;
        assert!(result.means(&case.heap, &context).is_err());
        assert!(result.moments(&case.heap, &context).is_err());
        assert!(result.parameter_errors(&case.heap, &context).is_err());
    }
    Ok(())
}
