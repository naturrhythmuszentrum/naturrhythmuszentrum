// SPDX-License-Identifier: AGPL-3.0-only
//! Metered rational endpoint arithmetic and independently checked enclosure evidence.
//!
//! Both entry points mutate the graph when they retain checked evidence:
//! ```
//! use nrz_cas::{Cas, Fault};
//! use nrz_cas::limits::Budget;
//! use nrz_cas::enclosure::{EnclosureRequest, RefinementPolicy, EnclosureCertificate};
//! fn calls(cas: &mut Cas, request: &EnclosureRequest, policy: RefinementPolicy,
//!          certificate: &EnclosureCertificate, budget: &mut Budget) -> Result<(), Fault> {
//!     cas.enclose(request, policy, budget, &mut |_| false)?;
//!     cas.check_enclosure(request, policy, certificate, budget, &mut |_| false)?;
//!     Ok(())
//! }
//! ```
//! A shared reference does not grant an alternate producer route:
//! ```compile_fail,E0596
//! use nrz_cas::{Cas, Fault};
//! use nrz_cas::limits::Budget;
//! use nrz_cas::enclosure::{EnclosureRequest, RefinementPolicy};
//! fn shared(cas: &Cas, request: &EnclosureRequest, policy: RefinementPolicy,
//!           budget: &mut Budget) -> Result<(), Fault> {
//!     cas.enclose(request, policy, budget, &mut |_| false)?;
//!     Ok(())
//! }
//! ```
//! Nor does it grant a non-retaining checker route:
//! ```compile_fail,E0596
//! use nrz_cas::{Cas, Fault};
//! use nrz_cas::limits::Budget;
//! use nrz_cas::enclosure::{EnclosureRequest, RefinementPolicy, EnclosureCertificate};
//! fn shared(cas: &Cas, request: &EnclosureRequest, policy: RefinementPolicy,
//!           certificate: &EnclosureCertificate, budget: &mut Budget) -> Result<(), Fault> {
//!     cas.check_enclosure(request, policy, certificate, budget, &mut |_| false)?;
//!     Ok(())
//! }
//! ```

mod checker;
mod interval;
mod model;
mod producer;
mod roots;
mod scope;
mod service;
#[cfg(test)]
mod tests;
mod wire;

/// Location of enclosure validation and resource accounting.
const AT: crate::Location = crate::Location::new("rational enclosure evaluator");
/// Distinct interruption marker, never a mathematical refusal.
const INTERRUPTED: crate::Location = crate::Location::new("enclosure interrupted");

/// Charge the deterministic boundary before observing interruption.
fn boundary(
    budget: &mut crate::budget::Budget,
    interrupt: &mut impl FnMut(crate::budget::BudgetUsage) -> bool,
) -> Result<(), crate::Fault> {
    budget.work(1, AT)?;
    if interrupt(budget.usage()) {
        return Err(crate::Fault::invalid(
            crate::InvalidCode::Policy,
            INTERRUPTED,
        ));
    }
    Ok(())
}

/// Conservative retained receipt size including all endpoint components and original handles.
pub(crate) fn footprint(value: &CheckedEnclosure) -> u64 {
    value.certificate.records.iter().fold(
        u64::try_from(value.certificate.scope.len())
            .unwrap_or(u64::MAX)
            .saturating_add(request_bytes(&value.request))
            .saturating_add(interval_bytes(&value.interval))
            .saturating_add(512),
        |size, interval| size.saturating_add(interval_bytes(interval)),
    )
}

/// Reserve a worst-case receipt from the admitted source size and component ceiling.
fn receipt_reservation(
    cas: &crate::Cas,
    request: &EnclosureRequest,
    scope_len: usize,
    budget: &crate::budget::Budget,
) -> Result<u64, crate::Fault> {
    let count = cas
        .state
        .expression(cas.checked(&request.target)?)?
        .unfolded;
    let endpoint = budget
        .limits()
        .component_bits
        .div_ceil(8)
        .saturating_mul(16)
        .saturating_add(128);
    Ok(count
        .saturating_add(1)
        .saturating_mul(endpoint)
        .saturating_add(u64::try_from(scope_len).unwrap_or(u64::MAX))
        .saturating_add(request_bytes(request))
        .saturating_add(512))
}

/// Size the supplied transcript before checking or copying any of its records.
fn certificate_bytes(value: &EnclosureCertificate) -> u64 {
    value.records.iter().fold(
        u64::try_from(value.scope.len())
            .unwrap_or(u64::MAX)
            .saturating_add(128),
        |bytes, record| bytes.saturating_add(interval_bytes(record)),
    )
}

