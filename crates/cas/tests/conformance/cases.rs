// SPDX-License-Identifier: AGPL-3.0-only
//! Independent native case values, domains, overlap refusals and ownership contracts.

use crate::testing::cases::Fixture;
use crate::testing::common::TestResult;
use nrz_cas::proof::{DomainContract, Ownership, Statement};
use nrz_cas::request::{BoundaryFact, Outcome, Task};
use nrz_cas::syntax::{Operator, PredicateKind};
use nrz_cas::{FaultKind, InvalidCode};

/// The active manifest preserves the five native case schemas and refuses logical policy two.
/// ´test:integration:case-policy-five-preserves-native-manifest´
#[test]
fn case_policy_five_preserves_native_manifest() -> TestResult {
    let mut policy = nrz_cas::request::SearchPolicy::default();
    assert_eq!(policy.costs.key, "logical");
    assert_eq!(policy.costs.version, 5);
    assert_eq!(nrz_cas::proof::PRIMITIVES.len(), 52);
    let cases: Vec<_> = nrz_cas::proof::PRIMITIVES
        .iter()
        .zip(&policy.costs.weights)
        .map(|(primitive, weight)| (primitive.specification(), *weight))
        .filter(|(rule, _)| rule.key.starts_with("case."))
        .map(|(rule, weight)| (rule.key, rule.version, weight))
        .collect();
    assert_eq!(
        cases,
        vec![
            ("case.congruence", 1, 4),
            ("case.domain", 1, 2),
            ("case.eliminate", 1, 4),
            ("case.ordered", 1, 4),
            ("case.select", 1, 2)
        ]
    );
    let mut f = Fixture::new()?;
    let request = f.request(
        Task::Select {
            region: f.domain.clone(),
        },
        &[f.one.clone()],
    )?;
    policy.costs.version = 2;
    assert!(matches!(
        f.cas.run_with_policy(&request, &policy),
        Outcome::Invalid { .. }
    ));
    Ok(())
}

/// The first reached true clause owns zero, and swapping clauses changes the exact signature.
/// ´test:integration:ordered-case-priority-and-signature´
#[test]
fn ordered_case_priority_and_signature() -> TestResult {
    let mut f = Fixture::new()?;
    let nonnegative = f.predicate(PredicateKind::NonNegative, &[f.x.clone()])?;
    let at_zero = f.predicate(PredicateKind::Equal, &[f.x.clone(), f.zero.clone()])?;
    let zero = f.region(&at_zero, true)?;
    let two = f.number("2")?;
    let three = f.number("3")?;
    let first = f.ordered(
        &[
            (nonnegative.clone(), f.one.clone()),
            (at_zero.clone(), two.clone()),
        ],
        Some(three.clone()),
    )?;
    let (cases, original) = f.elaborate(&first)?;
    let request = f.request(
        Task::Select {
            region: zero.clone(),
        },
        &[cases],
    )?;
    assert!(matches!(f.proved(&request)?.witness.judgment().statement,
        Statement::Equality { ref right, .. } if *right == f.one));
    let swapped = f.ordered(
        &[(at_zero, two.clone()), (nonnegative, f.one.clone())],
        Some(three),
    )?;
    let (cases, _) = f.elaborate(&swapped)?;
    let request = f.request(Task::Select { region: zero }, &[cases])?;
    assert!(matches!(f.proved(&request)?.witness.judgment().statement,
        Statement::Equality { ref right, .. } if *right == two));
    let bytes = f.cas.export_witness(
        &original.witness,
        &mut crate::testing::budgets::derivation_budget()?,
    )?;
    let mut wrong = original.witness.judgment().clone();
    if let Statement::Elaboration { source, .. } = &mut wrong.statement {
        *source = swapped.syntax;
    }
    assert!(
        f.cas
            .replay(
                &bytes,
                &wrong,
                &mut crate::testing::budgets::derivation_budget()?
            )
            .is_err()
    );
    Ok(())
}

/// Affine associations can agree only at their exact singleton overlap.
/// ´test:integration:case-singleton-overlap-agreement´
#[test]
fn case_singleton_overlap_agreement() -> TestResult {
    let mut f = Fixture::new()?;
    let negative = f.term(Operator::Neg, &[f.x.clone()])?;
    let le = f.predicate(PredicateKind::NonNegative, &[negative])?;
    let ge = f.predicate(PredicateKind::NonNegative, &[f.x.clone()])?;
    let left = f.region(&le, true)?;
    let right = f.region(&ge, true)?;
    let compatible = f.cases(&[(left.clone(), f.x.clone()), (right.clone(), f.zero.clone())])?;
    let request = f.request(
        Task::Coverage {
            domain: f.domain.clone(),
        },
        &[compatible],
    )?;
    f.proved(&request)?;
    let two = f.number("2")?;
    let incompatible = f.cases(&[(left, f.one.clone()), (right, two)])?;
    let request = f.request(
        Task::Coverage {
            domain: f.domain.clone(),
        },
        &[incompatible],
    )?;
    assert!(matches!(f.cas.run(&request), Outcome::Refused { fault, .. }
        if fault.kind == FaultKind::Invalid(InvalidCode::OverlapConflict)));
    Ok(())
}

