// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded sparse arithmetic in explicitly ordered formal root or coefficient slots.

use super::AT;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, Handle, State, size};
use crate::model::Field;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::forms::{Form, Term, Variable};
use crate::solving::coefficients::model::{CoefficientFailure, Element};
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{invalid, vector};
use crate::solving::towers::materialize::{admit, fields, value};
use crate::solving::{PolynomialData, PolynomialTerm, SolveDraft};

/// Unsealed sparse scratch with its complete algebra and ordered variable frame.
#[derive(Debug)]
pub(crate) struct Polynomial {
    /// Coefficients never change algebra by implicit conversion.
    pub(crate) domain: Handle,
    /// Root and elementary slots retain their order and sorts.
    pub(crate) variables: Vec<Field>,
    /// Canonical decreasing graded-lex terms, containing only formal slots.
    pub(crate) form: Form,
}

/// A transformed exponent key borrows its coefficient until the final single copy.
struct Permuted<'a> {
    /// Total degree is unchanged by the validated variable bijection.
    degree: u64,
    /// Full exponent vector gives the exact graded-lex key.
    powers: Vec<u32>,
    /// Scratch normal forms retain their complete coefficient algebra.
    coefficient: &'a Element,
}

/// Variable permutation moves exponents and performs no coefficient arithmetic.
fn permuted_key<'a>(
    term: &'a Term,
    image: &[u32],
    budget: &mut Budget,
) -> Result<Permuted<'a>, Fault> {
    let mut powers = vector(image.len(), budget)?;
    powers.resize(image.len(), 0);
    let mut degree = 0_u64;
    for (variable, exponent) in &term.powers {
        budget.work(1, AT)?;
        budget.check(BudgetKind::IntegerExponent, u64::from(*exponent), AT)?;
        let Variable::Slot(slot) = variable else {
            return Err(invalid());
        };
        let destination = *image
            .get(usize::try_from(*slot).map_err(|_| invalid())?)
            .ok_or_else(invalid)?;
        *powers
            .get_mut(usize::try_from(destination).map_err(|_| invalid())?)
            .ok_or_else(invalid)? = *exponent;
        degree = degree
            .checked_add(u64::from(*exponent))
            .ok_or_else(invalid)?;
    }
    budget.check(BudgetKind::PolynomialDegree, degree, AT)?;
    Ok(Permuted {
        degree,
        powers,
        coefficient: &term.coefficient,
    })
}

/// Charge the degree comparison and every exponent actually compared.
fn permutation_order(
    left: &Permuted<'_>,
    right: &Permuted<'_>,
    budget: &mut Budget,
) -> Result<std::cmp::Ordering, Fault> {
    budget.work(1, AT)?;
    let order = left.degree.cmp(&right.degree);
    if order != std::cmp::Ordering::Equal {
        return Ok(order);
    }
    for (first, second) in left.powers.iter().zip(&right.powers) {
        budget.work(1, AT)?;
        let order = first.cmp(second);
        if order != std::cmp::Ordering::Equal {
            return Ok(order);
        }
    }
    Ok(std::cmp::Ordering::Equal)
}

/// A bijection preserves distinct keys; bounded insertion restores decreasing graded-lex order.
fn ordered_permutation<'a>(
    terms: &'a [Term],
    image: &[u32],
    budget: &mut Budget,
) -> Result<Vec<Permuted<'a>>, Fault> {
    budget.check(BudgetKind::PolynomialTerms, size(terms.len(), AT)?, AT)?;
    let mut ordered = vector(terms.len(), budget)?;
    for term in terms {
        budget.solving(BudgetKind::OrbitTerms, 1, AT)?;
        let transformed = permuted_key(term, image, budget)?;
        let mut position = 0;
        while let Some(previous) = ordered.get(position) {
            match permutation_order(&transformed, previous, budget)? {
                std::cmp::Ordering::Greater => break,
                std::cmp::Ordering::Equal => return Err(invalid()),
                std::cmp::Ordering::Less => position += 1,
            }
        }
        budget.work(size(ordered.len() - position, AT)?, AT)?;
        ordered.insert(position, transformed);
    }
    Ok(ordered)
}

/// Reconstruct the sparse ordered key and copy each exact coefficient exactly once.
fn permutation_form(
    ordered: Vec<Permuted<'_>>,
    engine: &Engine<'_>,
    budget: &mut Budget,
) -> Result<Form, Fault> {
    let mut terms = vector(ordered.len(), budget)?;
    for term in ordered {
        let mut powers = vector(term.powers.len(), budget)?;
        for (slot, exponent) in term.powers.into_iter().enumerate() {
            budget.work(1, AT)?;
            if exponent != 0 {
                powers.push((
                    Variable::Slot(u32::try_from(slot).map_err(|_| invalid())?),
                    exponent,
                ));
            }
        }
        terms.push(Term {
            powers,
            coefficient: checked(engine.copy(term.coefficient, budget))?,
        });
    }
    Ok(Form { terms })
}

/// Convert arithmetic faults without losing the exact failing ledger location.
fn checked<T>(result: Result<T, CoefficientFailure>) -> Result<T, Fault> {
    result.map_err(|error| fault(&error))
}

