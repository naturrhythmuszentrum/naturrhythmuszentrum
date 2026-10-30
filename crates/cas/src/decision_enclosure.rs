// SPDX-License-Identifier: AGPL-3.0-only
//! Exact original-target replay through the existing closed constant rules.

use std::cmp::Ordering;

use crate::budget::{Budget, BudgetKind};
use crate::decision::{CheckedRationalEnclosure, EnclosureOrigin, RationalEnclosure};
use crate::error::{Fault, FaultKind, InvalidCode, Location};
use crate::exact_constant::ExactConstant;
use crate::graph::Cas;
use crate::number::Rational;

/// Stable location for the closed rational enclosure lifecycle.
const AT: Location = Location::new("exact rational enclosure");

/// Unsupported origins and non-rational replay conclusions confer no authority.
const fn unsupported() -> Fault {
    Fault::invalid(InvalidCode::UnsupportedCertificate, AT)
}

/// Check the immutable contract and all original syntax before publishing owned evidence.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the public decision facade calls this private checker"
)]
pub(crate) fn check(
    cas: &Cas,
    contract: &RationalEnclosure,
    budget: &mut Budget,
) -> Result<CheckedRationalEnclosure, Fault> {
    if reusable(&contract.origin) {
        return retained(cas, budget, |budget| receipt(cas, contract, budget));
    }
    receipt(cas, contract, budget)
}

/// Complete output ownership must succeed before a newly retained record can be cited.
fn receipt(
    cas: &Cas,
    contract: &RationalEnclosure,
    budget: &mut Budget,
) -> Result<CheckedRationalEnclosure, Fault> {
    let bytes = validate_inner(cas, contract, budget)?;
    budget.check(BudgetKind::OutputBytes, bytes, AT)?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes.saturating_mul(2), AT)?;
    budget.work(bytes, AT)?;
    Ok(CheckedRationalEnclosure {
        contract: contract.clone(),
        bytes,
        uses_pi: contract.origin.uses_pi(),
    })
}

/// Validate the existing certificate in place without allocating an unused owned receipt.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the enclosure facade and selected Integer theorem share original-target validation"
)]
pub(crate) fn validate(
    cas: &Cas,
    contract: &RationalEnclosure,
    budget: &mut Budget,
) -> Result<u64, Fault> {
    if reusable(&contract.origin) {
        return retained(cas, budget, |budget| validate_inner(cas, contract, budget));
    }
    validate_inner(cas, contract, budget)
}

/// Lattice consumers currently retain only closed scalar prerequisites.
#[expect(
    clippy::redundant_pub_crate,
    reason = "lattice proof consumers require a closed scalar prerequisite"
)]
pub(crate) fn validate_closed(
    cas: &Cas,
    contract: &RationalEnclosure,
    budget: &mut Budget,
) -> Result<u64, Fault> {
    crate::decision_kernel::family::closed(contract)?;
    validate(cas, contract, budget)
}

/// Only origins with independently checked reusable judgments acquire the store guard.
const fn reusable(origin: &EnclosureOrigin) -> bool {
    matches!(
        origin,
        EnclosureOrigin::ScalarKernel(_) | EnclosureOrigin::PiBound { .. }
    )
}

/// Both judgment kinds publish atomically and share one cumulative storage reservation.
fn retained<T>(
    cas: &Cas,
    budget: &mut Budget,
    operation: impl FnOnce(&mut Budget) -> Result<T, Fault>,
) -> Result<T, Fault> {
    crate::decision_kernel::cache::synchronized(cas, budget, |budget| {
        crate::decision_kernel::cache::atomic(cas, budget, |budget| {
            crate::decision_kernel::cache::retained(cas, budget, operation)
        })
    })
}

/// Replay the supported origin while its caller holds any required store guard.
fn validate_inner(
    cas: &Cas,
    contract: &RationalEnclosure,
    budget: &mut Budget,
) -> Result<u64, Fault> {
    if let EnclosureOrigin::ScalarKernel(certificate) = &contract.origin {
        return crate::decision_kernel::validate(cas, contract, certificate, budget);
    }
    if let EnclosureOrigin::PiBound {
        expression,
        certificate,
    } = &contract.origin
    {
        return crate::decision_pi::validate(cas, contract, expression, certificate, budget)
            .map_err(fragment);
    }
    let EnclosureOrigin::ExactRational { expression, value } = &contract.origin else {
        return Err(unsupported());
    };
    budget.work(4, AT)?;
    cas.checked(expression)?;
    if expression != &contract.target {
        return Err(Fault::invalid(InvalidCode::EndpointMismatch, AT));
    }
    identities(cas, contract)?;
    let bytes = 384_u64
        .saturating_add(number_bytes(&contract.lower))
        .saturating_add(number_bytes(&contract.upper))
        .saturating_add(number_bytes(value));
    let scratch = budget
        .limits()
        .component_bits
        .div_ceil(8)
        .saturating_mul(32)
        .saturating_add(1_024);
    budget.with_retained_payload(bytes.saturating_add(scratch), AT, |budget| {
        replay(cas, contract, value, budget)
    })?;
    Ok(bytes)
}

