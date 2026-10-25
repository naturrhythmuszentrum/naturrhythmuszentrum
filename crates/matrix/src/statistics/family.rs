// SPDX-License-Identifier: AGPL-3.0-only
//! One authenticated original model joins native operator and signed root parameters.

use super::{MomentAssurance, MomentRef, MomentView, enclosed};
use crate::{
    Heap, HeapError, JointLimits, JointModel, PublishedLcSpectrum, SpectralError as Error,
};
use nrz_bindings::{
    Budget,
    readings::algebra::{Location, limits::BudgetKind},
};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};
use std::rc::Rc;

/// Cumulative caller location for this closed cross-output consumer.
const AT: Location = Location::new("LC operator and root covariance");
/// The complete ordered operator and signed-root parameter rectangle.
type Covariance<P> = [[Enclosure<P>; 8]; 8];

/// Borrowed published parameters and the original source ordering, without another model owner.
struct Capture<'h, P: Scalar> {
    /// Already validated native operator parameters and their original lineage.
    moments: MomentView<'h, P>,
    /// Original immutable joint definition, borrowed without copying any source.
    model: &'h JointModel<P>,
    /// Physical root-derivative position for each operator sensitivity column.
    positions: [usize; 2],
}

impl Heap {
    /// Enclose one published operator/root statistical family, including both cross orders.
    ///
    /// Rows and columns are `(B00, B10, B01, B11, Re+, Im+, Re-, Im-)`: the native
    /// normalized operator in column order, followed by the physical signed root rates.
    /// Both diagonal blocks are read from their published owners. The cross block contracts
    /// their sensitivities through the same original model and exact derivative identity.
    /// This snapshot has `LinearizedMoments` scope, with no nonlinear mean or support claim.
    /// Every call revalidates the original sources and both complete published outputs.
    /// # Errors
    /// Refuses different models, substituted sources, selections, references or derivatives,
    /// absent native statistics, stale values or companions, and exhausted caller resources.
    pub fn lc_operator_root_covariance<P: Scalar + 'static>(
        &self,
        operator: MomentRef<P>,
        roots: &PublishedLcSpectrum<P>,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<[[Enclosure<P>; 8]; 8], Error> {
        storage::<P>(budget)?;
        budget.work(8, AT)?;
        let captured = capture(self, operator, roots)?;
        let terms = captured.model.representation().terms();
        // 192 fixed copies/arithmetic slots, four block checks, and three operations per factor term.
        let work = terms
            .checked_mul(12)
            .and_then(|value| value.checked_add(196))
            .ok_or(HeapError::ExtentOverflow)?;
        budget.work(
            u64::try_from(work).map_err(|_| HeapError::ExtentOverflow)?,
            AT,
        )?;
        let diagonal = roots
            .root_covariance(self, context)?
            .ok_or(Error::Identity)?;
        let partials = roots
            .evaluation()
            .root_partials(self, context)?
            .ok_or(Error::Identity)?;
        let source = source_covariance(&captured, context, work)?;
        assemble(&captured, &diagonal, &partials, &source, context)
    }
}

/// Fixed output and scratch owners are admitted before numerical capture; retained proofs are borrowed.
fn storage<P: Scalar>(budget: &Budget) -> Result<(), Error> {
    let output = size_of::<Covariance<P>>();
    // Root covariance, signed partials, source covariance, one sensitivity row and arithmetic slots.
    let staged = output + size_of::<Capture<'_, P>>() + 34 * size_of::<Enclosure<P>>();
    budget.check(
        BudgetKind::StagedPayloadBytes,
        u64::try_from(staged).map_err(|_| HeapError::ExtentOverflow)?,
        AT,
    )?;
    budget.check(
        BudgetKind::OutputBytes,
        u64::try_from(output).map_err(|_| HeapError::ExtentOverflow)?,
        AT,
    )?;
    Ok(())
}

/// Identity is stricter than equal covariance numbers or equivalent mathematical maps.
fn capture<'h, P: Scalar + 'static>(
    heap: &'h Heap,
    operator: MomentRef<P>,
    roots: &PublishedLcSpectrum<P>,
) -> Result<Capture<'h, P>, Error> {
    let moments = heap.moments(operator)?;
    let record = heap
        .statistics
        .records
        .get(operator.index)
        .ok_or(Error::Identity)?;
    let plan = roots.evaluation().plan();
    let (companion, columns) = plan.statistical_selection().ok_or(Error::Identity)?;
    let (identity, sources) = plan.statistical_identity().ok_or(Error::Identity)?;
    if identity != moments.model()
        || record.lineage.roots != [companion.index]
        || moments.assurance() != MomentAssurance::LinearizedMoments
        || moments.source_indices().len() != 2
    {
        return Err(Error::Identity);
    }
    original(&moments, plan)?;
    let mut positions = [0; 2];
    for (slot, &column) in moments.source_indices().iter().enumerate() {
        let position = columns
            .iter()
            .position(|&original| original == column)
            .ok_or(Error::Identity)?;
        if moments.source_ids().get(column) != Some(&sources[position]) {
            return Err(Error::Identity);
        }
        positions[slot] = position;
    }
    let model = record
        .lineage
        .model
        .data
        .downcast_ref::<JointModel<P>>()
        .ok_or(Error::Identity)?;
    Ok(Capture {
        moments,
        model,
        positions,
    })
}