impl Polynomial {
    /// Attach a computed form only after copying the bounded interpretation frame.
    pub(crate) fn with_form(&self, form: Form, budget: &mut Budget) -> Result<Self, Fault> {
        Ok(Self {
            domain: self.domain.clone(),
            variables: fields(&self.variables, budget)?,
            form,
        })
    }

    /// Scalar constants include the empty zero polynomial and retain unused slots.
    pub(crate) fn constant(
        domain: &Handle,
        variables: &[Field],
        coefficient: Element,
        state: &State,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let engine = Engine { state, domain };
        budget.check(
            BudgetKind::PolynomialVariables,
            size(variables.len(), AT)?,
            AT,
        )?;
        Ok(Self {
            domain: domain.clone(),
            variables: fields(variables, budget)?,
            form: checked(Form::constant(coefficient, &engine, budget))?,
        })
    }

    /// Construct zero or one through the same coefficient ledger.
    pub(crate) fn identity(
        &self,
        nonzero: bool,
        state: &State,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let engine = Engine {
            state,
            domain: &self.domain,
        };
        Self::constant(
            &self.domain,
            &self.variables,
            checked(engine.constant(nonzero, budget))?,
            state,
            budget,
        )
    }

    /// A bound formal slot is never an extra computational tower level.
    pub(crate) fn variable(
        &self,
        slot: u32,
        state: &State,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        if usize::try_from(slot).map_err(|_| invalid())? >= self.variables.len() {
            return Err(invalid());
        }
        let engine = Engine {
            state,
            domain: &self.domain,
        };
        self.with_form(
            checked(Form::variable(Variable::Slot(slot), &engine, budget))?,
            budget,
        )
    }

    /// Copy coefficients and keys with charge; no scratch clone bypasses the budget.
    pub(crate) fn copy(&self, state: &State, budget: &mut Budget) -> Result<Self, Fault> {
        let engine = Engine {
            state,
            domain: &self.domain,
        };
        self.with_form(checked(self.form.copy(&engine, budget))?, budget)
    }

    /// Arithmetic only combines identical algebras and complete ordered slot frames.
    fn compatible(&self, other: &Self, budget: &mut Budget) -> Result<(), Fault> {
        budget.work(size(self.variables.len(), AT)?.max(1), AT)?;
        if (&self.domain, &self.variables) != (&other.domain, &other.variables) {
            return Err(invalid());
        }
        Ok(())
    }

    /// Exact addition reserves uncancelled sparse terms in the shared arithmetic engine.
    pub(crate) fn add(
        &self,
        other: &Self,
        state: &State,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        self.compatible(other, budget)?;
        let engine = Engine {
            state,
            domain: &self.domain,
        };
        self.with_form(
            checked(self.form.add(&other.form, &engine, budget))?,
            budget,
        )
    }

    /// Negation needs no field inversion or specialized nonvanishing assertion.
    pub(crate) fn neg(&self, state: &State, budget: &mut Budget) -> Result<Self, Fault> {
        let engine = Engine {
            state,
            domain: &self.domain,
        };
        self.with_form(checked(self.form.neg(&engine, budget))?, budget)
    }

    /// Exact multiplication independently charges all term pairs and recursive coefficients.
    pub(crate) fn mul(
        &self,
        other: &Self,
        state: &State,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        self.compatible(other, budget)?;
        let engine = Engine {
            state,
            domain: &self.domain,
        };
        self.with_form(
            checked(self.form.mul(&other.form, &engine, budget))?,
            budget,
        )
    }

    /// Repeated squaring is bounded by the ordinary integer exponent ceiling.
    pub(crate) fn pow(
        &self,
        exponent: u32,
        state: &State,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        let engine = Engine {
            state,
            domain: &self.domain,
        };
        self.with_form(checked(self.form.pow(exponent, &engine, budget))?, budget)
    }

