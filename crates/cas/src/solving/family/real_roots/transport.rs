// SPDX-License-Identifier: AGPL-3.0-only
//! Monomial coefficient signs are proposed in original Real syntax and checked by existing rules.

use crate::budget::Budget;
use crate::error::{Fault, FaultKind, InvalidCode};
use crate::graph::{Cas, Handle};
use crate::model::{Field, Operator, PredicateKind};
use crate::proof_model::{Certificate, Statement};
use crate::solving::coefficients::{
    model::{Element, Value},
    polynomial::Polynomial,
    scalar::Scalar,
};
use crate::solving::real::construction::Frame;
use crate::solving::{RealSignConclusion, SolveCertificate, SolveSchema};

/// Keep proposed Real syntax in its original header and region.
fn expression(
    cas: &mut Cas,
    region: &Handle,
    operator: Operator,
    operands: &[Handle],
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let frame = cas.state.region(region.logical_id())?;
    let header = cas.handle(frame.header);
    let scope = frame.scope.clone();
    crate::solving::family::build::expression(
        cas,
        &header,
        &scope,
        operator,
        Field::Real,
        operands,
        budget,
    )
}

/// Render only one rational monomial in the declared coordinate order.
fn monomial(
    cas: &mut Cas,
    region: &Handle,
    polynomial: &Polynomial,
    coordinates: &Handle,
    budget: &mut Budget,
) -> Result<Option<Handle>, Fault> {
    let [term] = polynomial.terms.as_slice() else {
        return Ok(None);
    };
    let Scalar::Rational(scalar) = &term.coefficient else {
        return Ok(None);
    };
    let value = crate::solving::coefficients::scalar::rational_copy(scalar, budget)?;
    let mut result = expression(cas, region, Operator::Rational(value), &[], budget)?;
    for (index, exponent) in term.powers.iter().enumerate() {
        budget.work(1, super::super::AT)?;
        if *exponent == 0 {
            continue;
        }
        let frame = crate::solving::graph::coordinates(&cas.state, coordinates.logical_id())?;
        let coordinate = frame.coordinates.get(index).ok_or_else(super::invalid)?;
        budget.work(
            crate::graph::size(coordinate.key.len(), super::super::AT)?,
            super::super::AT,
        )?;
        let key = coordinate.key.clone();
        let mut factor = expression(cas, region, Operator::Free(key), &[], budget)?;
        if *exponent > 1 {
            factor = expression(
                cas,
                region,
                Operator::IntegerPower(i64::from(*exponent)),
                &[factor],
                budget,
            )?;
        }
        result = expression(cas, region, Operator::Mul, &[result, factor], budget)?;
    }
    Ok(Some(result))
}

/// Propose a positive orientation only after all multiplicative signs are available.
pub(super) fn guard(
    cas: &mut Cas,
    region: &Handle,
    value: &Element,
    budget: &mut Budget,
) -> Result<Option<(Handle, bool, bool)>, Fault> {
    let Value::Fraction(fraction) = &value.value else {
        return Ok(None);
    };
    let crate::solving::CoefficientDomainData::Fraction { coordinates, .. } =
        crate::solving::coefficients::storage::domain(&cas.state, &value.domain)?
    else {
        return Ok(None);
    };
    let coordinates = coordinates.clone();
    let Some(top) = monomial(cas, region, &fraction.numerator, &coordinates, budget)? else {
        return Ok(None);
    };
    let Some(bottom) = monomial(cas, region, &fraction.denominator, &coordinates, budget)? else {
        return Ok(None);
    };
    let quotient = expression(cas, region, Operator::Div, &[top, bottom], budget)?;
    let proposal = match crate::strict_sign::certificate(cas, &quotient, region, budget) {
        Ok(proposal) => proposal,
        Err(Fault {
            kind: FaultKind::Invalid(InvalidCode::StrictSignUnavailable),
            ..
        }) => return Ok(None),
        Err(error) => return Err(error),
    };
    let negative = proposal.steps.last().ok_or_else(super::invalid)?.conclusion
        == RealSignConclusion::Negative;
    let subject = if negative {
        expression(cas, region, Operator::Neg, &[quotient], budget)?
    } else {
        quotient
    };
    Ok(Some((subject, negative, true)))
}

