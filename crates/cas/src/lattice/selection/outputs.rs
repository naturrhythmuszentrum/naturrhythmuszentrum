// SPDX-License-Identifier: AGPL-3.0-only
//! Exact tuple and representative descriptors keep capacity conversion separate.

use super::{LatticeEnclosure, LatticeSelection};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, LogicalId, Vertex, size};
use crate::model::{AtomicSort, Extent, Field, Operator, Sort};
use crate::number::Rational;
use crate::proof_model::{Certificate, Statement, VerifiedWitness};
use crate::rule::Primitive;

/// Output construction and metadata access own their costs independently of selection.
const AT: Location = Location::new("lattice exact outputs");

/// Wrong proof roots or mismatched local output identities cannot grant descriptor authority.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::EndpointMismatch, AT)
}

/// Immutable local descriptors retaining the selected cell and a portable reconstruction identity.
///
/// The selection is borrowed, so output or materialization refusal leaves its complete evidence
/// with the caller. No physical coordinate meaning or live-storage attachment is introduced.
#[derive(Debug)]
pub struct CheckedLatticeOutputs<'a> {
    /// Original checked selection, with all hypotheses and enclosure dependencies.
    selection: &'a VerifiedWitness,
    /// Complete original question and ordered mathematical Integer tuple.
    question: &'a LatticeSelection,
    /// Exact coordinate expression r = q - B*k, including all original zero products.
    representative: Handle,
    /// Checked q = B*k + r in the original parameter context.
    identity: VerifiedWitness,
}

impl CheckedLatticeOutputs<'_> {
    /// Ordered exact Integer descriptors, independent of machine representation or capacity.
    #[must_use]
    pub fn tuple(&self) -> &[Handle] {
        &self.question.values
    }

    /// Original mathematical question with its complete target, basis, offset and range identity.
    #[must_use]
    pub const fn question(&self) -> &LatticeSelection {
        self.question
    }

    /// Complete selected-cell witness retaining all original assumptions and proof revisions.
    #[must_use]
    pub const fn selection(&self) -> &VerifiedWitness {
        self.selection
    }

    /// Exact representative r = q - B*k; the original offset is not subtracted again.
    #[must_use]
    pub const fn representative(&self) -> &Handle {
        &self.representative
    }

    /// Independently replayable q = B*k + r identity; selection evidence remains separate.
    #[must_use]
    pub const fn identity(&self) -> &VerifiedWitness {
        &self.identity
    }
}

