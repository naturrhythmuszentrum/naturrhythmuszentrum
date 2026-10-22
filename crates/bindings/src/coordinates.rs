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

    /// Import an exact closed constant through CAS drafts, preserving the shared check ledger.
    pub(super) fn constant(
        &mut self,
        header: &Handle,
        value: &nrz_cas::ExactConstant,
        budget: &mut Budget,
    ) -> Result<Coordinate, Fault> {
        if let Some(number) = value.as_rational() {
            return self.push(header, Operator::Rational(number.clone()), &[], budget);
        }
        let drafts = value.expression_drafts(Reference::Existing(header.clone()), budget)?;
        let mut coordinates: Vec<Coordinate> = Vec::new();
        coordinates
            .try_reserve_exact(drafts.len())
            .map_err(|_| Fault::allocation(Location::new("constant coordinate drafts")))?;
        for draft in drafts {
            let operands = draft
                .operands
                .iter()
                .map(|reference| {
                    let Reference::Local(index) = reference else {
                        return Err(unsupported());
                    };
                    usize::try_from(*index)
                        .ok()
                        .and_then(|index| coordinates.get(index))
                        .cloned()
                        .ok_or_else(unsupported)
                })
                .collect::<Result<Vec<_>, _>>()?;
            coordinates.push(self.push(header, draft.operator, &operands, budget)?);
        }
        coordinates.pop().ok_or_else(unsupported)
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

#[cfg(test)]
mod tests {
    use super::Coordinates;
    use nrz_cas::domain::HeaderDraft;
    use nrz_cas::limits::{Budget, BudgetKind, Limits};
    use nrz_cas::syntax::{Constant, Draft, Environment};
    use nrz_cas::{Cas, ExactConstant, Fault, FaultKind};

    fn import(value: &ExactConstant, work: u64) -> Result<u64, Fault> {
        let mut cas = Cas::new(Limits::default())?;
        let mut setup = Budget::new(Limits::default())?;
        let header = cas
            .admit(
                &[Draft::Header(HeaderDraft::Core(Environment::default()))],
                &mut setup,
            )?
            .pop()
            .ok_or_else(|| {
                Fault::invalid(
                    nrz_cas::InvalidCode::Reference,
                    nrz_cas::Location::new("coordinate test header"),
                )
            })?;
        let mut budget = Budget::new(Limits {
            work,
            ..Limits::default()
        })?;
        let mut coordinates = Coordinates::default();
        let root = coordinates.constant(&header, value, &mut budget)?;
        coordinates.publish(&mut cas, &root, &mut budget)?;
        Ok(budget.usage().work)
    }

    /// Exact graph import and atomic coordinate publication share the caller's N/N-1 ledger.
    /// ´test:unit:constant-coordinate-work-boundary´
    #[test]
    fn constant_coordinate_work_boundary() -> Result<(), Box<dyn std::error::Error>> {
        let value = ExactConstant::named(Constant::Pi, &mut Budget::new(Limits::default())?)?;
        let work = import(&value, Limits::default().work)?;
        assert_eq!(import(&value, work)?, work);
        assert!(matches!(
            import(&value, work.checked_sub(1).ok_or("empty work")?),
            Err(Fault {
                kind: FaultKind::Exhausted {
                    budget: BudgetKind::Work,
                    ..
                },
                ..
            })
        ));
        Ok(())
    }
}
