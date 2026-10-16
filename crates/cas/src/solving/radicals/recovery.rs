// SPDX-License-Identifier: AGPL-3.0-only
//! Reversible elimination of the discovered character transform, with no fixed-degree inverse.

use super::construction::{Construction, admit};
use crate::budget::Budget;
use crate::graph::{Cas, Draft, ExpressionDraft, Handle, Reference};
use crate::model::{AtomicSort, Extent, Field, Operator, Sort};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::model::{CoefficientFailure, Element, EvaluationScope};
use crate::solving::coefficients::storage::{invalid, reserve, vector};
use crate::solving::{
    CertificateExactLinearEquivalence, RowOperation, SolveCertificate, SolveSchema, SolveStatement,
    StatementRank,
};

/// Exact row-major coefficients and the reversible operations that produced them.
struct Elimination {
    /// Matrix width, shared by rows and columns.
    width: usize,
    /// Mutable echelon proposal, independently reconstructed during replay.
    entries: Vec<Element>,
    /// Every elementary row addition, in execution order.
    operations: Vec<RowOperation<Handle>>,
}

/// The coefficient algebra fixes the meaning of every scalar matrix entry.
fn matrix(
    cas: &mut Cas,
    construction: &Construction,
    entries: &[Element],
    map: &Handle,
    width: u32,
    budget: &mut Budget,
) -> Result<Handle, CoefficientFailure> {
    let mut operands = vector(entries.len(), budget)?;
    for entry in entries {
        operands.push(Reference::Existing(
            construction.literal(cas, entry, map, budget)?,
        ));
    }
    Ok(admit(
        cas,
        &Draft::Expression(ExpressionDraft {
            header: Reference::Existing(construction.header.clone()),
            scope: Vec::new(),
            operator: Operator::Matrix {
                rows: width,
                columns: width,
            },
            sort: Sort::Value(AtomicSort::Matrix {
                field: Field::Complex,
                rows: Extent::Literal(width),
                columns: Extent::Literal(width),
            }),
            operands,
            space: None,
            binding: None,
        }),
        budget,
    )?)
}

impl Elimination {
    /// Generate the transform directly from its primitive character and degree.
    fn new(
        cas: &Cas,
        unity: &Element,
        width: u32,
        budget: &mut Budget,
    ) -> Result<Self, CoefficientFailure> {
        let engine = Engine {
            state: &cas.state,
            domain: &unity.domain,
        };
        let count = usize::try_from(width.checked_mul(width).ok_or_else(invalid)?)
            .map_err(|_| invalid())?;
        let mut entries = vector(count, budget)?;
        for row in 0..width {
            for column in 0..width {
                entries.push(engine.pow(
                    unity,
                    row.checked_mul(column).ok_or_else(invalid)? % width,
                    budget,
                )?);
            }
        }
        Ok(Self {
            width: usize::try_from(width).map_err(|_| invalid())?,
            entries,
            operations: Vec::new(),
        })
    }

    /// Row addition is reversible without a pivot-scaling hypothesis.
    fn eliminate(
        &mut self,
        cas: &mut Cas,
        construction: &mut Construction,
        map: &Handle,
        scope: &EvaluationScope<'_>,
        budget: &mut Budget,
    ) -> Result<Element, CoefficientFailure> {
        let engine = Engine {
            state: &cas.state,
            domain: &self.entries.first().ok_or_else(invalid)?.domain,
        };
        let mut determinant = engine.constant(true, budget)?;
        for column in 0..self.width {
            let pivot = self
                .entries
                .get(column * self.width + column)
                .ok_or_else(invalid)?;
            let engine = Engine {
                state: &cas.state,
                domain: &pivot.domain,
            };
            determinant = engine.mul(&determinant, pivot, budget)?;
            let inverse = super::roots::inverse(&engine, pivot, scope, budget)?;
            unit(cas, construction, pivot, &inverse, map, budget)?;
            for row in column + 1..self.width {
                self.subtract(cas, construction, map, (row, column), &inverse, budget)?;
            }
        }
        Ok(determinant)
    }

