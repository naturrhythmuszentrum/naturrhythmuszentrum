// SPDX-License-Identifier: AGPL-3.0-only
//! Independent integer identities and rational guard evaluation of the returned Real strata.

use super::{Complex, specialize};
use crate::graph::{Cas, Handle};
use crate::proof_model::{Certificate, Statement, VerifiedWitness};
use crate::solving::{SolutionBranchKind, SolveCertificate, SolveStatement, SolveView};
use crate::testing::common::TestResult;

/// Horner evaluation uses checked native integers, with no solver coefficient or sign operation.
fn at(coefficients: &[i64], point: i64) -> TestResult<i64> {
    coefficients
        .iter()
        .rev()
        .try_fold(0_i64, |value, coefficient| {
            value
                .checked_mul(point)
                .and_then(|value| value.checked_add(*coefficient))
                .ok_or_else(|| "Real integer oracle overflow".into())
        })
}

/// Exact signs, derivatives and factors justify the named root inventories before comparison.
fn identities() -> TestResult {
    let cubic = [1, -3, 0, 1];
    let values: Vec<_> = [-2, -1, 0, 1, 2]
        .iter()
        .map(|point| at(&cubic, *point))
        .collect::<TestResult<_>>()?;
    assert_eq!(values, [-1, 3, 1, -1, 3]);
    // Three disjoint sign changes exhaust a cubic; neither rational candidate is a root.
    assert_ne!(at(&cubic, -1)?, 0);
    assert_ne!(at(&cubic, 1)?, 0);
    assert_eq!(super::super::derivative(&[1, 1, 0, 1])?, [1, 0, 3]);
    assert_eq!((at(&[1, 1, 0, 1], -1)?, at(&[1, 1, 0, 1], 0)?), (-1, 1));
    // The derivative is 1+3x², so this sign change is the unique root.
    assert_eq!(super::super::multiply(&[1, -2, 1], &[2, 1])?, [2, -3, 0, 1]);
    assert_eq!(super::super::multiply(&[2, 1], &[4, -2, 1])?, [8, 0, 0, 1]);
    // x²-2x+4=(x-1)²+3 and x²+1 are strictly positive.
    assert_eq!(super::super::multiply(&[-1, 1], &[-1, 1])?, [1, -2, 1]);
    assert_eq!(super::super::multiply(&[-1, 1], &[1, 1])?, [-1, 0, 1]);
    Ok(())
}

type Sample = ([i64; 4], Option<usize>, &'static [u32]);

/// Inputs are original ascending coefficients, independent expected distinct counts and multiplicities.
const fn fixtures() -> [Sample; 12] {
    [
        ([1, -3, 0, 1], Some(3), &[]),
        ([1, 1, 0, 1], Some(1), &[]),
        ([8, 0, 0, 1], Some(1), &[]),
        ([3, -3, 0, 1], Some(1), &[]),
        ([2, -3, 0, 1], Some(2), &[1, 2]),
        ([0, 0, 0, 1], Some(1), &[3]),
        ([1, 0, 1, 0], Some(0), &[]),
        ([-1, 0, 1, 0], Some(2), &[1, 1]),
        ([1, -2, 1, 0], Some(1), &[2]),
        ([-2, 1, 0, 0], Some(1), &[1]),
        ([1, 0, 0, 0], Some(0), &[]),
        ([0, 0, 0, 0], None, &[]),
    ]
}

/// Every sample selects one actual returned branch; expected counts never come from the certificate.
fn branch(
    cas: &Cas,
    set: &Handle,
    bindings: &[(&str, Complex)],
    count: Option<usize>,
) -> TestResult<usize> {
    let SolveView::SolutionSet(set) = cas.solve_view(set)? else {
        return Err("Real oracle set".into());
    };
    let mut selected = Vec::new();
    for (index, handle) in set.branches.iter().enumerate() {
        let SolveView::SolutionBranch(value) = cas.solve_view(handle)? else {
            return Err("Real oracle branch".into());
        };
        if !specialize::region(cas, value.region.logical_id(), bindings)? {
            continue;
        }
        match (&value.kind, count) {
            (SolutionBranchKind::Empty, Some(0)) => {}
            (SolutionBranchKind::Finite { tuples }, Some(expected)) => {
                assert_eq!(tuples.len(), expected);
            }
            (SolutionBranchKind::Image { family }, None) => {
                specialize::identity_family(cas, family)?;
            }
            _ => return Err("Real oracle branch shape".into()),
        }
        selected.push(index);
    }
    let [index] = selected.as_slice() else {
        return Err("Real oracle unique stratum".into());
    };
    Ok(*index)
}

/// Repeated-root multiplicities are read from the accepted factor theorem, then compared to integer products.
fn multiplicities(
    cas: &Cas,
    steps: &[crate::proof_model::ApplicationDraft],
    bindings: &[(&str, Complex)],
    expected: &[u32],
) -> TestResult {
    if expected.is_empty() {
        return Ok(());
    }
    let mut seen = 0;
    for step in steps {
        let Certificate::Solve(certificate) = &step.certificate else {
            continue;
        };
        let SolveCertificate::ProductCompleteness(value) = certificate.as_ref() else {
            continue;
        };
        if !specialize::region(cas, value.region.logical_id(), bindings)? {
            continue;
        }
        let mut actual: Vec<_> = value
            .factors
            .iter()
            .map(|factor| factor.multiplicity)
            .collect();
        actual.sort_unstable();
        assert_eq!(actual, expected);
        seen += 1;
    }
    assert_eq!(seen, 1);
    Ok(())
}

/// All degree and discriminant cases are independently specialized only after universal search finishes.
pub(crate) fn samples(cas: &Cas, witness: &VerifiedWitness) -> TestResult<Vec<usize>> {
    identities()?;
    let Statement::Solve(statement) = &witness.judgment().statement else {
        return Err("Real oracle claim".into());
    };
    let SolveStatement::Coverage(claim) = statement.as_ref() else {
        return Err("Real oracle coverage".into());
    };
    let steps = super::super::audit::applications(cas, witness)?;
    let mut inventory = Vec::new();
    for (coefficients, count, repeated) in fixtures() {
        let bindings: Vec<_> = ["d", "c", "b", "a"]
            .into_iter()
            .zip(coefficients.map(|value| Complex::integer(i128::from(value))))
            .collect();
        inventory.push(branch(cas, &claim.set, &bindings, count)?);
        multiplicities(cas, &steps, &bindings, repeated)?;
    }
    assert_eq!(
        inventory
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        12
    );
    Ok(inventory)
}
