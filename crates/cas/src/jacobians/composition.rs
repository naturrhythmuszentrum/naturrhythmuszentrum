// SPDX-License-Identifier: AGPL-3.0-only
//! Propose a chain-rule contraction and check it against a direct original-domain derivative.

use super::{AT, ComposedJacobian, JacobianReadings, SourceCoordinates, invalid, storage};
use crate::budget::{Budget, BudgetKind};
use crate::calculus::Deriver;
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Handle, LogicalId, size};
use crate::model::{AtomicSort, Field, Operator, Sort};

/// Borrowed checked inputs keep a complete shared-source contraction together.
struct Chain<'a> {
    /// Mathematical hypothesis context.
    context: &'a Handle,
    /// Original expression over reading variables and optional direct sources.
    original: &'a Handle,
    /// Outer reading/source differentiation order.
    outer: &'a [String],
    /// Simultaneous shared-source replacements.
    replacements: &'a [crate::substitution::Substitution],
    /// Full reading/source coordinate column.
    mapping: &'a Handle,
    /// Ordered original source directions.
    sources: &'a [String],
    /// Independently proved Jacobian matrix.
    direct: &'a Handle,
}

impl Cas {
    /// Compose exact reading partials over one shared ordered source coordinate graph.
    ///
    /// The map substitutes all readings simultaneously. Direct appearances of source
    /// variables contribute their identity rows, so mixed readings introduce no new
    /// independent directions. The proposed outer partial contraction gains authority
    /// only from equality to the independently checked direct source Jacobian.
    /// # Errors
    /// Refuses overlapping source/reading declarations, fixed reading directions,
    /// cyclic map coefficients, incompatible shapes, unresolved original or substituted
    /// domains, unsupported syntax and exhausted cumulative resources.
    pub fn compose_reading_jacobian(
        &mut self,
        context: &Handle,
        original: &Handle,
        coordinates: &SourceCoordinates,
        readings: &JacobianReadings,
        budget: &mut Budget,
    ) -> Result<ComposedJacobian, Fault> {
        self.proof_limits(budget, |cas, budget| {
            cas.transaction(budget, |cas, budget| {
                let outer = cas.reading_directions(context, coordinates, readings, budget)?;
                let original_key = cas.expression_reuse_key(context, original, budget)?;
                let retained = size(original_key.as_bytes().len(), AT)?.saturating_mul(4);
                storage::retaining(retained, budget, |budget| {
                    let (replacements, mapping, map_representation) =
                        cas.jacobian_reading_values(context, coordinates, readings, budget)?;
                    let map_bytes = size(map_representation.bytes.len(), AT)?
                        .saturating_mul(8)
                        .saturating_add(
                            size(
                                readings
                                    .variables
                                    .len()
                                    .saturating_add(coordinates.sources.len()),
                                AT,
                            )?
                            .saturating_mul(
                                size(cas.state.context(cas.checked(context)?)?.header.len(), AT)?
                                    .saturating_add(256),
                            ),
                        );
                    storage::retaining(map_bytes, budget, |budget| {
                        let replaced = cas.substitute(original, &replacements, budget)?;
                        let key =
                            cas.source_jacobian_key(context, &replaced, coordinates, budget)?;
                        let direct = cas.source_jacobian(&key, budget)?;
                        drop(key);
                        let (composed, equality) = storage::retaining(
                            super::cache::payload(&direct)?,
                            budget,
                            |budget| {
                                cas.check_reading_chain(
                                    &Chain {
                                        context,
                                        original,
                                        outer: &outer,
                                        replacements: &replacements,
                                        mapping: &mapping,
                                        sources: &coordinates.sources,
                                        direct: direct.matrix(),
                                    },
                                    budget,
                                )
                            },
                        )?;
                        let result_bytes = retained
                            .saturating_add(map_bytes)
                            .saturating_add(super::cache::payload(&direct)?)
                            .saturating_add(size(equality.bytes.len(), AT)?.saturating_mul(8));
                        budget.check(BudgetKind::OutputBytes, result_bytes, AT)?;
                        budget.check(BudgetKind::StagedPayloadBytes, result_bytes, AT)?;
                        Ok(ComposedJacobian {
                            original: original.clone(),
                            original_key,
                            readings: readings.clone(),
                            map_representation,
                            direct,
                            composed,
                            equality,
                        })
                    })
                })
            })
        })
    }