    /// Prepay each stored operation and apply it to the entire row, including earlier columns.
    fn subtract(
        &mut self,
        cas: &mut Cas,
        construction: &Construction,
        map: &Handle,
        position: (usize, usize),
        inverse: &Element,
        budget: &mut Budget,
    ) -> Result<(), CoefficientFailure> {
        let (row, pivot) = position;
        let engine = Engine {
            state: &cas.state,
            domain: &inverse.domain,
        };
        let factor = engine.neg(
            &engine.mul(&self.entries[row * self.width + pivot], inverse, budget)?,
            budget,
        )?;
        for column in 0..self.width {
            let multiple =
                engine.mul(&factor, &self.entries[pivot * self.width + column], budget)?;
            let index = row * self.width + column;
            self.entries[index] = engine.add(&self.entries[index], &multiple, budget)?;
        }
        let value = construction.literal(cas, &factor, map, budget)?;
        reserve(&mut self.operations, 1, budget)?;
        self.operations.push(RowOperation::AddMultiple {
            destination: u32::try_from(row).map_err(|_| invalid())?,
            source: u32::try_from(pivot).map_err(|_| invalid())?,
            value,
        });
        Ok(())
    }
}

/// A checked scalar inverse-product identity supplies precisely the pivot's nonvanishing evidence.
fn unit(
    cas: &mut Cas,
    construction: &mut Construction,
    pivot: &Element,
    inverse: &Element,
    map: &Handle,
    budget: &mut Budget,
) -> Result<(), CoefficientFailure> {
    let engine = Engine {
        state: &cas.state,
        domain: &pivot.domain,
    };
    let one = engine.constant(true, budget)?;
    let pivot = construction.literal(cas, pivot, map, budget)?;
    let inverse = construction.literal(cas, inverse, map, budget)?;
    let left = construction.expression(cas, Operator::Mul, &[pivot, inverse], false, budget)?;
    let right = construction.literal(cas, &one, map, budget)?;
    super::proof::identity(construction, [&left, &right], map, budget)?;
    construction
        .steps
        .last_mut()
        .ok_or_else(invalid)?
        .conclusion = crate::proof_model::Statement::Equality {
        left,
        right,
        domain: construction.region.clone(),
        domain_contract: crate::proof_model::DomainContract::DefinedAndEqual,
    };
    Ok(())
}

/// Independently replay every row operation and both rank bounds of the actual character matrix.
pub(crate) fn certify(
    cas: &mut Cas,
    construction: &mut Construction,
    unity: &Element,
    map: &Handle,
    width: u32,
    scope: &EvaluationScope<'_>,
    budget: &mut Budget,
) -> Result<(), CoefficientFailure> {
    let mut elimination = Elimination::new(cas, unity, width, budget)?;
    let original = matrix(cas, construction, &elimination.entries, map, width, budget)?;
    let determinant = elimination.eliminate(cas, construction, map, scope, budget)?;
    let minor = construction.literal(cas, &determinant, map, budget)?;
    let reduced = matrix(cas, construction, &elimination.entries, map, width, budget)?;
    let mut pivots = vector(elimination.width, budget)?;
    pivots.extend(0..width);
    let certificate_pivots = crate::solving::coefficients::storage::copied(&pivots, budget)?;
    construction.push(
        SolveSchema::ExactLinearEquivalence,
        SolveStatement::Rank(StatementRank {
            matrix: original.clone(),
            domain: unity.domain.clone(),
            rank: width,
            pivots,
            minor: minor.clone(),
            elimination: reduced.clone(),
        }),
        SolveCertificate::ExactLinearEquivalence(CertificateExactLinearEquivalence {
            matrix: original,
            augmented_columns: 0,
            operations: elimination.operations,
            pivots: certificate_pivots,
            minor,
            reduced,
            region: construction.region.clone(),
        }),
        budget,
    )?;
    Ok(())
}
