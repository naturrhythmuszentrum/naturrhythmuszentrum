// SPDX-License-Identifier: AGPL-3.0-only
//! Independent rational samples check returned factors and the entire degree-drop inventory.

use super::{Complex, Rational, Ring, compare, multiply_factor, number};
use crate::graph::{Cas, ExpressionAttachment, Handle, LogicalId, Vertex};
use crate::model::{Operator, PredicateKind};
use crate::solving::{CertificateProductCompleteness, SolveCertificate, SolveStatement, SolveView};
use crate::testing::common::TestResult;

/// Evaluate original scalar syntax without any coefficient normalizer or branch planner.
fn expression(cas: &Cas, id: LogicalId, bindings: &[(&str, Complex)]) -> TestResult<Complex> {
    let value = cas.state.expression(id)?;
    let argument = |index: usize| -> TestResult<Complex> {
        expression(
            cas,
            *value.operands.get(index).ok_or("sample operand")?,
            bindings,
        )
    };
    match &value.operator {
        Operator::Rational(value) => Ok(Complex {
            real: number(value)?,
            imaginary: Rational::integer(0),
        }),
        Operator::Free(key) => bindings
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| *value)
            .ok_or_else(|| "sample declaration".into()),
        Operator::Neg => argument(0)?.neg(),
        Operator::Add => argument(0)?.add(argument(1)?),
        Operator::Sub => argument(0)?.add(argument(1)?.neg()?),
        Operator::Mul => argument(0)?.mul(argument(1)?),
        Operator::Div => argument(0)?.div(argument(1)?),
        Operator::IntegerPower(power) => argument(0)?.pow(u32::try_from(*power)?),
        _ => Err("sample expression fragment".into()),
    }
}

/// A guard is evaluated from its original predicate operands, without asking regional arithmetic.
fn predicate(cas: &Cas, id: LogicalId, bindings: &[(&str, Complex)]) -> TestResult<bool> {
    let (kind, arguments) = match cas.state.node(id)? {
        Vertex::Condition(value) => (&value.kind, &value.arguments),
        Vertex::Expression(value) => {
            let Operator::Predicate(kind) = &value.operator else {
                return Err("sample predicate".into());
            };
            (kind, &value.operands)
        }
        _ => return Err("sample condition".into()),
    };
    match (kind, arguments.as_slice()) {
        (PredicateKind::Equal, [left, right]) => {
            Ok(expression(cas, *left, bindings)? == expression(cas, *right, bindings)?)
        }
        (PredicateKind::NonZero, [value]) => {
            Ok(expression(cas, *value, bindings)? != Complex::integer(0))
        }
        _ => Err("sample guard fragment".into()),
    }
}

