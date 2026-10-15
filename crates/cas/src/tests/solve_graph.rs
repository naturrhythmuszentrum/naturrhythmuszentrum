// SPDX-License-Identifier: AGPL-3.0-only
//! Crate-visible typed solving dependency checks.

use crate::testing::common::TestResult;
use crate::testing::solve_graph::Fixture;

/// The stored problem has an original-predicate edge and no proof dependency.
/// ´test:crate:solve-problem-dependencies´
#[test]
fn solve_problem_dependencies() -> TestResult {
    let fixture = Fixture::new()?;
    let id = fixture.cas.checked(&fixture.problem)?;
    let index = fixture
        .cas
        .state
        .indices
        .get(&id)
        .ok_or("problem index missing")?;
    let edges: Vec<_> = fixture
        .cas
        .state
        .graph
        .edges(*index)
        .map(|edge| edge.weight())
        .collect();
    assert!(
        edges
            .iter()
            .any(|edge| matches!(edge, crate::graph::Edge::OriginalPredicate))
    );
    assert!(!edges.iter().any(|edge| matches!(
        edge,
        crate::graph::Edge::Justifies | crate::graph::Edge::Premise(_)
    )));
    assert_eq!(fixture.request()?.context, fixture.context);
    assert_eq!(
        fixture.cas.context_header(&fixture.context)?,
        fixture.header
    );
    Ok(())
}

/// A complete generated receipt cannot replace the required original-source receipt.
/// ´test:crate:solve-requires-original-receipt´
#[test]
fn solve_requires_original_receipt() -> TestResult {
    let mut fixture = Fixture::new()?;
    let mut request = fixture.request()?;
    request
        .inputs
        .first_mut()
        .ok_or("input missing")?
        .receipt
        .original = false;
    let before = fixture.cas.counts();
    let crate::request::Outcome::Invalid { fault, .. } = fixture.cas.run(&request) else {
        return Err("generated receipt accepted".into());
    };
    assert_eq!(
        fault.kind,
        crate::FaultKind::Invalid(crate::InvalidCode::Receipt)
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// Family products and bodies are ordinary typed dependencies, never proof back-references.
/// ´test:crate:solve-family-dependency-roles´
#[test]
fn solve_family_dependency_roles() -> TestResult {
    let mut fixture = Fixture::new()?;
    let draft = crate::testing::solve_graph::family_draft(&mut fixture, false)?;
    let family = crate::testing::solve_graph::admit(&mut fixture.cas, &draft)?;
    let index = fixture
        .cas
        .state
        .indices
        .get(&family.logical_id())
        .ok_or("family index missing")?;
    let roles: Vec<_> = fixture
        .cas
        .state
        .graph
        .edges(*index)
        .map(|edge| edge.weight())
        .collect();
    assert!(
        roles
            .iter()
            .any(|role| matches!(role, crate::graph::Edge::FamilyBody))
    );
    assert!(!roles.iter().any(|role| matches!(
        role,
        crate::graph::Edge::Justifies | crate::graph::Edge::Premise(_)
    )));
    Ok(())
}

/// The original denominator is a distinct ordered dependency of the retained problem.
/// ´test:crate:solve-original-denominator-edge´
#[test]
fn solve_original_denominator_edge() -> TestResult {
    let (fixture, _) = crate::testing::solve_graph::quotient_request(false)?;
    let index = fixture
        .cas
        .state
        .indices
        .get(&fixture.problem.logical_id())
        .ok_or("problem index missing")?;
    let roles: Vec<_> = fixture
        .cas
        .state
        .graph
        .edges(*index)
        .map(|edge| edge.weight())
        .collect();
    assert!(
        roles
            .iter()
            .any(|role| matches!(role, crate::graph::Edge::OriginalDenominator(0)))
    );
    Ok(())
}
