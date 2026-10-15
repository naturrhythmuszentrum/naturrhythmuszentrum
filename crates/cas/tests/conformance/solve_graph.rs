// SPDX-License-Identifier: AGPL-3.0-only
//! Public solving graph admission and immutable view contracts.

use crate::testing::common::TestResult;
use crate::testing::solve_graph::{Fixture, admit};
use nrz_cas::limits::{Budget, Limits};
use nrz_cas::solving::{
    CoefficientDomainData, Coordinate, CoordinateRole, CoordinateSpaceData, SolutionSetData,
    SolveDraft, SolveView, TupleData,
};
use nrz_cas::syntax::{Draft, Field, VertexKind};
use nrz_cas::{FaultKind, InvalidCode, Reference};

/// Proposed empty sets have no completeness flag and intern by their exact problem.
/// ´test:integration:solve-set-is-syntax-only´
#[test]
fn solve_set_is_syntax_only() -> TestResult {
    let mut fixture = Fixture::new()?;
    let draft = Draft::Solve(SolveDraft::SolutionSet(SolutionSetData {
        problem: Reference::Existing(fixture.problem.clone()),
        branches: Vec::new(),
    }));
    let set = admit(&mut fixture.cas, &draft)?;
    assert_eq!(set.logical_id().kind, VertexKind::SolutionSet);
    let before = fixture.cas.counts();
    assert_eq!(admit(&mut fixture.cas, &draft)?, set);
    assert_eq!(fixture.cas.counts(), before);
    let SolveView::SolutionSet(value) = fixture.cas.solve_view(&set)? else {
        return Err("wrong view".into());
    };
    assert_eq!(value.branches, []);
    assert_eq!(value.problem, fixture.problem);
    Ok(())
}

