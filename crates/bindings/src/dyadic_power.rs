// SPDX-License-Identifier: AGPL-3.0-only
//! Original power syntax and its checked whole-domain enclosure stay paired as preparation data.

use nrz_cas::enclosure::{
    CheckedEnclosure, CoordinateBound, EnclosureGoal, EnclosureOutcome, EnclosureReport,
    EnclosureRequest, Interval, RefinementPolicy, UndecidedReason,
};
use nrz_cas::limits::{Budget, BudgetKind, BudgetUsage};
use nrz_cas::source::SyntaxNode;
use nrz_cas::syntax::{AtomicSort, Field, Operator, Sort};
use nrz_cas::{Cas, Fault, Handle, Location, Rational};
use std::cmp::Ordering;

/// Work is charged to the caller's existing exact ledger at this preparation boundary.
const AT: Location = Location::new("dyadic power original correspondence");

/// Original-syntax, carrier, exact and stopped-refinement failures remain distinct.
#[derive(Debug, thiserror::Error)]
pub enum DyadicPowerPreparationError {
    /// The original does not have the admitted real scalar power shape.
    #[error("unsupported original dyadic power expression")]
    UnsupportedOriginal,
    /// The exact exponent lies outside the numerical member's range.
    #[error("original power exponent outside (0, 1]")]
    ExponentOutsideDomain,
    /// An in-range exact exponent does not fit the numerical carriers.
    #[error("original power exponent exceeds signed/unsigned carriers")]
    ExponentCarrierLimit,
    /// A direct preparation call supplied invalid represented endpoint data.
    #[error("invalid represented dyadic power domain")]
    InvalidRepresentedDomain,
    /// Exact syntax, arithmetic, encoding or bounded Work refused.
    #[error(transparent)]
    Exact(#[from] Fault),
    /// A completed CAS attempt had an outcome outside the closed owner allow-list.
    #[error("dyadic power enclosure stopped: {0:?}")]
    Stopped(Box<EnclosureReport>),
    /// An accepted outcome without a checked bound cannot establish authority.
    #[error("dyadic power enclosure outcome lacks checked evidence: {0:?}")]
    MissingChecked(Box<EnclosureReport>),
}

/// Actual checked result and its bounded canonical proof bytes, still requiring its producing CAS.
#[derive(Debug)]
pub struct DyadicPowerPreparation {
    /// Actual returned checker result, including original request and interval.
    checked: CheckedEnclosure,
    /// Explicit refinement policy used for this attempt.
    policy: RefinementPolicy,
    /// Bounded canonical certificate encoding for the matrix proof-sensitive seal.
    proof_bytes: Vec<u8>,
}

/// Authenticated original operands and exact literal before carrier conversion.
struct Original {
    /// Original free-coordinate identity.
    base: Handle,
    /// Already reduced rational literal exponent.
    exponent: Rational,
}

impl DyadicPowerPreparation {
    /// Inspect the original exponent and return data-only numerical carriers.
    /// # Errors
    /// Refuses foreign or unsupported syntax, out-of-range exponents and carrier exhaustion.
    pub fn original_exponent(
        cas: &Cas,
        target: &Handle,
        context: &Handle,
        budget: &mut Budget,
    ) -> Result<(i64, u64), DyadicPowerPreparationError> {
        let original = inspect(cas, target, context, budget)?;
        carriers(&original.exponent, budget)
    }

