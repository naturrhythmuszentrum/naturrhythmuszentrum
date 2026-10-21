// SPDX-License-Identifier: AGPL-3.0-only
//! Strictly degree-decreasing radical discovery, linked choices and inverse character recovery.

use super::construction::Construction;
use super::discovery::{Attempt, Candidate, Discovery};
use super::roots::{Values, inverse};
use crate::budget::{Budget, BudgetKind};
use crate::graph::{Cas, Handle};
use crate::solving::SolveReason;
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::maps::canonical;
use crate::solving::coefficients::model::{
    CoefficientFailure, Element, EvaluationScope, GuardedObligation, UnitDecision,
};
use crate::solving::coefficients::storage::{invalid, reserve, vector};
use crate::solving::elementary::recipes::{self, GeneralRecipe, Recipe};
use crate::solving::symmetric::algebra::Polynomial;
use crate::solving::towers::dense::Dense;

pub(crate) use super::audit::Audit;

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
) -> Result<Result<Vec<Element>, GuardedObligation>, CoefficientFailure> {
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
    let reciprocal = match engine.inverse(&divisor, scope, budget)? {
        UnitDecision::Unit { inverse, .. } => inverse,
        UnitDecision::Undetermined(obligation) => return Ok(Err(obligation)),
        UnitDecision::Zero(_) => {
            return Err(CoefficientFailure::Unsupported(
                SolveReason::MissingUnitProof,
            ));
        }
        UnitDecision::Split(_) => {
            return Err(CoefficientFailure::Unsupported(
                SolveReason::ComponentChoicePending,
            ));
        }
    };
    let recovered = engine.mul(pairing, &reciprocal, budget)?;
    let mut values = vector(powers.len(), budget)?;
    for position in 0..powers.len() {
        values.push(if position == *slot {
            engine.copy(&recovered, budget)?
        } else {
            engine.copy(chosen, budget)?
        });
    }
    Ok(Ok(values))
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

/// Degree-generic choices preserve the defining root relations and enter the complete proof proposal.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Choices {
    /// Replace a primitive unity by its inverse before constructing the character frame.
    pub(crate) inverse_unity: bool,
    /// Rotate a lifted character by powers of its compatible primitive unity.
    pub(crate) rotation: u32,
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
    /// Explicit root choices are semantic inputs.
    pub(crate) choices: Choices,
    /// Optional request-local formal discovery reuse across parameter cells.
    pub(crate) cache: Option<&'a mut super::cache::Cache>,
    /// Independently sealed regional consequences in this invocation's exact scope.
    pub(crate) units: Vec<crate::proof_model::VerifiedWitness>,
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
    /// A reusable character frame is constructed from the actual polynomial degree.
    fn primitive(
        &mut self,
        source: &GeneralRecipe,
        map: &Handle,
    ) -> Result<(Values, Polynomial, u32), CoefficientFailure> {
        let degree = u32::try_from(
            source
                .coefficients
                .coefficients
                .len()
                .checked_sub(1)
                .ok_or_else(invalid)?,
        )
        .map_err(|_| invalid())?;
        let mut primitive = super::roots::unity(
            self.cas,
            self.construction,
            &source.coefficients.domain,
            map,
            degree,
            self.budget,
        )?;
        if self.choices.inverse_unity {
            let engine = Engine {
                state: &self.cas.state,
                domain: &primitive.domain,
            };
            let unity = primitive.roots.first_mut().ok_or_else(invalid)?;
            *unity = engine.pow(
                unity,
                degree.checked_sub(1).ok_or_else(invalid)?,
                self.budget,
            )?;
        }
        let frame = frame(self.cas, &primitive.domain, degree, self.budget)?;
        Ok((primitive, frame, degree))
    }

    /// The cursor contributes its exact visited prefix and a strictly smaller candidate.
    fn enumerate(
        &mut self,
        frame: &Polynomial,
        unity: &Element,
        degree: u32,
        depth: u64,
    ) -> Result<(Candidate, u32, u32), CoefficientFailure> {
        if let Some(cache) = &self.cache
            && let Some(found) = cache.lookup(frame, unity, depth, &self.cas.state, self.budget)?
        {
            self.mark("reuse-discovery", depth)?;
            return Ok(found);
        }
        let mut discovery = Discovery::new(degree, depth, self.budget)?;
        let audit = &mut *self.audit;
        let region = self.scope.region;
        let candidate = discovery
            .next(
                frame,
                unity,
                &self.cas.state,
                self.budget,
                |phase, seed, budget| audit.seed(phase, region, depth, seed, budget),
            )?
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
        if let Some(cache) = &mut self.cache {
            cache.remember(
                &candidate,
                unity,
                (index, power),
                &self.cas.state,
                self.budget,
            )?;
        }
        Ok((candidate, index, power))
    }

    /// Observe graph staging and each replay expansion separately from discovery arithmetic.
    fn obligations(
        &mut self,
        candidate: &Candidate,
        source: &Dense,
        maps: [&Handle; 2],
        depth: u64,
    ) -> Result<(), CoefficientFailure> {
        self.mark("orbit-proof-start", depth)?;
        let orbit =
            candidate
                .orbit
                .application(self.cas, &self.construction.context, self.budget)?;
        reserve(&mut self.construction.steps, 1, self.budget)?;
        self.construction.steps.push(orbit);
        self.mark("orbit-proof-end", depth)?;
        for (position, coefficient) in candidate.resolvent.coefficients.iter().enumerate() {
            self.audit.indexed(
                "reduction-proof-start",
                position,
                self.scope.region,
                depth,
                self.budget,
            )?;
            let mut input = super::formal::Input {
                source,
                maps,
                scope: self.scope,
                audit: self.audit,
            };
            super::formal::certify(
                self.cas,
                self.construction,
                coefficient,
                &mut input,
                self.budget,
            )?;
            self.audit.indexed(
                "reduction-proof-end",
                position,
                self.scope.region,
                depth,
                self.budget,
            )?;
        }
        Ok(())
    }

    /// Visit generic seeds and retain every symmetric expansion obligation independently.
    fn descend(
        &mut self,
        source: &GeneralRecipe,
        map: &Handle,
        depth: u64,
    ) -> Result<Descent, CoefficientFailure> {
        self.mark("primitive", depth)?;
        let (primitive, frame, degree) = self.primitive(source, map)?;
        self.mark("orbit-discovery", depth)?;
        let (candidate, index, power) = self.enumerate(
            &frame,
            primitive.roots.first().ok_or_else(invalid)?,
            degree,
            depth,
        )?;
        self.mark("symmetric-proof", depth)?;
        self.obligations(
            &candidate,
            &source.coefficients,
            [&primitive.map, map],
            depth,
        )?;
        self.mark("coefficient-images", depth)?;
        let images = super::substitute::images(
            &source.coefficients,
            &frame,
            self.scope,
            &self.cas.state,
            self.budget,
        )?;
        self.mark("resolvent-substitution", depth)?;
        let target = super::substitute::resolvent(
            &candidate.resolvent,
            &images,
            &self.cas.state,
            self.budget,
        )?;
        self.mark("pairing", depth)?;
        let pairing = self.pairing(&primitive, &images, &frame, &source.coefficients, map)?;
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
        primitive: &Values,
        images: &[Polynomial],
        frame: &Polynomial,
        source: &Dense,
        input_map: &Handle,
    ) -> Result<(Element, Vec<u32>), CoefficientFailure> {
        let degree = u32::try_from(frame.variables.len()).map_err(|_| invalid())?;
        let unity = primitive.roots.first().ok_or_else(invalid)?;
        let pairing = if let Some(cache) = &mut self.cache {
            cache.pairing(frame, unity, degree, &self.cas.state, self.budget)?
        } else {
            super::pairing::discover(frame, unity, degree, &self.cas.state, self.budget)?
        }
        .ok_or(CoefficientFailure::Unsupported(
            SolveReason::RootSelectionExistencePending,
        ))?;
        let mut input = super::formal::Input {
            source,
            maps: [&primitive.map, input_map],
            scope: self.scope,
            audit: self.audit,
        };
        super::formal::certify(
            self.cas,
            self.construction,
            &pairing.reduction,
            &mut input,
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
        self.mark("inverse-transform-proof", depth)?;
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
        self.mark("character-lift", depth)?;
        let extension = lift(
            self.cas,
            self.construction,
            &smaller,
            descent.power,
            self.budget,
        )?;
        self.recovered(source, &descent, extension, width, depth)
    }

    /// Preserve the original fast path when no additional regional consequence was needed.
    fn premises(
        &mut self,
    ) -> Result<Option<Vec<crate::proof_model::VerifiedWitness>>, CoefficientFailure> {
        if self.units.is_empty() {
            return Ok(None);
        }
        let bytes =
            self.scope
                .premises
                .iter()
                .chain(&self.units)
                .try_fold(0_u64, |total, witness| {
                    crate::solving::coefficients::storage::add(
                        total,
                        crate::graph::size(witness.bytes.len(), super::AT)?,
                    )
                })?;
        self.budget.check(
            crate::budget::BudgetKind::StagedPayloadBytes,
            bytes,
            super::AT,
        )?;
        self.budget.work(bytes, super::AT)?;
        let mut result =
            crate::solving::coefficients::storage::copied(self.scope.premises, self.budget)?;
        reserve(&mut result, self.units.len(), self.budget)?;
        result.extend(self.units.iter().cloned());
        Ok(Some(result))
    }

    /// A proposed identity is independently replayed before it can affect inversion.
    fn consequence(&mut self, obligation: &GuardedObligation) -> Result<bool, CoefficientFailure> {
        let Some(witness) =
            super::consequence::certify(self.cas, self.construction, obligation, self.budget)?
        else {
            return Ok(false);
        };
        reserve(&mut self.units, 1, self.budget)?;
        self.units.push(witness);
        Ok(true)
    }

    /// Resolve a missing unit in this exact cell before retrying linked character recovery.
    fn characters(
        &mut self,
        chosen: &Element,
        product: &Element,
        descent: &Descent,
    ) -> Result<Vec<Element>, CoefficientFailure> {
        loop {
            let premises = self.premises()?;
            let scope = EvaluationScope {
                context: self.scope.context,
                region: self.scope.region,
                premises: premises.as_deref().unwrap_or(self.scope.premises),
                localization: self.scope.localization,
            };
            let engine = Engine {
                state: &self.cas.state,
                domain: &chosen.domain,
            };
            match linked(
                &engine,
                chosen,
                product,
                &descent.powers,
                descent.index,
                &scope,
                self.budget,
            )? {
                Ok(values) => return Ok(values),
                Err(obligation) => {
                    if !self.consequence(&obligation)? {
                        return self.pending(obligation);
                    }
                }
            }
        }
    }

    /// Recover a complete set from one compatible character choice and the discovered product.
    fn recovered(
        &mut self,
        source: &GeneralRecipe,
        descent: &Descent,
        extension: super::construction::Extension,
        width: u32,
        depth: u64,
    ) -> Result<Values, CoefficientFailure> {
        self.mark("linked-recovery", depth)?;
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
        let unity = canonical(
            &self.cas.state,
            descent.primitive.roots.first().ok_or_else(invalid)?,
            &extension.domain,
            self.budget,
        )?;
        if !width.is_multiple_of(descent.power) {
            return Err(invalid().into());
        }
        let exponent = (width / descent.power)
            .checked_mul(self.choices.rotation % descent.power)
            .ok_or_else(invalid)?;
        let chosen = engine.mul(
            &extension.generator,
            &engine.pow(&unity, exponent, self.budget)?,
            self.budget,
        )?;
        let characters = self.characters(&chosen, &product, descent)?;
        self.mark("inverse-transform", depth)?;
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

    /// Unresolved base polynomials preserve their full domain when offered for a regional split.
    fn pending<T>(&mut self, obligation: GuardedObligation) -> Result<T, CoefficientFailure> {
        use crate::solving::coefficients::model::Value;
        if matches!(
            crate::solving::coefficients::storage::domain(&self.cas.state, &obligation.domain)?,
            crate::solving::CoefficientDomainData::Fraction { .. }
        ) {
            let numerator = obligation.polynomial;
            let denominator = numerator.one(self.budget)?;
            let engine = Engine {
                state: &self.cas.state,
                domain: &obligation.domain,
            };
            self.audit.pending = Some(engine.element(Value::Fraction(
                crate::solving::coefficients::fraction::Fraction {
                    numerator,
                    denominator,
                },
            )));
        }
        Err(CoefficientFailure::Unsupported(
            SolveReason::SpecializationGuardPending,
        ))
    }

    /// Exact scoped decisions branch only after supplied unit identities have been independently checked.
    fn decide(&mut self, value: &Element) -> Result<bool, CoefficientFailure> {
        use crate::solving::coefficients::model::ZeroDecision;
        loop {
            let premises = self.premises()?;
            let scope = EvaluationScope {
                context: self.scope.context,
                region: self.scope.region,
                premises: premises.as_deref().unwrap_or(self.scope.premises),
                localization: self.scope.localization,
            };
            let engine = Engine {
                state: &self.cas.state,
                domain: &value.domain,
            };
            match engine.zero_test(value, &scope, self.budget)? {
                ZeroDecision::Zero(_) => return Ok(true),
                ZeroDecision::Nonzero(_) => return Ok(false),
                ZeroDecision::Split(_) => {
                    return Err(CoefficientFailure::Unsupported(
                        SolveReason::ComponentChoicePending,
                    ));
                }
                ZeroDecision::Undetermined(obligation) => {
                    if !self.consequence(&obligation)? {
                        return self.pending(obligation);
                    }
                }
            }
        }
    }

    /// Cost observations name the exact scope and actual recursive depth.
    fn mark(&mut self, phase: &'static str, depth: u64) -> Result<(), CoefficientFailure> {
        Ok(self
            .audit
            .mark(phase, self.scope.region, depth, self.budget)?)
    }

    /// Regional degree decisions precede the generic affine translation.
    fn recipe(&mut self, polynomial: &Dense, depth: u64) -> Result<Recipe, CoefficientFailure> {
        self.mark("degree-decisions", depth)?;
        if polynomial.coefficients.len() <= 5 {
            for coefficient in polynomial.coefficients.iter().rev() {
                if !self.decide(coefficient)? {
                    break;
                }
            }
        }
        self.mark("affine-translation", depth)?;
        let engine = Engine {
            state: &self.cas.state,
            domain: &polynomial.domain,
        };
        match recipes::derive(&engine, polynomial, self.scope, self.budget) {
            Err(CoefficientFailure::Unsupported(SolveReason::SpecializationGuardPending))
                if polynomial.coefficients.len() > 5 =>
            {
                Err(CoefficientFailure::Unsupported(
                    SolveReason::GenericRadicalRouteUnavailable,
                ))
            }
            result => result,
        }
    }

    /// Degree drop and translation precede binomials; unavailable strategies seal no obstruction.
    pub(crate) fn derive(
        &mut self,
        polynomial: &Dense,
        map: &Handle,
        depth: u64,
    ) -> Result<Values, CoefficientFailure> {
        let recipe = self.recipe(polynomial, depth)?;
        self.mark("recipe-decisions", depth)?;
        match recipe {
            Recipe::Binomial(mut recipe) => {
                if self.decide(&recipe.radicand)? {
                    let engine = Engine {
                        state: &self.cas.state,
                        domain: &recipe.radicand.domain,
                    };
                    recipe.radicand = engine.constant(false, self.budget)?;
                }
                self.mark("binomial-roots", depth)?;
                super::roots::binomial(self.cas, self.construction, &recipe, map, self.budget)
            }
            Recipe::General(_) if polynomial.coefficients.len() > 5 => Err(
                CoefficientFailure::Unsupported(SolveReason::GenericRadicalRouteUnavailable),
            ),
            Recipe::General(recipe) => {
                for coefficient in recipe
                    .coefficients
                    .coefficients
                    .iter()
                    .skip(1)
                    .take(recipe.coefficients.coefficients.len().saturating_sub(2))
                {
                    self.decide(coefficient)?;
                }
                self.general(&recipe, map, depth)
            }
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
