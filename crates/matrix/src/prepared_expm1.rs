// SPDX-License-Identifier: AGPL-3.0-only
//! Prepare exact authority once, then freeze its graph for native binary64 reuse.

use nrz_bindings::{
    Budget,
    readings::algebra::{
        Cas, Fault, Location, Rational,
        enclosure::prepared_expm1::{Counts, Outcome, Policy, Prepared, Request},
        limits::BudgetKind,
        syntax::Operator,
    },
};
use nrz_scalar::{
    ArithmeticContext, Expm1Failure, Expm1Input, Expm1Output, Expm1Parameters, Expm1Program,
};
use std::cmp::Ordering;

#[path = "prepared_exp.rs"]
mod exp;
pub use exp::{ExpEvaluation, ExpPreparationPolicy, PreparedExp, PreparedExpError};

#[path = "prepared_sigmoid.rs"]
mod sigmoid;
pub use sigmoid::{
    PreparedSigmoid, PreparedSigmoidError, SigmoidEvaluation, SigmoidPreparationPolicy,
};

/// Shared preparation and admission accounting location.
const AT: Location = Location::new("prepared expm1 lowering");

/// Admission preserves exact resource causes and separate native refusals.
#[derive(Debug, thiserror::Error)]
pub enum PreparedExpm1Error {
    /// Exact preparation, correspondence or cumulative resource admission failed.
    #[error("expm1 preparation: {0}")]
    Exact(#[from] Fault),
    /// The bounded exact preparation did not certify the target.
    #[error("expm1 target not certifiable under preparation policy")]
    TargetNotCertifiable,
    /// The requested represented execution domain is not inside the original claim.
    #[error("expm1 native domain does not match original claim")]
    Domain,
    /// Structural, arithmetic or numerical application refusal.
    #[error("expm1 native program: {0}")]
    Native(#[from] Expm1Failure),
}

/// A scalar-level native plan retaining the exact authority that prepared it.
///
/// Construction calls `Cas::prepare_expm1` itself and takes that returned plan.
/// The immutable borrow prevents mutation of the graph for this owner's entire
/// lifetime. There is no admission of a separately supplied `Prepared`, no new
/// CAS authentication API, and no region, binding or masked-application authority.
/// The admission and batch preludes deliberately use `Cas::with_retained_graph`:
/// only CAS can distinguish a ledger's existing registration from a fresh owner.
/// This bookkeeping exception never reaches the native cell loop.
#[derive(Debug)]
pub struct PreparedExpm1<'a> {
    /// Frozen producing graph; never read by the numerical execution loop.
    graph: &'a Cas,
    /// Complete original claim and its checked parameterized schedule.
    authority: Box<Prepared>,
    /// Fixed native coefficients and export parameters, admitted once.
    program: Expm1Program,
    /// Logical owned bytes including the returned exact evidence and native program.
    bytes: u64,
}

/// Native readings attached to the original shared authority, without a new certificate.
#[derive(Clone, Copy, Debug)]
pub struct Expm1Evaluation<'a> {
    /// Complete finite numerical readings from this admitted application.
    output: Expm1Output,
    /// Borrowing the plan's evidence also keeps its frozen graph owner alive.
    authority: &'a Prepared,
}

impl Expm1Evaluation<'_> {
    /// Numerical readings; an extracted copy is data, not a new exact authority.
    #[must_use]
    pub const fn reading(&self) -> Expm1Output {
        self.output
    }

    /// Original operation, domain, target and checked import/restoration schedule.
    #[must_use]
    pub const fn authority(&self) -> &Prepared {
        self.authority
    }
}

