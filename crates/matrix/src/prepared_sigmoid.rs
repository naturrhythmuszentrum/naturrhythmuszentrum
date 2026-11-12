// SPDX-License-Identifier: AGPL-3.0-only
//! Original sigmoid authority paired with a checked negative-domain exponential basis.

use super::{PreparedExpm1, PreparedExpm1Error, finalize_owner, represented};
use nrz_bindings::{
    Budget,
    readings::algebra::{
        Cas, Fault, Handle, Location, Rational, Reference,
        enclosure::{
            CoordinateBound, EnclosureGoal, EnclosureRequest, Interval,
            prepared_expm1::{Counts, Policy, Prepared, Request},
        },
        limits::BudgetKind,
        syntax::{Draft, ExpressionDraft, Operator, Sort},
    },
};
use nrz_scalar::{
    ArithmeticContext, ExpProgram, SigmoidFailure, SigmoidInput, SigmoidOutput, SigmoidProgram,
};

/// Original-to-basis admission and standalone materialization location.
const AT: Location = Location::new("prepared sigmoid");

/// Revision-one auxiliary precision, separately checked against the final target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SigmoidPreparationPolicy {
    /// Original absolute endpoint target.
    pub bits: u32,
    /// Relative basis target, exactly min(bits,40)+4.
    pub requested_bits: u32,
    /// Native arithmetic and composition argument revision, exactly one.
    pub revision: u32,
}

/// Admission keeps correspondence, exact resources and native outcomes distinct.
#[derive(Debug, thiserror::Error)]
pub enum PreparedSigmoidError {
    /// Original expression is not the admitted sigmoid of one direct operand.
    #[error("sigmoid operation correspondence")]
    Operation,
    /// Original exact box differs from the represented original domain.
    #[error("sigmoid original domain correspondence")]
    Domain,
    /// The separately supplied final target differs from the policy target.
    #[error("sigmoid target correspondence")]
    Target,
    /// Auxiliary precision differs from the finite recorded policy.
    #[error("sigmoid auxiliary policy correspondence")]
    Policy,
    /// The arithmetic or composition revision is not admitted.
    #[error("sigmoid revision correspondence")]
    Revision,
    /// Exact resource or syntax admission failed, retaining its complete cause.
    #[error("sigmoid exact admission: {0}")]
    Exact(#[from] Fault),
    /// Checked numerical lowering failed with its original typed cause.
    #[error("sigmoid basis admission: {0}")]
    Admission(#[from] PreparedExpm1Error),
    /// No basis was certified within the finite search; no final impossibility claim.
    #[error("sigmoid basis unavailable under preparation policy")]
    BasisPreparation {
        /// Actual transformed request submitted to CAS.
        auxiliary: Box<Request>,
        /// Actual precision selection, distinct from final numerical evidence.
        policy: SigmoidPreparationPolicy,
        /// Actual finite CAS search capacities used for this attempt.
        search: Policy,
    },
    /// Native composition refused a represented operand or its final target.
    #[error("sigmoid native composition: {0}")]
    Native(#[from] SigmoidFailure),
}

/// Borrowed original request and frozen producing graph, without masked authority.
#[derive(Debug)]
pub struct PreparedSigmoid<'a> {
    /// Immutable original operation and exact domain retained beside the basis.
    original: &'a EnclosureRequest,
    /// Checked basis and immutable graph lifetime.
    basis: PreparedExpm1<'a>,
    /// Exact represented original domain.
    domain: [f64; 2],
    /// Bound target, finite auxiliary precision and native revision.
    policy: SigmoidPreparationPolicy,
    /// Complete logical payload, including borrowed original request inventory.
    bytes: u64,
}

/// Successful readings associated with the represented operand and checked basis.
#[derive(Clone, Copy, Debug)]
pub struct SigmoidEvaluation<'a> {
    /// Original represented operand and absolute target.
    input: SigmoidInput,
    /// Endpoint enclosure and independent nominal readings.
    output: SigmoidOutput,
    /// Borrowed checked exponential-minus-one authority.
    basis: &'a Prepared,
}

impl SigmoidEvaluation<'_> {
    /// Original represented operand association.
    #[must_use]
    pub const fn input(&self) -> SigmoidInput {
        self.input
    }
    /// Complete endpoint, centre, radius and numerical-error readings.
    #[must_use]
    pub const fn reading(&self) -> SigmoidOutput {
        self.output
    }
    /// Checked basis, not a separate sigmoid CAS certificate.
    #[must_use]
    pub const fn basis(&self) -> &Prepared {
        self.basis
    }
    /// Final absolute target, independent of auxiliary precision.
    #[must_use]
    pub const fn bits(&self) -> u32 {
        self.input.bits
    }
}

