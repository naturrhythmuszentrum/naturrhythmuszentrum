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
mod exp;
mod interval;
mod log;
mod model;
mod planning;
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
    exp::bytes(&value.certificate.exponentials)
        .saturating_add(log::bytes(&value.certificate.logarithms))
        .saturating_add(
            value.certificate.records.iter().fold(
                u64::try_from(value.certificate.scope.len())
                    .unwrap_or(u64::MAX)
                    .saturating_add(request_bytes(&value.request))
                    .saturating_add(interval_bytes(&value.interval))
                    .saturating_add(512),
                |size, interval| size.saturating_add(interval_bytes(interval)),
            ),
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
    // A log record adds twelve series intervals and four rational reduction fields;
    // sixteen interval slots also cover their fixed metadata and the result record.
    let multiplier = if cas
        .state
        .expression(cas.checked(&request.target)?)?
        .enclosure_exp
        .present
    {
        24
    } else if cas
        .state
        .expression(cas.checked(&request.target)?)?
        .enclosure_logs
    {
        16
    } else {
        1
    };
    Ok(count
        .saturating_mul(multiplier)
        .saturating_add(1)
        .saturating_mul(endpoint)
        .saturating_add(u64::try_from(scope_len).unwrap_or(u64::MAX))
        .saturating_add(request_bytes(request))
        .saturating_add(512))
}

/// Size the supplied transcript before checking or copying any of its records.
fn certificate_bytes(value: &EnclosureCertificate) -> u64 {
    exp::bytes(&value.exponentials)
        .saturating_add(log::bytes(&value.logarithms))
        .saturating_add(
            value.records.iter().fold(
                u64::try_from(value.scope.len())
                    .unwrap_or(u64::MAX)
                    .saturating_add(128),
                |bytes, record| bytes.saturating_add(interval_bytes(record)),
            ),
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

pub use exp::{ExpEndpoint, ExpEvidence, ExpSeries};
pub use interval::Interval;
pub use log::{LogEndpoint, LogEvidence, LogSeries};
pub use model::{
    CheckedEnclosure, CoordinateBound, EnclosureCertificate, EnclosureGoal, EnclosureOutcome,
    EnclosureReport, EnclosureRequest, RefinementPolicy, UndecidedReason,
};

/// Closed formal register; no unimplemented analytic schema grants authority.
pub const ENCLOSURE_SCHEMAS: [crate::emptiness::LemmaSchema; 4] = [
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
    crate::emptiness::LemmaSchema {
        name: "enclosure.log-atanh",
        label: "thm:cas:enclosure-log-atanh",
        version: 1,
        standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
        statement: "The positive target reduced as x=2^k*m is enclosed by finite atanh sums and geometric tails, including the one-third instance for log two. The law includes the analytic identity, convergence, monotonicity and tail inequality; finite tests do not prove those analytic premises.",
        premises: "Original `DomainGuard`, `StrictSign` for the argument, exact `Equality` for reduction, `IntegerRange` for the exponent and term count, and `Enclosure` for reconstructed partial sums and log two.",
    },
    crate::emptiness::LemmaSchema {
        name: "enclosure.exp-taylor",
        label: "thm:cas:enclosure-exp-taylor",
        version: 1,
        standing: crate::proof::Standing::Lemma(crate::proof::LemmaProvenance::CasChecked),
        statement: "The real exponential lies in the checked Taylor-tail enclosure after exact binary reduction, interval squarings and negative-argument reciprocal. The analytic series, addition law and monotonicity belong to this law; finite samples do not prove them.",
        premises: "Original `DomainGuard`, exact reduction `Equality`, `NonNegative` reduced argument, `IntegerRange` for reduction and degree, `StrictSign` for the tail denominator, and `Enclosure` for every recurrence and reconstruction.",
    },
];

/// Exhaustive implemented-law dispatcher; analytic families have no accepting arm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnclosureLaw {
    /// Exact rational endpoint relations and outward coarsening.
    RationalInclusion,
    /// Zero-capable square-root grid relations, composed with checked rational operands.
    RootGrid,
    /// Finite atanh tails over positive inputs, with checked operand laws.
    LogAtanh,
    /// Checked Taylor tails, binary squaring and negative-argument reciprocals.
    ExpTaylor,
}

impl EnclosureLaw {
    /// Registered schema name, semantic version, premises and standing.
    #[must_use]
    pub const fn formal_schema(self) -> &'static crate::emptiness::LemmaSchema {
        match self {
            Self::RationalInclusion => &ENCLOSURE_SCHEMAS[0],
            Self::RootGrid => &ENCLOSURE_SCHEMAS[1],
            Self::LogAtanh => &ENCLOSURE_SCHEMAS[2],
            Self::ExpTaylor => &ENCLOSURE_SCHEMAS[3],
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
        Operator::Neg | Operator::NaturalLog | Operator::Exponential => 1,
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

/// Fixed-size log metadata avoids an unpaid closure walk during sizing preflight.
pub(crate) fn log_metadata(
    state: &crate::graph::State,
    operator: &crate::model::Operator,
    operands: &[crate::graph::LogicalId],
) -> Result<(bool, Option<u32>), crate::Fault> {
    use crate::model::Operator;
    if !supported_shape(state, operator, operands)? {
        return Ok((false, None));
    }
    if matches!(operator, Operator::NaturalLog) {
        let child = state.expression(operands[0])?;
        let bound = if let Operator::Rational(value) = &child.operator {
            let (n, d) = value.component_bits();
            u32::try_from(n.abs_diff(d))
                .ok()
                .and_then(|k| k.checked_add(1))
        } else {
            None
        };
        return Ok((true, bound));
    }
    let mut found = false;
    let mut bound = Some(0);
    for id in operands {
        let child = state.expression(*id)?;
        if child.enclosure_logs {
            found = true;
            bound = bound.zip(child.enclosure_log_bound).map(|(a, b)| a.max(b));
        }
    }
    Ok((found, bound))
}

/// Resident capability summary separates family presence from a known reduction bound.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct AnalyticShape {
    /// Whether the original expression contains this implemented analytic family.
    pub(crate) present: bool,
    /// A conservative reduction bound, absent for an unevaluated composite operand.
    pub(crate) reduction: Option<u32>,
}

/// Fixed-size exponential metadata avoids an unpaid closure walk during sizing preflight.
pub(crate) fn exp_metadata(
    state: &crate::graph::State,
    operator: &crate::model::Operator,
    operands: &[crate::graph::LogicalId],
) -> Result<AnalyticShape, crate::Fault> {
    use crate::model::Operator;
    if !supported_shape(state, operator, operands)? {
        return Ok(AnalyticShape::default());
    }
    if matches!(operator, Operator::Exponential) {
        let child = state.expression(operands[0])?;
        let bound = if let Operator::Rational(value) = &child.operator {
            let (n, d) = value.component_bits();
            u32::try_from(n.saturating_add(2).saturating_sub(d)).ok()
        } else {
            None
        };
        return Ok(AnalyticShape {
            present: true,
            reduction: bound,
        });
    }
    let mut found = false;
    let mut bound = Some(0);
    for id in operands {
        let child = state.expression(*id)?;
        if child.enclosure_exp.present {
            found = true;
            bound = bound
                .zip(child.enclosure_exp.reduction)
                .map(|(a, b)| a.max(b));
        }
    }
    Ok(AnalyticShape {
        present: found,
        reduction: bound,
    })
}
