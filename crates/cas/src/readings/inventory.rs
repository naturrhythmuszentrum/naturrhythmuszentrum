// SPDX-License-Identifier: AGPL-3.0-only
//! Public inventory boundaries authenticate roots and reuse canonical witness closure machinery.

use super::inventory_build::build;
use super::inventory_model::DependencyInventory;
use super::{AT, check, invalid};
use crate::budget::Budget;
use crate::encoding::Encoder;
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Handle, size};
use crate::proof_model::{Statement, VerifiedWitness};
use crate::wire_build::{Entity, IndexedTable, envelope, indexed_table};

impl Cas {
    /// Inventory a defined reading or its exact projection from its original syntax closure.
    ///
    /// Retains every declaration, ordered selection and defining equation. Base entries
    /// are the exact nonzero support of composed affine demands at each named source.
    /// Both Real coordinates of a Complex scalar name its single scalar entry `(0,0)`.
    /// # Errors
    /// Refuses foreign or bare declaration handles, unsupported reading sources, and
    /// exhausted cumulative work, canonical-table or temporary/output payload resources.
    pub fn reading_dependencies(
        &self,
        reading: &Handle,
        budget: &mut Budget,
    ) -> Result<DependencyInventory, Fault> {
        budget.witness_records.enabled = true;
        self.inventory_limits(budget, |budget| {
            let id = self.checked(reading)?;
            if !check::applicable(&self.state, id, id)? {
                return Err(invalid(InvalidCode::ReadingDefinition));
            }
            let table = indexed_table(self, Entity::Object(id), budget)?;
            build(self, reading, table, budget)
        })
    }

    /// Inventory a verified reading equality, including every retained proof premise.
    ///
    /// The canonical table comes from the witness's own authenticated proof root. All
    /// earlier equality applications contribute their original endpoints. Fresh replay
    /// produces equal portable contents even when local graph identifiers differ.
    /// # Errors
    /// Refuses foreign or changed witness caches, nonreading judgments, unsupported
    /// retained sources and exhausted cumulative construction or payload resources.
    pub fn witness_dependencies(
        &self,
        witness: &VerifiedWitness,
        budget: &mut Budget,
    ) -> Result<DependencyInventory, Fault> {
        budget.witness_records.enabled = true;
        self.inventory_limits(budget, |budget| {
            let table = self.reading_witness_table(witness, budget)?;
            build(self, witness.proof(), table, budget)
        })
    }

    /// Authenticate every retained witness field against its graph-owned application.
    fn reading_witness_table(
        &self,
        witness: &VerifiedWitness,
        budget: &mut Budget,
    ) -> Result<IndexedTable, Fault> {
        let id = self.checked(witness.proof())?;
        let application = crate::proof_view::application(self, id, budget)?;
        let mut out = Encoder::new(budget, AT);
        crate::statement::judgment(witness.judgment(), &mut out)?;
        let bytes = out.finish();
        budget.work(size(bytes.len(), AT)?, AT)?;
        drop(bytes);
        budget.work(size(witness.hypotheses.len(), AT)?, AT)?;
        let same_judgment = application.judgment == witness.judgment;
        let same_hypotheses = application.conditions.hypotheses == witness.hypotheses;
        if !same_judgment || !same_hypotheses {
            return Err(invalid(InvalidCode::DependencyWitness));
        }
        let Statement::Equality { left, right, .. } = &application.judgment.statement else {
            return Err(invalid(InvalidCode::ReadingDefinition));
        };
        if !check::applicable(&self.state, self.checked(left)?, self.checked(right)?)? {
            return Err(invalid(InvalidCode::ReadingDefinition));
        }
        let root = Entity::Conclusion {
            proof: Some(id),
            judgment: application.judgment,
            hypotheses: application.conditions.hypotheses,
        };
        let table = indexed_table(self, root, budget)?;
        compare_witness(witness, &table, budget)?;
        Ok(table)
    }

    /// Every public inventory service intersects caller and store ceilings without resetting work.
    pub(super) fn inventory_limits<T>(
        &self,
        budget: &mut Budget,
        action: impl FnOnce(&mut Budget) -> Result<T, Fault>,
    ) -> Result<T, Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = action(budget);
        budget.restore_limits(previous);
        result
    }
}

/// Re-encoding binds canonical bytes to the same root as the inspected field views.
fn compare_witness(
    witness: &VerifiedWitness,
    table: &IndexedTable,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let (records, root, rules, assigned) = table;
    let meter = super::inventory_budget::table_meter(records, assigned, budget)?;
    let actual = meter.run(budget, |budget| {
        envelope(records, *root, rules, crate::wire::MAGIC, budget)
    })?;
    budget.work(size(actual.len().max(witness.bytes.len()), AT)?, AT)?;
    if actual != witness.bytes {
        return Err(invalid(InvalidCode::DependencyWitness));
    }
    Ok(())
}