impl<'a> PreparedExpm1<'a> {
    /// Prepare authority and lower a represented subdomain before freezing the graph.
    ///
    /// `domain` names binary64 endpoints inside the original exact claim. Native
    /// operands denote exactly their represented values, so nearest import is the
    /// identity. Other input representations require their own discharged import
    /// bound; `NumericLiteral` containment is not an implicit import permission.
    /// # Errors
    /// Preserves exact faults and refuses unavailable targets, domain mismatch,
    /// unsupported arithmetic or malformed native parameters.
    pub fn new(
        cas: &'a mut Cas,
        request: &Request,
        domain: [f64; 2],
        policy: Policy,
        context: &ArithmeticContext,
        budget: &mut Budget,
        counts: &mut Counts,
    ) -> Result<Self, PreparedExpm1Error> {
        let parts = owned_parts(
            cas,
            request,
            domain,
            policy,
            context,
            budget,
            counts,
            size_of::<Self>() as u64,
        )?;
        Ok(Self {
            graph: cas,
            bytes: parts.evidence.saturating_add(size_of::<Self>() as u64),
            authority: parts.authority,
            program: parts.program,
        })
    }

    /// Complete original claim and checked schedule retained by this immutable owner.
    #[must_use]
    pub fn authority(&self) -> &Prepared {
        &self.authority
    }

    /// Logical new ownership, excluding the separately retained producing graph.
    #[must_use]
    pub const fn owned_bytes(&self) -> u64 {
        self.bytes
    }

    /// Evaluate a new represented point or box without any exact operation.
    /// # Errors
    /// Preserves native domain, target, environment and below-normal refusals.
    pub fn evaluate(
        &self,
        input: Expm1Input,
        context: &ArithmeticContext,
    ) -> Result<Expm1Evaluation<'_>, Expm1Failure> {
        self.program
            .evaluate(input, context)
            .map(|output| Expm1Evaluation {
                output,
                authority: &self.authority,
            })
    }

    /// Prepay one bounded input inventory and return each cell's native outcome.
    ///
    /// The prelude charges input inventory traversal before any cell is read.
    /// The loop itself uses no exact Work, arithmetic, CAS call or certificate.
    /// # Errors
    /// Refuses count, byte, allocation or cumulative Work exhaustion before traversal.
    pub fn evaluate_batch(
        &self,
        inputs: &[Expm1Input],
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<Vec<Result<Expm1Evaluation<'_>, Expm1Failure>>, Fault> {
        budget.with_work_phase("prepared-expm1", "nrz.prelude", |budget| {
            let count = inputs.len() as u64;
            budget.check(BudgetKind::MatrixEntries, count, AT)?;
            let output = count
                .checked_mul(size_of::<Result<Expm1Evaluation<'_>, Expm1Failure>>() as u64)
                .ok_or_else(|| Fault::capacity(BudgetKind::OutputBytes, AT))?;
            let input = count
                .checked_mul(size_of::<Expm1Input>() as u64)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
            budget.check(BudgetKind::OutputBytes, output, AT)?;
            let retained = self.bytes.saturating_add(input).saturating_add(output);
            self.graph.with_retained_graph(budget, |budget| {
                budget.with_retained_payload(retained, AT, |budget| {
                    budget.work(count, AT)?;
                    let mut results = Vec::new();
                    results
                        .try_reserve_exact(inputs.len())
                        .map_err(|_| Fault::allocation(AT))?;
                    for &input in inputs {
                        results.push(self.evaluate(input, context));
                    }
                    Ok(results)
                })
            })
        })
    }
}

