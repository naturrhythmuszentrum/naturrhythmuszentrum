// SPDX-License-Identifier: AGPL-3.0-only
//! Original LC input transport retains the checked question and complete tuple roles.

mod inventory;
mod lifecycle;
mod output;
pub use output::{
    LcOmegaOutput, LcOmegaPrepareFailure, LcOmegaPrepared, LcOutputCellState, LcOutputDeclaration,
    LcOutputPending, LcOutputPublicationError,
};
mod readers;
use super::{ExecutionAccount, HostTileDeclaration, LoanError, charge};
use crate::{CellAxis, CellProduct, Coverage};
pub use inventory::LcInputInventory;
pub use lifecycle::{LcShapeApplication, LcShapeRequest, LcShapeTile};
use nrz_bindings::{BindingSymbols, LcForwardCover};
use nrz_scalar::Scalar;
pub use readers::{
    LcFrequencyPrelude, LcFrequencyTarget, LcInputCell, LcInputCells, LcInputProduct,
    LcInputReaders, LcInputReading, LcProductCell, LcProductError, LcQuantityDemand,
    LcRequestedCell, LcRequestedState, LcRequestedValues,
};
use std::marker::PhantomData;

/// Refusal of the closed original-input transport schema, without a physical conclusion.
#[derive(Debug, thiserror::Error)]
pub enum LcShapeError {
    /// Native authority, shape or resource refusal.
    #[error("LC shape: {0}")]
    Loan(#[from] LoanError),
    /// Original symbol declaration refused.
    #[error("LC shape declaration: {0}")]
    Declaration(#[from] nrz_bindings::AdmissionError),
}

/// Ordered native roles supplied afresh for each application.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LcShapeAxes {
    /// Exact original instance axis, never inferred by capacity.
    pub instances: CellAxis,
    /// Captured ordered sample role.
    pub samples: CellAxis,
    /// Complete bounded branch role in the cover's increasing integer order.
    pub branches: CellAxis,
    /// Instance/sample product, with instance outermost.
    pub instance_samples: CellProduct,
    /// Complete tuple, with branch innermost.
    pub combined: CellProduct,
    /// Same ordered sample and branch factors, used to reassociate the native projection.
    pub sample_branches: CellProduct,
}

/// Fixed-format transport of one original supplied input over a checked forward question.
///
/// This schema declares the original input and its copied output before numerical loans.
/// It preserves question association, not numerical satisfaction of the physical constraints.
/// All samples use the same original time query; other times require another schema.
/// Only complete scalar center/total/error representations and fixed extents are admitted.
#[derive(Debug)]
pub struct LcInputShape<P: Scalar> {
    /// Original checked relation, time query, full branch domain and revisions.
    question: LcForwardCover,
    /// Exact original supplied constraint, including repeated-constraint position.
    input: usize,
    /// Captured instance, repeated-query sample and branch cardinalities.
    counts: [usize; 3],
    /// Original source and copied-output inventories, in that order.
    symbols: [BindingSymbols; 2],
    /// Original execution account, including all exact preparation work.
    account: u64,
    /// Nonwrapping specialization identity; demand bits are not part of it.
    key: u64,
    /// Native format is part of the Rust type, never chosen by matching byte widths.
    marker: PhantomData<P>,
}
impl<P: Scalar> LcInputShape<P> {
    /// Declare the narrow input transport schema from the original checked question.
    /// # Errors
    /// Refuses missing inputs, mismatched branch cardinality, overflowing extents
    /// and exhausted original resources. Empty-domain and indexed-input checks are
    /// defence in depth: checked forward-cover admission already excludes them.
    pub fn declare(
        question: LcForwardCover,
        input: usize,
        counts: [usize; 3],
        account: &mut ExecutionAccount,
    ) -> Result<Self, LcShapeError> {
        let symbols =
            account
                .ledger
                .with_work_phase(super::FAMILY, "LC shape schema", |budget| {
                    charge(budget, 6)?;
                    let original = question
                        .relation()
                        .inputs()
                        .get(input)
                        .ok_or(LoanError::UnsupportedContract)?;
                    if matches!(
                        original.quantity(),
                        nrz_bindings::LcQuantity::Time(_) | nrz_bindings::LcQuantity::Phase(_)
                    ) {
                        return Err(LoanError::UnsupportedContract.into());
                    }
                    let width = question.cycles()[1]
                        .sub(&question.cycles()[0], budget)
                        .map_err(LoanError::from)?
                        .add(&nrz_bindings::readings::algebra::Rational::one(), budget)
                        .map_err(LoanError::from)?;
                    let (branches, denominator) = width
                        .small_ratio(budget)
                        .map_err(LoanError::from)?
                        .ok_or(LoanError::CountOverflow)?;
                    if denominator != 1
                        || usize::try_from(branches).ok() != Some(counts[2])
                        || counts[2] == 0
                    {
                        return Err(LoanError::LayoutMismatch.into());
                    }
                    let rows = counts[0]
                        .checked_mul(counts[1])
                        .ok_or(LoanError::CountOverflow)?;
                    let output = rows
                        .checked_mul(counts[2])
                        .ok_or(LoanError::CountOverflow)?;
                    let rows = u32::try_from(counts[0]).map_err(|_| LoanError::CountOverflow)?;
                    let output = u32::try_from(output).map_err(|_| LoanError::CountOverflow)?;
                    Ok::<_, LcShapeError>([
                        BindingSymbols::declare(
                            original.descriptor(),
                            [rows, 1],
                            [true; 2],
                            budget,
                        )?,
                        BindingSymbols::declare(
                            original.descriptor(),
                            [output, 1],
                            [true; 2],
                            budget,
                        )?,
                    ])
                })?;
        Ok(Self {
            question,
            input,
            counts,
            symbols,
            account: account.identity,
            key: crate::heap::identity::identity().map_err(LoanError::from)?,
            marker: PhantomData,
        })
    }

