// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit complete source models; equal numerical bytes never imply independence.

use super::ResultOf;
use nrz_matrix::{
    CompanionSetDraft, CovarianceBlock, DenseCovariance, Handle, Heap, JointLimits, JointModel,
    Source, SourceFactor, TargetMap, Value,
};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// Exact factor law or an enclosed decimal variance on the final source.
#[derive(Debug)]
pub(crate) enum Law<'a> {
    /// Represented coefficients and weight define the covariance exactly.
    Factor(&'a [f64], f64),
    /// All rows are fixed except the final source, with mathematical variance 1/10000.
    Resistor,
}

/// Attach one factor column to two imported regions, preserving every zero-variation row.
pub(crate) fn attach(heap: &mut Heap, inputs: [Handle<Value, f64>; 2], law: &Law<'_>) -> ResultOf {
    let limits = JointLimits::default();
    let mut sources = Vec::new();
    let mut means = Vec::new();
    let mut selections = Vec::new();
    for input in inputs {
        let view = heap.value(input)?;
        let centers = view.region().export()?;
        let first = sources.len();
        for _ in &centers {
            sources.push(Source::new(
                heap.symbol_binding(input)?.symbol().descriptor(),
                1,
            )?);
        }
        selections.push((first..sources.len()).collect::<Vec<_>>());
        means.push(
            centers
                .into_iter()
                .map(|value| Finite::new(value).map(Enclosure::singleton))
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    let model = joint(&sources, law, limits)?;
    let selected: [Vec<usize>; 2] = selections.try_into().map_err(|_| "source selection")?;
    let averages: [Vec<Enclosure<f64>>; 2] = means.try_into().map_err(|_| "mean selection")?;
    let targets = [
        TargetMap::capture(heap, inputs[0])?,
        TargetMap::capture(heap, inputs[1])?,
    ];
    let draft = CompanionSetDraft::new(model, targets, selected).with_means(
        heap,
        averages,
        &ArithmeticContext::new()?,
        limits,
    )?;
    heap.publish_joint(draft, limits)?;
    Ok(())
}

/// Admit the exact source parameter or an explicit enclosure of the decimal variance.
fn joint(sources: &[Source<f64>], law: &Law<'_>, limits: JointLimits) -> ResultOf<JointModel<f64>> {
    let blocks = vec![CovarianceBlock::Known; sources.len() * sources.len()];
    Ok(match law {
        Law::Factor(coefficients, weight) => JointModel::from_factor(
            sources,
            SourceFactor::new(
                sources.len(),
                coefficients,
                std::slice::from_ref(weight),
                limits,
            )?,
            &blocks,
            limits,
        )?,
        Law::Resistor => {
            let mut centers = vec![0.0; blocks.len()];
            *centers.last_mut().ok_or("variance")? = 0.0001;
            let mut family = vec![Enclosure::singleton(Finite::new(0.0)?); blocks.len()];
            *family.last_mut().ok_or("variance family")? =
                Enclosure::new(Scalar::next_down(0.0001_f64), Scalar::next_up(0.0001_f64))?;
            let dense = DenseCovariance::with_enclosures(
                sources.len(),
                &centers,
                &family,
                &ArithmeticContext::new()?,
                limits,
            )?;
            JointModel::from_dense(sources, dense, &blocks, limits)?
        }
    })
}
