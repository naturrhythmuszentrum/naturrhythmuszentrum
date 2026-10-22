// SPDX-License-Identifier: AGPL-3.0-only
//! Prepare every statistical region and target before the existing no-fail publication.

use super::{
    StatisticsError,
    allocation::RegionData,
    matrix::Matrix,
    store::{Lineage, Parameter, Record},
};
use crate::heap::identity::Key;
use crate::{Bound, Candidate, Dependency, OperationError, OutputRecord};
use nrz_bindings::{Descriptor, EvaluationDescriptor, PairingWitness, UncertaintyMode};
use nrz_scalar::{ArithmeticContext, Scalar};

/// Already prepared moment records; linking them cannot fail or run numerical code.
#[derive(Debug)]
pub(crate) struct PreparedMoments {
    /// Complete independent parameter and target relations.
    pub(crate) records: Vec<Record>,
}
/// Temporary parameter relation with positions in the appended region inventory.
struct PendingParameter {
    /// First of the center/error region pair.
    start: usize,
    /// Complete per-entry pairing and evaluation descriptors.
    descriptors: Vec<EvaluationDescriptor>,
}
/// Complete metadata prepared before assigning descriptor occupations.
struct PendingMoment {
    /// Both exact target versions.
    targets: [Dependency; 2],
    /// Original source model and checked lineage.
    lineage: Lineage,
    /// Full covariance relation.
    covariance: PendingParameter,
    /// Composed source sensitivity.
    sensitivity: PendingParameter,
    /// Local Jacobian.
    jacobian: PendingParameter,
    /// Retained checked derivative identity for the local native recipe.
    derivative: Option<std::rc::Rc<nrz_bindings::CheckedSensitivity>>,
    /// Explicit mean parameters, if declared.
    means: Option<PendingParameter>,
}

/// Prepare records and numerical storage as part of the ordinary atomic transaction.
pub(crate) fn prepare<P: Scalar + 'static, Q: Scalar + 'static>(
    bound: &mut Bound<'_, P, Q>,
    candidate: &Candidate<Q>,
    outputs: &[OutputRecord],
) -> Result<PreparedMoments, OperationError> {
    let expected = if bound.statistics.lineage.is_some() {
        outputs.len()
    } else {
        0
    };
    if candidate.statistics.len() != expected {
        return Err(StatisticsError::Contract.into());
    }
    if expected == 0 {
        return Ok(PreparedMoments {
            records: Vec::new(),
        });
    }
    let context = ArithmeticContext::new()?;
    let mut regions = Vec::new();
    let mut pending = Vec::new();
    for (slot, payload) in candidate.statistics.iter().enumerate() {
        let target = outputs
            .get(slot)
            .ok_or(StatisticsError::Contract)?
            .dependency
            .clone();
        pending.push(moment(
            bound,
            payload,
            target,
            slot,
            &mut regions,
            &context,
        )?);
    }
    bound
        .heap
        .statistics
        .records
        .try_reserve(pending.len())
        .map_err(|_| StatisticsError::ResourceLimit)?;
    let keys = super::allocation::append(bound, candidate, &regions)?;
    let records = pending
        .into_iter()
        .map(|draft| finish(draft, &keys))
        .collect::<Result<_, _>>()?;
    Ok(PreparedMoments { records })
}

