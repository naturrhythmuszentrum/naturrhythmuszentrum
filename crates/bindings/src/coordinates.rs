// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded coordinate drafts for one atomic CAS admission after physical checking.

use nrz_cas::limits::{Budget, BudgetKind};
use nrz_cas::syntax::{AtomicSort, Draft, ExpressionDraft, Field, Operator, Sort};
use nrz_cas::{Cas, Fault, Handle, Location, Reference};

use crate::original::unsupported;

/// A CAS reference with the two shape counters needed before batch admission.
#[derive(Clone)]
pub(crate) struct Coordinate {
    /// Existing syntax or an earlier draft in the same batch.
    pub(super) reference: Reference,
    /// Maximum expression depth, excluding header dependencies.
    depth: u64,
    /// Expression size with repeated operand occurrences counted separately.
    size: u64,
}

impl Coordinate {
    /// Seed a literal or free symbol whose original syntax has already been inspected.
    pub(super) const fn leaf(reference: Reference) -> Self {
        Self {
            reference,
            depth: 1,
            size: 1,
        }
    }
}

/// Child-before-parent drafts; the CAS remains the owner and validator of expression syntax.
#[derive(Default)]
pub(crate) struct Coordinates {
    /// One transaction's public CAS drafts, without another expression representation.
    drafts: Vec<Draft>,
}

impl Coordinates {
    /// Check local shape limits before retaining each draft at its original source location.
    pub(super) fn push(
        &mut self,
        header: &Handle,
        operator: Operator,
        operands: &[Coordinate],
        budget: &Budget,
    ) -> Result<Coordinate, Fault> {
        let at = Location::new("bindings coordinate draft");
        let index = u32::try_from(self.drafts.len())
            .map_err(|_| Fault::capacity(BudgetKind::StagedVertices, at))?;
        budget.check(BudgetKind::StagedVertices, u64::from(index) + 1, at)?;
        let (depth, size) = shape(operands, budget)?;
        self.drafts
            .try_reserve(1)
            .map_err(|_| Fault::allocation(at))?;
        self.drafts.push(Draft::Expression(ExpressionDraft {
            space: None,
            operator,
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            binding: None,
            operands: operands.iter().map(|item| item.reference.clone()).collect(),
        }));
        Ok(Coordinate {
            reference: Reference::Local(index),
            depth,
            size,
        })
    }

    /// Admit all drafts once and resolve the checked result's transaction-local reference.
    pub(super) fn publish(
        &self,
        cas: &mut Cas,
        coordinate: &Coordinate,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let handles = if self.drafts.is_empty() {
            Vec::new()
        } else {
            cas.admit(&self.drafts, budget)?
        };
        match &coordinate.reference {
            Reference::Existing(handle) => Ok(handle.clone()),
            Reference::Local(index) => usize::try_from(*index)
                .ok()
                .and_then(|index| handles.get(index).cloned())
                .ok_or_else(unsupported),
        }
    }
}

/// Mirror CAS expression shape accounting while operands still have local references.
fn shape(operands: &[Coordinate], budget: &Budget) -> Result<(u64, u64), Fault> {
    let at = Location::new("bindings coordinate shape");
    let mut depth = 1;
    let mut size = 1_u64;
    for operand in operands {
        depth = depth.max(
            operand
                .depth
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::Depth, at))?,
        );
        size = size
            .checked_add(operand.size)
            .ok_or_else(|| Fault::capacity(BudgetKind::PeakExpressionSize, at))?;
    }
    budget.check(BudgetKind::Depth, depth, at)?;
    budget.check(BudgetKind::PeakExpressionSize, size, at)?;
    Ok((depth, size))
}
