// SPDX-License-Identifier: AGPL-3.0-only
//! Base-local positive square witnesses support the two Real-parameter quadratic strata.

use super::invalid;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Draft, Handle, Reference};
use crate::model::{Operator, PredicateKind};
use crate::proof_model::{ApplicationDraft, Certificate, Statement};
use crate::solving::coefficients::{
    arithmetic::Engine, model::Element, proof::fault, storage::vector,
};
use crate::solving::radicals::construction::{Construction, admit};
use crate::solving::real::construction::Frame;
use crate::solving::towers::{dense::Dense, materialize};
use crate::solving::{
    CoefficientDomainData, SolveCertificate, SolveDraft, SolveSchema, SolveStatement,
    StatementRootExistence,
};

/// An untrusted sign proposal retains the original Real guard and base stratum.
struct Base {
    /// Base-only exact coefficient algebra.
    domain: Handle,
    /// Original Real base stratum.
    stratum: Handle,
    /// Original positively oriented Real subject.
    guard: Handle,
    /// Constant coefficient of the positive-square relation.
    constant: Element,
    /// Whether the Complex witness uses the imaginary unit.
    imaginary: bool,
}

/// Remove only unused fibre slots from an explicitly Real base polynomial.
fn project(
    cas: &Cas,
    value: &Element,
    target: &Handle,
    width: usize,
    budget: &mut Budget,
) -> Result<Option<Element>, Fault> {
    use crate::solving::coefficients::{
        fraction::Fraction,
        model::Value,
        polynomial::{Polynomial, Term},
    };
    let Value::Fraction(fraction) = &value.value else {
        return Ok(None);
    };
    let mut pair = Vec::new();
    for original in [&fraction.numerator, &fraction.denominator] {
        let mut terms = vector(original.terms.len(), budget)?;
        for term in &original.terms {
            if term
                .powers
                .get(width..)
                .ok_or_else(invalid)?
                .iter()
                .any(|power| *power != 0)
            {
                return Ok(None);
            }
            terms.push(Term {
                powers: term.powers.get(..width).ok_or_else(invalid)?.to_vec(),
                coefficient: term.coefficient.copy(budget)?,
            });
        }
        pair.push(Polynomial::new(width, original.ground, terms, budget)?);
    }
    let [numerator, denominator] = pair.as_slice() else {
        return Err(invalid());
    };
    let engine = Engine {
        state: &cas.state,
        domain: target,
    };
    Ok(Some(engine.element(Value::Fraction(Fraction::new(
        numerator,
        denominator,
        budget,
    )?))))
}

/// Match an existing positive base guard to either orientation of the actual radicand.
fn guard(
    cas: &Cas,
    stratum: &Handle,
    radicand: &Element,
    budget: &mut Budget,
) -> Result<Option<(Handle, bool)>, Fault> {
    let engine = Engine {
        state: &cas.state,
        domain: &radicand.domain,
    };
    let opposite = engine
        .neg(radicand, budget)
        .map_err(|error| fault(&error))?;
    let mut pending = vec![stratum.logical_id()];
    while let Some(id) = pending.pop() {
        budget.work(1, super::AT)?;
        match &cas.state.region(id)?.expression {
            crate::domain_model::RegionExpr::GuardTrue { base, predicate } => {
                let (kind, arguments) =
                    crate::solving::elementary::evidence::predicate(cas, *predicate)?;
                if let (PredicateKind::Positive, [subject]) = (kind, arguments) {
                    let value = crate::solving::solve_search::read::expression(
                        cas, &engine, *subject, "", stratum, budget,
                    )
                    .map_err(|error| fault(&error))?;
                    if let [value] = value.coefficients.as_slice() {
                        if engine
                            .formal_equal(value, radicand, budget)
                            .map_err(|error| fault(&error))?
                        {
                            return Ok(Some((cas.handle(*subject), false)));
                        }
                        if engine
                            .formal_equal(value, &opposite, budget)
                            .map_err(|error| fault(&error))?
                        {
                            return Ok(Some((cas.handle(*subject), true)));
                        }
                    }
                }
                pending.push(*base);
            }
            crate::domain_model::RegionExpr::Meet(members) => pending.extend(members),
            _ => {}
        }
    }
    Ok(None)
}

