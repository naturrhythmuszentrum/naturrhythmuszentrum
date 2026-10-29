// SPDX-License-Identifier: AGPL-3.0-only
//! Complete physical output admission preserves exact tuple and reconstruction evidence.

use super::{
    DecisionError, LatticeAttachment, LatticeDecisionCase, LatticeIndexKind, SelectedLattice,
    physical::PhysicalPreparations,
};
use crate::decision::storage;
use nrz_cas::{
    Cas, ExactConstant, Rational,
    lattice::{CheckedLatticeOutputs, LatticeConvention},
    limits::{Budget, BudgetUsage},
    proof::{Statement, VerifiedWitness},
    syntax::{AtomicSort, Operator, Sort},
};
use std::rc::Rc;

/// Fully checked owned outputs, assembled privately before publishing a selected snapshot.
pub(super) struct Values {
    /// Complete selected-cell theorem.
    pub(super) selection: VerifiedWitness,
    /// Original-coordinate reconstruction theorem.
    pub(super) reconstruction: VerifiedWitness,
    /// Exact mathematical Integer tuple.
    pub(super) indices: Vec<Rational>,
    /// Restored original physical coordinates.
    pub(super) representatives: Vec<Rc<ExactConstant>>,
    /// Checked coherent B*k differences shared with optional site construction.
    pub(super) displacements: Vec<Rc<ExactConstant>>,
    /// Unique checked arithmetic judgments using retained question constants.
    pub(super) preparations: PhysicalPreparations,
    /// Cumulative caller ledger snapshots.
    pub(super) usage: [BudgetUsage; 2],
    /// Conservative complete newly owned payload.
    pub(super) bytes: u64,
}

impl LatticeDecisionCase {
    /// Admit checked CAS outputs under the complete original physical attachment.
    ///
    /// Each Integer retains its declared index kind. Representatives restore through
    /// the original references and origins, including angular and affine point roles.
    /// # Errors
    /// Refuses foreign proofs, changed questions or conventions, invalid counted
    /// values and exhausted resources. No selected physical tuple is published on failure.
    pub fn admit(
        &self,
        graph: Rc<Cas>,
        outputs: &CheckedLatticeOutputs<'_>,
        attachment: LatticeAttachment,
        budget: &mut Budget,
    ) -> Result<SelectedLattice, DecisionError> {
        let before = budget.usage();
        self.validate_attachment(&attachment, budget)?;
        self.check_outputs(&graph, outputs, budget)?;
        let mut bytes = storage::witness_bytes(outputs.selection())?
            .saturating_add(storage::witness_bytes(outputs.identity())?)
            .saturating_add(512);
        storage::retained(bytes, budget)?;
        budget.work(bytes, storage::AT)?;
        let physical = budget.with_retained_payload(bytes, storage::AT, |budget| {
            self.physical_outputs(&graph, outputs, budget)
        })?;
        bytes = bytes.saturating_add(physical.preparations.usage().bytes);
        for value in &physical.indices {
            bytes = bytes.saturating_add(storage::rational_bytes(value));
        }
        storage::retained(bytes, budget)?;
        let values = Values {
            selection: outputs.selection().clone(),
            reconstruction: outputs.identity().clone(),
            indices: physical.indices,
            representatives: physical.representatives,
            preparations: physical.preparations,
            displacements: physical.displacements,
            usage: [before, budget.usage()],
            bytes,
        };
        Ok(SelectedLattice::new(
            graph,
            self.clone(),
            attachment,
            values,
        ))
    }

    /// Bind all mathematical identities and the exact convention before copying outputs.
    fn check_outputs(
        &self,
        graph: &Cas,
        outputs: &CheckedLatticeOutputs<'_>,
        budget: &mut Budget,
    ) -> Result<(), DecisionError> {
        let (selection, _) = graph.lattice_selection_evidence(outputs.selection(), budget)?;
        let expected = self.specification();
        let convention = match &outputs.selection().judgment().statement {
            Statement::LatticeSelection(_) => LatticeConvention::Floor,
            Statement::LatticeNearestSelection(_) => LatticeConvention::Nearest,
            _ => return Err(DecisionError::Case),
        };
        budget.work(
            u64::from(expected.basis.dimension())
                .saturating_mul(256)
                .saturating_add(8),
            storage::AT,
        )?;
        if selection != outputs.question()
            || selection.cover.index != *self.question()
            || selection.cover.forward != *expected.basis.forward().expression()
            || selection.cover.inverse != *expected.basis.inverse().map().expression()
            || selection.cover.determinant != *expected.basis.inverse().determinant()
            || selection.target != expected.target
            || convention != expected.convention
        {
            return Err(DecisionError::Case);
        }
        Ok(())
    }

    /// Restore original exact entries and retain each unique preparation once.
    fn physical_outputs(
        &self,
        graph: &Cas,
        outputs: &CheckedLatticeOutputs<'_>,
        budget: &mut Budget,
    ) -> Result<PhysicalOutputs, DecisionError> {
        let spec = self.specification();
        let mut indices = Vec::new();
        let count = spec.axes.len();
        indices
            .try_reserve_exact(count)
            .map_err(|_| DecisionError::Capacity)?;
        for (position, kind) in spec.indices.iter().enumerate() {
            let node = outputs
                .tuple()
                .get(position)
                .ok_or(DecisionError::LatticeShape)?;
            let (operator, sort) = graph.expression(node)?;
            let Operator::Rational(value) = operator else {
                return Err(DecisionError::Case);
            };
            if sort != &Sort::Value(AtomicSort::Integer) || !value.is_integer() {
                return Err(DecisionError::Case);
            }
            if let LatticeIndexKind::Count(descriptor) = kind {
                descriptor
                    .kind()
                    .count_domain()
                    .ok_or(DecisionError::CountKind)?
                    .check_with_budget(value, budget)?;
            }
            budget.work(storage::rational_bytes(value), storage::AT)?;
            indices.push(value.clone());
        }
        let mut prepared = PhysicalPreparations::default();
        let mut representatives = Vec::new();
        let mut displacements = Vec::new();
        representatives
            .try_reserve_exact(count)
            .map_err(|_| DecisionError::Capacity)?;
        displacements
            .try_reserve_exact(count)
            .map_err(|_| DecisionError::Capacity)?;
        for axis in &spec.axes {
            let row = prepared.row(axis, &indices, outputs.tuple(), budget)?;
            representatives.push(row.representative);
            displacements.push(row.displacement);
        }
        Ok(PhysicalOutputs {
            indices,
            representatives,
            displacements,
            preparations: prepared,
        })
    }
}

/// Exact values share retained preparation evidence rather than copying constant graphs.
struct PhysicalOutputs {
    /// Exact ordered mathematical tuple.
    indices: Vec<Rational>,
    /// Restored physical coordinates, borrowing no mutable graph.
    representatives: Vec<Rc<ExactConstant>>,
    /// Checked physical displacements for the optional site.
    displacements: Vec<Rc<ExactConstant>>,
    /// One checked judgment per original physical source and exact Integer identity.
    preparations: PhysicalPreparations,
}
