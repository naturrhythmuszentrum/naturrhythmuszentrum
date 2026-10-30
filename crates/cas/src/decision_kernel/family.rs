// SPDX-License-Identifier: AGPL-3.0-only
//! Qualified physical box premises remain separate from complete native input arithmetic.

mod meaning;
mod retention;
#[expect(
    clippy::redundant_pub_crate,
    reason = "scalar proof consumers share qualified dependency retention"
)]
pub(crate) use retention::{dependencies, retain};

use super::model::{
    ScalarKernelCertificate, ScalarKernelFamily, ScalarKernelFamilyInput, ScalarKernelLaw,
    ScalarKernelRecord,
};
use crate::budget::Budget;
use crate::decision::RationalEnclosure;
use crate::domain::HypothesisRef;
use crate::graph::{Cas, Handle, LogicalId, Vertex};
use crate::model::{AtomicSort, Field, Operator, PredicateKind, Sort};
use crate::{Fault, InvalidCode, Location};
use std::cmp::Ordering;

/// Source identity, scope and exact assumed bounds are authenticated here.
const PREMISE: Location = Location::new("scalar kernel family premise");
/// A nominal numerical input cannot cover a larger authenticated physical family.
const COVERAGE: Location = Location::new("scalar kernel family coverage");

/// Changed source assumptions cannot authorize a physical enclosure.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::EndpointMismatch, PREMISE)
}

impl Cas {
    /// Authenticate an explicit physical source box without discharging its endpoint assumptions.
    ///
    /// The two qualified predicates must state source-minus-lower and upper-minus-source
    /// are nonnegative in ancestors of the supplied context. This proves no measurement model.
    /// # Errors
    /// Refuses foreign or changed sources, scopes, slots, endpoints, reversed boxes and limits.
    pub fn check_scalar_kernel_family(
        &self,
        context: &Handle,
        family: &ScalarKernelFamily,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        budget.work(5, PREMISE)?;
        let context_id = self.checked(context)?;
        let source = self.state.expression(self.checked(&family.source)?)?;
        if source.header != self.state.context(context_id)?.header_id
            || source.sort != Sort::Value(AtomicSort::Scalar(Field::Real))
            || !source.scope.is_empty()
            || source.binding.is_some()
            || source.space.is_some()
            || source.domain.is_some()
            || source.attachment.is_some()
            || !matches!(source.operator, Operator::Free(_))
        {
            return Err(invalid());
        }
        family.lower.check(budget, PREMISE)?;
        family.upper.check(budget, PREMISE)?;
        if family.lower.compare(&family.upper, budget)? == Ordering::Greater {
            return Err(Fault::invalid(InvalidCode::RefutedDomain, PREMISE));
        }
        for (index, premise) in family.premises.iter().enumerate() {
            endpoint(self, context_id, family, premise, index == 0, budget)?;
        }
        Ok(())
    }
}

/// A qualified predicate authenticates both its original source and literal bound.
fn endpoint(
    cas: &Cas,
    context: LogicalId,
    family: &ScalarKernelFamily,
    premise: &HypothesisRef,
    lower: bool,
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(5, PREMISE)?;
    if !crate::context_check::ancestor(cas, cas.checked(&premise.context)?, context, budget)? {
        return Err(invalid());
    }
    let predicate = crate::context_check::predicate(cas, premise).map_err(|_| invalid())?;
    let Vertex::Condition(predicate) = cas.state.node(cas.checked(&predicate)?)? else {
        return Err(invalid());
    };
    if predicate.kind != PredicateKind::NonNegative {
        return Err(invalid());
    }
    let [difference] = predicate.arguments.as_slice() else {
        return Err(invalid());
    };
    let difference = cas.state.expression(*difference)?;
    let [left, right] = difference.operands.as_slice() else {
        return Err(invalid());
    };
    if difference.operator != Operator::Sub {
        return Err(invalid());
    }
    let (source, bound, expected) = if lower {
        (*left, *right, &family.lower)
    } else {
        (*right, *left, &family.upper)
    };
    let Operator::Rational(actual) = &cas.state.expression(bound)?.operator else {
        return Err(invalid());
    };
    if source != cas.checked(&family.source)? || actual != expected {
        return Err(invalid());
    }
    Ok(())
}

/// Replay a conditional source inventory without entering the closed-constant reconstruction cache.
pub(super) fn validate(
    cas: &Cas,
    contract: &RationalEnclosure,
    certificate: &ScalarKernelCertificate,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut found = false;
    for (index, record) in certificate.records.iter().enumerate() {
        budget.work(super::record_bytes(record), COVERAGE)?;
        if let ScalarKernelLaw::Family(input) = &record.law {
            cas.check_scalar_kernel_family(&certificate.context, &input.family, budget)?;
            super::relations::check(
                cas,
                cas.checked(&contract.target)?,
                record,
                &certificate.records[..index],
                budget,
            )?;
            found = true;
        }
    }
    if !found {
        return Err(Fault::invalid(
            InvalidCode::UnsupportedCertificate,
            COVERAGE,
        ));
    }
    meaning::check(cas, certificate, budget)?;
    // The prepaid inventory walk authenticates the original before non-family arithmetic.
    for (index, record) in certificate.records.iter().enumerate() {
        if !matches!(record.law, ScalarKernelLaw::Family(_)) {
            super::relations::check(
                cas,
                cas.checked(&contract.target)?,
                record,
                &certificate.records[..index],
                budget,
            )?;
        }
    }
    let last = certificate.records.last().ok_or_else(invalid)?;
    let same_lower = last.lower == contract.lower;
    let same_upper = last.upper == contract.upper;
    if !(same_lower && same_upper) {
        return Err(Fault::invalid(InvalidCode::EndpointMismatch, COVERAGE));
    }
    Ok(())
}

/// Both the authenticated physical family and the full native total box must be covered.
pub(super) fn input(
    record: &ScalarKernelRecord,
    value: &ScalarKernelFamilyInput,
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(3, COVERAGE)?;
    value.center.check(budget, COVERAGE)?;
    value.radius.check(budget, COVERAGE)?;
    if value.radius.is_negative() {
        return Err(Fault::invalid(InvalidCode::RefutedDomain, COVERAGE));
    }
    let lower = value.center.sub(&value.radius, budget)?;
    let upper = value.center.add(&value.radius, budget)?;
    for (left, right) in [
        (&record.lower, &lower),
        (&lower, &value.family.lower),
        (&value.family.upper, &upper),
        (&upper, &record.upper),
    ] {
        if left.compare(right, budget)? == Ordering::Greater {
            return Err(Fault::invalid(InvalidCode::DecisionBox, COVERAGE));
        }
    }
    Ok(())
}

/// All source handles, assumptions and four rational fields are prepaid before copying.
pub(super) fn bytes(value: &ScalarKernelFamilyInput) -> u64 {
    [
        &value.center,
        &value.radius,
        &value.family.lower,
        &value.family.upper,
    ]
    .into_iter()
    .fold(320_u64, |total, value| {
        total.saturating_add(super::number_bytes(value))
    })
}

/// Scalar conditional receipts are not silently accepted by closed lattice proof consumers.
#[expect(
    clippy::redundant_pub_crate,
    reason = "lattice proof consumers retain only closed scalar prerequisites"
)]
pub(crate) fn closed(contract: &RationalEnclosure) -> Result<(), Fault> {
    if let crate::decision::EnclosureOrigin::ScalarKernel(certificate) = &contract.origin
        && certificate.version == 3
    {
        return Err(Fault::invalid(
            InvalidCode::UnsupportedCertificate,
            COVERAGE,
        ));
    }
    Ok(())
}
