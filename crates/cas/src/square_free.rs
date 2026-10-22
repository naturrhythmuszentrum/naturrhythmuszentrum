// SPDX-License-Identifier: AGPL-3.0-only
//! Untrusted characteristic-zero multiplicity-layer production over exact rationals.
//!
//! # Certified square-free layers
//!
//! `SquareFree` reconstructs a nonzero univariate polynomial over Q from scalar rational/real syntax, normalizes its leading coefficient, differentiates its coefficient vector and computes exact extended Euclidean certificates. Successive gcd/quotient layers follow the characteristic-zero recurrence in `docs/design/cas-design.md`. Each certificate states both divisibilities and a Bézout combination; the checker verifies those coefficient identities without running the producer's Euclidean loop. It also verifies the recurrence, monicity, nonconstant factors, strictly increasing positive multiplicities, each factor's derivative gcd, every pairwise gcd, the complete product identity and the exact displayed endpoint factors. Layers need not be irreducible; x²+1 remains a valid single layer over Q.
//!
//! `SquareFreeCertificate`, `SquareFreeLayer` and `GcdCertificate` are untrusted public data with ascending-degree canonical rational vectors. `ApplicationDraft::certificate` uses `Certificate::SquareFree(Box<SquareFreeCertificate>)` only for `poly.square-free` version 1, with logical weight 8; other primitives reject that payload. `square_free_certificate` borrows checked evidence from the witness's graph application. The typed application contains the complete certificate; no earlier wire layout is retained. Every decoded coefficient/vector and retained certificate is bounded before copying or arithmetic. The producer publishes the factor expression and its replayed proof in one transaction.
//!
//! A nonzero constant returns its scalar and no factors. Zero returns `Outcome::Unsupported` with `UnsupportedReason::ZeroPolynomialMultiplicity`; non-rational coefficients and expressions outside the univariate fragment have distinct reasons. Positive-characteristic and complex coefficient requests remain unsupported. Degree, coefficient/intermediate bits, arithmetic work, retained payload and witness bytes have explicit refusal paths. The degree-five fixture, a hand-authored Bézout certificate for x²+1, independent binomial examples, rational coefficients, endpoint/certificate tampering, canonical replay, byte determinism and exact witness-size boundary are acceptance tests. None of these claims infer roots or physical meaning.

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, LogicalId, size};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::multiplicity::{GcdCertificate, SquareFreeCertificate, SquareFreeLayer};
use crate::number::Rational;
use crate::producer::{Plan, step, term};
use crate::proof_model::VerifiedWitness;
use crate::rule::Primitive;
use crate::scalar_model::Relation;
use crate::univariate::Polynomial;

/// Polynomial production location.
const AT: Location = Location::new("square-free generation");

/// Reject non-polynomial sorts/scopes and reconstruct a bounded exact coefficient vector.
pub(crate) fn coefficients(
    cas: &Cas,
    context: LogicalId,
    input: LogicalId,
    variable: &str,
    budget: &mut Budget,
) -> Result<Polynomial, Fault> {
    let environment = &cas.state.context(context)?.environment;
    budget.work(size(cas.state.context(context)?.header.len(), AT)?, AT)?;
    let source = cas.state.expression(input)?;
    if !matches!(environment.field, Field::Rational | Field::Real)
        || source.sort != Sort::Value(AtomicSort::Scalar(environment.field.clone()))
        || !source.scope.is_empty()
        || !environment
            .declarations
            .iter()
            .any(|declaration| declaration.key == variable && declaration.sort == source.sort)
    {
        return Err(Fault::invalid(InvalidCode::PolynomialFragment, AT));
    }
    let values = crate::polynomial::univariate(&cas.state, input, variable, budget).map_err(
        |mut fault| {
            if fault.kind == crate::error::FaultKind::Invalid(InvalidCode::UnsupportedFragment) {
                fault.kind = crate::error::FaultKind::Invalid(InvalidCode::PolynomialFragment);
            }
            fault
        },
    )?;
    Polynomial::new(&values, budget)
}

/// Generate a complete untrusted certificate, reserving the existing graph while doing arithmetic.
pub(crate) fn produce(
    cas: &mut Cas,
    context: LogicalId,
    input: LogicalId,
    variable: &str,
    budget: &mut Budget,
) -> Result<Plan, Fault> {
    let previous = budget.reserve_temporary(cas.counts().payload_bytes, AT)?;
    let result = (|| {
        let value = coefficients(cas, context, input, variable, budget)?;
        generate(&value, budget)
    })();
    budget.restore_limits(previous);
    let certificate = result?;
    let retained = certificate.payload(budget)?;
    let previous = budget.reserve_temporary(retained, AT)?;
    let output = materialize(cas, input, variable, &certificate, budget);
    budget.restore_limits(previous);
    let output = output?;
    let mut proposal = step(
        cas,
        Primitive::SquareFree,
        Relation::SquareFree {
            variable: variable.into(),
            field: Field::Rational,
        },
        input,
        output,
        Vec::new(),
    );
    proposal.certificate = Some(certificate);
    let mut plan = Plan::default();
    let _root = plan.push(proposal, budget)?;
    Ok(plan)
}

