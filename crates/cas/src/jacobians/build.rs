// SPDX-License-Identifier: AGPL-3.0-only
//! Exact scalar production and original matrix expansion share one caller ledger.

use super::{AT, SourceJacobian, SourceJacobianKey, invalid, storage};
use crate::budget::{Budget, BudgetKind};
use crate::calculus::Deriver;
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Handle, LogicalId, size};
use crate::model::{AtomicSort, Extent, Field, Operator, Sort};
use crate::proof_model::{Statement, VerifiedWitness};

/// Explicit column entries and their original matrix equality, when needed.
type Expansion = (Vec<LogicalId>, Option<VerifiedWitness>);

impl Cas {
    /// Derive the exact ordered Jacobian and replay its complete partial proof fresh.
    ///
    /// Supported originals are Real scalar rational-function expressions, explicit
    /// Real matrices of them and ordered products of such matrices. Matrices flatten
    /// in logical column order. All original domain obligations must discharge from
    /// the retained context. Construction is atomic and never resets the ledger.
    /// # Errors
    /// Refuses foreign keys, unsupported syntax, unresolved or refuted original
    /// domains, missing partials and exhausted construction, checking or replay limits.
    pub fn source_jacobian(
        &mut self,
        key: &SourceJacobianKey,
        budget: &mut Budget,
    ) -> Result<SourceJacobian, Fault> {
        self.proof_limits(budget, |cas, budget| {
            cas.transaction(budget, |cas, budget| {
                let context = cas.checked(&key.context)?;
                let original = cas.checked(&key.original)?;
                storage::retaining(key.payload(), budget, |budget| {
                    let (entries, representation) = cas.jacobian_entries(key, original, budget)?;
                    let retained = size(entries.len(), AT)?.saturating_mul(128).saturating_add(
                        representation.as_ref().map_or(0, |value| {
                            size(value.bytes.len(), AT)
                                .unwrap_or(u64::MAX)
                                .saturating_mul(4)
                        }),
                    );
                    storage::retaining(retained, budget, |budget| {
                        let mut deriver = Deriver::default();
                        let root = deriver.jacobian(
                            cas,
                            context,
                            &entries,
                            &key.coordinates.sources,
                            budget,
                        )?;
                        let plan = deriver.plan.selected(root, budget)?;
                        let checked = plan.verify(cas, context, budget)?;
                        let witness = crate::proof_view::native_witness(cas, checked, budget)?;
                        if !witness.is_discharged() {
                            return Err(invalid(InvalidCode::JacobianDomain));
                        }
                        let Statement::Jacobian { output, .. } = &witness.judgment().statement
                        else {
                            return Err(invalid(InvalidCode::Rule));
                        };
                        budget.check(
                            BudgetKind::OutputBytes,
                            key.payload()
                                .saturating_add(retained)
                                .saturating_add(size(witness.bytes.len(), AT)?.saturating_mul(4)),
                            AT,
                        )?;
                        Ok(SourceJacobian {
                            key: key.clone(),
                            matrix: output.clone(),
                            representation,
                            witness,
                        })
                    })
                })
            })
        })
    }

    /// Check original domains before normalization, including an empty derivative direction set.
    fn jacobian_entries(
        &mut self,
        key: &SourceJacobianKey,
        original: LogicalId,
        budget: &mut Budget,
    ) -> Result<Expansion, Fault> {
        let mut conditions = crate::conditions::Conditions::default();
        conditions.endpoint(&self.state, self.checked(&key.context)?, original, budget)?;
        if !conditions.residuals.is_empty() {
            return Err(invalid(InvalidCode::JacobianDomain));
        }
        if self.state.expression(original)?.sort == Sort::Value(AtomicSort::Scalar(Field::Real)) {
            let mut result = storage::reserve(1, budget)?;
            result.push(original);
            return Ok((result, None));
        }
        self.jacobian_matrix_entries(key, original, budget)
    }

    /// The ordinary matrix coefficient checker independently certifies producer expansion.
    fn jacobian_matrix_entries(
        &mut self,
        key: &SourceJacobianKey,
        original: LogicalId,
        budget: &mut Budget,
    ) -> Result<Expansion, Fault> {
        let (rows, columns) = crate::polynomial::matrix::shape(&self.state, original)?;
        let mut entries = crate::readings::exact_expand::expand(self, original, budget)?;
        for entry in &mut entries {
            *entry = crate::polynomial::normalize(self, *entry, budget)?;
        }
        storage::retaining(
            size(entries.len(), AT)?.saturating_mul(128),
            budget,
            |budget| {
                let explicit =
                    self.jacobian_rectangle(&key.context, rows, columns, &entries, budget)?;
                let representation =
                    self.prove_reading_equality(&key.context, &key.original, &explicit, budget)?;
                if !representation.is_discharged() {
                    return Err(invalid(InvalidCode::JacobianDomain));
                }
                self.replay_jacobian_fact(&representation, budget)?;
                let mut ordered = storage::reserve(entries.len(), budget)?;
                for position in 0..entries.len() {
                    let index =
                        u64::try_from(position).map_err(|_| invalid(InvalidCode::ReadingShape))?;
                    let row = index % u64::from(rows);
                    let column = index / u64::from(rows);
                    let offset = row
                        .saturating_mul(u64::from(columns))
                        .saturating_add(column);
                    ordered.push(
                        *entries
                            .get(
                                usize::try_from(offset)
                                    .map_err(|_| invalid(InvalidCode::ReadingShape))?,
                            )
                            .ok_or_else(|| invalid(InvalidCode::ReadingShape))?,
                    );
                }
                Ok((ordered, Some(representation)))
            },
        )
    }

    /// Compare the expected canonical judgment with independent fresh-store replay.
    pub(super) fn replay_jacobian_fact(
        &self,
        witness: &VerifiedWitness,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let expected = crate::wire_build::judgment_key(self, witness.judgment(), budget)?;
        storage::retaining(size(expected.len(), AT)?, budget, |budget| {
            let mut fresh = Self::new(self.limits.clone())?;
            let checked = fresh.inspect(&witness.bytes, budget)?;
            let actual = crate::wire_build::judgment_key(&fresh, checked.judgment(), budget)?;
            budget.work(size(expected.len().max(actual.len()), AT)?, AT)?;
            if expected != actual {
                return Err(invalid(InvalidCode::EndpointMismatch));
            }
            Ok(())
        })
    }

    /// Build an explicit Real rectangle using ordinary graph admission.
    pub(super) fn jacobian_rectangle(
        &mut self,
        context: &Handle,
        rows: u32,
        columns: u32,
        entries: &[LogicalId],
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let header = self.state.context(self.checked(context)?)?.header_id;
        let id = self.generated(
            header,
            Operator::Matrix { rows, columns },
            Sort::Value(AtomicSort::Matrix {
                field: Field::Real,
                rows: Extent::Literal(rows),
                columns: Extent::Literal(columns),
            }),
            entries,
            budget,
        )?;
        Ok(self.handle(id))
    }
}
