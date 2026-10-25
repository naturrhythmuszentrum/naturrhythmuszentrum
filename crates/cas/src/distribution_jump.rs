// SPDX-License-Identifier: AGPL-3.0-only
//! Complete first-order jump checking from finite polynomial cells and exact oriented traces.

use crate::budget::Budget;
use crate::distribution_model::{DistributionStatement, RegularCertificate};
use crate::distribution_syntax::{AT, invalid};
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::Operator;
use crate::number::Rational;
use crate::proof_model::{ApplicationDraft, Certificate, CheckedApplication, Statement};
use crate::univariate::Polynomial;

/// Independently check the regular derivative and every interior right-minus-left jump.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    if draft.certificate != Certificate::None {
        return Err(invalid());
    }
    let Statement::Distribution(value) = &draft.conclusion else {
        return Err(invalid());
    };
    let DistributionStatement::Derivative {
        distribution,
        derivative,
        space,
        order: 1,
    } = value.as_ref()
    else {
        return Err(invalid());
    };
    let embedded = cas.state.expression(distribution.logical_id())?;
    if embedded.operator != Operator::RegularEmbed {
        return Err(invalid());
    }
    let function = cas.handle(*embedded.operands.first().ok_or_else(invalid)?);
    let source = crate::distribution_regular::evidence(&function, space, premises)?;
    let terms = crate::distribution_linear::terms(cas, derivative.logical_id(), &[], budget)?;
    let mut regular = None;
    let mut impulses = Vec::new();
    for (term, coefficient) in terms.values() {
        let term = term.as_ref().ok_or_else(invalid)?;
        let expression = cas.state.expression(term.logical_id())?;
        let child = *expression.operands.first().ok_or_else(invalid)?;
        match expression.operator {
            Operator::RegularEmbed if regular.is_none() => {
                regular = Some((cas.handle(child), coefficient));
            }
            Operator::Delta { order: 0 } => impulses.push((
                crate::distribution_check::rational(cas, child)?.clone(),
                coefficient.clone(),
            )),
            _ => return Err(invalid()),
        }
    }
    let target = regular
        .map(|(function, factor)| {
            Ok::<_, Fault>((
                crate::distribution_regular::evidence(&function, space, premises)?,
                factor,
            ))
        })
        .transpose()?;
    regular_derivative(cas, source, target, budget)?;
    check_impulses(source, &impulses, budget)
}