/// Only a monic quadratic with zero linear term and a proved base orientation qualifies.
fn candidate(
    cas: &mut Cas,
    construction: &Construction,
    source: &Dense,
    budget: &mut Budget,
) -> Result<Option<Base>, Fault> {
    let [constant, linear, leading] = source.coefficients.as_slice() else {
        return Ok(None);
    };
    let engine = Engine {
        state: &cas.state,
        domain: &source.domain,
    };
    if !engine
        .formal_zero(linear, budget)
        .map_err(|error| fault(&error))?
        || !engine
            .formal_equal(
                leading,
                &engine
                    .constant(true, budget)
                    .map_err(|error| fault(&error))?,
                budget,
            )
            .map_err(|error| fault(&error))?
    {
        return Ok(None);
    }
    let over = cas
        .state
        .region(construction.region.logical_id())?
        .lies_over
        .clone()
        .ok_or_else(invalid)?;
    let family = super::mapping::family_data(&cas.state, &cas.handle(over.family))?;
    let base = super::mapping::base_data(&cas.state, &family.base)?;
    let coordinates = base.coordinates.clone();
    let width = crate::solving::graph::coordinates(&cas.state, coordinates.logical_id())?
        .coordinates
        .len();
    let CoefficientDomainData::Fraction { base: ground, .. } =
        crate::solving::coefficients::storage::domain(&cas.state, &source.domain)?
    else {
        return Ok(None);
    };
    if !matches!(
        crate::solving::coefficients::storage::domain(&cas.state, ground)?,
        CoefficientDomainData::Rational
    ) {
        return Ok(None);
    }
    let ground = ground.clone();
    let domain = materialize::admit(
        cas,
        SolveDraft::CoefficientDomain(CoefficientDomainData::Fraction {
            base: Reference::Existing(ground),
            coordinates: Reference::Existing(coordinates),
        }),
        budget,
    )?;
    let Some(mut constant) = project(cas, constant, &domain, width, budget)? else {
        return Ok(None);
    };
    let engine = Engine {
        state: &cas.state,
        domain: &domain,
    };
    let radicand = engine
        .neg(&constant, budget)
        .map_err(|error| fault(&error))?;
    let stratum = cas.handle(over.stratum);
    let Some((guard, imaginary)) = guard(cas, &stratum, &radicand, budget)? else {
        return Ok(None);
    };
    if imaginary {
        constant = engine
            .neg(&constant, budget)
            .map_err(|error| fault(&error))?;
    }
    Ok(Some(Base {
        domain,
        stratum,
        guard,
        constant,
        imaginary,
    }))
}

/// Explicit equality transports the original positive guard to the exact square-root radicand.
fn sign(
    cas: &mut Cas,
    frame: &mut Frame,
    base: &Base,
    map: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    use crate::solving::{CertificateRealSign, RealSignConclusion, RealSignRule, RealSignStep};
    let constant = frame.literal(cas, &base.constant, map, budget)?;
    let subject = frame.expression(cas, Operator::Neg, &[constant], false, budget)?;
    let equality = equality(cas, frame, &base.guard, &subject, &base.domain, budget)?;
    let predicate = admit(
        cas,
        &Draft::Condition {
            header: Reference::Existing(frame.header.clone()),
            scope: Vec::new(),
            kind: PredicateKind::Positive,
            arguments: vec![Reference::Existing(subject.clone())],
        },
        budget,
    )?;
    let region = frame.region.clone();
    frame.append(
        crate::rule::Primitive::Solve(SolveSchema::RealSign),
        Statement::PredicateFact {
            predicate,
            region: region.clone(),
        },
        Certificate::Solve(Box::new(SolveCertificate::RealSign(CertificateRealSign {
            subject: subject.clone(),
            region,
            root: 1,
            steps: vec![
                RealSignStep {
                    rule: RealSignRule::Hypothesis,
                    subject: base.guard.clone(),
                    conclusion: RealSignConclusion::Positive,
                    arguments: Vec::new(),
                    premises: Vec::new(),
                    exponent: 0,
                },
                RealSignStep {
                    rule: RealSignRule::Identity,
                    subject,
                    conclusion: RealSignConclusion::Positive,
                    arguments: vec![0],
                    premises: vec![equality],
                    exponent: 0,
                },
            ],
        }))),
        budget,
    )?;
    Ok(())
}