    /// Original checked question, including every original repeated constraint.
    #[must_use]
    pub const fn question(&self) -> &LcForwardCover {
        &self.question
    }
    /// Original supplied-constraint position; equal physical descriptors do not merge inputs.
    #[must_use]
    pub const fn input_index(&self) -> usize {
        self.input
    }
    /// Immutable specialization identity; changing time, representation or extents needs a new one.
    #[must_use]
    pub const fn specialization_key(&self) -> u64 {
        self.key
    }
    /// Original source declaration on the application's exact instance axis.
    /// # Errors
    /// Refuses another account, incorrect role products and unsupported extents.
    pub fn source(
        &self,
        axes: &LcShapeAxes,
        account: &mut ExecutionAccount,
    ) -> Result<HostTileDeclaration<P>, LoanError> {
        self.axes(axes, account)?;
        HostTileDeclaration::check(
            self.symbols[0].clone(),
            axes.instances,
            Coverage::RepresentedInputs,
            account,
        )
    }
    /// Original copied-output declaration on the complete qualified tuple axis.
    /// # Errors
    /// Refuses another account, incorrect role products and unsupported extents.
    pub fn destination(
        &self,
        axes: &LcShapeAxes,
        account: &mut ExecutionAccount,
    ) -> Result<HostTileDeclaration<P>, LoanError> {
        self.axes(axes, account)?;
        HostTileDeclaration::check(
            self.symbols[1].clone(),
            axes.combined.axis(),
            Coverage::RepresentedInputs,
            account,
        )
    }
    /// Project the complete tuple to its original instance without a materialized intermediate.
    /// # Errors
    /// Refuses incompatible ordered products, changed specialization and exhausted Work.
    pub fn projection(
        &self,
        axes: &LcShapeAxes,
        account: &mut ExecutionAccount,
    ) -> Result<CellProduct, LoanError> {
        self.axes(axes, account)
    }
    /// Compare identities as well as cardinalities before any native allocation or load.
    fn axes(
        &self,
        axes: &LcShapeAxes,
        account: &mut ExecutionAccount,
    ) -> Result<CellProduct, LoanError> {
        account.authenticate(self.account)?;
        account
            .ledger
            .with_work_phase(super::FAMILY, "LC shape roles", |budget| {
                charge(budget, 7)?;
                if [
                    axes.instances.capacity(),
                    axes.samples.capacity(),
                    axes.branches.capacity(),
                ] != self.counts
                    || axes.instance_samples.factors() != [axes.instances, axes.samples]
                    || axes.combined.factors() != [axes.instance_samples.axis(), axes.branches]
                    || axes.sample_branches.factors() != [axes.samples, axes.branches]
                {
                    return Err(LoanError::LayoutMismatch);
                }
                Ok(axes
                    .combined
                    .reassociate_outer(axes.instance_samples, axes.sample_branches)?)
            })
    }
}

#[cfg(test)]
mod tests;

/// Private bridge layout evidence is shared with the existing native inventory control.
#[cfg(test)]
pub(super) fn bridge_layout<P: nrz_scalar::Scalar>() {
    output::bridge::layout::<P>();
}