    /// Prepare one width-only original enclosure from exactly represented endpoint data.
    ///
    /// The receipt is reset at entry and includes this phase's complete attempted prefix.
    /// # Errors
    /// Preserves exact and stopped-refinement detail without admitting a missing checked bound.
    #[expect(
        clippy::too_many_arguments,
        reason = "the caller's graph, exact ledger and receipt remain separate"
    )]
    pub fn prepare(
        cas: &mut Cas,
        target: &Handle,
        context: &Handle,
        domain: [f64; 2],
        policy: RefinementPolicy,
        budget: &mut Budget,
        interrupt: &mut impl FnMut(BudgetUsage) -> bool,
        work_used: &mut u64,
    ) -> Result<Self, DyadicPowerPreparationError> {
        let start = budget.usage().work;
        *work_used = 0;
        let result = prepare(cas, target, context, domain, policy, budget, interrupt);
        *work_used = budget.usage().work.saturating_sub(start);
        result
    }

    /// Actual checked mathematical bound and its original request.
    #[must_use]
    pub const fn judgment(&self) -> &CheckedEnclosure {
        &self.checked
    }

    /// Policy that produced this checked result.
    #[must_use]
    pub const fn policy(&self) -> RefinementPolicy {
        self.policy
    }

    /// Complete canonical proof bytes; these are data until paired with the producing CAS.
    #[must_use]
    pub fn proof_bytes(&self) -> &[u8] {
        &self.proof_bytes
    }

    /// Actual retained proof-buffer capacity for the matrix owner's byte inventory.
    #[must_use]
    pub const fn proof_capacity(&self) -> usize {
        self.proof_bytes.capacity()
    }

    /// CAS-owned conservative logical allowance of this returned checked receipt.
    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        self.checked.retained_bytes()
    }
}

/// Authenticate the original, its header and its ordered free-coordinate/literal operands.
fn inspect(
    cas: &Cas,
    target: &Handle,
    context: &Handle,
    budget: &mut Budget,
) -> Result<Original, DyadicPowerPreparationError> {
    budget.work(1, AT)?;
    let syntax = cas.describe_syntax(target, budget)?;
    budget.work(3, AT)?;
    if syntax.header != cas.context_header(context, budget)?
        || !syntax.scope.is_empty()
        || syntax.binding.is_some()
    {
        return Err(DyadicPowerPreparationError::UnsupportedOriginal);
    }
    budget.work(1, AT)?;
    let SyntaxNode::Expression(expression) = syntax.node else {
        return Err(DyadicPowerPreparationError::UnsupportedOriginal);
    };
    budget.work(2, AT)?;
    if expression.sort != Sort::Value(AtomicSort::Scalar(Field::Real))
        || !matches!(expression.operator, Operator::PositivePower)
    {
        return Err(DyadicPowerPreparationError::UnsupportedOriginal);
    }
    let operands = cas.operands(target, budget)?;
    budget.work(
        u64::try_from(operands.len()).map_err(|_| Fault::capacity(BudgetKind::Work, AT))?,
        AT,
    )?;
    let [base, exponent] = operands.as_slice() else {
        return Err(DyadicPowerPreparationError::UnsupportedOriginal);
    };
    let base_syntax = cas.describe_syntax(base, budget)?;
    let exponent_syntax = cas.describe_syntax(exponent, budget)?;
    budget.work(6, AT)?;
    if base_syntax.header != syntax.header
        || exponent_syntax.header != syntax.header
        || !base_syntax.scope.is_empty()
        || !exponent_syntax.scope.is_empty()
        || base_syntax.binding.is_some()
        || exponent_syntax.binding.is_some()
    {
        return Err(DyadicPowerPreparationError::UnsupportedOriginal);
    }
    budget.work(2, AT)?;
    let (SyntaxNode::Expression(base_expression), SyntaxNode::Expression(exponent_expression)) =
        (base_syntax.node, exponent_syntax.node)
    else {
        return Err(DyadicPowerPreparationError::UnsupportedOriginal);
    };
    budget.work(4, AT)?;
    if base_expression.sort != Sort::Value(AtomicSort::Scalar(Field::Real))
        || !matches!(base_expression.operator, Operator::Free(_))
        || exponent_expression.sort != Sort::Value(AtomicSort::Scalar(Field::Real))
    {
        return Err(DyadicPowerPreparationError::UnsupportedOriginal);
    }
    let Operator::Rational(exponent) = exponent_expression.operator else {
        return Err(DyadicPowerPreparationError::UnsupportedOriginal);
    };
    Ok(Original {
        base: base.clone(),
        exponent,
    })
}

