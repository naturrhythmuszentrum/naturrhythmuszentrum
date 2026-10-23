// SPDX-License-Identifier: AGPL-3.0-only
//! Exact derivatives retain physical identity beyond the portable mathematical key.

use super::{PositiveReciprocalMap, ReadingError, SourceJacobianUsage, support};
use nrz_cas::{
    Cas,
    jacobians::{SourceCoordinates, SourceJacobianKey},
    limits::{Budget, BudgetKind},
    syntax::Operator,
};

/// Prepared exact identity, built once before repeated derivative uses.
#[derive(Clone, Debug)]
pub struct ReciprocalJacobianKey {
    /// Original whole-family operator and complete physical input meanings.
    definition: PositiveReciprocalMap,
    /// Original syntax, context, declarations, guards, representation and proof revision.
    pub(crate) native: SourceJacobianKey,
    /// Each independent derivative column's position in the original physical inputs.
    positions: Vec<usize>,
    /// Complete key-construction usage including every earlier caller operation.
    usage: SourceJacobianUsage,
    /// Conservative retained operation, portable key and index storage.
    pub(crate) bytes: u64,
}

impl ReciprocalJacobianKey {
    /// Keep all unfixed formal inputs as sources in their original declared order.
    ///
    /// Fixed names must be distinct formal inputs. They remain physically checked,
    /// strictly positive and present in every original denominator; they add no column.
    /// # Errors
    /// Refuses foreign original evidence, unknown/repeated fixed names and cumulative limits.
    pub fn new(
        cas: &mut Cas,
        definition: &PositiveReciprocalMap,
        fixed: &[String],
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        let before = budget.usage();
        cas.validate_reuse_key(definition.reuse_key(), budget)?;
        let (coordinates, positions) = coordinates(cas, definition, fixed, budget)?;
        let native = budget.with_retained_payload(
            definition.retained_payload_bytes(),
            support::AT,
            |budget| {
                cas.source_jacobian_key(
                    definition.frame().context(),
                    definition.original().expression(),
                    &coordinates,
                    budget,
                )
            },
        )?;
        let key_bytes = u64::try_from(native.as_bytes().len()).map_err(|_| ReadingError::Shape)?;
        let bytes = definition
            .retained_payload_bytes()
            .checked_add(key_bytes.checked_mul(4).ok_or(ReadingError::Shape)?)
            .and_then(|value| value.checked_add(1024))
            .ok_or(ReadingError::Shape)?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, support::AT)?;
        budget.check(BudgetKind::OutputBytes, bytes, support::AT)?;
        Ok(Self {
            definition: definition.clone(),
            native,
            positions,
            usage: SourceJacobianUsage {
                before,
                after: budget.usage(),
            },
            bytes,
        })
    }

    /// Complete original typed definition retained before differentiating any zero.
    #[must_use]
    pub const fn definition(&self) -> &PositiveReciprocalMap {
        &self.definition
    }

    /// Portable mathematical identity, including ordered sources and fixed declarations.
    #[must_use]
    pub const fn native(&self) -> &SourceJacobianKey {
        &self.native
    }

    /// Original physical input index of each derivative column.
    #[must_use]
    pub fn positions(&self) -> &[usize] {
        &self.positions
    }

    /// Complete key-construction snapshots on the caller ledger.
    #[must_use]
    pub const fn usage(&self) -> SourceJacobianUsage {
        self.usage
    }

    /// Compare actual mathematical key bytes and physical identities, never stored proof payloads.
    pub(crate) fn same(&self, other: &Self, budget: &mut Budget) -> Result<bool, ReadingError> {
        let native = self
            .native
            .as_bytes()
            .len()
            .max(other.native.as_bytes().len());
        let relation = self
            .definition
            .reuse_key()
            .as_bytes()
            .len()
            .max(other.definition.reuse_key().as_bytes().len());
        let count = native
            .checked_add(relation)
            .and_then(|value| value.checked_add(self.definition.inputs().len()))
            .ok_or(ReadingError::Shape)?;
        budget.work(
            u64::try_from(count).map_err(|_| ReadingError::Shape)?,
            support::AT,
        )?;
        if self.native != other.native
            || self.definition.inputs() != other.definition.inputs()
            || self.definition.reuse_key() != other.definition.reuse_key()
        {
            return Ok(false);
        }
        let left = self.definition.frame().entries();
        let right = other.definition.frame().entries();
        if left.len() != right.len() {
            return Ok(false);
        }
        for (first, second) in left.iter().zip(right) {
            budget.work(1, support::AT)?;
            if !first.shares_definition(second) {
                budget.work(self.bytes.max(other.bytes), support::AT)?;
                if first != second {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}

/// A fixed subset changes derivative columns, never original-domain validation.
fn coordinates(
    cas: &Cas,
    definition: &PositiveReciprocalMap,
    fixed: &[String],
    budget: &mut Budget,
) -> Result<(SourceCoordinates, Vec<usize>), ReadingError> {
    let mut names = support::reserve(definition.inputs().len(), budget)?;
    for input in definition.inputs() {
        let (_, operator, _, _) = support::inspect(cas, input, budget)?;
        let Operator::Free(name) = operator else {
            return Err(ReadingError::Definition);
        };
        names.push(name);
    }
    for (index, name) in fixed.iter().enumerate() {
        budget.work(
            u64::try_from(names.len() + index).map_err(|_| ReadingError::Shape)?,
            support::AT,
        )?;
        if !names.contains(name) || fixed[..index].contains(name) {
            return Err(ReadingError::Definition);
        }
    }
    let mut sources = support::reserve(names.len(), budget)?;
    let mut positions = support::reserve(names.len(), budget)?;
    for (index, name) in names.into_iter().enumerate() {
        budget.work(
            u64::try_from(fixed.len()).map_err(|_| ReadingError::Shape)?,
            support::AT,
        )?;
        if !fixed.contains(&name) {
            sources.push(name);
            positions.push(index);
        }
    }
    Ok((
        SourceCoordinates {
            sources,
            fixed: fixed.to_vec(),
        },
        positions,
    ))
}
