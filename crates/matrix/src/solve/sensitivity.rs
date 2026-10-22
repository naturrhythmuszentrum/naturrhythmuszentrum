// SPDX-License-Identifier: AGPL-3.0-only
//! Certified differential solves preserve the complete joint operator/RHS source model.

use super::{FactorRef, SolveContract, SolveError};
use crate::statistics::contraction::Work;
use crate::statistics::{Affine, Matrix, MomentPayload, enclosed, singleton};
use crate::{Bound, NormalizedView, OperationError, RecipeEvidence, StatisticsError};
use nrz_bindings::{CheckedSensitivity, Context, Declaration, Descriptor, Evidence, SolveSymbols};
use nrz_scalar::{ArithmeticContext, Enclosure, Scalar};
use std::rc::Rc;

/// Independent residual and forward bounds for one exact nominal differential direction.
#[derive(Clone, Debug, PartialEq)]
pub struct SensitivityEvidence {
    /// Column in concatenated logical operator/RHS entry order.
    direction: usize,
    /// Joint residual upper bound, including the enclosed differential RHS parameters.
    residual: f64,
    /// Joint forward radius under the same certified inverse operator.
    radius: f64,
}
impl SensitivityEvidence {
    /// Logical input coordinate differentiated.
    #[must_use]
    pub const fn direction(&self) -> usize {
        self.direction
    }
    /// Certified residual norm for all solution columns together.
    #[must_use]
    pub const fn residual_norm(&self) -> f64 {
        self.residual
    }
    /// Numerical parameter bound; this is never a physical input radius.
    #[must_use]
    pub const fn radius(&self) -> f64 {
        self.radius
    }
}

/// Admit every differential solve, nominal enclosure and materialized Jacobian before allocation.
pub(crate) fn work(contract: &SolveContract) -> Result<usize, OperationError> {
    let columns = input_entries(contract)?;
    columns
        .checked_add(2)
        .and_then(|count| count.checked_mul(contract.work))
        .ok_or_else(|| SolveError::ResourceLimit.into())
}
/// Preserve the exact concatenated logical input order, excluding identity RHS placeholders.
fn input_entries(contract: &SolveContract) -> Result<usize, OperationError> {
    contract.inputs.iter().try_fold(0_usize, |count, input| {
        count
            .checked_add(input.shape().extent().entries())
            .ok_or_else(|| SolveError::ResourceLimit.into())
    })
}

/// Bindings checks the defining equations and their partials without seeing any heap numbers.
pub(crate) fn check(contract: &SolveContract) -> Result<Rc<CheckedSensitivity>, OperationError> {
    let operator = contract.inputs.first().ok_or(SolveError::Contract)?;
    let order = operator.shape().extent().rows();
    let columns = contract.solution.shape().extent().columns();
    let symbols = SolveSymbols {
        order,
        right_hand_sides: columns,
        operator: (0..order * order)
            .map(|index| format!("a{index}"))
            .collect(),
        right_hand_side: (0..order * columns)
            .map(|index| format!("b{index}"))
            .collect(),
        solution: (0..order * columns)
            .map(|index| format!("x{index}"))
            .collect(),
    };
    let keys = symbols
        .operator
        .iter()
        .chain(&symbols.right_hand_side)
        .chain(&symbols.solution)
        .map(String::as_str)
        .collect::<Vec<_>>();
    let mut builder = Context::with_real_symbols(
        operator.shape().declarations().registry().clone(),
        nrz_bindings::Limits::default(),
        &keys,
    )
    .map_err(|cause| sensitivity_error(cause.into()))?;
    declare(&mut builder, &symbols.operator, operator.descriptor())?;
    let identity = Descriptor::ratio();
    let rhs = contract
        .inputs
        .get(1)
        .map_or(&identity, crate::RegionContract::descriptor);
    declare(&mut builder, &symbols.right_hand_side, rhs)?;
    declare(
        &mut builder,
        &symbols.solution,
        contract.solution.descriptor(),
    )?;
    Ok(Rc::new(
        builder
            .check_solve_sensitivity(&symbols, &nrz_bindings::LimitsOverride::default())
            .map_err(sensitivity_error)?,
    ))
}
/// Every original placeholder receives its physical scalar descriptor before differentiation.
fn declare(
    builder: &mut Context,
    keys: &[String],
    descriptor: &Descriptor,
) -> Result<(), OperationError> {
    for key in keys {
        builder
            .declare(
                key.clone(),
                Declaration::Input {
                    descriptor: descriptor.clone(),
                    nonzero: false,
                    evidence: Evidence::default(),
                },
            )
            .map_err(|cause| sensitivity_error(cause.into()))?;
    }
    Ok(())
}
/// Preserve the typed symbolic cause, including any unresolved relation or domain obligation.
fn sensitivity_error(cause: nrz_bindings::SensitivityError) -> OperationError {
    StatisticsError::Sensitivity(Rc::new(cause)).into()
}

