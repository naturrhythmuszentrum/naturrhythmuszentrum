// SPDX-License-Identifier: AGPL-3.0-only
//! Original physical declarations and sealed conditional LC correspondence.

mod correspondence;
mod owner;
pub use correspondence::LcOmegaCorrespondence;
pub use owner::{LcOmegaDeclarationRequest, LcOmegaDeclarationToken};

use crate::readings::EntryMeaning;
use crate::{AdmissionError, BinaryOp, BindingSymbols, LcForwardCover, LcQuantity, Registry};
use nrz_cas::{
    Location, Rational,
    limits::{Budget, BudgetKind},
};
use std::fmt;

/// Accounting location for original-declaration visits.
const AT: Location = Location::new("LC omega originals");

/// Borrowed proposed physical inventory, in original supplied-constraint order.
/// The caller already holds these symbols; this input grants no mathematical authority.
#[derive(Clone, Copy, Debug)]
pub struct LcOmegaOriginalsRequest<'a> {
    /// Exact checked forward question, including time and branch cover.
    pub question: &'a LcForwardCover,
    /// Source/mapped pair for every supplied constraint, including repeats.
    pub inputs: &'a [[BindingSymbols; 2]],
    /// Original inductance and capacitance positions, in that order.
    pub positions: [usize; 2],
    /// Fixed instance, sample and branch cardinalities.
    pub counts: [usize; 3],
    /// Already declared angular-frequency value and its two companions.
    pub output: &'a BindingSymbols,
    /// Original source-free exact one-radian interpretation.
    pub radian: &'a EntryMeaning,
    /// Registry admitting inverse time times angle as angular frequency.
    pub registry: &'a Registry,
}

/// Checked borrowed physical originals, not a correspondence or declaration token.
///
/// No root theorem, replay, native join or numerical publication is authorized.
/// All original owners must outlive this view. No symbols can be extracted from it.
///
/// ```compile_fail,E0616
/// use nrz_bindings::LcOmegaOriginals;
/// fn escape(value: &LcOmegaOriginals<'_>) { let _ = value.output; }
/// ```
///
/// ```compile_fail,E0599
/// use nrz_bindings::LcOmegaOriginals;
/// fn escape(value: &LcOmegaOriginals<'_>) { let _ = value.symbols(); }
/// ```
pub struct LcOmegaOriginals<'a> {
    /// One borrowed question keeps all original constraints and query metadata live.
    question: &'a LcForwardCover,
    /// Ordered borrowed original symbol inventory, never copied or exposed.
    inputs: &'a [[BindingSymbols; 2]],
    /// Original ordered physical operand positions.
    positions: [usize; 2],
    /// Fixed tuple specialization.
    counts: [usize; 3],
    /// Private borrowed requested result identity.
    output: &'a BindingSymbols,
    /// Exact original angle definition identity.
    radian: &'a EntryMeaning,
}

impl fmt::Debug for LcOmegaOriginals<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LcOmegaOriginals")
            .field("positions", &self.positions)
            .field("counts", &self.counts)
            .finish_non_exhaustive()
    }
}

