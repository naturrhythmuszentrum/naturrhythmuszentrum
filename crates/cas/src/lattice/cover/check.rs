// SPDX-License-Identifier: AGPL-3.0-only
//! Closed product inference checks original affine identities and scalar scope.

use super::{AT, LatticeCover, invalid};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, LogicalId, size};
use crate::model::{MatrixOperation, Operator};
use crate::number::Rational;
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, DomainContract, ScopedConditions, Statement,
};
use crate::rule::Primitive;

/// An exact literal is required for the homogeneous row and identity diagonal.
fn literal(cas: &Cas, id: LogicalId, one: bool, budget: &mut Budget) -> Result<(), Fault> {
    budget.work(1, AT)?;
    let value = cas.state.expression(id)?;
    let Operator::Rational(number) = &value.operator else {
        return Err(invalid());
    };
    if number
        != &if one {
            Rational::one()
        } else {
            Rational::zero()
        }
    {
        return Err(invalid());
    }
    Ok(())
}

/// Typed roles and original homogeneous entries are checked even for a formal statement.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the statement kernel and portable replay share these crate-private lattice hooks"
)]
pub(crate) fn roles(cas: &Cas, value: &LatticeCover, budget: &mut Budget) -> Result<u32, Fault> {
    let question = &value.index.basis;
    let home = cas.state.context(cas.checked(&question.context)?)?;
    let basis = cas.checked(&question.basis)?;
    let offset = cas.checked(&question.offset)?;
    let (rows, columns) = cas.lattice_rectangle(basis, home.header_id)?;
    let dimension = rows.checked_add(1).ok_or_else(invalid)?;
    if rows == 0
        || rows != columns
        || cas.lattice_rectangle(offset, home.header_id)? != (rows, 1)
        || size(value.index.families.len(), AT)? != u64::from(rows)
    {
        return Err(invalid());
    }
    budget.check(
        BudgetKind::MatrixEntries,
        u64::from(dimension).saturating_mul(u64::from(dimension)),
        AT,
    )?;
    for handle in [&value.forward, &value.inverse] {
        let id = cas.checked(handle)?;
        if cas.lattice_rectangle(id, home.header_id)? != (dimension, dimension) {
            return Err(invalid());
        }
        let entries = &cas.state.expression(id)?.operands;
        let width = usize::try_from(dimension).map_err(|_| invalid())?;
        for (column, entry) in entries.iter().skip(width * (width - 1)).enumerate() {
            literal(cas, *entry, column == width - 1, budget)?;
        }
    }
    let forward = &cas.state.expression(cas.checked(&value.forward)?)?.operands;
    let coefficients = &cas.state.expression(basis)?.operands;
    let offsets = &cas.state.expression(offset)?.operands;
    let width = usize::try_from(rows).map_err(|_| invalid())?;
    for row in 0..width {
        for column in 0..width {
            budget.work(1, AT)?;
            if forward[row * (width + 1) + column] != coefficients[row * width + column] {
                return Err(invalid());
            }
        }
        budget.work(1, AT)?;
        if forward[row * (width + 1) + width] != offsets[row] {
            return Err(invalid());
        }
    }
    for family in &value.index.families {
        crate::decision_cut::family::roles(cas, family)?;
    }
    cas.lattice_range_product(&value.index, budget)?;
    Ok(rows)
}

/// Every premise must prove a complete equality in the original common parameter frame.
fn equality<'a>(
    cas: &Cas,
    value: &LatticeCover,
    premise: &'a CheckedApplication,
    primitive: Primitive,
) -> Result<(&'a crate::graph::Handle, &'a crate::graph::Handle), Fault> {
    let Statement::Equality {
        left,
        right,
        domain,
        domain_contract: DomainContract::DefinedAndEqual,
    } = &premise.judgment.statement
    else {
        return Err(invalid());
    };
    let home = cas
        .state
        .context(cas.checked(&value.index.basis.context)?)?;
    if premise.rule != primitive
        || premise.judgment.context != value.index.basis.context
        || cas.checked(domain)? != home.admissible
        || !premise.conditions.residuals.is_empty()
    {
        return Err(invalid());
    }
    Ok((left, right))
}