/// Foreign product links and forward references fail without publishing a batch prefix.
/// ´test:integration:solve-admission-rollback´
#[test]
fn solve_admission_rollback() -> TestResult {
    let mut fixture = Fixture::new()?;
    let other = Fixture::new()?;
    let before = fixture.cas.counts();
    let drafts = [
        Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Gaussian,
        )),
        Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
            header: Reference::Existing(other.header),
            coordinates: Vec::new(),
        })),
    ];
    let fault = fixture
        .cas
        .admit(&drafts, &mut Budget::new(Limits::default())?)
        .expect_err("foreign header accepted");
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::ForeignHandle));
    assert_eq!(fixture.cas.counts(), before);
    let forward = Draft::Solve(SolveDraft::CoefficientDomain(
        CoefficientDomainData::Quotient {
            level: Reference::Local(0),
        },
    ));
    assert!(
        fixture
            .cas
            .admit(
                std::slice::from_ref(&forward),
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// Coordinate order and field are checked, and an empty product has one empty tuple.
/// ´test:integration:solve-product-and-empty-tuple´
#[test]
fn solve_product_and_empty_tuple() -> TestResult {
    let mut fixture = Fixture::new()?;
    let empty = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
            header: Reference::Existing(fixture.header.clone()),
            coordinates: Vec::new(),
        })),
    )?;
    let tuple = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::Tuple(TupleData {
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            coordinates: Reference::Existing(empty),
            entries: Vec::new(),
        })),
    )?;
    let SolveView::Tuple(value) = fixture.cas.solve_view(&tuple)? else {
        return Err("wrong tuple view".into());
    };
    assert_eq!(value.entries, []);
    let coordinate = Coordinate {
        key: "x".into(),
        field: Field::Real,
        role: CoordinateRole::Unknown,
    };
    let duplicate = Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
        header: Reference::Existing(fixture.header),
        coordinates: vec![coordinate.clone(), coordinate],
    }));
    assert!(
        fixture
            .cas
            .admit(
                std::slice::from_ref(&duplicate),
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    Ok(())
}

/// A bound family slot is admitted while a free use of its declaration is rejected atomically.
/// ´test:integration:solve-family-capture-refusal´
#[test]
fn solve_family_capture_refusal() -> TestResult {
    let mut fixture = Fixture::new()?;
    let valid = crate::testing::solve_graph::family_draft(&mut fixture, false)?;
    let family = admit(&mut fixture.cas, &valid)?;
    assert!(matches!(
        fixture.cas.solve_view(&family)?,
        SolveView::FamilyMap(_)
    ));
    let escaped = crate::testing::solve_graph::family_draft(&mut fixture, true)?;
    let before = fixture.cas.counts();
    assert!(
        fixture
            .cas
            .admit(
                std::slice::from_ref(&escaped),
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// Canonical parent constants allow monic extensions at every earlier quotient level.
/// ´test:integration:solve-monic-nested-towers´
#[test]
fn solve_monic_nested_towers() -> TestResult {
    use nrz_cas::solving::{
        CoefficientValueData, CoefficientValueObject, ExtensionLevelData, PolynomialData,
        PolynomialTerm, SquareFreeRequest,
    };
    use nrz_cas::syntax::AtomicSort;
    let mut cas = nrz_cas::Cas::new(Limits::default())?;
    let mut domain = admit(
        &mut cas,
        &Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Rational,
        )),
    )?;
    let mut one = admit(
        &mut cas,
        &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
            domain: Reference::Existing(domain.clone()),
            value: CoefficientValueData::Rational {
                value: nrz_cas::Rational::one(),
            },
        })),
    )?;
    for _ in 0..2 {
        let polynomial = admit(
            &mut cas,
            &Draft::Solve(SolveDraft::Polynomial(PolynomialData {
                domain: Reference::Existing(domain.clone()),
                variables: vec![Field::Rational],
                terms: vec![PolynomialTerm {
                    exponents: vec![1],
                    coefficient: Reference::Existing(one.clone()),
                }],
            })),
        )?;
        let level = admit(
            &mut cas,
            &Draft::Solve(SolveDraft::ExtensionLevel(ExtensionLevelData {
                parent: Reference::Existing(domain),
                polynomial: Reference::Existing(polynomial),
                generator: vec![AtomicSort::Scalar(Field::Rational)],
                square_free: SquareFreeRequest::NotRequested,
            })),
        )?;
        domain = admit(
            &mut cas,
            &Draft::Solve(SolveDraft::CoefficientDomain(
                CoefficientDomainData::Quotient {
                    level: Reference::Existing(level),
                },
            )),
        )?;
        one = admit(
            &mut cas,
            &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
                domain: Reference::Existing(domain.clone()),
                value: CoefficientValueData::Tower {
                    coefficients: vec![Reference::Existing(one)],
                },
            })),
        )?;
    }
    assert!(matches!(
        cas.solve_view(&one)?,
        SolveView::CoefficientValue(_)
    ));
    Ok(())
}

/// A finite solution proposal cannot leave an original unknown as a free family parameter.
/// ´test:integration:solve-finite-branch-capture-refusal´
#[test]
fn solve_finite_branch_capture_refusal() -> TestResult {
    use nrz_cas::solving::{SolutionBranchData, SolutionBranchKind};
    use nrz_cas::syntax::{AtomicSort, Operator, Sort};
    let mut fixture = Fixture::new()?;
    let SolveView::SolveProblem(problem) = fixture.cas.solve_view(&fixture.problem)? else {
        return Err("problem missing".into());
    };
    let coordinates = Reference::Existing(problem.unknowns.clone());
    let region = Reference::Existing(problem.region.clone());
    let domain = Reference::Existing(problem.domain.clone());
    let entry = admit(
        &mut fixture.cas,
        &crate::testing::solve_graph::expression(
            Reference::Existing(fixture.header.clone()),
            Operator::Free("x".into()),
            Sort::Value(AtomicSort::Scalar(Field::Real)),
            Vec::new(),
        ),
    )?;
    let tuple = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::Tuple(TupleData {
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            coordinates,
            entries: vec![Reference::Existing(entry)],
        })),
    )?;
    let branch = Draft::Solve(SolveDraft::SolutionBranch(SolutionBranchData {
        problem: Reference::Existing(fixture.problem.clone()),
        region,
        domain,
        kind: SolutionBranchKind::Finite {
            tuples: vec![Reference::Existing(tuple)],
        },
    }));
    let before = fixture.cas.counts();
    assert!(
        fixture
            .cas
            .admit(
                std::slice::from_ref(&branch),
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}
