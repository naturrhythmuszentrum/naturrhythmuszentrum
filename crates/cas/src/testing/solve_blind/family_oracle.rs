// SPDX-License-Identifier: AGPL-3.0-only
//! Actual mixed factors and strata are inspected with independent exact arithmetic.

use super::{Complex, Rational, Ring, compare, multiply_factor, specialize};
use crate::graph::{Cas, ExpressionAttachment, Handle};
use crate::proof::{Certificate, Statement, VerifiedWitness};
use crate::solving::{
    CertificateProductCompleteness, SolutionBranchKind, SolveCertificate, SolveStatement, SolveView,
};
use crate::testing::common::TestResult;

/// Read the retained coefficient payload, without invoking normalization or root discovery.
fn coefficient(cas: &Cas, expression: &Handle) -> TestResult<Handle> {
    let Some(ExpressionAttachment::ExactCoefficient { value, .. }) =
        cas.state.expression(expression.logical_id())?.attachment
    else {
        return Err("family oracle literal".into());
    };
    Ok(cas.handle(value))
}

/// Every actual factor is multiplied in the independently decoded quotient ring.
fn product(
    cas: &Cas,
    value: &CertificateProductCompleteness,
    bindings: &[(&str, Complex)],
    expected: &[(i64, i64)],
) -> TestResult<Vec<u32>> {
    let scalar = coefficient(cas, &value.scalar)?;
    let SolveView::CoefficientValue(leading) = cas.solve_view(&scalar)? else {
        return Err("family oracle scalar".into());
    };
    let SolveView::SolveProblem(problem) = cas.solve_view(&value.problem)? else {
        return Err("family oracle problem".into());
    };
    let ring = Ring::specialized(cas, &leading.domain, bindings, &problem.coefficient_domain)?;
    let mut result = vec![ring.value(cas, &scalar)?];
    let mut multiplicities = Vec::new();
    for factor in &value.factors {
        let root = ring.value(cas, &coefficient(cas, &factor.root)?)?;
        let mut residual = ring.zero();
        for term in expected.iter().rev() {
            residual = ring.mul(&residual, &root)?;
            residual[0] = residual[0].add(Rational::integer(i128::from(term.0)))?;
            if term.1 != 0 {
                residual[1] = residual[1].add(Rational::integer(i128::from(term.1)))?;
            }
        }
        assert_eq!(residual, ring.zero());
        for _ in 0..factor.multiplicity {
            result = multiply_factor(&ring, &result, &root)?;
        }
        multiplicities.push(factor.multiplicity);
    }
    compare(&ring, &result, expected)?;
    multiplicities.sort_unstable();
    Ok(multiplicities)
}

/// Count actual returned tuple cells only after independently evaluating their exact region syntax.
fn cells(
    cas: &Cas,
    witness: &VerifiedWitness,
    bindings: &[(&str, Complex)],
    expected: usize,
) -> TestResult {
    let Statement::Solve(statement) = &witness.judgment().statement else {
        return Err("family oracle claim".into());
    };
    let SolveStatement::Coverage(claim) = statement.as_ref() else {
        return Err("family oracle coverage".into());
    };
    let SolveView::SolutionSet(set) = cas.solve_view(&claim.set)? else {
        return Err("family oracle set".into());
    };
    let mut active = 0;
    for handle in &set.branches {
        let SolveView::SolutionBranch(branch) = cas.solve_view(handle)? else {
            return Err("family oracle branch".into());
        };
        if specialize::region(cas, branch.region.logical_id(), bindings)? {
            let SolutionBranchKind::Finite { tuples } = &branch.kind else {
                return Err("family oracle finite roots".into());
            };
            assert_eq!(tuples.len(), expected);
            active += 1;
        }
    }
    assert_eq!(active, 1);
    Ok(())
}

/// Independent negative, zero and positive samples retain arbitrary nonreal fibre coefficients.
pub(crate) fn quadratic(cas: &Cas, witness: &VerifiedWitness, sign: i8) -> TestResult {
    let sample = match sign {
        -1 => -9,
        0 => 0,
        1 => 16,
        _ => return Err("family oracle sign".into()),
    };
    let bindings = [
        ("r", Complex::integer(sample)),
        (
            "c",
            Complex {
                real: Rational::integer(2),
                imaginary: Rational::integer(3),
            },
        ),
    ];
    cells(cas, witness, &bindings, if sign == 0 { 1 } else { 2 })?;
    let steps = super::super::audit::applications(cas, witness)?;
    let mut products = 0;
    for step in steps {
        let Certificate::Solve(certificate) = &step.certificate else {
            continue;
        };
        let SolveCertificate::ProductCompleteness(value) = certificate.as_ref() else {
            continue;
        };
        if specialize::region(cas, value.region.logical_id(), &bindings)? {
            let multiplicities = product(
                cas,
                value,
                &bindings,
                &[(-i64::try_from(sample)?, 0), (0, 0), (1, 0)],
            )?;
            assert_eq!(multiplicities, if sign == 0 { vec![2] } else { vec![1, 1] });
            products += 1;
        }
    }
    assert_eq!(products, 1);
    Ok(())
}

/// A nonreal cubic sample verifies every returned root and the full product independently.
pub(crate) fn cubic(cas: &Cas, witness: &VerifiedWitness) -> TestResult {
    let bindings = [
        ("r", Complex::integer(8)),
        (
            "c",
            Complex {
                real: Rational::integer(2),
                imaginary: Rational::integer(3),
            },
        ),
    ];
    cells(cas, witness, &bindings, 3)?;
    let mut products = 0;
    for step in super::super::audit::applications(cas, witness)? {
        let Certificate::Solve(certificate) = &step.certificate else {
            continue;
        };
        let SolveCertificate::ProductCompleteness(value) = certificate.as_ref() else {
            continue;
        };
        assert_eq!(
            product(cas, value, &bindings, &[(-8, 0), (2, 3), (0, 0), (1, 0)])?,
            [1, 1, 1]
        );
        products += 1;
    }
    assert_eq!(products, 1);
    Ok(())
}
