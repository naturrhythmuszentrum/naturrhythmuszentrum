// SPDX-License-Identifier: AGPL-3.0-only
//! Exact affine maps retain a native homogeneous matrix expression and its ordered operands.

use super::{affine::length, invalid, reserve};
use crate::budget::Budget;
use crate::error::{Fault, InvalidCode};
use crate::exact_constant::ExactConstant;
use crate::graph::{Cas, Draft, Handle, LogicalId, Reference};
use crate::model::{AtomicSort, Extent, Field, Operator, Sort};

/// A store-authenticated exact map `y=T*z+d`, represented by `[T,d;0,1]`.
///
/// Coefficients may be closed constants or scalar expressions over declared family
/// parameters. The last coordinate is the homogeneous constant, never a new variable.
#[derive(Clone, Debug)]
pub struct ExactAffineMap {
    /// Ordered source coordinate count.
    pub(super) inputs: u32,
    /// Ordered target coordinate count.
    pub(super) outputs: u32,
    /// Complete native matrix syntax, including every original coefficient and offset.
    pub(super) expression: Handle,
}

impl ExactAffineMap {
    /// Ordered input coordinate count, excluding the homogeneous constant.
    #[must_use]
    pub const fn inputs(&self) -> u32 {
        self.inputs
    }

    /// Ordered output coordinate count, excluding the homogeneous constant.
    #[must_use]
    pub const fn outputs(&self) -> u32 {
        self.outputs
    }

    /// Exact native matrix expression for proof, export and portable reuse identity.
    #[must_use]
    pub const fn expression(&self) -> &Handle {
        &self.expression
    }
}

impl Cas {
    /// Retain scalar coefficient expressions in their exact row-major coordinate order.
    ///
    /// The pair is `(inputs, outputs)`. Every coefficient must inhabit the context's
    /// Real scalar declaration snapshot; numerical values cannot supply hypotheses.
    /// # Errors
    /// Refuses foreign handles, incompatible sorts or shapes, and exhausted resources.
    pub fn exact_reading_map(
        &mut self,
        context: &Handle,
        dimensions: (u32, u32),
        coefficients: &[Handle],
        offsets: &[Handle],
        budget: &mut Budget,
    ) -> Result<ExactAffineMap, Fault> {
        self.proof_limits(budget, |cas, budget| {
            cas.transaction(budget, |cas, budget| {
                let header = cas.state.context(cas.checked(context)?)?.header_id;
                let (inputs, outputs) = dimensions;
                let coefficients_match = coefficients.len() == length(inputs, outputs)?;
                let offsets_match = offsets.len() == length(outputs, 1)?;
                if !coefficients_match || !offsets_match {
                    return Err(invalid(InvalidCode::ReadingShape));
                }
                super::storage::slots(
                    coefficients.len().saturating_add(offsets.len()),
                    budget,
                    |budget| {
                        let mut values = reserve(coefficients.len(), budget)?;
                        for coefficient in coefficients {
                            values.push(cas.checked(coefficient)?);
                        }
                        let mut shifts = reserve(offsets.len(), budget)?;
                        for offset in offsets {
                            shifts.push(cas.checked(offset)?);
                        }
                        cas.map_from_entries(header, dimensions, &values, &shifts, budget)
                    },
                )
            })
        })
    }

    /// Import finite exact constants, including `Pi/180`, without approximating them.
    /// # Errors
    /// Refuses malformed shapes, incompatible contexts and exhausted cumulative resources.
    pub fn constant_reading_map(
        &mut self,
        context: &Handle,
        dimensions: (u32, u32),
        coefficients: &[ExactConstant],
        offsets: &[ExactConstant],
        budget: &mut Budget,
    ) -> Result<ExactAffineMap, Fault> {
        self.proof_limits(budget, |cas, budget| {
            cas.transaction(budget, |cas, budget| {
                let header = cas.context_header(context)?;
                let values = cas.constant_entries(&header, coefficients, budget)?;
                let shifts = cas.constant_entries(&header, offsets, budget)?;
                cas.exact_reading_map(context, dimensions, &values, &shifts, budget)
            })
        })
    }

