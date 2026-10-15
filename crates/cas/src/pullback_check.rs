// SPDX-License-Identifier: AGPL-3.0-only
//! Independent zero completeness, simplicity, endpoint and absolute-Jacobian checks.

use crate::budget::{Budget, BudgetKind};
use crate::case_syntax::invalid;
use crate::distribution_model::DistributionStatement;
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, size};
use crate::model::Operator;
use crate::number::Rational;
use crate::proof_model::{ApplicationDraft, Certificate, CheckedApplication, Statement};
use crate::pullback_geometry::{AT, interval, map_error};
use crate::pullback_model::{PullbackCertificate, PullbackMode, PullbackValue};
use crate::rule::Primitive;
use crate::univariate::Polynomial;

impl Cas {
    /// Borrow the checked complete roots, slopes and inverse absolute Jacobians.
    ///
    /// # Errors
    /// Refuses foreign witnesses and proofs belonging to another schema.
    pub fn pullback_certificate(
        &self,
        witness: &crate::proof_model::VerifiedWitness,
    ) -> Result<&PullbackCertificate, Fault> {
        let crate::graph::Vertex::Proof(application) =
            self.state.node(self.checked(witness.proof())?)?
        else {
            return Err(invalid());
        };
        let crate::graph::Vertex::ScopedApplication(application) = self.state.node(*application)?
        else {
            return Err(invalid());
        };
        let Certificate::Pullback(value) = &application.certificate else {
            return Err(invalid());
        };
        Ok(value)
    }
}

/// The proposed distribution must be exactly the delta pullback certified by this rule.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let (Statement::Distribution(statement), Certificate::Pullback(evidence)) =
        (&draft.conclusion, &draft.certificate)
    else {
        return Err(invalid());
    };
    let DistributionStatement::Pullback(claim) = statement.as_ref() else {
        return Err(invalid());
    };
    if !premises.is_empty() {
        return Err(invalid());
    }
    measure(evidence, budget)?;
    let support = source(cas, claim, budget)?;
    let coefficients =
        crate::pullback_polynomial::coefficients(cas, &claim.map, &claim.target_space, budget)?;
    if coefficients.last() != Some(&evidence.leading) {
        return Err(incomplete());
    }
    match (draft.rule, claim.mode) {
        (Primitive::DistAffinePullback, PullbackMode::Affine) => {
            if evidence.zeros.len() != 1 {
                return Err(incomplete());
            }
            crate::pullback_geometry::affine(cas, claim, &coefficients, budget)?;
        }
        (Primitive::DistSimpleZeroPullback, PullbackMode::SimpleZeros) => {
            let source_interval = interval(cas, &claim.source_space, budget)?;
            if claim.inverse.is_some()
                || !support.is_zero()
                || source_interval.lower.is_some()
                || source_interval.upper.is_some()
            {
                return Err(map_error());
            }
        }
        _ => return Err(invalid()),
    }
    zeros(&coefficients, &support, evidence, budget)?;
    if claim.mode == PullbackMode::SimpleZeros {
        factorization(&coefficients, evidence, budget)?;
    }
    crate::pullback_output::check(cas, claim, evidence, budget)
}

/// A source delta retains its exact support point and declared original space.
pub(crate) fn source(
    cas: &Cas,
    claim: &PullbackValue,
    budget: &mut Budget,
) -> Result<Rational, Fault> {
    crate::distribution_check::membership(
        cas,
        cas.checked(&claim.source)?,
        &claim.source_space,
        &[],
        budget,
    )?;
    let input = cas.state.expression(cas.checked(&claim.source)?)?;
    if input.operator != (Operator::Delta { order: 0 }) {
        return Err(invalid());
    }
    let [point] = input.operands.as_slice() else {
        return Err(invalid());
    };
    Ok(crate::distribution_check::rational(cas, *point)?.clone())
}

/// Each listed point is a simple preimage and its positive Jacobian is the reciprocal absolute slope.
fn zeros(
    coefficients: &[Rational],
    support: &Rational,
    evidence: &PullbackCertificate,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let derivative = Polynomial::new(coefficients, budget)?.derivative(budget)?;
    let mut previous: Option<&Rational> = None;
    for zero in &evidence.zeros {
        if previous.is_some_and(|point| *point == zero.point) {
            return Err(multiple());
        }
        if let Some(point) = previous
            && !point.compare(&zero.point, budget)?.is_lt()
        {
            return Err(incomplete());
        }
        previous = Some(&zero.point);
        if crate::split_polynomial::endpoint(coefficients, &zero.point, budget)? != *support {
            return Err(incomplete());
        }
        let slope =
            crate::split_polynomial::endpoint(&derivative.coefficients, &zero.point, budget)?;
        if slope.is_zero() {
            return Err(multiple());
        }
        let absolute = if slope.is_negative() {
            slope.negated(budget)?
        } else {
            slope.clone()
        };
        if slope != zero.slope
            || zero.inverse_jacobian.is_negative()
            || zero.inverse_jacobian.mul(&absolute, budget)? != Rational::one()
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/// A complete product identity proves exhaustive rational roots without a root-finding oracle.
fn factorization(
    coefficients: &[Rational],
    evidence: &PullbackCertificate,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut product = Polynomial::one(budget)?;
    for zero in &evidence.zeros {
        let factor = Polynomial::new(&[zero.point.negated(budget)?, Rational::one()], budget)?;
        product = product.mul(&factor, budget)?;
    }
    if product.scale(&evidence.leading, budget)?.coefficients != coefficients {
        return Err(incomplete());
    }
    Ok(())
}

/// Every supplied root is charged before coefficient copies, including roots outside the interval.
pub(crate) fn measure(value: &PullbackCertificate, budget: &mut Budget) -> Result<(), Fault> {
    budget.specialized(
        BudgetKind::PullbackZeros,
        size(value.zeros.len(), AT)?,
        false,
        AT,
    )?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        size(value.zeros.len(), AT)?.saturating_mul(512),
        AT,
    )?;
    value.leading.check(budget, AT)?;
    for zero in &value.zeros {
        for number in [&zero.point, &zero.slope, &zero.inverse_jacobian] {
            number.check(budget, AT)?;
        }
        budget.work(512, AT)?;
    }
    Ok(())
}

/// Repeated or nonsimple roots do not satisfy the delta composition theorem.
pub(crate) const fn multiple() -> Fault {
    Fault::invalid(InvalidCode::PullbackMultipleZero, AT)
}

/// An omitted or extraneous zero cannot establish exhaustive support.
pub(crate) const fn incomplete() -> Fault {
    Fault::invalid(InvalidCode::PullbackIncompleteZeros, AT)
}
