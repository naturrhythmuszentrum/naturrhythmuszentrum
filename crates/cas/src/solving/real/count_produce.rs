// SPDX-License-Identifier: AGPL-3.0-only
//! Generic exact Euclidean proposals for the independently replayed Real count checker.

use super::leaf::Leaf;
use crate::graph::{Handle, Reference};
use crate::model::{AtomicSort, Field};
use crate::proof_model::VerifiedWitness;
use crate::solving::coefficients::{
    arithmetic::Engine,
    model::{CoefficientFailure, Element, EvaluationScope},
    storage::{invalid, reserve, vector},
};
use crate::solving::towers::{dense::Dense, euclid, materialize};
use crate::solving::{
    CertificateExtensionUnit, CertificateRealRootCount, RootRelationData, SolveCertificate,
    SolveDraft, SolveReason, SolveSchema, SolveStatement, StatementRealRootCount, StatementUnit,
};

/// Arithmetic failure preserves its exact capability boundary rather than inventing a remainder.
fn complete<T>(value: Result<T, euclid::Pending>) -> Result<T, CoefficientFailure> {
    value.map_err(|_| CoefficientFailure::Unsupported(SolveReason::MissingUnitProof))
}

/// Complete finite arithmetic data before any graph-level count claim is proposed.
struct Chain {
    /// Exact derivative of the original polynomial.
    derivative: Dense,
    /// Actual monic derivative gcd with its computed Bezout identity.
    gcd: Dense,
    /// Square-free quotient of the original polynomial.
    support: Dense,
    /// Descending signed Sturm remainder chain.
    polynomials: Vec<Dense>,
    /// Gcd support identities followed by every signed-division quotient.
    quotients: Vec<Dense>,
}

/// Materialized chain handles and their earlier unit and infinity-sign premise slots.
type ChainEntries = (Vec<Handle>, Vec<u32>, Vec<u32>);

/// The existing exact extended gcd and division routines generate every quotient from the actual input.
fn chain(
    cas: &crate::graph::Cas,
    source: &Dense,
    scope: &EvaluationScope<'_>,
    budget: &mut crate::budget::Budget,
) -> Result<Chain, CoefficientFailure> {
    let derivative = crate::solving::towers::check_split::derivative(cas, source, budget)?;
    let bezout = complete(euclid::gcd(source, &derivative, &cas.state, scope, budget)?)?;
    let (support, _) = complete(euclid::divide(
        source,
        &bezout.gcd,
        &cas.state,
        scope,
        budget,
    )?)?;
    let (derivative_quotient, _) = complete(euclid::divide(
        &derivative,
        &bezout.gcd,
        &cas.state,
        scope,
        budget,
    )?)?;
    let mut polynomials = vector(4, budget)?;
    polynomials.push(support.copy(&cas.state, budget)?);
    polynomials.push(crate::solving::towers::check_split::derivative(
        cas, &support, budget,
    )?);
    let mut quotients = vector(6, budget)?;
    quotients.extend([derivative_quotient, bezout.first, bezout.second]);
    let engine = Engine {
        state: &cas.state,
        domain: &source.domain,
    };
    let negative = engine.neg(&engine.constant(true, budget)?, budget)?;
    loop {
        let count = polynomials.len();
        let previous = polynomials.get(count - 2).ok_or_else(invalid)?;
        let current = polynomials.get(count - 1).ok_or_else(invalid)?;
        let (quotient, remainder) = complete(euclid::divide(
            previous, current, &cas.state, scope, budget,
        )?)?;
        reserve(&mut quotients, 1, budget)?;
        quotients.push(quotient);
        if remainder.coefficients.is_empty() {
            break;
        }
        reserve(&mut polynomials, 1, budget)?;
        polynomials.push(remainder.scaled(&negative, 0, &cas.state, budget)?);
    }
    Ok(Chain {
        derivative,
        gcd: bezout.gcd,
        support,
        polynomials,
        quotients,
    })
}