/// Preserve exact range before conversion into the finite numerical carriers.
fn carriers(
    exponent: &Rational,
    budget: &mut Budget,
) -> Result<(i64, u64), DyadicPowerPreparationError> {
    let zero = Rational::from_ratio(0, 1, budget)?;
    let one = Rational::from_ratio(1, 1, budget)?;
    if exponent.compare(&zero, budget)? != Ordering::Greater
        || exponent.compare(&one, budget)? == Ordering::Greater
    {
        return Err(DyadicPowerPreparationError::ExponentOutsideDomain);
    }
    exponent
        .small_ratio(budget)?
        .ok_or(DyadicPowerPreparationError::ExponentCarrierLimit)
}

/// Reinspect the original and move only a checked bound from the closed allowed outcomes.
fn prepare(
    cas: &mut Cas,
    target: &Handle,
    context: &Handle,
    domain: [f64; 2],
    policy: RefinementPolicy,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
) -> Result<DyadicPowerPreparation, DyadicPowerPreparationError> {
    budget.work(1, AT)?;
    let original = inspect(cas, target, context, budget)?;
    carriers(&original.exponent, budget)?;
    if !domain.iter().all(|x| x.is_finite()) || domain[0] > domain[1] {
        return Err(DyadicPowerPreparationError::InvalidRepresentedDomain);
    }
    budget.work(1, AT)?;
    let interval = Interval::new(
        represented(domain[0], budget)?,
        represented(domain[1], budget)?,
        budget,
    )?;
    let request = EnclosureRequest {
        target: target.clone(),
        context: context.clone(),
        coordinates: vec![CoordinateBound {
            coordinate: original.base,
            interval,
        }],
        goal: EnclosureGoal::Width,
    };
    let report = cas.enclose(&request, policy, budget, interrupt)?;
    let checked = completed(report)?;
    let proof_bytes = checked.certificate().to_bytes(budget)?;
    Ok(DyadicPowerPreparation {
        checked,
        policy,
        proof_bytes,
    })
}

/// Accept only a completed checked bound or an unfinished width goal with checked inclusion.
fn completed(mut report: EnclosureReport) -> Result<CheckedEnclosure, DyadicPowerPreparationError> {
    match report.outcome {
        EnclosureOutcome::Bound | EnclosureOutcome::Undecided(UndecidedReason::WidthLimit) => {
            report
                .checked
                .take()
                .ok_or_else(|| DyadicPowerPreparationError::MissingChecked(Box::new(report)))
        }
        EnclosureOutcome::Exhausted
        | EnclosureOutcome::Undecided(
            UndecidedReason::WorkExhausted
            | UndecidedReason::DomainBoundary
            | UndecidedReason::Straddle,
        )
        | EnclosureOutcome::Interrupted
        | EnclosureOutcome::Unsupported
        | EnclosureOutcome::Proved
        | EnclosureOutcome::Refuted => Err(DyadicPowerPreparationError::Stopped(Box::new(report))),
    }
}

/// Decode one finite binary64 value into its exact represented rational.
fn represented(value: f64, budget: &mut Budget) -> Result<Rational, Fault> {
    let bits = value.to_bits();
    let field = (bits >> 52) & 2047;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, exponent) = if field == 0 {
        (fraction, -1074)
    } else {
        (
            fraction + (1_u64 << 52),
            i64::try_from(field).map_err(|_| Fault::capacity(BudgetKind::IntegerExponent, AT))?
                - 1075,
        )
    };
    let sign = if value.is_sign_negative() { -1 } else { 1 };
    Rational::from_ratio(
        sign * i64::try_from(significand)
            .map_err(|_| Fault::capacity(BudgetKind::ComponentBits, AT))?,
        1,
        budget,
    )?
    .mul(
        &Rational::from_ratio(2, 1, budget)?.pow(exponent, budget)?,
        budget,
    )
}

#[cfg(test)]
mod tests;