impl Cas {
    /// Borrow the complete original tuple question and enclosure of a certified selection.
    /// # Errors
    /// Refuses foreign witnesses, other roots, residual obligations and exhausted Work.
    pub fn lattice_selection_evidence<'a>(
        &'a self,
        witness: &VerifiedWitness,
        budget: &mut Budget,
    ) -> Result<(&'a LatticeSelection, &'a LatticeEnclosure), Fault> {
        budget.work(4, AT)?;
        let Vertex::Proof(application) = self.state.node(self.checked(witness.proof())?)? else {
            return Err(invalid());
        };
        let Vertex::ScopedApplication(proof) = self.state.node(*application)? else {
            return Err(invalid());
        };
        let Statement::LatticeSelection(selection) = &proof.judgment.statement else {
            return Err(invalid());
        };
        let Certificate::LatticeSelection(enclosure) = &proof.certificate else {
            return Err(invalid());
        };
        if proof.rule != Primitive::CaseSelect || !proof.conditions.residuals.is_empty() {
            return Err(invalid());
        }
        Ok((selection, enclosure))
    }

    /// Construct exact tuple/representative descriptors and check their reconstruction identity.
    ///
    /// Scalar products retain the original B entries and exact literal embeddings of the checked
    /// Integers. The existing matrix equality checker proves q = B*k + r before returning any output.
    /// A translated floor lattice keeps its offset in the representative, since r lies in
    /// phi+B*[0,1)^d.
    /// # Errors
    /// Refuses invalid selections, unsupported identities and exhausted output/proof resources.
    /// Construction is atomic; failure preserves the selected tuple and spends attempted Work.
    pub fn lattice_selection_outputs<'a>(
        &mut self,
        witness: &'a VerifiedWitness,
        budget: &mut Budget,
    ) -> Result<CheckedLatticeOutputs<'a>, Fault> {
        self.proof_limits(budget, |cas, budget| {
            cas.transaction(budget, |cas, budget| {
                let (actual, _) = cas.lattice_selection_evidence(witness, budget)?;
                let Statement::LatticeSelection(question) = &witness.judgment().statement else {
                    return Err(invalid());
                };
                let count = size(question.values.len(), AT)?;
                budget.check(BudgetKind::MatrixEntries, count.saturating_mul(count), AT)?;
                let bytes = super::super::cover::check::payload(&question.cover, budget)?
                    .saturating_add(count.saturating_mul(256))
                    .saturating_add(512);
                budget.work(bytes, AT)?;
                if actual != question.as_ref() {
                    return Err(invalid());
                }
                budget.with_retained_payload(bytes, AT, |budget| {
                    let (representative, reconstruction) =
                        cas.lattice_output_columns(question, budget)?;
                    let identity = cas.prove_reading_equality(
                        &question.cover.index.basis.context,
                        &question.target,
                        &reconstruction,
                        budget,
                    )?;
                    if !identity.is_discharged() {
                        return Err(Fault::invalid(InvalidCode::PendingPremise, AT));
                    }
                    budget.check(
                        BudgetKind::OutputBytes,
                        size(identity.exported_byte_len(), AT)?.saturating_add(512),
                        AT,
                    )?;
                    Ok(CheckedLatticeOutputs {
                        selection: witness,
                        question,
                        representative,
                        identity,
                    })
                })
            })
        })
    }

    /// Convert one nonnegative coordinate within its declared inclusive u64 capacity.
    /// # Errors
    /// Refuses unproved selections, missing coordinates, negative/oversized values and budgets.
    /// The exact mathematical tuple and its witness remain intact on every refusal.
    pub fn materialize_lattice_count(
        &self,
        witness: &VerifiedWitness,
        coordinate: usize,
        capacity: u64,
        budget: &mut Budget,
    ) -> Result<u64, Fault> {
        self.lattice_selected_coordinate(witness, coordinate, budget)?
            .materialize_count(capacity, budget)
    }

    /// Convert one signed coordinate within its declared inclusive i64 interval.
    /// # Errors
    /// Refuses reversed intervals, foreign/unproved selections, missing coordinates,
    /// nonrepresentable or out-of-capacity Integers and exhausted budgets, without altering k.
    pub fn materialize_lattice_index(
        &self,
        witness: &VerifiedWitness,
        coordinate: usize,
        capacity: (i64, i64),
        budget: &mut Budget,
    ) -> Result<i64, Fault> {
        if capacity.0 > capacity.1 {
            return Err(Fault::invalid(InvalidCode::Policy, AT));
        }
        let value = self.lattice_selected_coordinate(witness, coordinate, budget)?;
        let refused = || Fault::invalid(InvalidCode::DecisionMaterializationCapacity, AT);
        let (value, denominator) = value.small_ratio(budget)?.ok_or_else(refused)?;
        let lower = value >= capacity.0;
        let upper = value <= capacity.1;
        if denominator != 1 || !lower || !upper {
            return Err(refused());
        }
        Ok(value)
    }

    /// Authenticate the complete tuple before reading a coordinate's exact literal.
    fn lattice_selected_coordinate(
        &self,
        witness: &VerifiedWitness,
        coordinate: usize,
        budget: &mut Budget,
    ) -> Result<&Rational, Fault> {
        let (selection, _) = self.lattice_selection_evidence(witness, budget)?;
        budget.work(1, AT)?;
        let value = selection
            .values
            .get(coordinate)
            .ok_or_else(|| Fault::invalid(InvalidCode::SelectionOutOfRange, AT))?;
        super::literal(self, value, &selection.cover.index.basis.context)
    }

    /// Expand exact ordered products into explicit Real columns without normalization.
    fn lattice_output_columns(
        &mut self,
        question: &LatticeSelection,
        budget: &mut Budget,
    ) -> Result<(Handle, Handle), Fault> {
        let home = self.checked(&question.cover.index.basis.context)?;
        let basis = self.checked(&question.cover.index.basis.basis)?;
        let target = self.checked(&question.target)?;
        let count = question.values.len();
        let mut embedded = reserve(count, budget)?;
        let mut residuals = reserve(count, budget)?;
        let mut restored = reserve(count, budget)?;
        for value in &question.values {
            let number = super::literal(self, value, &question.cover.index.basis.context)?;
            let (numerator, denominator) = number.component_bits();
            let bytes = numerator
                .div_ceil(8)
                .saturating_add(denominator.div_ceil(8))
                .saturating_add(64);
            budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
            budget.work(bytes, AT)?;
            let number = number.clone();
            embedded.push(scalar(self, home, Operator::Rational(number), &[], budget)?);
        }
        for row in 0..count {
            let mut product = scalar(
                self,
                home,
                Operator::Rational(Rational::zero()),
                &[],
                budget,
            )?;
            for (column, value) in embedded.iter().enumerate() {
                budget.work(1, AT)?;
                let coefficient = self.state.expression(basis)?.operands[row * count + column];
                let term = scalar(self, home, Operator::Mul, &[coefficient, *value], budget)?;
                product = scalar(self, home, Operator::Add, &[product, term], budget)?;
            }
            let original = self.state.expression(target)?.operands[row];
            let remainder = scalar(self, home, Operator::Sub, &[original, product], budget)?;
            residuals.push(remainder);
            restored.push(scalar(
                self,
                home,
                Operator::Add,
                &[product, remainder],
                budget,
            )?);
        }
        Ok((
            column(self, home, &residuals, budget)?,
            column(self, home, &restored, budget)?,
        ))
    }
}

/// Bound each temporary column before reserving its finite ordered inventory.
fn reserve(count: usize, budget: &mut Budget) -> Result<Vec<LogicalId>, Fault> {
    let bytes = size(count, AT)?.saturating_mul(64);
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(bytes, AT)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(AT))?;
    Ok(result)
}

/// Scalar constructors stay in the original declaration frame and use native exact operators.
fn scalar(
    cas: &mut Cas,
    home: LogicalId,
    operator: Operator,
    operands: &[LogicalId],
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    cas.generated(
        home,
        operator,
        Sort::Value(AtomicSort::Scalar(Field::Real)),
        operands,
        budget,
    )
}

/// The returned column preserves original coordinate order and exact scalar operands.
fn column(
    cas: &mut Cas,
    home: LogicalId,
    entries: &[LogicalId],
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let rows =
        u32::try_from(entries.len()).map_err(|_| Fault::capacity(BudgetKind::MatrixEntries, AT))?;
    let id = cas.generated(
        home,
        Operator::Matrix { rows, columns: 1 },
        Sort::Value(AtomicSort::Matrix {
            field: Field::Real,
            rows: Extent::Literal(rows),
            columns: Extent::Literal(1),
        }),
        entries,
        budget,
    )?;
    Ok(cas.handle(id))
}