/// Recursive containment tests every exact zero/nonzero ancestor, including local-context meets.
fn region(cas: &Cas, id: LogicalId, bindings: &[(&str, Complex)]) -> TestResult<bool> {
    use crate::domain_model::RegionExpr;
    match &cas.state.region(id)?.expression {
        RegionExpr::Ambient => Ok(true),
        RegionExpr::GuardTrue {
            base,
            predicate: guard,
        } => Ok(region(cas, *base, bindings)? && predicate(cas, *guard, bindings)?),
        RegionExpr::GuardFalse {
            base,
            predicate: guard,
        } => Ok(region(cas, *base, bindings)? && !predicate(cas, *guard, bindings)?),
        RegionExpr::Meet(members) => {
            for member in members {
                if !region(cas, *member, bindings)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Err("sample region fragment".into()),
    }
}

/// Preserve the returned exact coefficient and all of its true quotient parents during substitution.
fn coefficient(cas: &Cas, handle: &Handle) -> TestResult<Handle> {
    let value = cas.state.expression(handle.logical_id())?;
    let Some(ExpressionAttachment::ExactCoefficient { value, .. }) = &value.attachment else {
        return Err("sample exact coefficient".into());
    };
    Ok(cas.handle(*value))
}

/// Multiply actual linked factors in an independently evaluated quotient ring.
fn product(
    cas: &Cas,
    certificate: &CertificateProductCompleteness,
    bindings: &[(&str, Complex)],
    original: &[(i64, i64)],
) -> TestResult {
    let scalar = coefficient(cas, &certificate.scalar)?;
    let SolveView::CoefficientValue(value) = cas.solve_view(&scalar)? else {
        return Err("sample leading scalar".into());
    };
    let SolveView::SolveProblem(problem) = cas.solve_view(&certificate.problem)? else {
        return Err("sample original problem".into());
    };
    let ring = Ring::specialized(cas, &value.domain, bindings, &problem.coefficient_domain)?;
    let mut polynomial = vec![ring.value(cas, &scalar)?];
    for factor in &certificate.factors {
        let root = ring.value(cas, &coefficient(cas, &factor.root)?)?;
        for _ in 0..factor.multiplicity {
            polynomial = multiply_factor(&ring, &polynomial, &root)?;
        }
    }
    compare(&ring, &polynomial, original)
}

/// The zero family independently exposes its whole-coordinate identity map.
fn identity_family(cas: &Cas, family: &Handle) -> TestResult {
    let SolveView::FamilyMap(map) = cas.solve_view(family)? else {
        return Err("sample identity family".into());
    };
    let SolveView::Tuple(tuple) = cas.solve_view(&map.body)? else {
        return Err("sample identity tuple".into());
    };
    let [entry] = tuple.entries.as_slice() else {
        return Err("sample identity extent".into());
    };
    assert_eq!(
        cas.state.expression(entry.logical_id())?.operator,
        Operator::Bound { depth: 0, slot: 0 }
    );
    assert!(matches!(
        cas.state.region(map.region.logical_id())?.expression,
        crate::domain_model::RegionExpr::Ambient
    ));
    Ok(())
}

/// Each sample belongs to exactly one final branch, with its independently computed degree shape.
fn branch(
    cas: &Cas,
    set: &Handle,
    bindings: &[(&str, Complex)],
    degree: Option<usize>,
) -> TestResult<usize> {
    use crate::solving::SolutionBranchKind;
    let SolveView::SolutionSet(set) = cas.solve_view(set)? else {
        return Err("sample set".into());
    };
    let mut selected = Vec::new();
    for (index, handle) in set.branches.iter().enumerate() {
        let SolveView::SolutionBranch(value) = cas.solve_view(handle)? else {
            return Err("sample branch".into());
        };
        if region(cas, value.region.logical_id(), bindings)? {
            match (&value.kind, degree) {
                (SolutionBranchKind::Image { family }, None) => identity_family(cas, family)?,
                (SolutionBranchKind::Empty, Some(0)) => {}
                (SolutionBranchKind::Finite { tuples }, Some(degree)) if degree > 0 => {
                    assert_ne!(tuples.len(), 0);
                    assert!(tuples.len() <= degree);
                }
                _ => return Err("sample degree branch".into()),
            }
            selected.push(index);
        }
    }
    let [index] = selected.as_slice() else {
        return Err("sample does not have exactly one stratum".into());
    };
    Ok(*index)
}

/// One original cubic coefficient vector, before any trailing zero degree drop.
type Sample = [(i64, i64); 4];

/// Fixed post-search samples include every degree and a genuinely nonreal cubic.
fn fixtures() -> Vec<Sample> {
    let rational = [
        [-4, -15, 0, 1],
        [1, -3, 0, 1],
        [2, -3, 0, 1],
        [8, 0, 0, 1],
        [0, 0, 0, 1],
        [-2, 1, 1, 0],
        [1, 0, 1, 0],
        [2, 3, 0, 0],
        [7, 0, 0, 0],
        [0, 0, 0, 0],
    ];
    rational
        .into_iter()
        .map(|values| values.map(|value| (value, 0)))
        .chain(std::iter::once([(0, 2), (-2, 1), (-1, -1), (1, 0)]))
        .collect()
}

/// The active leaf and each retained factor product are checked against the original sample.
fn sample(
    cas: &Cas,
    set: &Handle,
    steps: &[crate::proof::ApplicationDraft],
    keys: &[&str],
    values: &Sample,
) -> TestResult<usize> {
    assert_eq!(keys.len(), values.len());
    let bindings: Vec<_> = keys
        .iter()
        .zip(values)
        .map(|(key, value)| {
            (
                *key,
                Complex {
                    real: Rational::integer(i128::from(value.0)),
                    imaginary: Rational::integer(i128::from(value.1)),
                },
            )
        })
        .collect();
    let degree = values.iter().rposition(|value| *value != (0, 0));
    let leaf = branch(cas, set, &bindings, degree)?;
    let mut products = 0;
    for step in steps {
        let crate::proof::Certificate::Solve(certificate) = &step.certificate else {
            continue;
        };
        let SolveCertificate::ProductCompleteness(certificate) = certificate.as_ref() else {
            continue;
        };
        if region(cas, certificate.region.logical_id(), &bindings)? {
            let count = degree.ok_or("zero family product")? + 1;
            product(cas, certificate, &bindings, &values[..count])?;
            products += 1;
        }
    }
    assert_eq!(products, usize::from(degree.is_some()));
    Ok(leaf)
}

/// Returned leaf indices compare renamed decision trees without comparing unrelated header bytes.
pub(crate) fn samples(
    cas: &Cas,
    witness: &crate::proof::VerifiedWitness,
    keys: &[&str],
) -> TestResult<Vec<usize>> {
    let crate::proof::Statement::Solve(statement) = &witness.judgment().statement else {
        return Err("sample solve claim".into());
    };
    let SolveStatement::Coverage(claim) = statement.as_ref() else {
        return Err("sample coverage".into());
    };
    let steps = super::super::audit::applications(cas, witness)?;
    fixtures()
        .iter()
        .map(|values| sample(cas, &claim.set, &steps, keys, values))
        .collect()
}

/// Undo the independently supplied declaration bijection in the actual split expressions.
fn renamed_expression(cas: &Cas, id: LogicalId, keys: &[&str]) -> TestResult<String> {
    let value = cas.state.expression(id)?;
    let head = match &value.operator {
        Operator::Rational(value) => format!("{:?}", number(value)?),
        Operator::Free(key) => format!(
            "slot{}",
            keys.iter()
                .position(|candidate| *candidate == key)
                .ok_or("renaming declaration")?
        ),
        Operator::Neg => "neg".into(),
        Operator::Add => "add".into(),
        Operator::Sub => "sub".into(),
        Operator::Mul => "mul".into(),
        Operator::Div => "div".into(),
        Operator::IntegerPower(power) => format!("power{power}"),
        _ => return Err("renaming expression fragment".into()),
    };
    let arguments = value
        .operands
        .iter()
        .map(|id| renamed_expression(cas, *id, keys))
        .collect::<TestResult<Vec<_>>>()?;
    Ok(format!("{head}({})", arguments.join(",")))
}

/// Every generated guard, including guards not hit by samples, has the same renamed syntax.
pub(crate) fn guards(
    cas: &Cas,
    witness: &crate::proof::VerifiedWitness,
    keys: &[&str],
) -> TestResult<Vec<String>> {
    let mut result = Vec::new();
    for step in super::super::audit::applications(cas, witness)? {
        let crate::proof::Certificate::Solve(certificate) = step.certificate else {
            continue;
        };
        let SolveCertificate::TotalPredicateSplit(split) = certificate.as_ref() else {
            continue;
        };
        if split.mode == crate::solving::PredicateSplitMode::ZeroNonzero {
            assert_eq!(split.children.len(), 2);
            result.push(renamed_expression(
                cas,
                split.expression.logical_id(),
                keys,
            )?);
        }
    }
    assert!(result.len() >= 4);
    Ok(result)
}