/// Use one factor for the nominal enclosure and every differential, then contract joint sources.
pub(crate) fn evaluate<P: Scalar + 'static, Q: Scalar + 'static>(
    bound: &Bound<'_, P, Q>,
    contract: &SolveContract,
    views: &[NormalizedView<'_, P>],
    factor: &FactorRef<P>,
    evidence: &mut [RecipeEvidence],
) -> Result<Vec<MomentPayload<Q>>, OperationError> {
    if bound.statistics.lineage.is_none() {
        return Ok(Vec::new());
    }
    if bound.statistics.derivative.is_none() {
        return Err(SolveError::Contract.into());
    }
    let context = ArithmeticContext::new()?;
    let mut certificates = Vec::new();
    let map = differential(contract, views, factor, &context, &mut certificates);
    let Some(RecipeEvidence::Solve(claim)) = evidence.last_mut() else {
        return Err(SolveError::Contract.into());
    };
    claim.sensitivities = certificates;
    bound.statistics.solve_moments(transfer(map?)?, &context)
}

/// Retain each certified differential even if a later direction or covariance contraction refuses.
fn differential<P: Scalar + 'static>(
    contract: &SolveContract,
    views: &[NormalizedView<'_, P>],
    factor: &FactorRef<P>,
    context: &ArithmeticContext,
    certificates: &mut Vec<SensitivityEvidence>,
) -> Result<Affine<P>, OperationError> {
    let operator = views.first().ok_or(SolveError::Contract)?;
    let mut counter = Work::new(work(contract)?);
    let rhs = nominal_rhs(contract, views)?;
    let (nominal, _) = enclosed_solve(operator, &rhs, factor, &mut counter, context)?;
    let columns = input_entries(contract)?;
    let mut map = Affine {
        jacobian: Matrix::zeros(nominal.extent.entries(), columns)?,
        offsets: Matrix::zeros(nominal.extent.entries(), 1)?,
    };
    *certificates = crate::binding::buffer(columns)?;
    for direction in 0..columns {
        let rhs = differential_rhs(&nominal, direction)?;
        let (derivative, mut proof) =
            enclosed_solve(operator, &rhs, factor, &mut counter, context)?;
        proof.direction = direction;
        certificates.push(proof);
        for (row, (&center, &truth)) in derivative.centers.iter().zip(&derivative.truth).enumerate()
        {
            map.jacobian.set(row, direction, center, truth)?;
        }
    }
    let multiplier = if contract.identity.is_some() {
        P::from_u8(2)
    } else {
        P::ONE
    };
    for (row, (&center, &truth)) in nominal.centers.iter().zip(&nominal.truth).enumerate() {
        map.offsets.set(
            row,
            0,
            multiplier * center,
            singleton(multiplier)?.mul(truth, context)?,
        )?;
    }
    restore(contract, &mut map, context)?;
    Ok(map)
}
/// Source-free identity columns remain separate from physical right-hand sides.
fn nominal_rhs<P: Scalar + 'static>(
    contract: &SolveContract,
    views: &[NormalizedView<'_, P>],
) -> Result<Matrix<P>, OperationError> {
    let extent = contract.solution.shape().extent();
    let mut result = Matrix::zeros(extent.rows(), extent.columns())?;
    for column in 0..extent.columns() {
        for row in 0..extent.rows() {
            let center = if contract.identity.is_some() {
                if row == column { P::ONE } else { P::ZERO }
            } else {
                views.get(1).ok_or(SolveError::Contract)?.get(row, column)?
            };
            result.set(row, column, center, singleton(center)?)?;
        }
    }
    Ok(result)
}
/// Form `delta B - delta A X_0`, including all common RHS columns for an operator direction.
fn differential_rhs<P: Scalar>(
    nominal: &Matrix<P>,
    direction: usize,
) -> Result<Matrix<P>, OperationError> {
    let order = nominal.extent.rows();
    let square = order.checked_mul(order).ok_or(SolveError::ResourceLimit)?;
    let mut result = Matrix::zeros(order, nominal.extent.columns())?;
    if direction < square {
        for column in 0..nominal.extent.columns() {
            let index = nominal.extent.index(direction / order, column)?;
            let center = -*nominal.centers.get(index).ok_or(SolveError::Contract)?;
            result.set(
                direction % order,
                column,
                center,
                nominal.entry(direction / order, column)?.negated(),
            )?;
        }
    } else {
        let entry = direction - square;
        result.set(entry % order, entry / order, P::ONE, singleton(P::ONE)?)?;
    }
    Ok(result)
}

/// A parameter RHS is enclosed independently; the same inverse proof bounds its exact solution.
pub(crate) fn enclosed_solve<P: Scalar + 'static>(
    operator: &NormalizedView<'_, P>,
    rhs: &Matrix<P>,
    factor: &FactorRef<P>,
    work: &mut Work,
    context: &ArithmeticContext,
) -> Result<(Matrix<P>, SensitivityEvidence), OperationError> {
    let centers = factor.backend.solve_entries(rhs.extent, &rhs.centers)?;
    let mut residuals = crate::binding::buffer(centers.len())?;
    for column in 0..rhs.extent.columns() {
        for row in 0..rhs.extent.rows() {
            let mut residual = rhs.entry(row, column)?;
            for index in 0..operator.extent().columns() {
                work.charge()?;
                let center = *centers
                    .get(rhs.extent.index(index, column)?)
                    .ok_or(SolveError::Contract)?;
                residual = residual.sub(
                    singleton(operator.get(row, index)?)?.mul(singleton(center)?, context)?,
                    context,
                )?;
            }
            residuals.push(residual.abs().upper().get());
        }
    }
    let residual = nrz_scalar::norm_upper(&residuals, context)?.get();
    let radius = singleton(factor.inverse_norm())?
        .mul(singleton(residual)?, context)?
        .upper()
        .get();
    let truth = centers
        .iter()
        .map(|&center| enclosed(center, radius, context))
        .collect::<Result<_, _>>()?;
    Ok((
        Matrix {
            extent: rhs.extent,
            centers,
            truth,
        },
        SensitivityEvidence {
            direction: 0,
            residual: residual.to_f64(),
            radius: radius.to_f64(),
        },
    ))
}
/// Restoration acts on every derivative and both covariance axes; origins affect only offsets.
fn restore<P: Scalar>(
    contract: &SolveContract,
    map: &mut Affine<P>,
    context: &ArithmeticContext,
) -> Result<(), OperationError> {
    let Some(conversion) = &contract.restoration else {
        return Ok(());
    };
    let lowered = crate::coordinate_numeric::LoweredMap::<P>::new(
        &conversion.map,
        conversion.literals,
        context,
    )?;
    for (center, truth) in map.jacobian.centers.iter_mut().zip(&mut map.jacobian.truth) {
        *center = lowered.scale.center().get() * *center;
        *truth = lowered.scale.enclosure().mul(*truth, context)?;
    }
    for (center, truth) in map.offsets.centers.iter_mut().zip(&mut map.offsets.truth) {
        *center = lowered.scale.center().get() * *center + lowered.shift.center().get();
        *truth = lowered
            .scale
            .enclosure()
            .mul(*truth, context)?
            .add(lowered.shift.enclosure(), context)?;
    }
    Ok(())
}
/// Publication keeps the statically distinct output format while refusing implicit conversion.
fn transfer<P: Scalar + 'static, Q: Scalar + 'static>(
    map: Affine<P>,
) -> Result<Affine<Q>, OperationError> {
    Ok(Affine {
        jacobian: transfer_matrix(map.jacobian)?,
        offsets: transfer_matrix(map.offsets)?,
    })
}
/// Same-format endpoints are copied exactly; f32/f64 promotion is a separate transaction.
fn transfer_matrix<P: Scalar + 'static, Q: Scalar + 'static>(
    matrix: Matrix<P>,
) -> Result<Matrix<Q>, OperationError> {
    let centers = matrix
        .centers
        .into_iter()
        .map(crate::conversion::same_format)
        .collect::<Result<_, _>>()?;
    let truth = matrix
        .truth
        .into_iter()
        .map(|entry| {
            Ok::<_, OperationError>(Enclosure::new(
                crate::conversion::same_format(entry.lower().get())?,
                crate::conversion::same_format(entry.upper().get())?,
            )?)
        })
        .collect::<Result<_, _>>()?;
    Ok(Matrix {
        extent: matrix.extent,
        centers,
        truth,
    })
}

#[cfg(test)]
mod tests {
    use super::differential_rhs;
    use crate::solve_testing::TestResult;
    use crate::statistics::{Matrix, enclosed};
    use nrz_scalar::ArithmeticContext;

    /// Private differential construction cannot replace the nominal enclosure by rounded centers.
    /// ´test:unit:solve-direction-retains-nominal-solution-enclosure´
    #[test]
    fn solve_direction_retains_nominal_solution_enclosure() -> TestResult {
        let context = ArithmeticContext::new()?;
        let mut nominal = Matrix::zeros(1, 2)?;
        nominal.set(0, 0, 1.0_f64, enclosed(1.0, 0.25, &context)?)?;
        nominal.set(0, 1, 2.0, enclosed(2.0, 0.25, &context)?)?;
        let rhs = differential_rhs(&nominal, 0)?;
        for column in 0..2 {
            assert_eq!(rhs.entry(0, column)?, nominal.entry(0, column)?.negated());
        }
        Ok(())
    }
}
