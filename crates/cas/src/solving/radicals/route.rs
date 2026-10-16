// SPDX-License-Identifier: AGPL-3.0-only
//! Strictly degree-decreasing radical discovery, linked choices and inverse character recovery.

use super::construction::Construction;
use super::discovery::{Attempt, Discovery};
use super::roots::{Values, inverse};
use crate::budget::{Budget, BudgetKind};
use crate::graph::{Cas, Handle};
use crate::solving::SolveReason;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::maps::canonical;
use crate::solving::coefficients::model::{CoefficientFailure, Element, EvaluationScope};
use crate::solving::coefficients::storage::{invalid, reserve, vector};
use crate::solving::elementary::recipes::{self, GeneralRecipe, Recipe};
use crate::solving::symmetric::algebra::Polynomial;
use crate::solving::towers::dense::Dense;

/// Diagnostic enumeration is separate from mathematical evidence and carries no completeness bit.
#[derive(Debug, Default)]
pub(crate) struct Audit {
    /// All seeds visited by recursive discovery, in deterministic order.
    pub(crate) attempts: Vec<Attempt>,
    /// Exponents of the last discovered cross-character invariant.
    pub(crate) pairing_powers: Vec<u32>,
}

/// Extract an evaluated constant only after all formal symmetric slots have been substituted.
fn constant(
    polynomial: &Polynomial,
    engine: &Engine<'_>,
    budget: &mut Budget,
) -> Result<Element, CoefficientFailure> {
    match polynomial.form.terms.as_slice() {
        [] => engine.constant(false, budget),
        [term] if term.powers.is_empty() => engine.copy(&term.coefficient, budget),
        _ => Err(invalid().into()),
    }
}

/// A primitive character frame has exactly the original degree, independent of source coefficient names.
fn frame(
    cas: &Cas,
    domain: &Handle,
    degree: u32,
    budget: &mut Budget,
) -> Result<Polynomial, CoefficientFailure> {
    budget.check(
        BudgetKind::PolynomialVariables,
        u64::from(degree),
        super::AT,
    )?;
    let width = usize::try_from(degree).map_err(|_| invalid())?;
    let mut fields = vector(width, budget)?;
    fields.resize(width, crate::model::Field::Complex);
    let engine = Engine {
        state: &cas.state,
        domain,
    };
    Ok(Polynomial::constant(
        domain,
        &fields,
        engine.constant(false, budget)?,
        &cas.state,
        budget,
    )?)
}

/// Recover all characters only when the discovered product isolates exactly one missing linear factor.
fn linked(
    engine: &Engine<'_>,
    chosen: &Element,
    pairing: &Element,
    powers: &[u32],
    index: u32,
    scope: &EvaluationScope<'_>,
    budget: &mut Budget,
) -> Result<Vec<Element>, CoefficientFailure> {
    let selected =
        usize::try_from(index.checked_sub(1).ok_or_else(invalid)?).map_err(|_| invalid())?;
    let mut missing = vector(powers.len(), budget)?;
    missing.extend(
        powers
            .iter()
            .enumerate()
            .filter(|(slot, _)| *slot != selected),
    );
    let [(slot, exponent)] = missing.as_slice() else {
        return Err(CoefficientFailure::Unsupported(
            SolveReason::GenericRadicalRouteUnavailable,
        ));
    };
    if **exponent != 1 {
        return Err(CoefficientFailure::Unsupported(
            SolveReason::RootSelectionExistencePending,
        ));
    }
    let divisor = engine.pow(chosen, *powers.get(selected).ok_or_else(invalid)?, budget)?;
    let recovered = engine.mul(pairing, &inverse(engine, &divisor, scope, budget)?, budget)?;
    let mut values = vector(powers.len(), budget)?;
    for position in 0..powers.len() {
        values.push(if position == *slot {
            engine.copy(&recovered, budget)?
        } else {
            engine.copy(chosen, budget)?
        });
    }
    Ok(values)
}

/// Invert the degree-generic character transform, retaining the actual affine shift.
fn recover(
    cas: &Cas,
    characters: &[Element],
    unity: &Element,
    shift: &Element,
    scope: &EvaluationScope<'_>,
    budget: &mut Budget,
) -> Result<Vec<Element>, CoefficientFailure> {
    let degree = characters.len().checked_add(1).ok_or_else(invalid)?;
    let order = u32::try_from(degree).map_err(|_| invalid())?;
    let engine = Engine {
        state: &cas.state,
        domain: &unity.domain,
    };
    let scalar = crate::number::Rational::from_ratio(i64::from(order), 1, budget)?;
    let reciprocal = inverse(&engine, &engine.rational(&scalar, budget)?, scope, budget)?;
    let shift = canonical(&cas.state, shift, &unity.domain, budget)?;
    let mut values = vector(degree, budget)?;
    for row in 0..order {
        let mut sum = engine.constant(false, budget)?;
        for (column, character) in characters.iter().enumerate() {
            let column = u32::try_from(column + 1).map_err(|_| invalid())?;
            let exponent = (order - row.checked_mul(column).ok_or_else(invalid)? % order) % order;
            let factor = engine.pow(unity, exponent, budget)?;
            sum = engine.add(&sum, &engine.mul(&factor, character, budget)?, budget)?;
        }
        values.push(engine.sub(&engine.mul(&sum, &reciprocal, budget)?, &shift, budget)?);
    }
    Ok(values)
}