    /// Import each closed constant through its existing exact expression drafts.
    fn constant_entries(
        &mut self,
        header: &Handle,
        entries: &[ExactConstant],
        budget: &mut Budget,
    ) -> Result<Vec<Handle>, Fault> {
        super::storage::slots(entries.len(), budget, |budget| {
            let mut result = reserve(entries.len(), budget)?;
            for entry in entries {
                let drafts =
                    entry.expression_drafts(Reference::Existing(header.clone()), budget)?;
                let drafts: Vec<_> = drafts.into_iter().map(Draft::Expression).collect();
                let root = self
                    .admit(&drafts, budget)?
                    .pop()
                    .ok_or_else(|| invalid(InvalidCode::Reference))?;
                result.push(root);
            }
            Ok(result)
        })
    }

    /// Compose `outer(inner(z))` by native ordered matrix multiplication.
    /// # Errors
    /// Refuses foreign maps, incompatible dimensions or snapshots and exhausted resources.
    pub fn compose_reading_maps(
        &mut self,
        outer: &ExactAffineMap,
        inner: &ExactAffineMap,
        budget: &mut Budget,
    ) -> Result<ExactAffineMap, Fault> {
        self.proof_limits(budget, |cas, budget| {
            cas.transaction(budget, |cas, budget| {
                let left = cas.checked(&outer.expression)?;
                let right = cas.checked(&inner.expression)?;
                if outer.inputs != inner.outputs {
                    return Err(invalid(InvalidCode::ReadingShape));
                }
                let header = cas.state.expression(left)?.header;
                let rows = extent(outer.outputs)?;
                let columns = extent(inner.inputs)?;
                let id = cas.generated(
                    header,
                    Operator::MatrixMultiply,
                    rectangle(rows, columns),
                    &[left, right],
                    budget,
                )?;
                Ok(ExactAffineMap {
                    inputs: inner.inputs,
                    outputs: outer.outputs,
                    expression: cas.handle(id),
                })
            })
        })
    }

    /// A single native constructor retains every original scalar, including zeros.
    pub(super) fn map_from_entries(
        &mut self,
        header: LogicalId,
        dimensions: (u32, u32),
        coefficients: &[LogicalId],
        offsets: &[LogicalId],
        budget: &mut Budget,
    ) -> Result<ExactAffineMap, Fault> {
        let (inputs, outputs) = dimensions;
        let rows = extent(outputs)?;
        let columns = extent(inputs)?;
        super::storage::slots(length(rows, columns)?, budget, |budget| {
            let mut entries = reserve(length(rows, columns)?, budget)?;
            for row in 0..outputs {
                let start = length(row, inputs)?;
                let end = start
                    .checked_add(length(inputs, 1)?)
                    .ok_or_else(|| invalid(InvalidCode::ReadingShape))?;
                entries.extend_from_slice(
                    coefficients
                        .get(start..end)
                        .ok_or_else(|| invalid(InvalidCode::ReadingShape))?,
                );
                entries.push(
                    *offsets
                        .get(length(row, 1)?)
                        .ok_or_else(|| invalid(InvalidCode::ReadingShape))?,
                );
            }
            let zero = self.map_scalar(
                header,
                Operator::Rational(crate::Rational::zero()),
                &[],
                budget,
            )?;
            let one = self.map_scalar(
                header,
                Operator::Rational(crate::Rational::one()),
                &[],
                budget,
            )?;
            entries.resize(length(rows, columns)?.saturating_sub(1), zero);
            entries.push(one);
            let expression = self.generated(
                header,
                Operator::Matrix { rows, columns },
                rectangle(rows, columns),
                &entries,
                budget,
            )?;
            Ok(ExactAffineMap {
                inputs,
                outputs,
                expression: self.handle(expression),
            })
        })
    }

    /// All generated coefficient arithmetic stays scalar; matrix multiplication stays ordered.
    pub(super) fn map_scalar(
        &mut self,
        header: LogicalId,
        operator: Operator,
        operands: &[LogicalId],
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        self.generated(
            header,
            operator,
            Sort::Value(AtomicSort::Scalar(Field::Real)),
            operands,
            budget,
        )
    }
}

/// Homogeneous extents must not wrap into another shape.
pub(super) fn extent(count: u32) -> Result<u32, Fault> {
    count
        .checked_add(1)
        .ok_or_else(|| invalid(InvalidCode::ReadingShape))
}

/// The native exact matrix vocabulary uses literal ordered Real rectangles.
pub(super) const fn rectangle(rows: u32, columns: u32) -> Sort {
    Sort::Value(AtomicSort::Matrix {
        field: Field::Real,
        rows: Extent::Literal(rows),
        columns: Extent::Literal(columns),
    })
}
