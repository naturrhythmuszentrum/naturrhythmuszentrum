// SPDX-License-Identifier: AGPL-3.0-only
//! Checked native Jacobians at named operating points with full smooth-domain checks.
//!
//! # Nonlinear moments and numerical obligations
//!
//! Matrix products propagate the full joint covariance of both ordered entry vectors through the operation-specific Jacobian. When both operands vary, the result carries `MomentAssurance::LinearizedMoments`; an explicitly modeled zero-variation factor with an exact declared mean at the operating point preserves `AffineMoments`. Missing source coverage cannot stand in for that fixed-factor declaration. Native square, reciprocal and positive-root kernels retain a bindings-owned `CheckedSensitivity`, exposed through `MomentView::checked_sensitivity`, and evaluate its closed derivative schedule with a separate certified Jacobian error region. No heap numbers enter the symbolic graph.
//!
//! Nonlinear covariance describes the stated local linearization, never the actual nonlinear moments. These results have no declared output mean; even when input means are known, the nominal output is not promoted to an expectation. Subsequent affine operations retain inherited `LinearizedMoments`. Reciprocal requires a whole deterministic box separated from zero. Statistical square root requires a strictly positive operating point and neighborhood, so it refuses at zero even when deterministic square root is valid. The deterministic total continues to use the full direct interval image, including nonlinear remainder; statistical covariance never enters that total.

use super::{
    StatisticsError,
    affine::Affine,
    matrix::{Matrix, singleton},
};
use crate::{CheckedOperation, Elementwise, OperationError, binding::NativeRecipe};
use nrz_bindings::{CheckedSensitivity, SensitivityKernel};
use nrz_scalar::{ArithmeticContext, Enclosure, Scalar};

/// Discharge the derivative's numerical premises on the same complete box used by D.
pub(crate) fn map<P: Scalar + 'static, Q: Scalar + 'static>(
    operation: &CheckedOperation,
    slot: usize,
    points: &[Matrix<P>],
    derivative: Option<&CheckedSensitivity>,
    context: &ArithmeticContext,
) -> Result<Affine<Q>, OperationError> {
    if let NativeRecipe::Kernel(contract) = &operation.recipe {
        match contract.kind() {
            crate::KernelKind::Product => return super::product::map(operation, points, context),
            crate::KernelKind::Elementwise(function) => {
                let proof = derivative.ok_or(StatisticsError::Contract)?;
                return elementwise(
                    points.first().ok_or(StatisticsError::Contract)?,
                    function,
                    proof,
                    context,
                );
            }
            _ => {}
        }
    }
    super::affine::map(operation, slot, context)
}

/// One replayed scalar proof authorizes each diagonal entry, with separately checked typed zeros.
fn elementwise<P: Scalar + 'static, Q: Scalar + 'static>(
    point: &Matrix<P>,
    function: Elementwise,
    proof: &CheckedSensitivity,
    context: &ArithmeticContext,
) -> Result<Affine<Q>, OperationError> {
    let count = point.extent.entries();
    let kernel = proof
        .entries()
        .first()
        .ok_or(StatisticsError::Contract)?
        .kernel();
    let mut result = Affine {
        jacobian: Matrix::zeros(count, count)?,
        offsets: Matrix::zeros(count, 1)?,
    };
    for (entry, (&center, &domain)) in point.centers.iter().zip(&point.truth).enumerate() {
        smooth(center, domain, function, entry)?;
        let center = crate::conversion::same_format(center)?;
        let (candidate, truth) = coefficient(center, kernel, context)?;
        result.jacobian.set(entry, entry, candidate, truth)?;
    }
    Ok(result)
}

/// A positive operating root needs a strictly positive whole smooth neighborhood.
fn smooth<P: Scalar>(
    center: P,
    domain: Enclosure<P>,
    function: Elementwise,
    entry: usize,
) -> Result<(), OperationError> {
    let admitted = match function {
        Elementwise::Square => true,
        Elementwise::Reciprocal => domain.lower().get() > P::ZERO || domain.upper().get() < P::ZERO,
        Elementwise::SquareRoot => center > P::ZERO && domain.lower().get() > P::ZERO,
    };
    if !admitted {
        return Err(StatisticsError::Domain { function, entry }.into());
    }
    Ok(())
}

/// Evaluate exactly J's closed schedule, retaining both the represented derivative and its error.
fn coefficient<P: Scalar>(
    center: P,
    kernel: SensitivityKernel,
    context: &ArithmeticContext,
) -> Result<(P, Enclosure<P>), OperationError> {
    let nominal = singleton(center)?;
    let two = P::from_u8(2);
    Ok(match kernel {
        SensitivityKernel::Square { coordinate: 0 } => {
            (two * center, singleton(two)?.mul(nominal, context)?)
        }
        SensitivityKernel::Reciprocal { coordinate: 0 } => (
            -P::ONE / (center * center),
            singleton(-P::ONE)?.div(nominal.square(context)?, context)?,
        ),
        SensitivityKernel::PositiveRoot {
            coordinate: 0,
            output: 0,
        } => (
            P::ONE / (two * center.sqrt()),
            singleton(P::ONE)?.div(
                singleton(two)?.mul(nominal.sqrt(context)?, context)?,
                context,
            )?,
        ),
        _ => return Err(StatisticsError::Contract.into()),
    })
}

#[cfg(test)]
mod tests {
    use super::coefficient;
    use crate::kernel_testing::TestResult;
    use crate::{OperationError, StatisticsError};
    use nrz_bindings::SensitivityKernel;
    use nrz_scalar::ArithmeticContext;

    /// A checked relation cannot authorize a different numerical coordinate schedule.
    /// ´test:unit:native-derivative-schedule-rejects-unmatched-coordinates´
    #[test]
    fn native_derivative_schedule_rejects_unmatched_coordinates() -> TestResult {
        let context = ArithmeticContext::new()?;
        for kernel in [
            SensitivityKernel::Square { coordinate: 1 },
            SensitivityKernel::PositiveRoot {
                coordinate: 0,
                output: 1,
            },
        ] {
            assert!(matches!(
                coefficient(2.0_f64, kernel, &context),
                Err(OperationError::Statistics(StatisticsError::Contract))
            ));
        }
        Ok(())
    }
}