impl<'a> LcOmegaOriginals<'a> {
    /// Validate all original positions, shapes, physical kinds and companion targets.
    /// This fixed-size view allocates no owner or graph. Its byte checks cover only
    /// the new inline view; callers retain/reserve their existing original inventory.
    /// # Errors
    /// Preserves physical, companion, arithmetic and cumulative resource refusals.
    pub fn check(
        request: LcOmegaOriginalsRequest<'a>,
        budget: &mut Budget,
    ) -> Result<Self, AdmissionError> {
        budget.with_work_phase("LC omega originals", "admission", |budget| {
            budget.work(1, AT)?;
            let bytes = u64::try_from(std::mem::size_of::<Self>())
                .map_err(|_| AdmissionError::ResourceLimit)?;
            budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
            budget.check(BudgetKind::OutputBytes, bytes, AT)?;
            let originals = request.question.relation().inputs();
            if originals.len() != request.inputs.len() || request.counts[2] == 0 {
                return Err(AdmissionError::ValueMismatch);
            }
            let tuples = request.counts.into_iter().try_fold(1_usize, |n, count| {
                n.checked_mul(count).ok_or(AdmissionError::ResourceLimit)
            })?;
            let mapped = u32::try_from(tuples).map_err(|_| AdmissionError::ResourceLimit)?;
            let source =
                u32::try_from(request.counts[0]).map_err(|_| AdmissionError::ResourceLimit)?;
            budget.work(1, AT)?;
            let cycles = request.question.cycles();
            let width = cycles[1]
                .sub(&cycles[0], budget)?
                .add(&Rational::one(), budget)?;
            let (branches, denominator) = width
                .small_ratio(budget)?
                .ok_or(AdmissionError::ValueMismatch)?;
            if denominator != 1 || usize::try_from(branches).ok() != Some(request.counts[2]) {
                return Err(AdmissionError::ValueMismatch);
            }
            for (position, quantity) in request
                .positions
                .into_iter()
                .zip([LcQuantity::Inductance, LcQuantity::Capacitance])
            {
                budget.work(1, AT)?;
                if originals.get(position).map(crate::LcInput::quantity) != Some(quantity) {
                    return Err(AdmissionError::ValueMismatch);
                }
            }
            for (original, pair) in originals.iter().zip(request.inputs) {
                budget.work(1, AT)?;
                for (symbols, rows) in pair.iter().zip([source, mapped]) {
                    check_symbols(symbols, original.descriptor(), rows, budget)?;
                }
            }
            budget.work(1, AT)?;
            let angle = LcQuantity::InitialPhase.descriptor()?;
            let rate = LcQuantity::Rate.descriptor()?;
            let omega = LcQuantity::AngularFrequency.descriptor()?;
            if request.radian.descriptor() != &angle
                || request.radian.magnitude().as_rational() != Some(&Rational::one())
                || request.radian.origin().is_some()
                || !request.radian.shift().is_zero()
            {
                return Err(AdmissionError::ValueMismatch);
            }
            budget.work(1, AT)?;
            if request.registry.binary(BinaryOp::Multiply, &rate, &angle)? != omega {
                return Err(AdmissionError::ValueMismatch);
            }
            check_symbols(request.output, &omega, mapped, budget)?;
            Ok(Self {
                question: request.question,
                inputs: request.inputs,
                positions: request.positions,
                counts: request.counts,
                output: request.output,
                radian: request.radian,
            })
        })
    }

    /// Compare already checked originals without returning their private symbol inventory.
    /// Equal-shaped fresh symbols and separately checked questions refuse.
    /// This is a borrowed identity check, never a root receipt or a matrix join.
    /// # Errors
    /// Refuses changed questions, positional/value identities, companions or output identity.
    pub fn check_same(&self, expected: &Self, budget: &mut Budget) -> Result<(), AdmissionError> {
        budget.with_work_phase("LC omega originals", "comparison", |budget| {
            budget.work(1, AT)?;
            if !self.question.shares_allocation(expected.question)
                || self.positions != expected.positions
                || self.counts != expected.counts
                || !self.radian.shares_definition(expected.radian)
                || self.inputs.len() != expected.inputs.len()
            {
                return Err(AdmissionError::ValueMismatch);
            }
            for (actual, expected) in self.inputs.iter().zip(expected.inputs) {
                for (actual, expected) in actual.iter().zip(expected) {
                    same_symbols(actual, expected, false, budget)?;
                }
            }
            same_symbols(self.output, expected.output, true, budget)
        })
    }
}

/// One visit per value and companion, including the complete fixed edge validation.
fn check_symbols(
    symbols: &BindingSymbols,
    descriptor: &crate::Descriptor,
    rows: u32,
    budget: &mut Budget,
) -> Result<(), AdmissionError> {
    budget.work(3, AT)?;
    symbols.validate()?;
    if symbols.bound().is_none() || symbols.error().is_none() {
        return Err(AdmissionError::CompanionMismatch);
    }
    symbols.value().check_descriptor(descriptor)?;
    if symbols.value().shape() != [rows, 1] {
        return Err(AdmissionError::ValueMismatch);
    }
    if symbols.value().target().is_some() {
        return Err(AdmissionError::ResultMismatch);
    }
    Ok(())
}

/// Compare every edge against an independently checked original inventory.
fn same_symbols(
    actual: &BindingSymbols,
    expected: &BindingSymbols,
    output: bool,
    budget: &mut Budget,
) -> Result<(), AdmissionError> {
    budget.work(1, AT)?;
    if actual.value() != expected.value() {
        return Err(if output {
            AdmissionError::ResultMismatch
        } else {
            AdmissionError::ValueMismatch
        });
    }
    budget.work(2, AT)?;
    if actual.bound() != expected.bound() || actual.error() != expected.error() {
        return Err(AdmissionError::CompanionMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