/// A local selection proves agreement only where the selected branch is requested.
/// ´test:integration:case-selection-local-compatibility´
#[test]
fn case_selection_local_compatibility() -> TestResult {
    let mut f = Fixture::new()?;
    let ge = f.predicate(PredicateKind::NonNegative, &[f.x.clone()])?;
    let positive = f.predicate(PredicateKind::Positive, &[f.x.clone()])?;
    let negative = f.region(&ge, false)?;
    let nonnegative = f.region(&ge, true)?;
    let positive = f.region(&positive, true)?;
    let two = f.number("2")?;
    let cases = f.cases(&[
        (negative.clone(), f.zero.clone()),
        (nonnegative, f.one.clone()),
        (positive, two),
    ])?;
    let request = f.request(Task::Select { region: negative }, &[cases])?;
    let result = f.proved(&request)?;
    assert!(matches!(&result.witness.judgment().statement,
        Statement::Equality { right, .. } if *right == f.zero));
    Ok(())
}

/// An undecided nonlinear cover and a proved missing singleton have distinct outcomes.
/// ´test:integration:case-unknown-totality-versus-hole´
#[test]
fn case_unknown_totality_versus_hole() -> TestResult {
    let mut f = Fixture::new()?;
    let square = f.term(Operator::Mul, &[f.x.clone(), f.x.clone()])?;
    let opaque = f.predicate(PredicateKind::Positive, &[square])?;
    let opaque = f.region(&opaque, true)?;
    let cases = f.cases(&[(opaque, f.one.clone())])?;
    let request = f.request(
        Task::Coverage {
            domain: f.domain.clone(),
        },
        &[cases],
    )?;
    assert!(matches!(
        f.cas.run(&request),
        Outcome::UnknownTotality { .. }
    ));
    let nonzero = f.predicate(PredicateKind::NonZero, &[f.x.clone()])?;
    let punctured = f.region(&nonzero, true)?;
    let cases = f.cases(&[(punctured, f.one.clone())])?;
    let request = f.request(
        Task::Coverage {
            domain: f.domain.clone(),
        },
        &[cases],
    )?;
    assert!(matches!(f.cas.run(&request), Outcome::Refused { fault, .. }
        if fault.kind == FaultKind::Invalid(InvalidCode::CoverageHole)));
    Ok(())
}

/// Cancellation is not total, while the explicitly filled point permits a two-direction merge.
/// ´test:integration:case-merge-both-definedness-directions´
#[test]
fn case_merge_both_definedness_directions() -> TestResult {
    let mut f = Fixture::new()?;
    let quotient = f.term(Operator::Div, &[f.x.clone(), f.x.clone()])?;
    let equal = Task::Equal {
        domain: f.domain.clone(),
        domain_contract: DomainContract::SameDefinedDomain,
    };
    let request = f.request(equal, &[quotient.clone(), f.one.clone()])?;
    assert!(matches!(f.cas.run(&request), Outcome::Refused { .. }));
    let nz = f.predicate(PredicateKind::NonZero, &[f.x.clone()])?;
    let nonzero = f.region(&nz, true)?;
    let zero = f.region(&nz, false)?;
    let cases = f.cases(&[
        (nonzero.clone(), quotient.clone()),
        (zero.clone(), f.one.clone()),
    ])?;
    let regions = vec![nonzero, zero];
    let request = f.request(
        Task::Merge {
            regions: regions.clone(),
            proposed_value: f.one.clone(),
        },
        std::slice::from_ref(&cases),
    )?;
    let result = f.proved(&request)?;
    assert!(result.primitive_steps >= 8);
    let request = f.request(
        Task::Merge {
            regions,
            proposed_value: quotient,
        },
        &[cases],
    )?;
    assert!(matches!(f.cas.run(&request), Outcome::Refused { .. }));
    Ok(())
}

/// One location supports independent function ownership; unsupported domains never imply absence.
/// ´test:integration:function-indexed-boundary-ownership´
#[test]
fn function_indexed_boundary_ownership() -> TestResult {
    let mut f = Fixture::new()?;
    let point = f.boundary(&f.zero.clone())?;
    let reciprocal = f.term(Operator::Div, &[f.one.clone(), f.x.clone()])?;
    let task = Task::Boundary {
        coordinate: "x".into(),
        point,
        requested_facts: vec![BoundaryFact::Ownership],
    };
    for (function, expected) in [
        (f.zero.clone(), Ownership::Present(f.zero.clone())),
        (f.one.clone(), Ownership::Present(f.one.clone())),
        (reciprocal, Ownership::Absent),
    ] {
        let request = f.request(task.clone(), std::slice::from_ref(&function))?;
        assert_eq!(
            f.proved(&request)?.witness.judgment().statement,
            Statement::Ownership {
                function,
                boundary: match &task {
                    Task::Boundary { point, .. } => point.clone(),
                    _ => return Err("wrong task".into()),
                },
                value: expected
            }
        );
    }
    let power = f.term(
        Operator::GeneralPower(nrz_cas::syntax::Branch::StrictReal),
        &[f.x.clone(), f.x.clone()],
    )?;
    let request = f.request(task, &[power])?;
    assert!(matches!(
        f.cas.run(&request),
        Outcome::PendingPremise { .. }
    ));
    Ok(())
}