/// The same native derivative and exact selected input owners establish reference and point agreement.
fn original<P: Scalar + 'static>(
    moments: &MomentView<'_, P>,
    plan: &crate::LcSpectrum<P>,
) -> Result<(), Error> {
    let inventory = moments.allocation_inventory();
    let derivative = inventory
        .operation()
        .and_then(crate::CheckedOperation::reciprocal_jacobian)
        .ok_or(Error::Identity)?;
    let root = plan.root_jacobian().ok_or(Error::Identity)?;
    let sensitivity = moments.sensitivity().ok_or(Error::Identity)?;
    let same_shape = moments.covariance().extent() == crate::Extent::new(4, 4)?
        && sensitivity.extent() == crate::Extent::new(4, 2)?;
    if !std::ptr::eq(derivative.native(), root.operator().native())
        || !same_shape
        || inventory.inputs().len() != 2
        || !moments.target_maps()[0].same_source(&moments.target_maps()[1])
    {
        return Err(Error::Identity);
    }
    for (dependency, input) in inventory.inputs().zip(plan.inputs()) {
        let selected = dependency.selected.as_ref().ok_or(Error::Identity)?;
        if !Rc::ptr_eq(&selected.group, &input.group) || selected.index != input.index {
            return Err(Error::Identity);
        }
    }
    Ok(())
}

/// Only the two participating original columns are contracted; unknown blocks never become zeros.
fn source_covariance<P: Scalar + 'static>(
    captured: &Capture<'_, P>,
    context: &ArithmeticContext,
    work: usize,
) -> Result<[[Enclosure<P>; 2]; 2], Error> {
    let columns = captured.moments.source_indices();
    let limits = JointLimits::new(2, 4, work);
    captured.model.require_blocks(columns, columns, limits)?;
    let mut result = [[Enclosure::singleton(Finite::ZERO); 2]; 2];
    for (row, entries) in result.iter_mut().enumerate() {
        for (column, entry) in entries.iter_mut().enumerate() {
            *entry = captured
                .model
                .covariance(columns[row], columns[column], context, limits)?;
        }
    }
    Ok(result)
}

/// Published diagonal blocks keep their parameter errors; cross entries share exact signed copies.
fn assemble<P: Scalar + 'static>(
    captured: &Capture<'_, P>,
    roots: &[[Enclosure<P>; 4]; 4],
    partials: &[[Enclosure<P>; 2]; 4],
    source: &[[Enclosure<P>; 2]; 2],
    context: &ArithmeticContext,
) -> Result<Covariance<P>, Error> {
    let mut result = [[Enclosure::singleton(Finite::ZERO); 8]; 8];
    let covariance = captured.moments.covariance();
    for (row, entries) in roots.iter().enumerate() {
        for (column, &entry) in entries.iter().enumerate() {
            result[row][column] = enclosed(
                covariance.get(row, column)?,
                covariance.error(row, column)?,
                context,
            )?;
            result[row + 4][column + 4] = entry;
        }
    }
    for row in 0..4 {
        let value = cross(captured, row, &partials[1], source, context)?;
        result[row][5] = value;
        result[5][row] = value;
        result[row][7] = value.negated();
        result[7][row] = value.negated();
    }
    Ok(result)
}

/// Source sensitivities are read from the operator output, not reconstructed from its covariance.
fn cross<P: Scalar + 'static>(
    captured: &Capture<'_, P>,
    row: usize,
    partials: &[Enclosure<P>; 2],
    source: &[[Enclosure<P>; 2]; 2],
    context: &ArithmeticContext,
) -> Result<Enclosure<P>, Error> {
    let sensitivity = captured.moments.sensitivity().ok_or(Error::Identity)?;
    let mut result = Enclosure::singleton(Finite::ZERO);
    for (column, entries) in source.iter().enumerate() {
        let left = enclosed(
            sensitivity.get(row, column)?,
            sensitivity.error(row, column)?,
            context,
        )?;
        for (index, &covariance) in entries.iter().enumerate() {
            let term = left
                .mul(covariance, context)?
                .mul(partials[captured.positions[index]], context)?;
            result = result.add(term, context)?;
        }
    }
    Ok(result)
}