impl Leaf<'_> {
    /// Fraction-field units retain an explicit reciprocal and complete auxiliary Bezout identity.
    fn leading_unit(
        &mut self,
        value: &Element,
        inverse: &Element,
    ) -> Result<u32, CoefficientFailure> {
        let engine = Engine {
            state: &self.cas.state,
            domain: &value.domain,
        };
        let one = Dense::one(&engine, self.budget)?;
        let zero = Dense::zero(&engine, self.budget)?;
        let modulus = one.scaled(
            &engine.constant(true, self.budget)?,
            1,
            &self.cas.state,
            self.budget,
        )?;
        let subject = one.scaled(value, 0, &self.cas.state, self.budget)?;
        let reciprocal = one.scaled(inverse, 0, &self.cas.state, self.budget)?;
        let element = materialize::value(self.cas, value, self.budget)?;
        let inverse = materialize::value(self.cas, inverse, self.budget)?;
        let modulus = materialize::polynomial(self.cas, &modulus, &[Field::Real], self.budget)?;
        let gcd = materialize::polynomial(self.cas, &one, &[Field::Real], self.budget)?;
        let subject = materialize::polynomial(self.cas, &subject, &[Field::Real], self.budget)?;
        let reciprocal =
            materialize::polynomial(self.cas, &reciprocal, &[Field::Real], self.budget)?;
        let zero = materialize::polynomial(self.cas, &zero, &[Field::Real], self.budget)?;
        let region = self.frame.region.clone();
        Ok(self.frame.push(
            SolveSchema::ExtensionUnit,
            SolveStatement::Unit(StatementUnit {
                value: element.clone(),
                inverse,
                domain: value.domain.clone(),
                region: region.clone(),
            }),
            SolveCertificate::ExtensionUnit(CertificateExtensionUnit {
                element,
                modulus: modulus.clone(),
                gcd,
                quotients: vec![subject, modulus],
                bezout: vec![reciprocal, zero],
                region,
            }),
            self.budget,
        )?)
    }

    /// Every chain leading unit is generated before its sign can be used at either infinity.
    fn chain_entries(
        &mut self,
        chain: &[Dense],
        premises: &[VerifiedWitness],
    ) -> Result<ChainEntries, CoefficientFailure> {
        let mut polynomials = vector(chain.len(), self.budget)?;
        let mut units = vector(chain.len(), self.budget)?;
        let mut signs = vector(chain.len().saturating_mul(2), self.budget)?;
        for polynomial in chain {
            let value = polynomial.coefficients.last().ok_or_else(invalid)?;
            let engine = Engine {
                state: &self.cas.state,
                domain: &value.domain,
            };
            let scope = EvaluationScope {
                context: &self.frame.context,
                region: &self.frame.region,
                premises,
                localization: &[],
            };
            let inverse = super::derive::inverse(&engine, value, &scope, self.budget)?;
            units.push(self.leading_unit(value, &inverse)?);
            signs.push(self.coefficient_sign(value)?.slot);
            polynomials.push(materialize::polynomial(
                self.cas,
                polynomial,
                &[Field::Real],
                self.budget,
            )?);
        }
        let second = crate::solving::coefficients::storage::copied(&signs, self.budget)?;
        signs.extend(second);
        Ok((polynomials, units, signs))
    }

    /// The count's relation is the complete normalized original polynomial in the original unknown.
    pub(crate) fn count_data(
        &mut self,
        polynomial: &Dense,
        premises: &[VerifiedWitness],
    ) -> Result<CertificateRealRootCount, CoefficientFailure> {
        self.mark("real-sturm-chain")?;
        let scope = EvaluationScope {
            context: &self.frame.context,
            region: &self.frame.region,
            premises,
            localization: &[],
        };
        let chain = chain(self.cas, polynomial, &scope, self.budget)?;
        let source = materialize::polynomial(self.cas, polynomial, &[Field::Real], self.budget)?;
        let relation = materialize::admit(
            self.cas,
            SolveDraft::RootRelation(RootRelationData {
                header: Reference::Existing(self.frame.header.clone()),
                scope: Vec::new(),
                ambient: Field::Real,
                polynomial: Reference::Existing(source),
                binder: vec![AtomicSort::Scalar(Field::Real)],
                map: Reference::Existing(self.map.clone()),
                region: Reference::Existing(self.frame.region.clone()),
                multiplicity: None,
            }),
            self.budget,
        )?;
        let (polynomials, units, signs) = self.chain_entries(&chain.polynomials, premises)?;
        let mut quotients = vector(chain.quotients.len(), self.budget)?;
        for quotient in &chain.quotients {
            quotients.push(materialize::polynomial(
                self.cas,
                quotient,
                &[Field::Real],
                self.budget,
            )?);
        }
        Ok(CertificateRealRootCount {
            relation,
            region: self.frame.region.clone(),
            cells: Vec::new(),
            square_free: materialize::polynomial(
                self.cas,
                &chain.support,
                &[Field::Real],
                self.budget,
            )?,
            derivative: materialize::polynomial(
                self.cas,
                &chain.derivative,
                &[Field::Real],
                self.budget,
            )?,
            gcd: materialize::polynomial(self.cas, &chain.gcd, &[Field::Real], self.budget)?,
            chain: polynomials,
            quotients,
            rescalings: Vec::new(),
            identities: Vec::new(),
            units,
            signs,
            counts: Vec::new(),
            exterior_left: 0,
            exterior_right: 0,
        })
    }

    /// The independently replayed primitive has the exact authored count statement as its subject.
    pub(crate) fn count_claim(
        &mut self,
        certificate: &CertificateRealRootCount,
    ) -> Result<u32, CoefficientFailure> {
        Ok(self.frame.push(
            SolveSchema::RealRootCount,
            SolveStatement::RealRootCount(StatementRealRootCount {
                relation: certificate.relation.clone(),
                region: certificate.region.clone(),
                cells: certificate.cells.clone(),
                counts: certificate.counts.clone(),
                exterior_left: certificate.exterior_left,
                exterior_right: certificate.exterior_right,
            }),
            SolveCertificate::RealRootCount(certificate.clone()),
            self.budget,
        )?)
    }
}