/// Compare polynomial coefficients on the exact common refinement of both certificates.
fn regular_derivative(
    cas: &Cas,
    source: &RegularCertificate,
    target: Option<(&RegularCertificate, &Rational)>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut roots: Vec<_> = source.regions.iter().map(Handle::logical_id).collect();
    if let Some((target, _)) = target {
        roots.extend(target.regions.iter().map(Handle::logical_id));
    }
    let roots = crate::case_syntax::canonical(cas, &roots, budget)?;
    let merged = crate::region_cache::partition(cas, &roots, budget)?;
    for index in 0..=merged.cuts.len() {
        let Some(coefficients) = cell(source, &merged.cuts, index, budget)? else {
            continue;
        };
        let expected = Polynomial::new(coefficients, budget)?.derivative(budget)?;
        let actual = match target {
            Some((certificate, factor)) => {
                let coefficients =
                    cell(certificate, &merged.cuts, index, budget)?.ok_or_else(invalid)?;
                Polynomial::new(coefficients, budget)?.scale(factor, budget)?
            }
            None => Polynomial::zero(),
        };
        if expected != actual {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Locate a refined open interval by exact cut order, never by a numerical sample.
pub(crate) fn cell<'a>(
    certificate: &'a RegularCertificate,
    cuts: &[Rational],
    index: usize,
    budget: &mut Budget,
) -> Result<Option<&'a Vec<Rational>>, Fault> {
    let mut position = 0;
    if let Some(lower) = index.checked_sub(1).and_then(|i| cuts.get(i)) {
        for cut in &certificate.partition.cuts {
            if cut.compare(lower, budget)? == std::cmp::Ordering::Greater {
                break;
            }
            position += 1;
        }
    }
    certificate
        .polynomials
        .get(position)
        .map(Option::as_ref)
        .ok_or_else(invalid)
}

/// Subtract independently evaluated traces and account for every proposed impulse.
fn check_impulses(
    source: &RegularCertificate,
    actual: &[(Rational, Rational)],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut consumed = vec![false; actual.len()];
    for (index, cut) in source.partition.cuts.iter().enumerate() {
        let (Some(left), Some(right)) = (
            source.polynomials.get(index).and_then(Option::as_ref),
            source.polynomials.get(index + 1).and_then(Option::as_ref),
        ) else {
            continue;
        };
        let left = crate::jet_polynomial::trace(left, cut, 0, budget)?;
        let right = crate::jet_polynomial::trace(right, cut, 0, budget)?;
        let expected = right.sub(&left, budget)?;
        let mut sum = Rational::zero();
        for (position, (point, coefficient)) in actual.iter().enumerate() {
            if point == cut {
                sum = sum.add(coefficient, budget)?;
                *consumed.get_mut(position).ok_or_else(invalid)? = true;
            }
        }
        if sum != expected {
            return Err(invalid());
        }
    }
    if consumed.iter().any(|value| !value) {
        return Err(invalid());
    }
    Ok(())
}

/// Propose the regular derivative representative from the certified open-cell polynomials.
pub(crate) fn regular_output(
    cas: &mut Cas,
    function: &Handle,
    space: &Handle,
    certificate: &RegularCertificate,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let definition = cas.state.space(space.logical_id())?.clone();
    let domain = cas.handle(definition.domain);
    let header = cas.expression_header(function)?;
    let mut arms = Vec::new();
    let mut common = None;
    let mut same = true;
    for (index, coefficients) in certificate.polynomials.iter().enumerate() {
        let Some(coefficients) = coefficients else {
            continue;
        };
        let derived = Polynomial::new(coefficients, budget)?.derivative(budget)?;
        let polynomial = crate::polynomial::materialize_univariate(
            cas,
            function.logical_id(),
            &definition.coordinate,
            &derived.coefficients,
            budget,
        )?;
        if common.is_some_and(|previous| previous != polynomial) {
            same = false;
        }
        common = Some(polynomial);
        let cell = crate::boundary_regions::cell(
            cas,
            &domain,
            &definition.coordinate,
            &certificate.partition.cuts,
            index.checked_mul(2).ok_or_else(invalid)?,
            budget,
        )?;
        arms.push(crate::case_syntax::admit(
            cas,
            crate::graph::Draft::CaseArm(crate::domain_model::CaseArmDraft {
                placements: Vec::new(),
                header: crate::graph::Reference::Existing(header.clone()),
                scope: Vec::new(),
                region: crate::graph::Reference::Existing(cell),
                value: crate::graph::Reference::Existing(cas.handle(polynomial)),
            }),
            budget,
        )?);
    }
    if same {
        return common.map(|id| cas.handle(id)).ok_or_else(invalid);
    }
    crate::case_syntax::admit(
        cas,
        crate::graph::Draft::Cases(crate::domain_model::CasesDraft {
            sort: crate::model::Sort::Value(crate::model::AtomicSort::Scalar(
                crate::model::Field::Real,
            )),
            header: crate::graph::Reference::Existing(header),
            scope: Vec::new(),
            domain: crate::graph::Reference::Existing(domain),
            arms: arms
                .into_iter()
                .map(crate::graph::Reference::Existing)
                .collect(),
        }),
        budget,
    )
}

/// Produce impulses through repeated polynomial differentiation and Horner evaluation.
pub(crate) fn impulses(
    certificate: &RegularCertificate,
    budget: &mut Budget,
) -> Result<Vec<(Rational, Rational)>, Fault> {
    let mut result = Vec::new();
    for (index, point) in certificate.partition.cuts.iter().enumerate() {
        let (Some(left), Some(right)) = (
            certificate.polynomials.get(index).and_then(Option::as_ref),
            certificate
                .polynomials
                .get(index + 1)
                .and_then(Option::as_ref),
        ) else {
            continue;
        };
        let jump = evaluate(right, point, budget)?.sub(&evaluate(left, point, budget)?, budget)?;
        if !jump.is_zero() {
            result.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
            result.push((point.clone(), jump));
        }
    }
    Ok(result)
}

/// Horner evaluation is separate from the checker's factorial-weighted trace computation.
fn evaluate(values: &[Rational], point: &Rational, budget: &mut Budget) -> Result<Rational, Fault> {
    let mut result = Rational::zero();
    for coefficient in values.iter().rev() {
        result = result.mul(point, budget)?.add(coefficient, budget)?;
    }
    Ok(result)
}