/// Build one complete relation after validating all mathematical shapes and source identities.
fn moment<P: Scalar + 'static, Q: Scalar + 'static>(
    bound: &Bound<'_, P, Q>,
    payload: &super::MomentPayload<Q>,
    target: Dependency,
    slot: usize,
    regions: &mut Vec<RegionData<Q>>,
    context: &ArithmeticContext,
) -> Result<PendingMoment, OperationError> {
    let output = bound
        .operation
        .outputs()
        .get(slot)
        .ok_or(StatisticsError::Contract)?;
    let lineage = bound
        .statistics
        .lineage
        .as_ref()
        .ok_or(StatisticsError::Contract)?;
    validate_payload(payload, lineage, &target)?;
    super::structure::output(output.shape(), payload, slot)?;
    validate_recipe(bound, payload)?;
    let quantity = output.descriptor();
    let covariance = parameter(
        &payload.covariance,
        covariance_descriptors(quantity, quantity, payload.covariance.extent.entries())?,
        None,
        regions,
        context,
    )?;
    let sensitivity = parameter(
        &payload.sensitivity,
        source_descriptors(quantity, lineage, payload.sensitivity.extent.rows())?,
        None,
        regions,
        context,
    )?;
    let jacobian = parameter(
        &payload.jacobian,
        derivative_descriptors(quantity, &bound.operation, payload.jacobian.extent.rows())?,
        None,
        regions,
        context,
    )?;
    let means = payload
        .means
        .as_ref()
        .map(|means| parameter(means, Vec::new(), Some(quantity), regions, context))
        .transpose()?;
    Ok(PendingMoment {
        targets: [target.clone(), target],
        lineage: payload.lineage.clone(),
        covariance,
        sensitivity,
        jacobian,
        derivative: payload.derivative.clone(),
        means,
    })
}
/// A private payload cannot substitute another model, omit columns or publish a diagonal surrogate.
fn validate_payload<P: Scalar>(
    payload: &super::MomentPayload<P>,
    lineage: &Lineage,
    target: &Dependency,
) -> Result<(), OperationError> {
    if payload.lineage.model.id != lineage.model.id
        || payload.lineage.columns != lineage.columns
        || payload.lineage.roots != lineage.roots
        || payload.lineage.assurance != lineage.assurance
        || payload.covariance.extent.rows() != target.layout().extent().entries()
        || payload.covariance.extent.rows() != payload.covariance.extent.columns()
        || payload.sensitivity.extent.rows() != payload.covariance.extent.rows()
        || payload.sensitivity.extent.columns() != lineage.columns.len()
    {
        return Err(StatisticsError::Contract.into());
    }
    Ok(())
}

/// The local derivative owner, complete operand columns and mean coverage are part of the sealed recipe.
fn validate_recipe<P: Scalar, Q: Scalar>(
    bound: &Bound<'_, P, Q>,
    payload: &super::MomentPayload<Q>,
) -> Result<(), OperationError> {
    let columns = bound
        .operation
        .inputs()
        .iter()
        .try_fold(0_usize, |sum, input| {
            sum.checked_add(input.shape().extent().entries())
                .ok_or(StatisticsError::ResourceLimit)
        })?;
    let same_derivative = match (&payload.derivative, &bound.statistics.derivative) {
        (Some(actual), Some(expected)) => std::rc::Rc::ptr_eq(actual, expected),
        (None, None) => true,
        _ => false,
    };
    let needs_means = payload.lineage.assurance == super::MomentAssurance::AffineMoments
        && bound
            .statistics
            .inputs
            .iter()
            .all(|input| input.means.is_some());
    if !same_derivative
        || payload.jacobian.extent.rows() != payload.covariance.extent.rows()
        || payload.jacobian.extent.columns() != columns
        || payload.means.is_some() != needs_means
    {
        return Err(StatisticsError::Contract.into());
    }
    if payload.means.as_ref().is_some_and(|means| {
        means.extent.rows() != payload.covariance.extent.rows() || means.extent.columns() != 1
    }) {
        return Err(StatisticsError::Contract.into());
    }
    Ok(())
}