/// Lift one resolvent root to the offered character power; other characters remain linked by a derived product.
fn lift(
    cas: &mut Cas,
    construction: &mut Construction,
    values: &Values,
    power: u32,
    budget: &mut Budget,
) -> Result<super::construction::Extension, CoefficientFailure> {
    let engine = Engine {
        state: &cas.state,
        domain: &values.domain,
    };
    let defining = super::roots::polynomial(
        &engine,
        values.roots.first().ok_or_else(invalid)?,
        power,
        budget,
    )?;
    Ok(construction.adjoin(cas, &defining, &values.map, None, budget)?)
}

/// One bounded recursive invocation shares both the graph and all arithmetic ledgers.
pub(crate) struct Route<'a, 's> {
    /// Owning transaction candidate.
    pub(crate) cas: &'a mut Cas,
    /// Unsealed graph application DAG.
    pub(crate) construction: &'a mut Construction,
    /// Exact regional unit and localization evidence.
    pub(crate) scope: &'a EvaluationScope<'s>,
    /// Deterministic enumeration log, without theorem authority.
    pub(crate) audit: &'a mut Audit,
    /// Caller-owned generation ledger.
    pub(crate) budget: &'a mut Budget,
}

/// A smaller recursive target and the generated data needed for linked recovery.
struct Descent {
    /// Actual polynomial after symmetric coefficient substitution.
    target: Dense,
    /// Primitive unity and the map of the coefficient algebra containing it.
    primitive: Values,
    /// Character index whose power produced the smaller orbit.
    index: u32,
    /// Offered character power, taken from the generic cursor.
    power: u32,
    /// Strictly greater recursion depth admitted by the smaller-resolvent producer.
    depth: u64,
    /// Generated symmetric cross-character product.
    pairing: Element,
    /// Exact exponent vector of that product.
    powers: Vec<u32>,
}

impl Route<'_, '_> {
    /// Visit generic seeds and independently retain every symmetric expansion obligation.
    fn descend(
        &mut self,
        source: &GeneralRecipe,
        map: &Handle,
        depth: u64,
    ) -> Result<Descent, CoefficientFailure> {
        let degree = u32::try_from(
            source
                .coefficients
                .coefficients
                .len()
                .checked_sub(1)
                .ok_or_else(invalid)?,
        )
        .map_err(|_| invalid())?;
        let primitive = super::roots::unity(
            self.cas,
            self.construction,
            &source.coefficients.domain,
            map,
            degree,
            self.budget,
        )?;
        let unity = primitive.roots.first().ok_or_else(invalid)?;
        let frame = frame(self.cas, &primitive.domain, degree, self.budget)?;
        let mut discovery = Discovery::new(degree, depth, self.budget)?;
        let candidate = discovery
            .next(&frame, unity, &self.cas.state, self.budget)?
            .ok_or(CoefficientFailure::Unsupported(
                SolveReason::GenericRadicalRouteUnavailable,
            ))?;
        let Some(Attempt {
            seed: crate::solving::symmetric::cursor::Seed::Cyclic { index, power },
            cardinality,
        }) = discovery.attempts.last()
        else {
            return Err(CoefficientFailure::Unsupported(
                SolveReason::GenericRadicalRouteUnavailable,
            ));
        };
        if cardinality.checked_add(1) != Some(candidate.resolvent.coefficients.len()) {
            return Err(invalid().into());
        }
        let (index, power) = (*index, *power);
        reserve(
            &mut self.audit.attempts,
            discovery.attempts.len(),
            self.budget,
        )?;
        self.audit.attempts.extend(discovery.attempts);
        let orbit =
            candidate
                .orbit
                .application(self.cas, &self.construction.context, self.budget)?;
        reserve(&mut self.construction.steps, 1, self.budget)?;
        self.construction.steps.push(orbit);
        for coefficient in &candidate.resolvent.coefficients {
            super::proof::reduction(
                self.cas,
                self.construction,
                coefficient,
                &primitive.map,
                self.budget,
            )?;
        }
        let images = super::substitute::images(
            &source.coefficients,
            &frame,
            self.scope,
            &self.cas.state,
            self.budget,
        )?;
        for coefficient in &candidate.resolvent.coefficients {
            super::proof::substitution(
                self.cas,
                self.construction,
                &coefficient.expression,
                &images,
                &primitive.map,
                self.budget,
            )?;
        }
        let target = super::substitute::resolvent(
            &candidate.resolvent,
            &images,
            &self.cas.state,
            self.budget,
        )?;
        let pairing = self.pairing(&frame, &primitive, &images, degree)?;
        Ok(Descent {
            target,
            primitive,
            index,
            power,
            depth: candidate.resolvent.depth,
            pairing: pairing.0,
            powers: pairing.1,
        })
    }