    /// Full normalization, rather than handle identity, decides duplicate orbit images.
    pub(crate) fn equals(
        &self,
        other: &Self,
        state: &State,
        budget: &mut Budget,
    ) -> Result<bool, Fault> {
        self.compatible(other, budget)?;
        if self.form.terms.len() != other.form.terms.len() {
            return Ok(false);
        }
        let engine = Engine {
            state,
            domain: &self.domain,
        };
        for (left, right) in self.form.terms.iter().zip(&other.form.terms) {
            budget.solving(BudgetKind::OrbitTerms, 2, AT)?;
            budget.work(size(self.variables.len(), AT)?.max(1), AT)?;
            if left.powers != right.powers
                || !checked(engine.formal_equal(&left.coefficient, &right.coefficient, budget))?
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Supply the dense fixed-width key needed by graph syntax and leading-lex elimination.
    pub(crate) fn exponents(&self, term: &Term, budget: &mut Budget) -> Result<Vec<u32>, Fault> {
        let mut result = vector(self.variables.len(), budget)?;
        result.resize(self.variables.len(), 0);
        for (variable, exponent) in &term.powers {
            let Variable::Slot(slot) = variable else {
                return Err(invalid());
            };
            *result
                .get_mut(usize::try_from(*slot).map_err(|_| invalid())?)
                .ok_or_else(invalid)? = *exponent;
        }
        Ok(result)
    }
}

impl Polynomial {
    /// Read only the named polynomial, retaining no authority beyond its canonical syntax.
    pub(crate) fn read(cas: &Cas, handle: &Handle, budget: &mut Budget) -> Result<Self, Fault> {
        cas.checked(handle)?;
        let data = crate::solving::coefficients::storage::polynomial(&cas.state, handle)?;
        let engine = Engine {
            state: &cas.state,
            domain: &data.domain,
        };
        let mut form = Form { terms: Vec::new() };
        for term in &data.terms {
            let mut powers = vector(term.exponents.len(), budget)?;
            for (slot, exponent) in term.exponents.iter().enumerate() {
                if *exponent != 0 {
                    powers.push((
                        Variable::Slot(u32::try_from(slot).map_err(|_| invalid())?),
                        *exponent,
                    ));
                }
            }
            let mut terms = vector(1, budget)?;
            terms.push(Term {
                powers,
                coefficient: checked(engine.read(&term.coefficient, budget))?,
            });
            form = checked(form.add(&Form { terms }, &engine, budget))?;
        }
        Ok(Self {
            domain: data.domain.clone(),
            variables: fields(&data.variables, budget)?,
            form,
        })
    }

    /// Materialize canonical scratch only inside the caller's private transaction.
    pub(crate) fn stage(&self, cas: &mut Cas, budget: &mut Budget) -> Result<Handle, Fault> {
        let mut terms = vector(self.form.terms.len(), budget)?;
        for term in &self.form.terms {
            terms.push(PolynomialTerm {
                exponents: self.exponents(term, budget)?,
                coefficient: crate::graph::Reference::Existing(value(
                    cas,
                    &term.coefficient,
                    budget,
                )?),
            });
        }
        admit(
            cas,
            SolveDraft::Polynomial(PolynomialData {
                domain: crate::graph::Reference::Existing(self.domain.clone()),
                variables: fields(&self.variables, budget)?,
                terms,
            }),
            budget,
        )
    }

    /// Permute exponent keys directly; every action, term visit and key comparison remains charged.
    pub(crate) fn permute(
        &self,
        image: &[u32],
        state: &State,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        super::super::orbits::permutation(image, &self.variables, budget)?;
        budget.solving(BudgetKind::OrbitPermutations, 1, AT)?;
        let engine = Engine {
            state,
            domain: &self.domain,
        };
        let ordered = ordered_permutation(&self.form.terms, image, budget)?;
        self.with_form(permutation_form(ordered, &engine, budget)?, budget)
    }

    /// Simultaneous formal substitution retains every image, including zero specializations.
    pub(crate) fn compose(
        &self,
        images: &[Self],
        state: &State,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        if images.len() != self.variables.len() {
            return Err(invalid());
        }
        let frame = images.first().unwrap_or(self);
        for image in images {
            frame.compatible(image, budget)?;
        }
        if frame.domain != self.domain {
            return Err(invalid());
        }
        let engine = Engine {
            state,
            domain: &self.domain,
        };
        let mut result = frame.identity(false, state, budget)?;
        for term in &self.form.terms {
            let mut product = Self::constant(
                &self.domain,
                &frame.variables,
                checked(engine.copy(&term.coefficient, budget))?,
                state,
                budget,
            )?;
            for (variable, exponent) in &term.powers {
                let Variable::Slot(slot) = variable else {
                    return Err(invalid());
                };
                let image = images
                    .get(usize::try_from(*slot).map_err(|_| invalid())?)
                    .ok_or_else(invalid)?;
                product = product.mul(&image.pow(*exponent, state, budget)?, state, budget)?;
            }
            result = result.add(&product, state, budget)?;
        }
        Ok(result)
    }

    /// Construct a single monomial after checking every exponent and the full total degree.
    pub(crate) fn monomial(
        &self,
        powers: &[u32],
        coefficient: Element,
        state: &State,
        budget: &mut Budget,
    ) -> Result<Self, Fault> {
        if powers.len() != self.variables.len() {
            return Err(invalid());
        }
        let total = powers
            .iter()
            .try_fold(0_u64, |sum, power| sum.checked_add(u64::from(*power)))
            .ok_or_else(|| Fault::capacity(BudgetKind::PolynomialDegree, AT))?;
        budget.check(BudgetKind::PolynomialDegree, total, AT)?;
        let mut key = vector(powers.len(), budget)?;
        for (slot, exponent) in powers.iter().enumerate() {
            if *exponent != 0 {
                key.push((
                    Variable::Slot(u32::try_from(slot).map_err(|_| invalid())?),
                    *exponent,
                ));
            }
        }
        let engine = Engine {
            state,
            domain: &self.domain,
        };
        let mut terms = vector(1, budget)?;
        if !checked(engine.formal_zero(&coefficient, budget))? {
            terms.push(Term {
                powers: key,
                coefficient,
            });
        }
        self.with_form(Form { terms }, budget)
    }
}