    /// Proposed partials gain authority only through equality to a checked direct derivative.
    fn check_reading_chain(
        &mut self,
        chain: &Chain<'_>,
        budget: &mut Budget,
    ) -> Result<(Handle, crate::proof_model::VerifiedWitness), Fault> {
        let partials =
            self.reading_partial_proposal(chain.context, chain.original, chain.outer, budget)?;
        let evaluated = self.substitute(&partials, chain.replacements, budget)?;
        let inner =
            self.reading_partial_proposal(chain.context, chain.mapping, chain.sources, budget)?;
        let composed = self.jacobian_product(chain.context, &evaluated, &inner, budget)?;
        let equality =
            self.prove_reading_equality(chain.context, &composed, chain.direct, budget)?;
        if !equality.is_discharged() {
            return Err(invalid(InvalidCode::JacobianDomain));
        }
        self.replay_jacobian_fact(&equality, budget)?;
        Ok((composed, equality))
    }

    /// Validate the complete union before any direction is synthesized or copied.
    fn reading_directions(
        &self,
        context: &Handle,
        coordinates: &SourceCoordinates,
        readings: &JacobianReadings,
        budget: &mut Budget,
    ) -> Result<Vec<String>, Fault> {
        let shape_matches = usize::try_from(readings.map.inputs()).ok()
            == Some(coordinates.sources.len())
            && usize::try_from(readings.map.outputs()).ok() == Some(readings.variables.len());
        if !shape_matches {
            return Err(invalid(InvalidCode::ReadingShape));
        }
        let count = coordinates
            .sources
            .len()
            .saturating_add(readings.variables.len());
        let header = &self.state.context(self.checked(context)?)?.header;
        let copied = size(count.saturating_add(coordinates.fixed.len()), AT)?;
        let width = size(header.len(), AT)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            copied.saturating_mul(width.saturating_add(128)),
            AT,
        )?;
        budget.work(width.saturating_mul(copied), AT)?;
        let mut variables = storage::reserve(count, budget)?;
        variables.extend_from_slice(&readings.variables);
        variables.extend_from_slice(&coordinates.sources);
        let contract = SourceCoordinates {
            sources: variables,
            fixed: coordinates.fixed.clone(),
        };
        self.check_source_coordinates(context, &contract, budget)?;
        Ok(contract.sources)
    }

    /// Untrusted outer proposals never enter the proof kernel as derivative premises.
    fn reading_partial_proposal(
        &mut self,
        context: &Handle,
        original: &Handle,
        variables: &[String],
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let root = self.checked(original)?;
        let inputs =
            if self.state.expression(root)?.sort == Sort::Value(AtomicSort::Scalar(Field::Real)) {
                vec![root]
            } else {
                self.reading_output_entries(root, budget)?
            };
        storage::retaining(
            size(inputs.len(), AT)?.saturating_mul(128),
            budget,
            |budget| {
                let mut deriver = Deriver::default();
                let result =
                    deriver.jacobian(self, self.checked(context)?, &inputs, variables, budget)?;
                Ok(deriver.plan.get(result)?.right.clone())
            },
        )
    }

    /// Native output coordinates use logical column order regardless of storage order.
    fn reading_output_entries(
        &mut self,
        root: LogicalId,
        budget: &mut Budget,
    ) -> Result<Vec<LogicalId>, Fault> {
        let (rows, columns) = crate::polynomial::matrix::shape(&self.state, root)?;
        let entries = crate::readings::exact_expand::expand(self, root, budget)?;
        let mut ordered = storage::reserve(entries.len(), budget)?;
        for index in 0..entries.len() {
            let index64 = size(index, AT)?;
            let offset = (index64 % u64::from(rows)).saturating_mul(u64::from(columns))
                + index64 / u64::from(rows);
            ordered.push(
                *entries
                    .get(usize::try_from(offset).map_err(|_| invalid(InvalidCode::ReadingShape))?)
                    .ok_or_else(|| invalid(InvalidCode::ReadingShape))?,
            );
        }
        Ok(ordered)
    }

    /// Keep ordered contraction as native matrix syntax for independent coefficient checking.
    fn jacobian_product(
        &mut self,
        context: &Handle,
        left: &Handle,
        right: &Handle,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let first = self.checked(left)?;
        let second = self.checked(right)?;
        let (rows, inner) = crate::polynomial::matrix::shape(&self.state, first)?;
        let (other, columns) = crate::polynomial::matrix::shape(&self.state, second)?;
        if inner != other {
            return Err(invalid(InvalidCode::ReadingShape));
        }
        budget.check(
            BudgetKind::MatrixEntries,
            u64::from(rows).saturating_mul(u64::from(columns)),
            AT,
        )?;
        let header = self.state.context(self.checked(context)?)?.header_id;
        let id = self.generated(
            header,
            Operator::MatrixMultiply,
            crate::model::Sort::Value(crate::model::AtomicSort::Matrix {
                field: Field::Real,
                rows: crate::model::Extent::Literal(rows),
                columns: crate::model::Extent::Literal(columns),
            }),
            &[first, second],
            budget,
        )?;
        Ok(self.handle(id))
    }
}