/// The Real proof is built entirely over S before its existence is pulled back to T.
fn witness(
    cas: &mut Cas,
    construction: &mut Construction,
    base: &Base,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let context = admit(
        cas,
        &Draft::Context(crate::domain_model::ContextDraft::Root {
            header: Reference::Existing(construction.header.clone()),
            admissible: Reference::Existing(base.stratum.clone()),
            assumptions: Vec::new(),
        }),
        budget,
    )?;
    let mut frame = Frame {
        inner: Construction {
            header: construction.header.clone(),
            context,
            region: base.stratum.clone(),
            steps: std::mem::take(&mut construction.steps),
        },
    };
    let images = crate::solving::real::render::images(cas, &frame, &base.domain, budget)
        .map_err(|error| fault(&error))?;
    let map = frame.mapping(cas, &base.domain, &images, budget)?;
    sign(cas, &mut frame, base, &map, budget)?;
    let engine = Engine {
        state: &cas.state,
        domain: &base.domain,
    };
    let coefficients = vec![
        engine
            .copy(&base.constant, budget)
            .map_err(|error| fault(&error))?,
        engine
            .constant(false, budget)
            .map_err(|error| fault(&error))?,
        engine
            .constant(true, budget)
            .map_err(|error| fault(&error))?,
    ];
    let polynomial = Dense::new(&engine, coefficients, budget).map_err(|error| fault(&error))?;
    let (selection, root) = frame.named(cas, &polynomial, &map, 2, budget)?;
    construction.steps = frame.inner.steps;
    let source = u32::try_from(
        construction
            .steps
            .len()
            .checked_sub(1)
            .ok_or_else(invalid)?,
    )
    .map_err(|_| invalid())?;
    let lifted = super::recipes::pullback(cas, &construction.region, budget)?;
    let subset = u32::try_from(construction.steps.len()).map_err(|_| invalid())?;
    construction.steps.push(ApplicationDraft {
        rule: crate::rule::Primitive::RegionConstruct,
        context: construction.context.clone(),
        conclusion: Statement::Subset {
            subset: construction.region.clone(),
            superset: lifted,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    });
    construction.steps.push(ApplicationDraft {
        rule: crate::rule::Primitive::ContextWeaken,
        context: construction.context.clone(),
        conclusion: Statement::Solve(Box::new(SolveStatement::RootExistence(
            StatementRootExistence {
                relation: selection,
                region: construction.region.clone(),
            },
        ))),
        premises: vec![source, subset],
        substitutions: Vec::new(),
        certificate: Certificate::None,
    });
    Ok(root)
}

/// The original Complex relation is inhabited by the explicitly placed Real witness.
pub(crate) fn choose(
    cas: &mut Cas,
    construction: &mut Construction,
    source: &Dense,
    target: &Handle,
    budget: &mut Budget,
) -> Result<Option<(Handle, Handle)>, Fault> {
    let Some(base) = candidate(cas, construction, source, budget)? else {
        return Ok(None);
    };
    let real = witness(cas, construction, &base, budget)?;
    super::root_selection::include(cas, construction, target, &real, base.imaginary, budget)
        .map(Some)
}

/// Existing Real identity replay compares declaration expressions with their exact base interpretation.
fn equality(
    cas: &mut Cas,
    frame: &mut Frame,
    left: &Handle,
    right: &Handle,
    domain: &Handle,
    budget: &mut Budget,
) -> Result<u32, Fault> {
    use crate::solving::{CertificateRealSign, RealSignConclusion, RealSignRule, RealSignStep};
    let unit = frame.unit(cas, domain, budget)?;
    let difference = frame.expression(
        cas,
        Operator::Sub,
        &[left.clone(), right.clone()],
        false,
        budget,
    )?;
    let region = frame.region.clone();
    frame.append(
        crate::rule::Primitive::Solve(SolveSchema::RealSign),
        Statement::Equality {
            left: left.clone(),
            right: right.clone(),
            domain: region.clone(),
            domain_contract: crate::proof_model::DomainContract::DefinedAndEqual,
        },
        Certificate::Solve(Box::new(SolveCertificate::RealSign(CertificateRealSign {
            subject: difference.clone(),
            region,
            root: 0,
            steps: vec![RealSignStep {
                rule: RealSignRule::Identity,
                subject: difference,
                conclusion: RealSignConclusion::Zero,
                arguments: Vec::new(),
                premises: vec![unit],
                exponent: 0,
            }],
        }))),
        budget,
    )
}

#[cfg(test)]
mod tests;