/// The context's actual region and header cannot be replaced by caller descriptions.
#[expect(
    clippy::redundant_pub_crate,
    reason = "the sibling closed replay checkers share this validation boundary"
)]
pub(super) fn identities(cas: &Cas, contract: &RationalEnclosure) -> Result<(), Fault> {
    let target = cas.state.expression(cas.checked(&contract.target)?)?;
    let context = cas.state.context(cas.checked(&contract.context)?)?;
    let family = cas.checked(&contract.input_family)?;
    if family != context.admissible {
        return Err(Fault::invalid(InvalidCode::EndpointMismatch, AT));
    }
    if target.header != context.header_id {
        return Err(Fault::invalid(InvalidCode::Scope, AT));
    }
    Ok(())
}

/// Check exact containment only after the complete original target has been replayed.
fn replay(
    cas: &Cas,
    contract: &RationalEnclosure,
    expected: &Rational,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for endpoint in [&contract.lower, &contract.upper, expected] {
        endpoint.check(budget, AT)?;
    }
    if contract.lower.compare(&contract.upper, budget)? == Ordering::Greater {
        return Err(Fault::invalid(InvalidCode::DecisionBox, AT));
    }
    let checked =
        ExactConstant::from_expression(cas, &contract.target, budget).map_err(fragment)?;
    let value = checked.as_rational().ok_or_else(unsupported)?;
    if value.compare(expected, budget)? != Ordering::Equal {
        return Err(Fault::invalid(InvalidCode::EndpointMismatch, AT));
    }
    let above_lower = contract.lower.compare(value, budget)? != Ordering::Greater;
    let below_upper = value.compare(&contract.upper, budget)? != Ordering::Greater;
    if above_lower && below_upper {
        Ok(())
    } else {
        Err(Fault::invalid(InvalidCode::DecisionBox, AT))
    }
}

/// Preserve domain and resource failures while exposing unsupported replay grammar uniformly.
fn fragment(fault: Fault) -> Fault {
    if fault.kind == FaultKind::Invalid(InvalidCode::UnsupportedFragment) {
        unsupported()
    } else {
        fault
    }
}

/// Account for rational components and allocation overhead without cloning them.
fn number_bytes(value: &Rational) -> u64 {
    let (numerator, denominator) = value.component_bits();
    numerator
        .div_ceil(8)
        .saturating_add(denominator.div_ceil(8))
        .saturating_mul(4)
        .saturating_add(128)
}

/// One exclusive caller's staged enclosure checks, ending at its final owned receipt.
///
/// This view permits receipt checking and exposes no general graph mutation. New
/// scalar judgments remain attempt-local until the enclosing callback succeeds.
#[derive(Debug)]
pub struct EnclosureAttempt<'a> {
    /// The owner keeps its exclusive guard while nested checkers borrow this graph.
    cas: &'a Cas,
}

impl Cas {
    /// Stage checked judgments through an enclosing consumer's final fallible copy or containment.
    ///
    /// Nested receipt checks use the supplied view without acquiring the owner lock again.
    /// Successful publication moves every staged scalar payload into the existing graph;
    /// a refusal drops the complete suffix and preserves spent caller Work.
    /// # Errors
    /// Preserves the callback's refusal or reports graph publication capacity exhaustion.
    pub fn with_enclosure_attempt<T, E>(
        &mut self,
        budget: &mut Budget,
        operation: impl FnOnce(&mut EnclosureAttempt<'_>, &mut Budget) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<Fault>,
    {
        crate::decision_kernel::cache::attempt(self, budget, |cas, budget| {
            operation(&mut EnclosureAttempt { cas }, budget)
        })
    }
}

impl EnclosureAttempt<'_> {
    /// Check the complete scalar receipt without publishing an inner judgment prefix.
    /// # Errors
    /// Refuses invalid scope, unsupported or false origins and exhausted caller resources.
    pub fn check_rational_enclosure(
        &mut self,
        contract: &RationalEnclosure,
        budget: &mut Budget,
    ) -> Result<CheckedRationalEnclosure, Fault> {
        let previous = budget.restrict_storage(&self.cas.limits);
        let result = check(self.cas, contract, budget);
        budget.restore_limits(previous);
        result
    }

    /// Check every original coordinate and the final box copy in this same staged attempt.
    /// # Errors
    /// Refuses malformed identities, any invalid coordinate or exhausted resources.
    pub fn check_rational_box_enclosure(
        &mut self,
        contract: &crate::lattice::RationalBoxEnclosure,
        budget: &mut Budget,
    ) -> Result<crate::lattice::CheckedRationalBoxEnclosure, Fault> {
        let previous = budget.restrict_storage(&self.cas.limits);
        let result = self.cas.rational_box_enclosure(contract, budget);
        budget.restore_limits(previous);
        result
    }
}
