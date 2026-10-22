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

/// One immutable expression/provenance pair and its already completed occurrence coverage.
struct CoveredRecord {
    /// Exact graph expression identity in this graph owner.
    expression: Handle,
    /// Exact original provenance used when constructing the derived occurrence root.
    provenance: Handle,
    /// Sealed receipt for that root; requests still validate it independently.
    input: CoveredInput,
}

/// A private construction lease with one bounded graph inspection ledger.
pub(crate) struct Graph<'a> {
    /// Actual owning CAS, borrowed fallibly from the bindings builder.
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
    /// Completed coverage shared only within this immutable graph owner and construction phase.
    covered: Vec<CoveredRecord>,
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
            covered: Vec::new(),
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
            Location::new("bindings sensitivity"),
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
                key: format!("bindings:sensitivity:nonzero:{coordinate}"),
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

    /// Materialize every use of a fixed native defining relation as original syntax.
    pub(crate) fn original(
        &mut self,
        syntax: Handle,
        key: &str,
    ) -> Result<nrz_cas::source::OriginalForm, SensitivityError> {
        let source = self.admit(Draft::Source {
            key: key.into(),
            text: key.into(),
        })?;
        let root = self
            .cas
            .materialize_original(&syntax, &source, &mut self.budget)?;
        Ok(nrz_cas::source::OriginalForm { syntax, root })
    }

    /// Cover a generated coefficient by a complete walk with original provenance.
    pub(crate) fn cover(
        &mut self,
        expression: &Handle,
        provenance: &Handle,
    ) -> Result<CoveredInput, SensitivityError> {
        for record in &self.covered {
            self.budget
                .work(1, Location::new("sensitivity coverage lookup"))?;
            if &record.expression == expression && &record.provenance == provenance {
                return Ok(record.input.clone());
            }
        }
        let root = self
            .cas
            .materialize_derived(expression, provenance, &mut self.budget)?;
        let receipt = self
            .cas
            .walk_derived(&root, &mut self.budget)
            .map_err(|error| error.fault)?
            .receipt;
        let input = CoveredInput { root, receipt };
        self.covered.push(CoveredRecord {
            expression: expression.clone(),
            provenance: provenance.clone(),
            input: input.clone(),
        });
        Ok(input)
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

    /// Request one scalar partial with an explicit coordinate, order, layer and domain.
    pub(crate) fn differentiate(
        &mut self,
        original: &CheckedExpression,
        coordinate: &str,
    ) -> Result<VerifiedWitness, SensitivityError> {
        let input = self.cover(&original.coordinates, &original.original().syntax)?;
        self.request(
            Task::Differentiate {
                coordinate: coordinate.into(),
                order: 1,
                layer: nrz_cas::domain::Layer::Pointwise,
                domain: self.domain.clone(),
            },
            vec![input],
        )
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

#[cfg(test)]
mod tests {
    use nrz_cas::limits::{Limits, LimitsOverride};
    use nrz_cas::request::Outcome;
    use nrz_cas::syntax::{AtomicSort, Declaration as Mathematical, Environment, Field, Sort};

    use super::Graph;
    use crate::{
        Context, Declaration, Descriptor, Evidence, ExponentPolicy, Registry, SensitivityError,
    };

    /// A real public-facade context for testing the private replay admission boundary.
    fn fixture() -> Result<Context, Box<dyn std::error::Error>> {
        let mut builder = Context::new(
            Registry::new(ExponentPolicy::Rational),
            Limits::default(),
            Environment {
                declarations: vec![Mathematical {
                    key: "x".into(),
                    display_name: "x".into(),
                    sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
                }],
                ..Environment::default()
            },
        )?;
        builder.declare(
            "x".into(),
            Declaration::Input {
                descriptor: Descriptor::ratio(),
                nonzero: false,
                evidence: Evidence::default(),
            },
        )?;
        Ok(builder)
    }

    /// Unsupported named-root derivative grammar cannot create an explicit derivative witness.
    /// ´test:unit:sensitivity-unsupported-cas-evidence-refusal´
    #[test]
    fn sensitivity_unsupported_cas_evidence_refusal() -> Result<(), Box<dyn std::error::Error>> {
        let builder = fixture()?;
        let original = builder.parse("unsupported derivative", "sqrt(x)")?;
        let checked = builder.check(&original)?;
        let mut graph = Graph::new(&builder, &LimitsOverride::default())?;
        let error = graph
            .jacobian(&[checked], &["x".into()])
            .err()
            .ok_or("unsupported CAS derivative accepted")?;
        let SensitivityError::Evidence(outcome) = error else {
            return Err("CAS outcome not retained".into());
        };
        assert!(matches!(
            *outcome,
            Outcome::Unresolved { .. } | Outcome::Unsupported { .. }
        ));
        Ok(())
    }

    /// Exhaustion cannot promote a partial or best-so-far witness into a closed schedule.
    /// ´test:unit:sensitivity-unfinished-cas-evidence-refusal´
    #[test]
    fn sensitivity_unfinished_cas_evidence_refusal() -> Result<(), Box<dyn std::error::Error>> {
        let builder = fixture()?;
        let original = builder.parse("limited derivative", "x^2")?;
        let checked = builder.check(&original)?;
        let mut graph = Graph::new(&builder, &LimitsOverride::default())?;
        graph.limits = LimitsOverride {
            work: Some(1),
            ..LimitsOverride::default()
        };
        let error = graph
            .jacobian(&[checked], &["x".into()])
            .err()
            .ok_or("unfinished CAS derivative accepted")?;
        let SensitivityError::Evidence(outcome) = error else {
            return Err("CAS outcome not retained".into());
        };
        assert!(matches!(*outcome, Outcome::Exhausted { .. }));
        Ok(())
    }
    /// Exact expression/provenance coverage is shared, while every cache comparison remains bounded.
    /// ´test:unit:sensitivity-coverage-sharing-preserves-provenance-and-work-limits´
    #[test]
    fn sensitivity_coverage_sharing_preserves_provenance_and_work_limits()
    -> Result<(), Box<dyn std::error::Error>> {
        let builder = fixture()?;
        let original = builder.parse("first source", "x^2")?;
        let checked = builder.check(&original)?;
        let other = builder.parse("second source", "x + 1")?;
        let mut graph = Graph::new(&builder, &LimitsOverride::default())?;
        let first = graph.cover(&checked.coordinates, &original.syntax)?;
        let previous = graph.budget.usage().work;
        let repeated = graph.cover(&checked.coordinates, &original.syntax)?;
        assert_eq!(repeated.root, first.root);
        assert_eq!(graph.budget.usage().work, previous + 1);
        let separate = graph.cover(&checked.coordinates, &other.syntax)?;
        assert_ne!(separate.root, first.root);
        for work in [0, 1, 2] {
            graph.budget = nrz_cas::limits::Budget::new(Limits {
                work,
                ..Limits::default()
            })?;
            assert_eq!(
                graph.cover(&checked.coordinates, &original.syntax).is_ok(),
                work >= 1
            );
        }
        Ok(())
    }
}
