// SPDX-License-Identifier: AGPL-3.0-only
//! Syntax construction retains original operands and the sole registered incidence.

use super::invalid;
use crate::budget::Budget;
use crate::domain_model::{CaseArmDraft, CasesDraft};
use crate::error::Fault;
use crate::graph::{Cas, Draft, ExpressionDraft, Handle, Reference};
use crate::model::{AtomicSort, Field, Operator, PredicateKind, Scope, Sort};
use crate::solving::radicals::construction::admit;
use crate::solving::{
    FamilyExpressionData, PlacementUse, ScalarPieceData, SolveDraft, SpacePlacementData,
};

impl Cas {
    /// Construct the native partial view of a local family operation.
    ///
    /// The original operands keep their declarations; the result has one association
    /// with every required Real-in-Complex placement, and no default association.
    ///
    /// # Errors
    /// Refuses an invalid family, operand role, region, operator or placement, or an exhausted budget.
    pub fn family_expression(
        &mut self,
        value: &FamilyExpressionData,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        self.transaction(budget, |cas, budget| build(cas, value, budget))
    }
}

/// Admit the body first so the association inventory comes from checked ordered dependencies.
fn build(
    cas: &mut Cas,
    value: &FamilyExpressionData,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let body = admit(
        cas,
        &Draft::Solve(SolveDraft::FamilyExpression(value.clone())),
        budget,
    )?;
    let resolved = super::body(cas, body.logical_id())?
        .ok_or_else(invalid)?
        .clone();
    let region = cas.state.region(resolved.region.logical_id())?;
    let header = cas.handle(region.header);
    let scope = region.scope.clone();
    let operands = crate::solving::sorts::placement_operands(&cas.state, &resolved, budget)?;
    let mut placements = crate::solving::coefficients::storage::vector(operands.len(), budget)?;
    for (operand, ambient) in operands {
        let source = if let Some(local) = super::body(cas, operand)? {
            local.ambient.clone()
        } else {
            cas.scalar_ambient(&cas.handle(operand))?
                .ok_or_else(invalid)?
        };
        let placement = admit(
            cas,
            &Draft::Solve(SolveDraft::SpacePlacement(SpacePlacementData {
                source: Reference::Existing(source),
                ambient: Reference::Existing(cas.handle(ambient)),
                specification: "placement.real-in-complex".into(),
                version: 1,
            })),
            budget,
        )?;
        let frame = incidence(cas, &header, &scope, budget)?;
        let piece = admit(
            cas,
            &Draft::Solve(SolveDraft::ScalarPiece(ScalarPieceData {
                placement: Reference::Existing(placement),
                piece: 0,
                region: Reference::Existing(resolved.region.clone()),
                local_frame: Reference::Existing(frame),
            })),
            budget,
        )?;
        placements.push(PlacementUse {
            operand: Reference::Existing(cas.handle(operand)),
            piece: Reference::Existing(piece),
        });
    }
    let arm = admit(
        cas,
        &Draft::CaseArm(CaseArmDraft {
            placements,
            header: Reference::Existing(header.clone()),
            scope: scope.clone(),
            region: Reference::Existing(resolved.region.clone()),
            value: Reference::Existing(body),
        }),
        budget,
    )?;
    admit(
        cas,
        &Draft::Cases(CasesDraft {
            sort: resolved.result,
            header: Reference::Existing(header),
            scope,
            domain: Reference::Existing(resolved.region),
            arms: vec![Reference::Existing(arm)],
        }),
        budget,
    )
}

/// Bind one query location and one local Real value without adding global declarations.
fn incidence(
    cas: &mut Cas,
    header: &Handle,
    outer: &Scope,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let mut scope = outer.clone();
    scope.push(vec![
        AtomicSort::Scalar(Field::Complex),
        AtomicSort::Scalar(Field::Real),
    ]);
    let query = expression(
        cas,
        header,
        &scope,
        Operator::Bound { depth: 0, slot: 0 },
        Field::Complex,
        &[],
        budget,
    )?;
    let local = expression(
        cas,
        header,
        &scope,
        Operator::Bound { depth: 0, slot: 1 },
        Field::Real,
        &[],
        budget,
    )?;
    let real = expression(
        cas,
        header,
        &scope,
        Operator::RealPart,
        Field::Real,
        std::slice::from_ref(&query),
        budget,
    )?;
    let imaginary = expression(
        cas,
        header,
        &scope,
        Operator::ImagPart,
        Field::Real,
        std::slice::from_ref(&query),
        budget,
    )?;
    let zero = expression(
        cas,
        header,
        &scope,
        Operator::Rational(crate::number::Rational::zero()),
        Field::Real,
        &[],
        budget,
    )?;
    let first = predicate(
        cas,
        header,
        &scope,
        PredicateKind::Equal,
        &[imaginary, zero],
        budget,
    )?;
    let second = predicate(
        cas,
        header,
        &scope,
        PredicateKind::Equal,
        &[real, local],
        budget,
    )?;
    predicate(
        cas,
        header,
        &scope,
        PredicateKind::And,
        &[first, second],
        budget,
    )
}

/// Ordinary scalar syntax stays exactly typed within the local incidence frame.
pub(super) fn expression(
    cas: &mut Cas,
    header: &Handle,
    scope: &Scope,
    operator: Operator,
    field: Field,
    operands: &[Handle],
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    admit(
        cas,
        &Draft::Expression(ExpressionDraft {
            header: Reference::Existing(header.clone()),
            scope: scope.clone(),
            operator,
            sort: Sort::Value(AtomicSort::Scalar(field)),
            space: None,
            binding: None,
            operands: operands.iter().cloned().map(Reference::Existing).collect(),
        }),
        budget,
    )
}

/// Incidence predicates are ordinary checked conditions, without a new certificate kind.
fn predicate(
    cas: &mut Cas,
    header: &Handle,
    scope: &Scope,
    kind: PredicateKind,
    operands: &[Handle],
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    admit(
        cas,
        &Draft::Expression(ExpressionDraft {
            header: Reference::Existing(header.clone()),
            scope: scope.clone(),
            operator: Operator::Predicate(kind),
            sort: Sort::Value(AtomicSort::Boolean),
            space: None,
            binding: None,
            operands: operands.iter().cloned().map(Reference::Existing).collect(),
        }),
        budget,
    )
}