/// Paired product descriptors remain independent from the homogeneous numerical storage schema.
pub(crate) fn covariance_descriptors(
    left: &Descriptor,
    right: &Descriptor,
    count: usize,
) -> Result<Vec<EvaluationDescriptor>, OperationError> {
    let witness =
        PairingWitness::uncertainty(UncertaintyMode::Covariance, left, Some(right), None)?;
    Ok(vec![EvaluationDescriptor::new(&witness); count])
}
/// Every source derivative retains output-over-source dimensions and both semantic kinds.
fn source_descriptors(
    quantity: &Descriptor,
    lineage: &Lineage,
    rows: usize,
) -> Result<Vec<EvaluationDescriptor>, OperationError> {
    let mut result = Vec::new();
    for &column in &lineage.columns {
        let source = lineage
            .model
            .quantities
            .get(column)
            .ok_or(StatisticsError::Contract)?;
        let descriptor = EvaluationDescriptor::new(&PairingWitness::sensitivity(quantity, source)?);
        result.extend(std::iter::repeat_n(descriptor, rows));
    }
    Ok(result)
}
/// Derivative columns follow the complete concatenated logical operand order.
fn derivative_descriptors(
    quantity: &Descriptor,
    operation: &crate::CheckedOperation,
    rows: usize,
) -> Result<Vec<EvaluationDescriptor>, OperationError> {
    let mut result = Vec::new();
    for input in operation.inputs() {
        let descriptor =
            EvaluationDescriptor::new(&PairingWitness::sensitivity(quantity, input.descriptor())?);
        let count = input
            .shape()
            .extent()
            .entries()
            .checked_mul(rows)
            .ok_or(StatisticsError::ResourceLimit)?;
        result.extend(std::iter::repeat_n(descriptor, count));
    }
    Ok(result)
}
/// Enclose the actual stored candidate before creating its error-target relation.
fn parameter<P: Scalar>(
    matrix: &Matrix<P>,
    descriptors: Vec<EvaluationDescriptor>,
    mean: Option<&Descriptor>,
    regions: &mut Vec<RegionData<P>>,
    context: &ArithmeticContext,
) -> Result<PendingParameter, OperationError> {
    if !descriptors.is_empty() && descriptors.len() != matrix.extent.entries() {
        return Err(StatisticsError::Contract.into());
    }
    let start = regions.len();
    let errors = matrix.errors(context)?;
    let quantity = mean.cloned().unwrap_or_else(Descriptor::ratio);
    let radius = if let Some(mean) = mean {
        nrz_bindings::uncertainty(UncertaintyMode::Radius, mean, None, None)
            .map_err(crate::HeapError::from)?
    } else {
        Descriptor::ratio()
    };
    regions.push(RegionData {
        extent: matrix.extent,
        quantity,
        values: matrix.centers.clone(),
    });
    regions.push(RegionData {
        extent: matrix.extent,
        quantity: radius,
        values: errors,
    });
    Ok(PendingParameter { start, descriptors })
}
/// Bind all prepared numerical error regions to their own parameter occupations.
fn finish(draft: PendingMoment, keys: &[Key]) -> Result<Record, OperationError> {
    Ok(Record {
        targets: draft.targets,
        lineage: draft.lineage,
        covariance: bind(draft.covariance, keys)?,
        sensitivity: Some(bind(draft.sensitivity, keys)?),
        jacobian: Some(bind(draft.jacobian, keys)?),
        derivative: draft.derivative,
        means: draft.means.map(|means| bind(means, keys)).transpose()?,
    })
}
/// Resolve one already reserved center/error pair without exposing it before commit.
fn bind(parameter: PendingParameter, keys: &[Key]) -> Result<Parameter, OperationError> {
    Ok(Parameter {
        regions: [
            *keys.get(parameter.start).ok_or(StatisticsError::Contract)?,
            *keys
                .get(parameter.start + 1)
                .ok_or(StatisticsError::Contract)?,
        ],
        descriptors: parameter.descriptors,
    })
}

#[cfg(test)]
mod tests {
    use crate::kernel_testing::{TestResult, axis, owner, shape};
    use crate::{
        Binding, CompanionSetDraft, CovarianceBlock, ExecutionLimits, Extent, Heap, HeapLimits,
        JointLimits, JointModel, OperationDraft, OperationError, Source, SourceFactor,
        StatisticsError, TargetMap,
    };
    use nrz_bindings::Descriptor;

    /// Private candidate corruption cannot publish a value after removing its moments.
    /// ´test:unit:statistical-publication-rejects-an-incomplete-candidate´
    #[test]
    fn statistical_publication_rejects_an_incomplete_candidate() -> TestResult {
        let declarations = owner()?;
        let coordinate = axis(1)?;
        let contract = shape(&declarations, &coordinate, &coordinate)?;
        let mut heap = Heap::new(HeapLimits::default())?;
        let input =
            heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
        let target = TargetMap::capture(&heap, input.value())?;
        let limits = JointLimits::default();
        let source = Source::new(&Descriptor::ratio(), 1)?;
        let model = JointModel::from_factor(
            &[source],
            SourceFactor::new(1, &[1.0], &[0.25], limits)?,
            &[CovarianceBlock::Known],
            limits,
        )?;
        heap.publish_joint(
            CompanionSetDraft::new(model, [target.clone(), target], [vec![0], vec![0]]),
            limits,
        )?;
        let count = heap.descriptors.iter().count();
        let operation = OperationDraft::copy(vec![contract]).check(ExecutionLimits::default())?;
        let bound = Binding::new(operation, vec![input.value()], ExecutionLimits::default())
            .validate(&mut heap)?;
        let mut candidate = bound.evaluate(bound.normalized_inputs()?)?;
        candidate.statistics.clear();
        let failure = bound
            .publish(candidate)
            .err()
            .ok_or("incomplete statistical bundle published")?;
        assert!(matches!(
            failure.cause(),
            OperationError::Statistics(StatisticsError::Contract)
        ));
        assert_eq!(heap.descriptors.iter().count(), count);
        assert_eq!(heap.statistics.records.len(), 0);
        Ok(())
    }
}
