// SPDX-License-Identifier: AGPL-3.0-only
//! Child-before-parent syntax with one independent predicate tree per original row.

use super::{AT, Result, accounting};
use crate::AdmissionError;
use nrz_cas::{
    Reference,
    domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr},
    limits::Budget,
    syntax::{
        AtomicSort, Branch, Declaration, Draft, Environment, ExpressionDraft, Field, Operator, Sort,
    },
};

/// One batch and the disjoint capacities retained while it is built.
pub(super) struct Builder {
    /// Every original occurrence keeps its own draft, even when CAS later interns it.
    pub(super) drafts: Vec<Draft>,
    /// Owned out-of-line payload already moved into drafts.
    heap: u64,
    /// Pool buffers and not-yet-moved rational payload.
    pub(super) external: u64,
}

/// Exact Real scalar sort for every numeric coordinate and operation.
pub(super) const fn real() -> Sort {
    Sort::Value(AtomicSort::Scalar(Field::Real))
}

impl Builder {
    /// Reserve the independently counted complete batch before emitting any record.
    pub(super) fn new(count: usize, external: u64, budget: &mut Budget) -> Result<Self> {
        u32::try_from(count).map_err(|_| AdmissionError::ResourceLimit)?;
        let drafts = budget
            .with_retained_payload(external, AT, |budget| accounting::allocate(count, budget))?;
        Ok(Self {
            drafts,
            heap: 0,
            external,
        })
    }

    /// Complete simultaneous draft/pool storage, with embedded vector headers counted once.
    pub(super) fn bytes(&self) -> Result<u64> {
        Ok(accounting::vector::<Draft>(self.drafts.capacity())? + self.heap + self.external)
    }

    /// Copy exactly one portable name, including ownership cleanup and its byte visits.
    pub(super) fn name(&self, name: &str, budget: &mut Budget) -> Result<String> {
        budget.with_retained_payload(self.bytes()?, AT, |budget| {
            accounting::edges(2, budget)?;
            accounting::expression(name.len() as u64, budget)?;
            budget.check(
                nrz_cas::limits::BudgetKind::StagedPayloadBytes,
                name.len() as u64,
                AT,
            )?;
            let mut copy = String::new();
            copy.try_reserve_exact(name.len())
                .map_err(|_| AdmissionError::ResourceLimit)?;
            copy.push_str(name);
            budget.check(
                nrz_cas::limits::BudgetKind::StagedPayloadBytes,
                copy.capacity() as u64,
                AT,
            )?;
            Ok(copy)
        })
    }

    /// Build the portable header and unrestricted ambient root context.
    pub(super) fn header(&mut self, names: &[&str], budget: &mut Budget) -> Result<()> {
        let mut declarations = budget.with_retained_payload(self.bytes()?, AT, |budget| {
            accounting::allocate(names.len(), budget)
        })?;
        self.heap += (declarations.capacity() * size_of::<Declaration>()) as u64;
        for name in names {
            let key = self.name(name, budget)?;
            self.heap += key.capacity() as u64;
            let display_name = self.name(name, budget)?;
            self.heap += display_name.capacity() as u64;
            declarations.push(Declaration {
                key,
                display_name,
                sort: real(),
            });
        }
        self.push(
            Draft::Header(HeaderDraft::Core(Environment {
                field: Field::Real,
                branch: Branch::StrictReal,
                declarations,
            })),
            budget,
        )?;
        self.push(
            Draft::Region(RegionDraft {
                header: Reference::Local(0),
                scope: Vec::new(),
                ambient: AmbientDomain::Assignments,
                expression: RegionExpr::Ambient,
            }),
            budget,
        )?;
        self.push(
            Draft::Context(ContextDraft::Root {
                header: Reference::Local(0),
                admissible: Reference::Local(1),
                assumptions: Vec::new(),
            }),
            budget,
        )?;
        Ok(())
    }

    /// Emit a complete expression; no adapter-side syntax lookup or equality test occurs.
    pub(super) fn node(
        &mut self,
        operator: Operator,
        operands: &[u32],
        boolean: bool,
        budget: &mut Budget,
    ) -> Result<u32> {
        let mut references = Vec::new();
        if !operands.is_empty() {
            references = budget.with_retained_payload(self.bytes()?, AT, |budget| {
                accounting::allocate(operands.len(), budget)
            })?;
            self.heap += (references.capacity() * size_of::<Reference>()) as u64;
            budget.with_retained_payload(self.bytes()?, AT, |budget| {
                accounting::expression(operands.len() as u64, budget)
            })?;
            references.extend(operands.iter().copied().map(Reference::Local));
        }
        match &operator {
            Operator::Free(name) => self.heap += name.capacity() as u64,
            Operator::Rational(value) => self.heap += accounting::rational(value),
            _ => {}
        }
        self.push(
            Draft::Expression(ExpressionDraft {
                space: None,
                operator,
                sort: if boolean {
                    Sort::Value(AtomicSort::Boolean)
                } else {
                    real()
                },
                header: Reference::Local(0),
                scope: Vec::new(),
                binding: None,
                operands: references,
            }),
            budget,
        )
    }

    /// One metadata event precedes each local record write.
    fn push(&mut self, draft: Draft, budget: &mut Budget) -> Result<u32> {
        let index = u32::try_from(self.drafts.len()).map_err(|_| AdmissionError::ResourceLimit)?;
        budget.with_retained_payload(self.bytes()?, AT, |budget| {
            accounting::expression(1, budget)
        })?;
        self.drafts.push(draft);
        Ok(index)
    }
}