    /// The first symmetric cross-character product is discovered by low-total-degree enumeration.
    fn pairing(
        &mut self,
        frame: &Polynomial,
        primitive: &Values,
        images: &[Polynomial],
        degree: u32,
    ) -> Result<(Element, Vec<u32>), CoefficientFailure> {
        let unity = primitive.roots.first().ok_or_else(invalid)?;
        let pairing = super::pairing::discover(frame, unity, degree, &self.cas.state, self.budget)?
            .ok_or(CoefficientFailure::Unsupported(
                SolveReason::RootSelectionExistencePending,
            ))?;
        super::proof::reduction(
            self.cas,
            self.construction,
            &pairing.reduction,
            &primitive.map,
            self.budget,
        )?;
        super::proof::substitution(
            self.cas,
            self.construction,
            &pairing.reduction.expression,
            images,
            &primitive.map,
            self.budget,
        )?;
        let product = pairing
            .reduction
            .expression
            .compose(images, &self.cas.state, self.budget)?;
        self.audit.pairing_powers =
            crate::solving::coefficients::storage::copied(&pairing.powers, self.budget)?;
        let engine = Engine {
            state: &self.cas.state,
            domain: &primitive.domain,
        };
        Ok((constant(&product, &engine, self.budget)?, pairing.powers))
    }

    /// Recursion lowers degree before any character root is selected or inverse is attempted.
    fn general(
        &mut self,
        source: &GeneralRecipe,
        map: &Handle,
        depth: u64,
    ) -> Result<Values, CoefficientFailure> {
        let descent = self.descend(source, map, depth)?;
        let primitive = descent.primitive.roots.first().ok_or_else(invalid)?;
        let width = u32::try_from(
            source
                .coefficients
                .coefficients
                .len()
                .checked_sub(1)
                .ok_or_else(invalid)?,
        )
        .map_err(|_| invalid())?;
        super::recovery::certify(
            self.cas,
            self.construction,
            primitive,
            &descent.primitive.map,
            width,
            self.scope,
            self.budget,
        )?;
        let smaller = self.derive(&descent.target, &descent.primitive.map, descent.depth)?;
        let extension = lift(
            self.cas,
            self.construction,
            &smaller,
            descent.power,
            self.budget,
        )?;
        let product = canonical(
            &self.cas.state,
            &descent.pairing,
            &extension.domain,
            self.budget,
        )?;
        let engine = Engine {
            state: &self.cas.state,
            domain: &extension.domain,
        };
        let characters = linked(
            &engine,
            &extension.generator,
            &product,
            &descent.powers,
            descent.index,
            self.scope,
            self.budget,
        )?;
        let unity = canonical(
            &self.cas.state,
            descent.primitive.roots.first().ok_or_else(invalid)?,
            &extension.domain,
            self.budget,
        )?;
        let roots = recover(
            self.cas,
            &characters,
            &unity,
            &source.shift,
            self.scope,
            self.budget,
        )?;
        Ok(Values {
            domain: extension.domain,
            map: extension.map,
            roots,
        })
    }

    /// Degree drop and translation precede binomials; unavailable strategies seal no obstruction.
    pub(crate) fn derive(
        &mut self,
        polynomial: &Dense,
        map: &Handle,
        depth: u64,
    ) -> Result<Values, CoefficientFailure> {
        let engine = Engine {
            state: &self.cas.state,
            domain: &polynomial.domain,
        };
        let recipe = match recipes::derive(&engine, polynomial, self.scope, self.budget) {
            Err(CoefficientFailure::Unsupported(SolveReason::SpecializationGuardPending))
                if polynomial.coefficients.len() > 5 =>
            {
                return Err(CoefficientFailure::Unsupported(
                    SolveReason::GenericRadicalRouteUnavailable,
                ));
            }
            result => result?,
        };
        match recipe {
            Recipe::Binomial(recipe) => {
                super::roots::binomial(self.cas, self.construction, &recipe, map, self.budget)
            }
            Recipe::General(_) if polynomial.coefficients.len() > 5 => Err(
                CoefficientFailure::Unsupported(SolveReason::GenericRadicalRouteUnavailable),
            ),
            Recipe::General(recipe) => self.general(&recipe, map, depth),
            Recipe::Constant(value) => Ok(Values {
                domain: value.domain,
                map: map.clone(),
                roots: Vec::new(),
            }),
            Recipe::Zero => Err(CoefficientFailure::Unsupported(
                SolveReason::CoveragePending,
            )),
        }
    }
}
