// SPDX-License-Identifier: AGPL-3.0-only
//! Native ordered matrix application is the exact result definition, with checked physical axes.

use super::super::{EntryMeaning, ReadingAxis, ReadingError, ReadingShape, support};
use super::{LinearOperator, ReadingOperationError};
use nrz_cas::{
    Cas, Handle,
    domain::Layer,
    limits::Budget,
    proof::Primitive,
    proof::{ApplicationDraft, Certificate, DomainContract, Judgment, Statement, VerifiedWitness},
    syntax::{AtomicSort, Extent, Field, Operator, Sort},
};

/// A closed ordered matrix application with its actual expression as result identity.
#[derive(Clone, Debug)]
pub struct OperatorApplication {
    /// Exact formal coefficient operator, including every original typed entry.
    operator: LinearOperator,
    /// Exact input reading with ordered row and column axes.
    input: ReadingShape,
    /// Native ordered `MatrixMultiply` expression; no independent Free result is equated to it.
    node: Handle,
    /// Ordered result row/codomain and column axes.
    axes: [ReadingAxis; 2],
    /// Complete physical result entries in logical column order.
    entries: Vec<EntryMeaning>,
    /// Scoped definedness of the actual native operation, not a numerical lowering theorem.
    witness: VerifiedWitness,
}

impl OperatorApplication {
    /// Construct and check the native exact ordered matrix application as its own result definition.
    /// # Errors
    /// Refuses wrong contraction axes, original physical coordinates, domains or finite limits.
    pub fn define(
        cas: &mut Cas,
        operator: &LinearOperator,
        input: &ReadingShape,
        budget: &mut Budget,
    ) -> Result<Self, ReadingOperationError> {
        let (node, axes, entries) = build(cas, operator, input, budget)?;
        let claim = expected(cas, operator.context(), &node)?;
        let step = ApplicationDraft {
            rule: Primitive::Reflexivity,
            context: claim.context.clone(),
            conclusion: claim.statement,
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::None,
        };
        let witness = cas.check_proof(operator.context(), &[step], budget)?;
        if !witness.is_discharged() {
            return Err(ReadingOperationError::Unproved);
        }
        Ok(Self {
            operator: operator.clone(),
            input: input.clone(),
            node,
            axes,
            entries,
            witness,
        })
    }
    /// Reconstruct the original ordered product before replaying its scoped definedness evidence.
    /// # Errors
    /// Refuses changed operands, order, shape, axes, physical meanings, domain or proof bytes.
    pub fn replay(
        cas: &mut Cas,
        operator: &LinearOperator,
        input: &ReadingShape,
        bytes: &[u8],
        budget: &mut Budget,
    ) -> Result<Self, ReadingOperationError> {
        let (node, axes, entries) = build(cas, operator, input, budget)?;
        let witness = cas.replay(bytes, &expected(cas, operator.context(), &node)?, budget)?;
        if !witness.is_discharged() {
            return Err(ReadingOperationError::Unproved);
        }
        Ok(Self {
            operator: operator.clone(),
            input: input.clone(),
            node,
            axes,
            entries,
            witness,
        })
    }
    /// Exact formal coefficient operator; physical metadata remains inspectable.
    #[must_use]
    pub const fn operator(&self) -> &LinearOperator {
        &self.operator
    }
    /// Exact original input reading, preserving its family and ordered selection.
    #[must_use]
    pub const fn input(&self) -> &ReadingShape {
        &self.input
    }
    /// Exact result identity: the original native ordered operation expression.
    #[must_use]
    pub const fn node(&self) -> &Handle {
        &self.node
    }
    /// Ordered result axes expected from any numerical output binding.
    #[must_use]
    pub const fn axes(&self) -> &[ReadingAxis; 2] {
        &self.axes
    }
    /// Every result coordinate's physical meaning and exact reference.
    #[must_use]
    pub fn entries(&self) -> &[EntryMeaning] {
        &self.entries
    }
    /// Exact definedness witness; no floating execution or reassociation is certified.
    #[must_use]
    pub const fn witness(&self) -> &VerifiedWitness {
        &self.witness
    }
}

/// Complete operation construction keeps order in native shaped CAS algebra.
fn build(
    cas: &mut Cas,
    operator: &LinearOperator,
    input: &ReadingShape,
    budget: &mut Budget,
) -> Result<(Handle, [ReadingAxis; 2], Vec<EntryMeaning>), ReadingOperationError> {
    if operator.context() != input.reading().family().context() {
        return Err(ReadingError::Unrelated.into());
    }
    if operator.axes()[1] != input.axes()[0] {
        return Err(ReadingOperationError::Axis);
    }
    let [inner, columns] = support::shape(input.reading().sort())?;
    let rows = u32::try_from(operator.codomain().len()).map_err(|_| ReadingError::Shape)?;
    let mut entries = support::reserve(
        usize::try_from(u64::from(rows) * u64::from(columns)).map_err(|_| ReadingError::Shape)?,
        budget,
    )?;
    for column in 0..columns {
        for (row, expected) in operator.domain().iter().enumerate() {
            budget.work(1, support::AT)?;
            let position = usize::try_from(column).map_err(|_| ReadingError::Shape)?
                * usize::try_from(inner).map_err(|_| ReadingError::Shape)?
                + row;
            let actual = input
                .reading()
                .entries()
                .get(position)
                .ok_or(ReadingOperationError::Input)?;
            expected
                .check(actual)
                .map_err(|_| ReadingOperationError::Input)?;
        }
        entries.extend_from_slice(operator.codomain());
    }
    let axes = [operator.axes()[0].clone(), input.axes()[1].clone()];
    let sort = Sort::Value(AtomicSort::Matrix {
        field: Field::Real,
        rows: Extent::Literal(rows),
        columns: Extent::Literal(columns),
    });
    let node = support::expression(
        cas,
        operator.context(),
        Operator::MatrixMultiply,
        sort,
        &[operator.node().clone(), input.reading().node().clone()],
        budget,
    )?;
    Ok((node, axes, entries))
}

/// A native result is defined by syntax, so the scoped equality names that exact expression twice.
fn expected(cas: &Cas, context: &Handle, node: &Handle) -> Result<Judgment, ReadingOperationError> {
    Ok(Judgment {
        context: context.clone(),
        layer: Layer::Pointwise,
        space: None,
        statement: Statement::Equality {
            left: node.clone(),
            right: node.clone(),
            domain: cas.admissible_region(context)?,
            domain_contract: DomainContract::DefinedAndEqual,
        },
        residuals: Vec::new(),
    })
}