/// Generate successive gcd layers; a nonzero constant has monic part one and no layers.
fn generate(value: &Polynomial, budget: &mut Budget) -> Result<SquareFreeCertificate, Fault> {
    if value.is_zero() {
        return Err(Fault::invalid(InvalidCode::ZeroPolynomial, AT));
    }
    value.prepay(budget)?;
    let scalar = value.leading()?.clone();
    let inverse = Rational::one().div(&scalar, budget)?;
    let monic = value.scale(&inverse, budget)?;
    let derivative = monic.derivative(budget)?;
    let initial_gcd = crate::euclidean::generate(&monic, &derivative, budget)?;
    let mut common = Polynomial::new(&initial_gcd.gcd, budget)?;
    let mut remaining = Polynomial::new(&initial_gcd.left_quotient, budget)?;
    let mut certificate = SquareFreeCertificate {
        scalar,
        initial_gcd,
        rounds: Vec::new(),
        layers: Vec::new(),
        square_free: Vec::new(),
        coprime: Vec::new(),
    };
    let mut multiplicity = 0_u64;
    while !remaining.is_one() {
        multiplicity = multiplicity
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialDegree, AT))?;
        budget.check(BudgetKind::PolynomialDegree, multiplicity, AT)?;
        if multiplicity > size(monic.degree().ok_or_else(invalid)?, AT)? {
            return Err(invalid());
        }
        let evidence = retained_gcd(&certificate, &remaining, &common, budget)?;
        let factor = Polynomial::new(&evidence.left_quotient, budget)?;
        remaining = Polynomial::new(&evidence.gcd, budget)?;
        common = Polynomial::new(&evidence.right_quotient, budget)?;
        if !factor.is_one() {
            certificate
                .layers
                .try_reserve(1)
                .map_err(|_| Fault::allocation(AT))?;
            certificate.layers.push(SquareFreeLayer {
                multiplicity,
                coefficients: factor.coefficients,
            });
        }
        certificate
            .rounds
            .try_reserve(1)
            .map_err(|_| Fault::allocation(AT))?;
        certificate.rounds.push(evidence);
        let _bytes = certificate.payload(budget)?;
    }
    if !common.is_one() {
        return Err(invalid());
    }
    certify_layers(&mut certificate, budget)?;
    Ok(certificate)
}

/// Reserve all evidence retained so far while constructing one more Euclidean certificate.
fn retained_gcd(
    certificate: &SquareFreeCertificate,
    left: &Polynomial,
    right: &Polynomial,
    budget: &mut Budget,
) -> Result<GcdCertificate, Fault> {
    let retained = certificate.payload(budget)?;
    let previous = budget.reserve_temporary(retained, AT)?;
    let result = crate::euclidean::generate(left, right, budget);
    budget.restore_limits(previous);
    result
}

/// Supply independent per-layer square-free and ordered pairwise coprimality certificates.
fn certify_layers(
    certificate: &mut SquareFreeCertificate,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for position in 0..certificate.layers.len() {
        let layer = certificate.layers.get(position).ok_or_else(invalid)?;
        let value = Polynomial::new(&layer.coefficients, budget)?;
        let derivative = value.derivative(budget)?;
        let evidence = retained_gcd(certificate, &value, &derivative, budget)?;
        certificate
            .square_free
            .try_reserve(1)
            .map_err(|_| Fault::allocation(AT))?;
        certificate.square_free.push(evidence);
        for later in position.checked_add(1).ok_or_else(invalid)?..certificate.layers.len() {
            let other = certificate.layers.get(later).ok_or_else(invalid)?;
            let other = Polynomial::new(&other.coefficients, budget)?;
            let evidence = retained_gcd(certificate, &value, &other, budget)?;
            certificate
                .coprime
                .try_reserve(1)
                .map_err(|_| Fault::allocation(AT))?;
            certificate.coprime.push(evidence);
            let _bytes = certificate.payload(budget)?;
        }
    }
    let _bytes = certificate.payload(budget)?;
    Ok(())
}

/// Retain an explicit ordered scalar product of positive integer powers of its monic factors.
fn materialize(
    cas: &mut Cas,
    input: LogicalId,
    variable: &str,
    certificate: &SquareFreeCertificate,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let mut output = term(
        cas,
        input,
        Operator::Rational(certificate.scalar.clone()),
        &[],
        budget,
    )?;
    for layer in &certificate.layers {
        let factor = crate::polynomial::materialize_univariate(
            cas,
            input,
            variable,
            &layer.coefficients,
            budget,
        )?;
        let exponent = i64::try_from(layer.multiplicity)
            .map_err(|_| Fault::capacity(BudgetKind::PolynomialDegree, AT))?;
        let power = term(
            cas,
            input,
            Operator::IntegerPower(exponent),
            &[factor],
            budget,
        )?;
        output = term(cas, input, Operator::Mul, &[output, power], budget)?;
    }
    Ok(output)
}

impl Cas {
    /// Borrow graph-owned checked multiplicity evidence without an unmetered certificate clone.
    ///
    /// # Errors
    /// Refuses foreign witnesses and witnesses for another mathematical relation.
    pub fn square_free_certificate(
        &self,
        witness: &VerifiedWitness,
    ) -> Result<&SquareFreeCertificate, Fault> {
        let value = crate::proof_storage::application(self, self.checked(witness.proof())?)?;
        value.certificate.as_ref().ok_or_else(invalid)
    }
}

/// Invalid producer intermediates cannot become checked conclusions.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Rule, AT)
}