/// Provenance-bound owned parts, moved into either borrowing or encapsulating ownership.
#[derive(Debug)]
pub(crate) struct OwnedParts {
    /// The actual authority returned by this preparation, never an admitted substitute.
    pub(crate) authority: Box<Prepared>,
    /// Inline fixed-capacity native program.
    pub(crate) program: Expm1Program,
    /// Conservative exact-evidence allowance, excluding graph and owner headers.
    pub(crate) evidence: u64,
}
impl OwnedParts {
    /// Native reuse borrows its paired authority without entering the graph.
    pub(crate) fn evaluate(
        &self,
        input: Expm1Input,
        context: &ArithmeticContext,
    ) -> Result<Expm1Evaluation<'_>, Expm1Failure> {
        self.program
            .evaluate(input, context)
            .map(|output| Expm1Evaluation {
                output,
                authority: &self.authority,
            })
    }
}
/// Only preparation can produce owned authority/program parts; the mutable borrow ends here.
#[expect(
    clippy::too_many_arguments,
    reason = "the shared prelude preserves the borrowed API and its header allowance"
)]
pub(crate) fn owned_parts(
    cas: &mut Cas,
    request: &Request,
    domain: [f64; 2],
    policy: Policy,
    context: &ArithmeticContext,
    budget: &mut Budget,
    counts: &mut Counts,
    header_bytes: u64,
) -> Result<OwnedParts, PreparedExpm1Error> {
    let Outcome::Ready(authority) = cas.prepare_expm1(request, policy, budget, counts)? else {
        return Err(PreparedExpm1Error::TargetNotCertifiable);
    };
    budget.with_work_phase("prepared-expm1", "nrz.lower", |budget| {
        let bytes = inventory(&authority, budget)?.saturating_add(header_bytes);
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        let retained = bytes.saturating_add(size_of::<[f64; 128]>() as u64);
        let program = cas.with_retained_graph(budget, |budget| {
            budget.with_retained_payload(retained, AT, |budget| {
                correspondence(cas, authority.request(), domain, budget)?;
                let proof = authority.proof();
                let mut coefficients = [0.0; 128];
                // Inventory admission prepaid all coefficient reads, before this walk.
                {
                    let _import = tracing::trace_span!(
                        "nrz.import",
                        family = "prepared-expm1-coefficients",
                        revision = 1_u64
                    )
                    .entered();
                    for (slot, value) in coefficients.iter_mut().zip(&proof.coefficients) {
                        *slot = coefficient(value.significand, value.exponent)?;
                    }
                }
                let last = proof.stages.last().ok_or(Expm1Failure::MalformedPlan)?;
                let absolute = last.error.add(&proof.absolute_tail, budget)?;
                let absolute_error = outward_power(&absolute, budget)?;
                let parameters = Expm1Parameters {
                    domain,
                    bits: authority.request().bits,
                    radius_bits: proof.radius_bits,
                    absolute_error,
                    environment: authority.request().environment,
                };
                let program =
                    Expm1Program::new(&coefficients[..proof.degree as usize], parameters, context)?;
                tracing::event!(name: "nrz.construct", target: "nrz::work", tracing::Level::TRACE,
                    family = "prepared-expm1", revision = 1_u64,
                    operation = "native program", count = 1_u64);
                Ok::<_, PreparedExpm1Error>(program)
            })
        })?;
        Ok(OwnedParts {
            authority,
            program,
            evidence: bytes - header_bytes,
        })
    })
}

/// Inventory ceilings and its traversal tariff precede every supplied-record walk.
fn inventory(p: &Prepared, b: &mut Budget) -> Result<u64, Fault> {
    let proof = p.proof();
    b.check(BudgetKind::PolynomialDegree, u64::from(proof.degree), AT)?;
    b.check(BudgetKind::ProofNodes, proof.stages.len() as u64, AT)?;
    b.check(
        BudgetKind::MatrixEntries,
        proof.coefficients.len() as u64,
        AT,
    )?;
    b.check(BudgetKind::WitnessBytes, proof.scope.len() as u64, AT)?;
    let records = proof.coefficients.len() as u64
        + 2 * proof.stages.len() as u64
        + 6
        + 2 * p.request().original.coordinates.len() as u64;
    b.work(records, AT)?;
    let number = |q: &Rational| {
        let (n, d) = q.component_bits();
        (n.div_ceil(8) + d.div_ceil(8))
            .saturating_mul(4)
            .saturating_add(128)
    };
    let mut bytes = (proof.scope.len() as u64)
        .saturating_add(p.statement_key().len() as u64)
        .saturating_add(1024)
        .saturating_add(32 * proof.coefficients.len() as u64);
    for q in [
        &proof.absolute_tail,
        &proof.relative_tail,
        &proof.import_ratio,
        &proof.central_ratio,
        &proof.endpoint_ratio,
        &proof.restoration_error,
    ] {
        bytes = bytes.saturating_add(number(q));
    }
    for stage in &proof.stages {
        bytes = bytes
            .saturating_add(number(&stage.magnitude))
            .saturating_add(number(&stage.error));
    }
    for coordinate in &p.request().original.coordinates {
        bytes = bytes
            .saturating_add(256)
            .saturating_add(number(coordinate.interval.lower()))
            .saturating_add(number(coordinate.interval.upper()));
    }
    Ok(bytes)
}

