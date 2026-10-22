// SPDX-License-Identifier: AGPL-3.0-only
//! Statistical solves require complete joint models and bounded derivative work.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{execute, output};
use crate::solve_testing::{TestResult, draft};
use crate::statistics_testing::{check, scalar};
use nrz_bindings::Descriptor;
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationError, StatisticsError,
};

/// Repeated A and B retain one source; their differential contributions cancel.
/// ´test:integration:square-solve-preserves-identical-operand-source´
#[test]
fn square_solve_preserves_identical_operand_source() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = scalar::<f64>(&mut heap)?;
    let result = output(&execute(&mut heap, draft(1, 1)?, vec![source.value(); 2])?)?;
    let moments = heap.moments(heap.moment_companion(result.value())?)?;
    check(moments.covariance(), &[Rational::new(0, 1)?])?;
    check(
        moments.jacobian().ok_or("Jacobian")?,
        &[Rational::new(-1, 2)?, Rational::new(1, 2)?],
    )?;
    assert!(moments.means().is_none());
    Ok(())
}
/// Missing source declarations and separate models refuse without inventing zero cross blocks.
/// ´test:integration:square-solve-requires-a-complete-joint-model´
#[test]
fn square_solve_requires_a_complete_joint_model() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let operator = scalar::<f64>(&mut heap)?;
    let missing = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
    let separate = scalar::<f64>(&mut heap)?;
    let limits = ExecutionLimits::default();
    let operation = draft(1, 1)?.check(limits)?;
    assert!(matches!(
        Binding::new(
            operation.clone(),
            vec![operator.value(), missing.value()],
            limits
        )
        .validate(&mut heap),
        Err(OperationError::Statistics(StatisticsError::MissingSource {
            slot: 1,
            ..
        }))
    ));
    assert!(matches!(
        Binding::new(operation, vec![operator.value(), separate.value()], limits)
            .validate(&mut heap),
        Err(OperationError::Statistics(
            StatisticsError::MissingJointModel
        ))
    ));
    assert_eq!(heap.provenance_usage().records(), 0);
    Ok(())
}
/// Differential solve costs are included in the statistical phase's exact admission boundary.
/// ´test:integration:solve-sensitivity-work-below-at-and-above´
#[test]
fn solve_sensitivity_work_below_at_and_above() -> TestResult {
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = scalar::<f64>(&mut heap)?;
    let operation = draft(1, 1)?.check(ExecutionLimits::default())?;
    let required = Binding::new(
        operation.clone(),
        vec![source.value(); 2],
        ExecutionLimits::default(),
    )
    .validate(&mut heap)?
    .statistical_work();
    for allowance in [required - 1, required, required + 1] {
        let result = Binding::new(
            operation.clone(),
            vec![source.value(); 2],
            ExecutionLimits::new(2, allowance, allowance),
        )
        .validate(&mut heap);
        if allowance < required {
            assert!(matches!(result, Err(OperationError::ResourceLimit)));
        } else {
            let bound = result?;
            let candidate = bound.evaluate(bound.normalized_inputs()?)?;
            let outcome = bound.publish(candidate)?;
            let value = output(&outcome)?.value();
            check(
                heap.moments(heap.moment_companion(value)?)?.covariance(),
                &[Rational::new(0, 1)?],
            )?;
        }
    }
    Ok(())
}

/// A failed covariance contraction retains every already valid differential certificate.
/// ´test:integration:solve-covariance-refusal-retains-completed-sensitivities´
#[test]
fn solve_covariance_refusal_retains_completed_sensitivities() -> TestResult {
    use nrz_matrix::{
        CompanionSetDraft, CovarianceBlock, JointLimits, JointModel, RecipeEvidence, Source,
        SourceFactor, TargetMap,
    };
    let mut heap = Heap::new(HeapLimits::default())?;
    let quantity = Descriptor::ratio();
    let operator = heap.import_represented(Extent::new(1, 1)?, &quantity, &[0.5_f64])?;
    let rhs = heap.import_represented(Extent::new(1, 1)?, &quantity, &[1.0_f64])?;
    let sources = [Source::new(&quantity, 1)?, Source::new(&quantity, 1)?];
    let joint = JointLimits::default();
    let model = JointModel::from_factor(
        &sources,
        SourceFactor::new(2, &[1.0, 0.0], &[1.0e308], joint)?,
        &std::array::from_fn::<_, 4, _>(|_| CovarianceBlock::Known),
        joint,
    )?;
    let targets = [
        TargetMap::capture(&heap, operator.value())?,
        TargetMap::capture(&heap, rhs.value())?,
    ];
    heap.publish_joint(
        CompanionSetDraft::new(model, targets, [vec![0], vec![1]]),
        joint,
    )?;
    let limits = ExecutionLimits::default();
    let bound = Binding::new(
        draft(1, 1)?.check(limits)?,
        vec![operator.value(), rhs.value()],
        limits,
    )
    .validate(&mut heap)?;
    let failure = bound
        .evaluate(bound.normalized_inputs()?)
        .err()
        .ok_or("overflowing covariance published")?;
    assert!(matches!(
        failure.cause(),
        OperationError::Statistics(StatisticsError::Arithmetic(_))
    ));
    let Some(RecipeEvidence::Solve(claim)) = failure.record().historical_evidence().first() else {
        return Err("solve evidence".into());
    };
    assert!(claim.is_complete());
    assert_eq!(claim.sensitivities().len(), 2);
    for (direction, proof) in claim.sensitivities().iter().enumerate() {
        assert_eq!(proof.direction(), direction);
        assert!(proof.residual_norm() >= 0.0 && proof.radius() >= 0.0);
    }
    drop(bound);
    assert_eq!(heap.provenance_usage().records(), 0);
    Ok(())
}
