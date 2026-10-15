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

/// Exact scalar attachments preserve the selected coefficient and its map as typed graph edges.
/// ´test:crate:solve-coefficient-attachment-edges´
#[test]
fn solve_coefficient_attachment_edges() -> TestResult {
    let (cas, witness) = crate::testing::solve_graph::coefficient_witness()?;
    let crate::proof_model::Statement::PredicateFact { predicate, .. } =
        &witness.judgment().statement
    else {
        return Err("predicate fact missing".into());
    };
    let crate::graph::Vertex::Condition(condition) = cas.state.node(cas.checked(predicate)?)?
    else {
        return Err("predicate missing".into());
    };
    let literal = condition.arguments.first().ok_or("coefficient missing")?;
    let index = cas
        .state
        .indices
        .get(literal)
        .ok_or("literal index missing")?;
    let roles: Vec<_> = cas
        .state
        .graph
        .edges(*index)
        .map(|edge| edge.weight())
        .collect();
    assert!(
        roles
            .iter()
            .any(|role| matches!(role, crate::graph::Edge::CoefficientLiteralValue))
    );
    assert!(
        roles
            .iter()
            .any(|role| matches!(role, crate::graph::Edge::CoefficientLiteralMap))
    );
    Ok(())
}

/// Graph-kind and wire-record namespaces stay distinct and decode through ordinary admission.
/// ´test:crate:solve-wire-kind-namespace´
#[test]
fn solve_wire_kind_namespace() -> TestResult {
    let mut cas = crate::Cas::new(crate::limits::Limits::default())?;
    let object = crate::testing::solve_graph::admit(
        &mut cas,
        &crate::syntax::Draft::Solve(crate::solving::SolveDraft::CoefficientDomain(
            crate::solving::CoefficientDomainData::Gaussian,
        )),
    )?;
    let mut ledger = crate::limits::Budget::new(crate::limits::Limits::default())?;
    let record = crate::wire_records::describe(
        &cas,
        &crate::wire_build::Entity::Object(cas.checked(&object)?),
        &mut ledger,
    )?;
    assert_eq!(record.tag, 17);
    assert_eq!(object.logical_id().kind.tag(), 20);
    assert_eq!(record.references.len(), 0);
    let mut receiver = crate::Cas::new(crate::limits::Limits::default())?;
    let mut reader = crate::wire_read::Reader::new(&record.payload, &mut ledger)?;
    let imported =
        crate::solving::wire::decode_object(&mut receiver, record.tag, &mut reader, &[])?;
    reader.finished()?;
    assert!(matches!(
        receiver.solve_view(&imported)?,
        crate::solving::SolveView::CoefficientDomain(
            crate::solving::CoefficientDomainData::Gaussian
        )
    ));
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

/// A selected scalar has one typed selection dependency and no ordinary operand.
/// ´test:crate:solve-root-attachment-edges´
#[test]
fn solve_root_attachment_edges() -> TestResult {
    let (cas, witness) = crate::testing::solve_graph::root_witness()?;
    let crate::proof_model::Statement::PredicateFact { predicate, .. } =
        &witness.judgment().statement
    else {
        return Err("predicate missing".into());
    };
    let crate::graph::Vertex::Condition(condition) = cas.state.node(cas.checked(predicate)?)?
    else {
        return Err("condition missing".into());
    };
    let literal = condition.arguments.first().ok_or("root literal missing")?;
    let expression = cas.state.expression(*literal)?;
    assert_eq!(expression.operands, []);
    let index = cas.state.indices.get(literal).ok_or("root index missing")?;
    let roles: Vec<_> = cas
        .state
        .graph
        .edges(*index)
        .map(|edge| edge.weight())
        .collect();
    assert!(
        roles
            .iter()
            .any(|role| matches!(role, crate::graph::Edge::SelectedBy))
    );
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
