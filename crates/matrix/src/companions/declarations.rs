// SPDX-License-Identifier: AGPL-3.0-only
//! Source-model parameters keep complete actual target identities before their float allocation.

use crate::binding::parameters;
use crate::{Extent, Heap, JointError, JointModel};
use nrz_bindings::{
    Descriptor, EvaluationDescriptor, PairingWitness, ParameterSymbol, ParameterTarget,
    UncertaintyMode,
};
use nrz_scalar::Scalar;

/// Every model parameter and optional mean error has its own actual mathematical declaration.
pub(super) fn prepare<P: Scalar + 'static>(
    heap: &Heap,
    model: &JointModel<P>,
    targets: &[super::TargetMap<P>; 2],
    means: Option<&super::MeanParameters<P>>,
) -> Result<Vec<ParameterSymbol>, JointError> {
    let sources = model
        .sources()
        .iter()
        .map(|source| {
            ParameterTarget::new(source.symbol().clone(), 0).map_err(crate::HeapError::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut declarations = match model.representation() {
        crate::CovarianceRepresentation::Factor(factor) => factor_declarations(factor, &sources)?,
        crate::CovarianceRepresentation::Dense(_) => covariance(&sources)?,
    };
    if let Some(means) = means {
        for (target, errors) in targets.iter().zip(&means.errors) {
            let handle = crate::Handle::<crate::Value, P>::new(heap.scope, target.target.key);
            let symbol = heap.symbol_binding(handle)?;
            let layout = if heap.symbols.selection(target.target.key).is_some() {
                crate::Layout::contiguous(target.layout().extent())?
            } else {
                target.layout().clone()
            };
            let selected = parameters::targets(symbol.symbol(), &layout)?;
            let mean = parameters::values(Extent::new(errors.len(), 1)?, selected)?;
            declarations.push(parameters::errors(&mean)?);
        }
    }
    Ok(declarations)
}

/// Dense covariance entries retain both exact source coordinates and their parameter-error targets.
fn covariance(sources: &[ParameterTarget]) -> Result<Vec<ParameterSymbol>, JointError> {
    let extent = Extent::new(sources.len(), sources.len())?;
    let mut descriptors = Vec::new();
    for right in sources {
        for left in sources {
            descriptors.push(EvaluationDescriptor::new(&PairingWitness::uncertainty(
                UncertaintyMode::Covariance,
                left.symbol().descriptor(),
                Some(right.symbol().descriptor()),
                None,
            )?));
        }
    }
    let center = parameters::paired(extent, &descriptors, sources, sources)?;
    let error = parameters::errors(&center)?;
    Ok(vec![center, error])
}

/// Explicit dimensionless factor coordinates define coefficient quotients without inventing scalar kinds.
fn factor_declarations<P: Scalar>(
    factor: &crate::SourceFactor<P>,
    sources: &[ParameterTarget],
) -> Result<Vec<ParameterSymbol>, JointError> {
    let mut columns = Vec::new();
    for _ in 0..factor.columns() {
        let symbol = crate::binding::symbols::declare(
            &Descriptor::ratio(),
            Extent::new(1, 1)?,
            [false, false],
            &mut crate::binding::symbols::budget()?,
        )?;
        columns
            .push(ParameterTarget::new(symbol.value().clone(), 0).map_err(crate::HeapError::from)?);
    }
    let mut descriptors = Vec::new();
    for column in &columns {
        for row in sources {
            descriptors.push(EvaluationDescriptor::new(&PairingWitness::sensitivity(
                row.symbol().descriptor(),
                column.symbol().descriptor(),
            )?));
        }
    }
    let coefficients = parameters::paired(
        Extent::new(factor.rows(), factor.columns())?,
        &descriptors,
        sources,
        &columns,
    )?;
    let weights = parameters::values(Extent::new(factor.columns(), 1)?, columns)?;
    Ok(vec![coefficients, weights])
}
