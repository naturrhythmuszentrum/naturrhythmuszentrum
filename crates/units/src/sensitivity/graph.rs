// SPDX-License-Identifier: AGPL-3.0-only
//! Budgeted public-facade graph construction; no numerical coordinates enter this store.

use std::cell::RefMut;

use nrz_cas::domain::{AssumptionDraft, ContextDraft};
use nrz_cas::limits::{Budget, BudgetKind, LimitsOverride};
use nrz_cas::proof::{DomainContract, VerifiedWitness};
use nrz_cas::request::{CoveredInput, Request, Task};
use nrz_cas::syntax::{AtomicSort, Draft, ExpressionDraft, Field, Operator, PredicateKind, Sort};
use nrz_cas::{Cas, Handle, Location, Rational, Reference};

use super::{SensitivityError, SensitivityWork};
use crate::{CheckedExpression, Context};

/// A private construction lease with one bounded graph inspection ledger.
pub(crate) struct Graph<'a> {
    /// Actual owning CAS, borrowed fallibly from the units builder.
    pub(crate) cas: RefMut<'a, Cas>,
    /// Semantic header shared by originals and generated coefficient recipes.
    pub(crate) header: Handle,
    /// Explicit mathematical premise context for this recipe.
    pub(crate) context: Handle,
    /// Exact requested region, retained and checked in each replayed statement.
    pub(crate) domain: Handle,
    /// Bounds construction and inspection across the complete recipe.
    pub(crate) budget: Budget,
    /// Maximum observed service work and total completed request count.
    pub(crate) work: SensitivityWork,
    /// Each CAS request retains the caller's explicit service limits.
    limits: LimitsOverride,
}

impl<'a> Graph<'a> {
    /// Open only private mutable access; the final checked object owns the frozen store.
    pub(crate) fn new(
        builder: &'a Context,
        limits: &LimitsOverride,
    ) -> Result<Self, SensitivityError> {
        let cas = builder.engine()?;
        Ok(Self {
            header: cas.context_header(&builder.semantic)?,
            domain: cas.admissible_region(&builder.semantic)?,
            context: builder.semantic.clone(),
            cas,
            budget: Budget::new(builder.cas_limits.resolve(limits)?)?,
            limits: limits.clone(),
            work: SensitivityWork::default(),
        })
    }

    /// Refuse excessive schedules before allocating cells or iterating their product.
    pub(crate) fn extent(&self, rows: usize, columns: usize) -> Result<usize, SensitivityError> {
        let count = rows
            .checked_mul(columns)
            .ok_or(SensitivityError::ResourceLimit)?;
        let amount = u64::try_from(count).map_err(|_| SensitivityError::ResourceLimit)?;
        self.budget.check(
            BudgetKind::MatrixEntries,
            amount,
            Location::new("units sensitivity"),
        )?;
        if rows == 0 || columns == 0 {
            return Err(SensitivityError::Coordinates);
        }
        Ok(count)
    }

    /// Construct one admitted scalar graph node, preserving operand order.
    pub(crate) fn term(
        &mut self,
        operator: Operator,
        operands: &[Handle],
    ) -> Result<Handle, SensitivityError> {
        let draft = Draft::Expression(ExpressionDraft {
            space: None,
            operator,
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            header: Reference::Existing(self.header.clone()),
            scope: Vec::new(),
            binding: None,
            operands: operands.iter().cloned().map(Reference::Existing).collect(),
        });
        self.admit(draft)
    }

    /// Keep all graph admission on the same finite construction ledger.
    pub(crate) fn admit(&mut self, draft: Draft) -> Result<Handle, SensitivityError> {
        self.cas
            .admit(&[draft], &mut self.budget)?
            .into_iter()
            .next()
            .ok_or(SensitivityError::WitnessMismatch)
    }

    /// Admit a symbolic or descriptor constant, never a stored numerical value.
    pub(crate) fn constant(&mut self, value: Rational) -> Result<Handle, SensitivityError> {
        self.term(Operator::Rational(value), &[])
    }

    /// Add explicit nonzero premises whose numerical discharge remains in the schedule.
    pub(crate) fn nonzero_context(
        &mut self,
        terms: &[(usize, Handle)],
    ) -> Result<(), SensitivityError> {
        let mut assumptions = Vec::new();
        for (coordinate, term) in terms {
            let predicate = self.admit(Draft::Condition {
                kind: PredicateKind::NonZero,
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                arguments: vec![Reference::Existing(term.clone())],
            })?;
            assumptions.push(AssumptionDraft {
                key: format!("units:sensitivity:nonzero:{coordinate}"),
                condition: Reference::Existing(predicate),
            });
        }
        self.context = self.admit(Draft::Context(ContextDraft::Root {
            header: Reference::Existing(self.header.clone()),
            admissible: Reference::Existing(self.domain.clone()),
            assumptions,
        }))?;
        Ok(())
    }

    /// Cover a generated coefficient by a complete walk with original provenance.
    pub(crate) fn cover(
        &mut self,
        expression: &Handle,
        provenance: &Handle,
    ) -> Result<CoveredInput, SensitivityError> {
        let root = self
            .cas
            .materialize_derived(expression, provenance, &mut self.budget)?;
        let receipt = self
            .cas
            .walk_derived(&root, &mut self.budget)
            .map_err(|error| error.fault)?
            .receipt;
        Ok(CoveredInput { root, receipt })
    }

    /// Request the public row-ordered assembly on this context's explicit domain.
    pub(crate) fn jacobian(
        &mut self,
        originals: &[CheckedExpression],
        coordinates: &[String],
    ) -> Result<VerifiedWitness, SensitivityError> {
        self.extent(originals.len(), coordinates.len())?;
        let mut inputs = Vec::new();
        for original in originals {
            inputs.push(self.cover(&original.coordinates, &original.original().syntax)?);
        }
        self.request(Task::Jacobian(coordinates.to_vec()), inputs)
    }

    /// Prove equality to a fixed native coefficient; callers cannot supply this formula.
    pub(crate) fn equal(
        &mut self,
        left: &Handle,
        right: &Handle,
    ) -> Result<VerifiedWitness, SensitivityError> {
        let inputs = vec![self.cover(left, left)?, self.cover(right, right)?];
        let witness = self.request(
            Task::Equal {
                domain: self.domain.clone(),
                domain_contract: DomainContract::DefinedAndEqual,
            },
            inputs,
        )?;
        super::replay::equality(&witness, left, right, &self.context, &self.domain)?;
        Ok(witness)
    }

    /// Only complete, discharged, freshly replayed service results cross this boundary.
    fn request(
        &mut self,
        task: Task,
        inputs: Vec<CoveredInput>,
    ) -> Result<VerifiedWitness, SensitivityError> {
        let request = Request {
            task,
            context: self.context.clone(),
            inputs,
            premises: Vec::new(),
            limits: self.limits.clone(),
        };
        self.work.construction = self.budget.usage().work;
        let outcome = self.cas.run(&request);
        if let nrz_cas::request::Outcome::Proved { result, usage, .. } = &outcome {
            self.work.generation = self.work.generation.max(usage.work);
            self.work.replay = self.work.replay.max(result.replay_usage.work);
            self.work.requests = self
                .work
                .requests
                .checked_add(1)
                .ok_or(SensitivityError::ResourceLimit)?;
        }
        super::replay::completed(outcome)
    }
}
