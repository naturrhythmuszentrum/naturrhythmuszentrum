// SPDX-License-Identifier: AGPL-3.0-only
//! Original exact references and selected scalar source identities precede every numerical read.

use super::{AT, Heap, HeapError, SpectralError as Error, input};
use crate::ReadingBinding;
use nrz_bindings::{
    Budget,
    readings::{
        LcSpectralLaw, ReadingError,
        algebra::{
            Cas, ExactConstant,
            limits::{BudgetKind, BudgetUsage},
        },
    },
};
use nrz_scalar::{LiteralLimits, Scalar};
use std::rc::Rc;

/// Prepared original LC operation and both selected L/C sources.
#[derive(Clone, Debug)]
pub struct LcSpectrum<P: Scalar>(pub(super) Rc<Plan<P>>);

/// Shared immutable law and its three exact positive constant spellings.
#[derive(Debug)]
pub(super) struct Plan<P: Scalar> {
    /// Checked original complete-root theorem and state/time similarity.
    pub(super) law: LcSpectralLaw,
    /// Ordered original L/C readings, retaining target and companion versions.
    pub(super) inputs: [ReadingBinding<P>; 2],
    /// Upper numerator, lower magnitude numerator and time reference, respectively.
    pub(super) parameters: Parameters<P>,
    /// Explicit bounded scalar literal policy.
    pub(super) literals: LiteralLimits,
    /// Newly owned plan and string allocations; shared evidence is counted by its owner.
    pub(super) bytes: u64,
    /// Complete caller snapshots around plan admission and exact export.
    pub(super) usage: [BudgetUsage; 2],
}

/// Reuse the existing string-owner niche without adding a field to deterministic records.
#[derive(Debug)]
pub(super) enum Parameters<P: Scalar> {
    /// Original three independently owned exact constant spellings.
    Constants([String; 3]),
    /// Statistical requests alone own their additional input or evaluated parameters.
    Statistical(Box<super::statistics::Extension<P>>),
}

impl<P: Scalar> LcSpectrum<P> {
    /// Statistical extensions retain the original constant owner directly.
    pub(super) fn constants(&self) -> &[String; 3] {
        match &self.0.parameters {
            Parameters::Constants(constants) => constants,
            Parameters::Statistical(extension) => extension.base.constants(),
        }
    }
    /// Optional statistical state has no allocation in a deterministic request.
    pub(super) fn extension(&self) -> Option<&super::statistics::Extension<P>> {
        match &self.0.parameters {
            Parameters::Constants(_) => None,
            Parameters::Statistical(extension) => Some(extension),
        }
    }
    /// Share the original deterministic owner while replacing only its parameter view.
    pub(super) fn extended(extension: super::statistics::Extension<P>) -> Self {
        let base = &extension.base.0;
        Self(Rc::new(Plan {
            law: base.law.clone(),
            inputs: base.inputs.clone(),
            literals: base.literals,
            bytes: base.bytes,
            usage: base.usage,
            parameters: Parameters::Statistical(Box::new(extension)),
        }))
    }
    /// Bind the exact law to the original scalar inputs and lower its rational constants.
    /// # Errors
    /// Refuses foreign graphs, changed symbols or references, nonidentity input/companion maps,
    /// unsupported exact exports and exhausted cumulative resources.
    pub fn new(
        graph: &Cas,
        law: LcSpectralLaw,
        inputs: [ReadingBinding<P>; 2],
        literals: LiteralLimits,
        budget: &mut Budget,
    ) -> Result<Self, Error> {
        let before = budget.usage();
        graph.validate_reuse_key(law.operator().reuse_key(), budget)?;
        input::check(&law, &inputs, budget)?;
        let constants = export(&law, budget)?;
        let bytes = storage::<P>(&constants)?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        Ok(Self(Rc::new(Plan {
            law,
            inputs,
            parameters: Parameters::Constants(constants),
            literals,
            bytes,
            usage: [before, budget.usage()],
        })))
    }
    /// Complete exact theorem and declared similarity/time definitions.
    #[must_use]
    pub fn law(&self) -> &LcSpectralLaw {
        &self.0.law
    }
    /// Ordered original L/C bindings, including every selected companion target.
    #[must_use]
    pub fn inputs(&self) -> &[ReadingBinding<P>; 2] {
        &self.0.inputs
    }
    /// Separate exact-export and planning costs on the cumulative caller ledger.
    #[must_use]
    pub fn usage(&self) -> [BudgetUsage; 2] {
        self.0.usage
    }
    /// Original deterministic plan/string allocation; statistical extensions report their own allowance.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
}

impl<P: Scalar + 'static> LcSpectrum<P> {
    /// Both complete selected companion inventories participate in every subsequent access.
    pub(super) fn validate(&self, heap: &Heap) -> Result<(), Error> {
        for source in self.inputs() {
            heap.validate_reading(source)?;
        }
        if let Some(extension) = self.extension() {
            super::statistics::validate(heap, &extension.input)?;
        }
        Ok(())
    }
}

/// Export constants from the sealed operator family; no sampled value supplies an exact coefficient.
fn export(law: &LcSpectralLaw, budget: &mut Budget) -> Result<[String; 3], Error> {
    let operator = law.operator();
    let upper = ExactConstant::from(operator.map().coefficients()[5].clone())
        .mul(operator.frame().entries()[1].magnitude(), budget)?;
    let lower = ExactConstant::from(operator.map().coefficients()[2].negated(budget)?)
        .mul(operator.frame().entries()[0].magnitude(), budget)?;
    let values = [&upper, &lower, law.time().magnitude()];
    let mut result = [String::new(), String::new(), String::new()];
    for (index, value) in values.into_iter().enumerate() {
        let rational = value
            .as_rational()
            .ok_or(nrz_bindings::UnitError::UnsupportedExactConstantExport)
            .map_err(ReadingError::from)?;
        result[index] = rational.exact_string(budget)?;
        budget.check(BudgetKind::StagedPayloadBytes, storage::<f64>(&result)?, AT)?;
    }
    Ok(result)
}

/// Charge each actual owned string buffer beside the new shared plan record.
fn storage<P: Scalar>(constants: &[String; 3]) -> Result<u64, Error> {
    let mut bytes = std::mem::size_of::<Plan<P>>() + 2 * std::mem::size_of::<usize>();
    for text in constants {
        bytes = bytes
            .checked_add(text.capacity())
            .ok_or(HeapError::ExtentOverflow)?;
    }
    Ok(u64::try_from(bytes).map_err(|_| HeapError::ExtentOverflow)?)
}