impl<'a> PreparedSigmoid<'a> {
    /// Bind the original sigmoid request before producing its auxiliary basis.
    ///
    /// The original request is borrowed for the owner's complete lifetime. A
    /// direct free coordinate is rebound to its negated box in the auxiliary
    /// request; a constant is negated exactly. This explicit substitution is
    /// justified by the native composition argument, not interval coincidence.
    /// # Errors
    /// Refuses operation, domain, target, policy and revision mismatches separately,
    /// preserving exact resources and unavailable-basis evidence.
    #[expect(
        clippy::too_many_arguments,
        reason = "original target and auxiliary policy are independently admitted"
    )]
    pub fn new(
        cas: &'a mut Cas,
        original: &'a EnclosureRequest,
        domain: [f64; 2],
        bits: u32,
        policy: SigmoidPreparationPolicy,
        search: Policy,
        context: &ArithmeticContext,
        budget: &mut Budget,
        counts: &mut Counts,
    ) -> Result<Self, PreparedSigmoidError> {
        let _span = tracing::trace_span!(
            "nrz.lower",
            family = "prepared-sigmoid-owner",
            revision = 1_u64,
            coordinates = original.coordinates.len(),
            ?domain,
            bits,
            ?policy,
            ?search,
            ?context,
            limits = ?budget.limits(),
            usage = ?budget.usage(),
            ?counts
        )
        .entered();
        let result = Self::prepare(
            cas, original, domain, bits, policy, search, context, budget, counts,
        );
        tracing::trace!(bytes = result.as_ref().ok().map(|owner| owner.bytes), error = ?result.as_ref().err());
        result
    }

    /// Complete fixed inventory admission precedes request traversal and graph construction.
    #[expect(
        clippy::too_many_arguments,
        reason = "private implementation preserves the complete public admission inputs"
    )]
    fn prepare(
        cas: &'a mut Cas,
        original: &'a EnclosureRequest,
        domain: [f64; 2],
        bits: u32,
        policy: SigmoidPreparationPolicy,
        search: Policy,
        context: &ArithmeticContext,
        budget: &mut Budget,
        counts: &mut Counts,
    ) -> Result<Self, PreparedSigmoidError> {
        if bits != policy.bits {
            return Err(PreparedSigmoidError::Target);
        }
        if policy.requested_bits != bits.min(40) + 4 {
            return Err(PreparedSigmoidError::Policy);
        }
        if policy.revision != 1 {
            return Err(PreparedSigmoidError::Revision);
        }
        if !domain.iter().all(|x| x.is_finite())
            || domain[0] < 0.0
            || domain[1] > 2.0
            || domain[0] > domain[1]
        {
            return Err(PreparedSigmoidError::Domain);
        }
        let count = original.coordinates.len() as u64;
        budget.check(BudgetKind::MatrixEntries, count, AT)?;
        if count > 1 {
            return Err(PreparedSigmoidError::Domain);
        }
        // The original inventory is borrowed, but still covered logically. All
        // admitted endpoint values have at most 1,075 component bits.
        let original_bytes = size_of::<EnclosureRequest>() as u64 + count * 1600;
        let extra = (size_of::<Self>() - size_of::<PreparedExpm1<'_>>()) as u64 + original_bytes;
        let auxiliary = budget.with_retained_payload(extra + 8192, AT, |budget| {
            budget.work(1 + count, AT)?;
            tracing::trace!(target = ?original.target, context = ?original.context, goal = ?original.goal);
            transform(cas, original, domain, policy, budget)
        })?;
        let result = budget.with_retained_payload(extra + 8192, AT, |budget| {
            PreparedExpm1::new(
                cas,
                &auxiliary,
                [-domain[1], -domain[0]],
                search,
                context,
                budget,
                counts,
            )
        });
        match result {
            Ok(basis) => {
                let bytes = basis.bytes.saturating_add(extra);
                finalize_owner(bytes, budget, AT, || Self {
                    original,
                    basis,
                    domain,
                    policy,
                    bytes,
                })
                .map_err(Into::into)
            }
            Err(PreparedExpm1Error::TargetNotCertifiable) => {
                budget.check(BudgetKind::OutputBytes, extra + 8192, AT)?;
                Err(PreparedSigmoidError::BasisPreparation {
                    auxiliary: Box::new(auxiliary),
                    policy,
                    search,
                })
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Bound finite precision selection; not final numerical evidence.
    #[must_use]
    pub const fn preparation_policy(&self) -> SigmoidPreparationPolicy {
        self.policy
    }
    /// Checked basis retained with the producing graph.
    #[must_use]
    pub fn basis(&self) -> &Prepared {
        self.basis.authority()
    }

    /// Reuse the frozen owner without exact Work, allocations or graph traversal.
    /// # Errors
    /// Preserves ordered native operand, basis and final-target refusals.
    pub fn evaluate(
        &self,
        input: SigmoidInput,
        context: &ArithmeticContext,
    ) -> Result<SigmoidEvaluation<'_>, SigmoidFailure> {
        let _span = tracing::trace_span!(
            "nrz.kernel",
            family = "prepared-sigmoid-owner",
            revision = 1_u64,
            domain = ?self.domain,
            policy = ?self.policy,
            bytes = self.bytes,
            original_coordinates = self.original.coordinates.len(),
            ?context
        )
        .entered();
        let exp = ExpProgram::new(&self.basis.program, self.policy.requested_bits);
        let result = SigmoidProgram::new(&exp, self.domain, self.policy.bits)?
            .evaluate(input, context)
            .map(|output| SigmoidEvaluation {
                input,
                output,
                basis: self.basis(),
            });
        tracing::trace!(succeeded = result.is_ok());
        result
    }

    /// Admit complete logical inventories and traversal before materializing ordered results.
    /// # Errors
    /// Preserves resource causes and earlier consumption; individual refusals keep their positions.
    pub fn evaluate_batch(
        &self,
        inputs: &[SigmoidInput],
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<Vec<Result<SigmoidEvaluation<'_>, SigmoidFailure>>, Fault> {
        let _span = tracing::trace_span!(
            "nrz.prelude",
            family = "prepared-sigmoid-owner",
            revision = 1_u64,
            domain = ?self.domain,
            policy = ?self.policy,
            bytes = self.bytes,
            original_coordinates = self.original.coordinates.len(),
            count = inputs.len(),
            ?context,
            limits = ?budget.limits(),
            usage = ?budget.usage()
        )
        .entered();
        let result = budget.with_work_phase("prepared-sigmoid", "nrz.prelude", |budget| {
            let count = inputs.len() as u64;
            budget.check(BudgetKind::MatrixEntries, count, AT)?;
            let output = count
                .checked_mul(size_of::<Result<SigmoidEvaluation<'_>, SigmoidFailure>>() as u64)
                .ok_or_else(|| Fault::capacity(BudgetKind::OutputBytes, AT))?;
            let input = count
                .checked_mul(size_of::<SigmoidInput>() as u64)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
            budget.check(BudgetKind::OutputBytes, output, AT)?;
            self.basis.graph.with_retained_graph(budget, |budget| {
                budget.with_retained_payload(
                    self.bytes.saturating_add(input).saturating_add(output),
                    AT,
                    |budget| {
                        budget.work(count, AT)?;
                        tracing::event!(name: "nrz.construct",
                            target: "nrz::work", tracing::Level::TRACE,
                            family = "prepared-sigmoid", revision = 1_u64,
                            operation = "batch allocation", count = 1_u64);
                        let mut results = Vec::new();
                        results
                            .try_reserve_exact(inputs.len())
                            .map_err(|_| Fault::allocation(AT))?;
                        for &input in inputs {
                            results.push(self.evaluate(input, context));
                        }
                        Ok(results)
                    },
                )
            })
        });
        tracing::trace!(count = result.as_ref().ok().map(Vec::len), error = ?result.as_ref().err());
        result
    }
}

/// Exact original operation and box, followed by an explicit negative-variable substitution.
fn transform(
    cas: &mut Cas,
    original: &EnclosureRequest,
    domain: [f64; 2],
    policy: SigmoidPreparationPolicy,
    budget: &mut Budget,
) -> Result<Request, PreparedSigmoidError> {
    let (operator, sort) = cas.expression(&original.target)?;
    if operator != &Operator::Sigmoid
        || original.goal != EnclosureGoal::Width
        || !cas.expression_scope(&original.target)?.is_empty()
    {
        return Err(PreparedSigmoidError::Operation);
    }
    let sort = sort.clone();
    let header = cas.expression_header(&original.target)?;
    if header != cas.context_header(&original.context)? {
        return Err(PreparedSigmoidError::Operation);
    }
    let operands = cas.operands(&original.target, budget)?;
    let [argument] = operands.as_slice() else {
        return Err(PreparedSigmoidError::Operation);
    };
    let lower = represented(domain[0], budget)?;
    let upper = represented(domain[1], budget)?;
    let (operator, _) = cas.expression(argument)?;
    let mut coordinates = Vec::new();
    let transformed = match operator {
        Operator::Free(_) => {
            let [coordinate] = original.coordinates.as_slice() else {
                return Err(PreparedSigmoidError::Domain);
            };
            check_number(coordinate.interval.lower(), budget)?;
            check_number(coordinate.interval.upper(), budget)?;
            tracing::trace!(?coordinate);
            if &coordinate.coordinate != argument
                || coordinate.interval.lower() != &lower
                || coordinate.interval.upper() != &upper
            {
                return Err(PreparedSigmoidError::Domain);
            }
            coordinates
                .try_reserve_exact(1)
                .map_err(|_| Fault::allocation(AT))?;
            coordinates.push(CoordinateBound {
                coordinate: argument.clone(),
                interval: Interval::new(upper.negated(budget)?, lower.negated(budget)?, budget)?,
            });
            argument.clone()
        }
        Operator::Rational(value) => {
            check_number(value, budget)?;
            if !original.coordinates.is_empty() || value != &lower || value != &upper {
                return Err(PreparedSigmoidError::Domain);
            }
            let negative = value.negated(budget)?;
            term(
                cas,
                &header,
                &sort,
                Operator::Rational(negative),
                &[],
                budget,
            )?
        }
        _ => return Err(PreparedSigmoidError::Operation),
    };
    let exponential = term(
        cas,
        &header,
        &sort,
        Operator::Exponential,
        &[transformed],
        budget,
    )?;
    let one = term(
        cas,
        &header,
        &sort,
        Operator::Rational(Rational::one()),
        &[],
        budget,
    )?;
    let target = term(
        cas,
        &header,
        &sort,
        Operator::Sub,
        &[exponential, one],
        budget,
    )?;
    Ok(Request {
        original: EnclosureRequest {
            target,
            context: original.context.clone(),
            coordinates,
            goal: EnclosureGoal::Width,
        },
        bits: policy.requested_bits,
        environment: policy.revision,
    })
}

/// Bound exact components before equality traversal of a represented operand.
fn check_number(value: &Rational, budget: &Budget) -> Result<(), PreparedSigmoidError> {
    let (numerator, denominator) = value.component_bits();
    if numerator > 1075 || denominator > 1075 {
        return Err(PreparedSigmoidError::Domain);
    }
    value.check(budget, AT)?;
    Ok(())
}

/// Fixed-arity admitted auxiliary syntax; its enclosing prelude funds the temporary inventories.
fn term(
    cas: &mut Cas,
    header: &Handle,
    sort: &Sort,
    operator: Operator,
    operands: &[Handle],
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let mut refs = Vec::new();
    refs.try_reserve_exact(operands.len())
        .map_err(|_| Fault::allocation(AT))?;
    refs.extend(operands.iter().cloned().map(Reference::Existing));
    let draft = Draft::Expression(ExpressionDraft {
        space: None,
        operator,
        sort: sort.clone(),
        header: Reference::Existing(header.clone()),
        scope: Vec::new(),
        binding: None,
        operands: refs,
    });
    cas.admit(&[draft], budget)?
        .into_iter()
        .next()
        .ok_or_else(|| Fault::allocation(AT))
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A 4,096-byte basis and 1,600-byte original inventory require 5,696 bytes.
    /// The construction finalizer admits N, refuses N−1 before assembly and preserves seven prior Work.
    /// # Panics
    /// Panics if the independent returned-owner output guard or refusal changes.
    /// ´claim:matrix:owner-sigmoid-output-boundary´
    /// ´test:unit:owner-sigmoid-output-boundary´
    #[test]
    fn owner_sigmoid_output_boundary() -> Result<(), Box<dyn std::error::Error>> {
        use nrz_bindings::readings::algebra::{Exhaustion, FaultKind, limits::Limits};
        for limit in [5696, 5695] {
            let mut budget = Budget::new(Limits {
                output_bytes: limit,
                ..Limits::default()
            })?;
            budget.work(7, AT)?;
            let assembled = std::cell::Cell::new(false);
            let result = finalize_owner(4096 + 1600, &budget, AT, || {
                assembled.set(true);
                5696_u64
            });
            if limit == 5696 {
                assert_eq!(result?, 5696);
                assert!(assembled.get());
            } else {
                assert!(!assembled.get(), "output admission precedes assembly");
                assert_eq!(
                    result.expect_err("final owner output").kind,
                    FaultKind::Exhausted {
                        reason: Exhaustion::Limit,
                        budget: BudgetKind::OutputBytes,
                        limit,
                        needed: 5696
                    }
                );
            }
            assert_eq!(budget.usage().work, 7);
        }
        Ok(())
    }
}