/// Bound the request payload retained in a checked receipt.
fn request_bytes(value: &EnclosureRequest) -> u64 {
    let start = match &value.goal {
        EnclosureGoal::Width => 0,
        EnclosureGoal::AtMost(n) | EnclosureGoal::LessThan(n) | EnclosureGoal::Equal(n) => {
            number_bytes(n)
        }
    };
    value
        .coordinates
        .iter()
        .fold(start.saturating_add(256), |sum, v| {
            sum.saturating_add(interval_bytes(&v.interval))
                .saturating_add(128)
        })
}

/// Conservatively account for exact rational components.
fn number_bytes(number: &crate::Rational) -> u64 {
    let (n, d) = number.component_bits();
    n.div_ceil(8)
        .saturating_add(d.div_ceil(8))
        .saturating_mul(4)
        .saturating_add(128)
}

/// Account for both endpoint components and interval metadata.
fn interval_bytes(value: &Interval) -> u64 {
    [value.lower(), value.upper()]
        .iter()
        .fold(128_u64, |sum, number| {
            let (n, d) = number.component_bits();
            sum.saturating_add(
                n.div_ceil(8)
                    .saturating_add(d.div_ceil(8))
                    .saturating_mul(4),
            )
        })
}

pub use interval::Interval;
pub use model::{
    CheckedEnclosure, CoordinateBound, EnclosureCertificate, EnclosureGoal, EnclosureOutcome,
    EnclosureReport, EnclosureRequest, RefinementPolicy, UndecidedReason,
};

/// Closed formal register; no unimplemented analytic schema grants authority.
pub const ENCLOSURE_SCHEMAS: [crate::emptiness::LemmaSchema; 2] = [
    crate::emptiness::LemmaSchema {
        name: "enclosure.rational-inclusion",
        label: "thm:cas:enclosure-rational-inclusion",
        version: 1,
        standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
        statement: "The original rational expression lies in the checked interval throughout its ordered box under its original context.",
        premises: "Original `DomainGuard`, exact `Equality` and endpoint `Enclosure` relations, with `NonZero` for each divisor; all operands are checked before any simplification.",
    },
    crate::emptiness::LemmaSchema {
        name: "enclosure.root-grid",
        label: "thm:cas:enclosure-root-grid",
        version: 1,
        standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
        statement: "The nonnegative square root lies between the checked dyadic endpoints; nested instances enclose fourth roots and rational operand steps retain their own inclusion law.",
        premises: "Original `DomainGuard`, `NonNegative` radicand, `IntegerRange` for the grid, exact `Equality` for scaling and operand `Enclosure`; every rational operand step is independently checked.",
    },
];

/// Exhaustive implemented-law dispatcher; analytic families have no accepting arm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnclosureLaw {
    /// Exact rational endpoint relations and outward coarsening.
    RationalInclusion,
    /// Zero-capable square-root grid relations, composed with checked rational operands.
    RootGrid,
}

impl EnclosureLaw {
    /// Registered schema name, semantic version, premises and standing.
    #[must_use]
    pub const fn formal_schema(self) -> &'static crate::emptiness::LemmaSchema {
        match self {
            Self::RationalInclusion => &ENCLOSURE_SCHEMAS[0],
            Self::RootGrid => &ENCLOSURE_SCHEMAS[1],
        }
    }
}

/// Derive a fixed-size grammar summary at ordinary original-expression admission.
pub(crate) fn supported_shape(
    state: &crate::graph::State,
    operator: &crate::model::Operator,
    operands: &[crate::graph::LogicalId],
) -> Result<bool, crate::Fault> {
    use crate::model::Operator;
    let arity = match operator {
        Operator::Rational(_) | Operator::Free(_) => 0,
        Operator::GeneralPower(crate::model::Branch::StrictReal)
            if operands.len() == 2
                && matches!(&state.expression(operands[1])?.operator, Operator::Rational(value) if value.is_half()) =>
        {
            2
        }
        Operator::Neg => 1,
        Operator::Add | Operator::Sub | Operator::Mul | Operator::Div => 2,
        _ => return Ok(false),
    };
    if operands.len() != arity {
        return Ok(false);
    }
    for id in operands {
        if !state.expression(*id)?.supported_enclosure {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Propagate the root-law family without walking the expression closure at preflight.
pub(crate) fn has_roots(
    state: &crate::graph::State,
    operator: &crate::model::Operator,
    operands: &[crate::graph::LogicalId],
) -> Result<bool, crate::Fault> {
    if !supported_shape(state, operator, operands)? {
        return Ok(false);
    }
    if matches!(operator, crate::model::Operator::GeneralPower(_)) {
        return Ok(true);
    }
    for id in operands {
        if state.expression(*id)?.enclosure_roots {
            return Ok(true);
        }
    }
    Ok(false)
}
