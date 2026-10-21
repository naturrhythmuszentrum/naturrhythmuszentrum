// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded production of explicit power identities; replay never invokes this search.

use super::storage::{invalid, vector};
use super::{nonzero::Factors, polynomial::Polynomial};
use crate::{
    budget::Budget,
    error::Fault,
    graph::{Handle, State},
};

/// An ordered finite combination of zero guard polynomials.
type Multiples = Vec<(Polynomial, Polynomial)>;

/// Unsealed data for f^k minus a product of scoped units as a guard combination.
pub(crate) struct Consequence {
    /// Positive power of the queried polynomial.
    pub(crate) exponent: u32,
    /// Exact ordered cofactors and zero guards.
    pub(crate) multiples: Multiples,
    /// Independently reconstructible nonzero product.
    pub(crate) product: Polynomial,
    /// Checked factors, including repetitions and the final nonzero scalar.
    pub(crate) factors: Vec<Polynomial>,
}

/// Leading reductions retain every cofactor without claiming a canonical ideal basis.
fn reduction(
    power: &Polynomial,
    relations: &[Polynomial],
    budget: &mut Budget,
) -> Result<(Polynomial, Multiples), Fault> {
    let mut remainder = power.copy(budget)?;
    let mut multiples = vector(relations.len(), budget)?;
    for relation in relations {
        let (cofactor, reduced) = super::gcd::division(&remainder, relation, budget)?;
        if !cofactor.zero(budget)? {
            multiples.push((cofactor, relation.copy(budget)?));
        }
        remainder = reduced;
    }
    Ok((remainder, multiples))
}

/// Try a single matching monomial when the default polynomial order gives an unhelpful remainder.
fn pivot(
    power: &Polynomial,
    relations: &[Polynomial],
    factors: &Factors,
    exponent: u32,
    budget: &mut Budget,
) -> Result<Option<Consequence>, Fault> {
    let Some(leading) = power.terms.first() else {
        return Ok(None);
    };
    for relation in relations {
        for term in &relation.terms {
            let Some(quotient) = super::gcd::quotient_term(leading, term, budget)? else {
                continue;
            };
            let mut terms = vector(1, budget)?;
            terms.push(quotient);
            let cofactor = Polynomial::new(power.variables, power.ground, terms, budget)?;
            let product = power.sub(&cofactor.mul(relation, budget)?, budget)?;
            if let Some(inventory) = factors.decompose(&product, budget)? {
                let mut multiples = vector(1, budget)?;
                multiples.push((cofactor, relation.copy(budget)?));
                return Ok(Some(Consequence {
                    exponent,
                    multiples,
                    product,
                    factors: inventory,
                }));
            }
        }
    }
    Ok(None)
}

/// Only contained guards in the queried frame enter this finite producer search.
pub(crate) fn search(
    state: &State,
    source: &Handle,
    polynomial: &Polynomial,
    region: &Handle,
    budget: &mut Budget,
) -> Result<Option<Consequence>, Fault> {
    let relations = super::regional::zeros(state, source, polynomial, region, budget)?;
    if relations.is_empty() {
        return Ok(None);
    }
    let mut factors = Factors::new(polynomial);
    super::regional::factors(state, source, region, &mut factors, budget)?;
    let mut bound = 0;
    for relation in &relations {
        bound = bound.max(relation.total_degree(budget)?.unwrap_or(0));
    }
    let bound = u32::try_from(bound).map_err(|_| invalid())?;
    let mut power = polynomial.one(budget)?;
    for exponent in 1..=bound {
        power = power.mul(polynomial, budget)?;
        let (product, multiples) = reduction(&power, &relations, budget)?;
        if !multiples.is_empty()
            && let Some(inventory) = factors.decompose(&product, budget)?
        {
            return Ok(Some(Consequence {
                exponent,
                multiples,
                product,
                factors: inventory,
            }));
        }
        if let Some(found) = pivot(&power, &relations, &factors, exponent, budget)? {
            return Ok(Some(found));
        }
    }
    Ok(None)
}