/// Preserve original endpoint association through exact represented-value comparisons.
fn correspondence(
    cas: &Cas,
    request: &Request,
    domain: [f64; 2],
    b: &mut Budget,
) -> Result<(), PreparedExpm1Error> {
    let valid = domain.iter().all(|x| x.is_finite())
        && domain[0] >= -2.0
        && domain[1] <= 0.0
        && domain[0] <= domain[1];
    if !valid {
        return Err(PreparedExpm1Error::Domain);
    }
    let lower = represented(domain[0], b)?;
    let upper = represented(domain[1], b)?;
    if let [coordinate] = request.original.coordinates.as_slice() {
        if lower.compare(coordinate.interval.lower(), b)? == Ordering::Less
            || upper.compare(coordinate.interval.upper(), b)? == Ordering::Greater
        {
            return Err(PreparedExpm1Error::Domain);
        }
    } else {
        let root = cas.operands(&request.original.target, b)?;
        let exponential = root.first().ok_or(PreparedExpm1Error::Domain)?;
        let operands = cas.operands(exponential, b)?;
        let argument = operands.first().ok_or(PreparedExpm1Error::Domain)?;
        let (Operator::Rational(value), _) = cas.expression(argument)? else {
            return Err(PreparedExpm1Error::Domain);
        };
        if &lower != value || &upper != value {
            return Err(PreparedExpm1Error::Domain);
        }
    }
    Ok(())
}

/// Exact binary64 decoding is used only during preparation, never by evaluation.
fn represented(value: f64, b: &mut Budget) -> Result<Rational, Fault> {
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
        b,
    )?
    .mul(&Rational::from_ratio(2, 1, b)?.pow(exponent, b)?, b)
}

/// Checked dyadics have at most 53 significand bits and an admitted exponent.
#[expect(
    clippy::cast_precision_loss,
    reason = "the checked significand has at most 53 bits"
)]
fn coefficient(significand: u64, exponent: i32) -> Result<f64, Expm1Failure> {
    if significand == 0 || significand > (1_u64 << 53) || !(-1022..=0).contains(&exponent) {
        return Err(Expm1Failure::MalformedPlan);
    }
    let field = u64::try_from(1023 + exponent).map_err(|_| Expm1Failure::MalformedPlan)?;
    Ok(significand as f64 * f64::from_bits(field << 52))
}

/// Choose an exact outward power of two by bounded comparisons, only during preparation.
fn outward_power(bound: &Rational, b: &mut Budget) -> Result<f64, PreparedExpm1Error> {
    let mut value = 32.0;
    if bound.is_negative() || bound.compare(&represented(value, b)?, b)? == Ordering::Greater {
        return Err(Expm1Failure::MalformedPlan.into());
    }
    if bound.is_zero() {
        return Ok(0.0);
    }
    for _ in 0..1080 {
        let smaller = value / 2.0;
        if smaller == 0.0 || bound.compare(&represented(smaller, b)?, b)? == Ordering::Greater {
            return Ok(value);
        }
        value = smaller;
    }
    Err(Expm1Failure::MalformedPlan.into())
}