/// Check the exact nonzero determinant quotient and both homogeneous multiplication orders.
fn affine(
    cas: &Cas,
    value: &LatticeCover,
    premises: &[CheckedApplication],
    dimension: u32,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let (left, right) = equality(cas, value, &premises[0], Primitive::RationalIdentity)?;
    literal(cas, cas.checked(right)?, true, budget)?;
    let quotient = cas.state.expression(cas.checked(left)?)?;
    let [numerator, denominator] = quotient.operands.as_slice() else {
        return Err(invalid());
    };
    if quotient.operator != Operator::Div || *denominator != cas.checked(&value.determinant)? {
        return Err(invalid());
    }
    let determinant = cas.state.expression(*numerator)?;
    if determinant.operator != Operator::MatrixMap(MatrixOperation::Determinant)
        || determinant.operands != [cas.checked(&value.forward)?]
    {
        return Err(invalid());
    }
    for (premise, outer, inner) in [
        (&premises[1], &value.forward, &value.inverse),
        (&premises[2], &value.inverse, &value.forward),
    ] {
        let (left, right) = equality(cas, value, premise, Primitive::Jets)?;
        let product = cas.state.expression(cas.checked(left)?)?;
        if product.operator != Operator::MatrixMultiply
            || product.operands != [cas.checked(outer)?, cas.checked(inner)?]
        {
            return Err(invalid());
        }
        let header = cas
            .state
            .context(cas.checked(&value.index.basis.context)?)?
            .header_id;
        let width = dimension.checked_add(1).ok_or_else(invalid)?;
        if cas.lattice_rectangle(cas.checked(right)?, header)? != (width, width) {
            return Err(invalid());
        }
        let width = usize::try_from(width).map_err(|_| invalid())?;
        for (position, entry) in cas
            .state
            .expression(cas.checked(right)?)?
            .operands
            .iter()
            .enumerate()
        {
            literal(cas, *entry, position / width == position % width, budget)?;
        }
    }
    Ok(())
}

/// Existence and uniqueness compose coordinate-wise after the exact affine bijection.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the statement kernel and portable replay share these crate-private lattice hooks"
)]
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    let Statement::LatticeCover(value) = &draft.conclusion else {
        return Err(invalid());
    };
    if draft.context != value.index.basis.context
        || draft.certificate != Certificate::None
        || !draft.substitutions.is_empty()
        || premises.len() != value.index.families.len().saturating_add(3)
    {
        return Err(invalid());
    }
    let bytes = crate::lattice::index::payload(&value.index, budget)?;
    budget.work(bytes, AT)?;
    budget.with_retained_payload(bytes, AT, |budget| {
        let dimension = roles(cas, value, budget)?;
        affine(cas, value, premises, dimension, budget)?;
        let home = cas.checked(&draft.context)?;
        cas.lattice_range_frame(home, budget)?;
        let mut axes = Vec::new();
        axes.try_reserve_exact(value.index.families.len().saturating_mul(2))
            .map_err(|_| Fault::allocation(AT))?;
        for (family, premise) in value.index.families.iter().zip(&premises[3..]) {
            cas.lattice_scalar_application(home, family, premise, budget)?;
            for handle in [&family.coordinate, &family.index] {
                let Operator::Free(name) = &cas.state.expression(cas.checked(handle)?)?.operator
                else {
                    return Err(invalid());
                };
                for existing in &axes {
                    budget.work(size(name.len(), AT)?.saturating_add(1), AT)?;
                    if *existing == name.as_str() {
                        return Err(invalid());
                    }
                }
                axes.push(name.as_str());
            }
        }
        cas.lattice_range_independence(&value.index, &axes, budget)?;
        let mut conditions = ScopedConditions::default();
        for premise in premises {
            crate::statement::merge(cas, &mut conditions, &premise.conditions, budget)?;
        }
        Ok(conditions)
    })
}

/// Charge all factor-owned rational payload before statement copies.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the statement kernel and portable replay share these crate-private lattice hooks"
)]
pub(crate) fn payload(value: &LatticeCover, budget: &mut Budget) -> Result<u64, Fault> {
    Ok(crate::lattice::index::payload(&value.index, budget)?.saturating_add(512))
}