/// Positive syntax and its exact coefficient interpretation are joined by a defined equality.
pub(super) fn sign(
    cas: &mut Cas,
    frame: &mut Frame,
    guard: &Handle,
    constant: &Element,
    map: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let region = frame.region.clone();
    let certificate = crate::strict_sign::certificate(cas, guard, &region, budget)?;
    let source = frame.append(
        crate::rule::Primitive::Solve(SolveSchema::RealSign),
        Statement::Solve(Box::new(crate::solving::SolveStatement::RealSign(
            crate::solving::StatementRealSign {
                subject: guard.clone(),
                region: region.clone(),
                conclusion: RealSignConclusion::Positive,
            },
        ))),
        Certificate::Solve(Box::new(SolveCertificate::RealSign(certificate))),
        budget,
    )?;
    let literal = frame.literal(cas, constant, map, budget)?;
    let subject = frame.expression(cas, Operator::Neg, &[literal], false, budget)?;
    let equality = super::equality(cas, frame, guard, &subject, &constant.domain, budget)?;
    transported(cas, frame, guard, &subject, [source, equality], budget)
}

/// Retain the checked sign and equality as explicit earlier application premises.
fn transported(
    cas: &mut Cas,
    frame: &mut Frame,
    guard: &Handle,
    subject: &Handle,
    premises: [u32; 2],
    budget: &mut Budget,
) -> Result<(), Fault> {
    use crate::solving::{CertificateRealSign, RealSignRule, RealSignStep};
    let region = frame.region.clone();
    let predicate = crate::solving::radicals::construction::admit(
        cas,
        &crate::graph::Draft::Condition {
            header: crate::graph::Reference::Existing(frame.header.clone()),
            scope: Vec::new(),
            kind: PredicateKind::Positive,
            arguments: vec![crate::graph::Reference::Existing(subject.clone())],
        },
        budget,
    )?;
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
                    subject: guard.clone(),
                    conclusion: RealSignConclusion::Positive,
                    arguments: Vec::new(),
                    premises: vec![premises[0]],
                    exponent: 0,
                },
                RealSignStep {
                    rule: RealSignRule::Identity,
                    subject: subject.clone(),
                    conclusion: RealSignConclusion::Positive,
                    arguments: vec![0],
                    premises: vec![premises[1]],
                    exponent: 0,
                },
            ],
        }))),
        budget,
    )?;
    Ok(())
}

/// Choose the reciprocal positive square so the original divisor remains visible in the witness.
pub(super) fn inverse_square(
    cas: &mut Cas,
    region: &Handle,
    constant: &Element,
    budget: &mut Budget,
) -> Result<(Element, Handle), Fault> {
    use crate::solving::coefficients::{arithmetic::Engine, proof::fault};
    let engine = Engine {
        state: &cas.state,
        domain: &constant.domain,
    };
    let inverse =
        crate::solving::coefficients::transport::reciprocal(&engine, constant, region, budget)
            .map_err(|error| fault(&error))?;
    let radicand = engine
        .neg(&inverse, budget)
        .map_err(|error| fault(&error))?;
    let Some((subject, false, _)) = guard(cas, region, &radicand, budget)? else {
        return Err(super::invalid());
    };
    Ok((inverse, subject))
}

/// Keep the selected positive Real square root as the explicit strict divisor.
pub(super) fn inverse_root(
    cas: &mut Cas,
    region: &Handle,
    root: &Handle,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let one = expression(
        cas,
        region,
        Operator::Rational(crate::number::Rational::one()),
        &[],
        budget,
    )?;
    expression(cas, region, Operator::Div, &[one, root.clone()], budget)
}

/// Component evaluation needs the checked product sign in the total context as an explicit premise.
pub(super) fn lift_sign(
    construction: &mut crate::solving::radicals::construction::Construction,
    source: usize,
    subset: u32,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::PredicateFact { predicate, .. } = &construction
        .steps
        .get(source)
        .ok_or_else(super::invalid)?
        .conclusion
    else {
        return Err(super::invalid());
    };
    let predicate = predicate.clone();
    let source = u32::try_from(source).map_err(|_| super::invalid())?;
    crate::solving::coefficients::storage::reserve(&mut construction.steps, 1, budget)?;
    construction
        .steps
        .push(crate::proof_model::ApplicationDraft {
            rule: crate::rule::Primitive::ContextWeaken,
            context: construction.context.clone(),
            conclusion: Statement::PredicateFact {
                predicate,
                region: construction.region.clone(),
            },
            premises: vec![source, subset],
            substitutions: Vec::new(),
            certificate: Certificate::None,
        });
    Ok(())
}
