// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical snapshots across crate-internal physical graph repacking.

use std::collections::BTreeMap;

use petgraph::graph::DiGraph;
use petgraph::visit::EdgeRef;

use crate::limits::Limits;
use crate::syntax::{Draft, Environment, ExpressionDraft, Operator};
use crate::testing::common::{TestResult, default_budget, real};
use crate::{Cas, Rational, Reference};

/// Repacking physical indices cannot change the logical graph's canonical bytes.
/// ´test:crate:physical-index-independence´
#[test]
fn physical_index_independence() -> TestResult {
    let mut cas = Cas::new(Limits::default())?;
    let mut budget = default_budget()?;
    let handles = cas.admit(
        &[
            Draft::Source {
                key: "source".into(),
                text: "1".into(),
            },
            Draft::Header(crate::test_api::domain::HeaderDraft::Core(
                Environment::default(),
            )),
            Draft::Region(crate::test_api::domain::RegionDraft {
                header: Reference::Local(1),
                scope: Vec::new(),
                ambient: crate::test_api::domain::AmbientDomain::Assignments,
                expression: crate::test_api::domain::RegionExpr::Ambient,
            }),
            Draft::Context(crate::test_api::domain::ContextDraft::Root {
                header: Reference::Local(1),
                admissible: Reference::Local(2),
                assumptions: Vec::new(),
            }),
            Draft::Expression(ExpressionDraft {
                operator: Operator::Rational(Rational::one()),
                sort: real(),
                header: Reference::Local(1),
                scope: Vec::new(),
                binding: None,
                operands: Vec::new(),
            }),
        ],
        &mut budget,
    )?;
    let before = cas.canonical_snapshot(&mut budget)?;
    let mut graph = DiGraph::new();
    let mut remap = BTreeMap::new();
    let mut indices = BTreeMap::new();
    for (id, old_index) in cas.state.indices.iter().rev() {
        let index = graph.add_node(cas.state.node(*id)?.clone());
        remap.insert(*old_index, index);
        indices.insert(*id, index);
    }
    for edge in cas.state.graph.edge_references() {
        let source = remap
            .get(&edge.source())
            .copied()
            .ok_or("missing edge source")?;
        let target = remap
            .get(&edge.target())
            .copied()
            .ok_or("missing edge target")?;
        let _edge = graph.add_edge(source, target, *edge.weight());
    }
    assert_ne!(cas.state.indices, indices);
    cas.state.graph = graph;
    cas.state.indices = indices;
    assert_eq!(cas.canonical_snapshot(&mut budget)?, before);
    let expression = handles.last().ok_or("missing expression handle")?;
    assert_eq!(
        cas.expression(expression)?.0,
        &Operator::Rational(Rational::one())
    );
    Ok(())
}

/// Subset positions and canonical cover members retain their distinct graph roles.
/// ´test:crate:scoped-statement-edges´
#[test]
fn scoped_statement_edges() -> TestResult {
    use crate::domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
    use crate::graph::{Edge, Vertex};
    use crate::proof::{ApplicationDraft, Certificate, Primitive, Statement};

    let mut cas = Cas::new(Limits::default())?;
    let mut drafts = vec![Draft::Header(HeaderDraft::Core(Environment::default()))];
    for expression in [
        RegionExpr::Ambient,
        RegionExpr::Empty,
        RegionExpr::Union(vec![Reference::Local(2), Reference::Local(1)]),
    ] {
        drafts.push(Draft::Region(RegionDraft {
            header: Reference::Local(0),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression,
        }));
    }
    drafts.push(Draft::Context(ContextDraft::Root {
        header: Reference::Local(0),
        admissible: Reference::Local(1),
        assumptions: Vec::new(),
    }));
    let handles = cas.admit(&drafts, &mut default_budget()?)?;
    let ambient = handles.get(1).ok_or("ambient region")?;
    let empty = handles.get(2).ok_or("empty region")?;
    let union = handles.get(3).ok_or("union region")?;
    let context = handles.get(4).ok_or("root context")?;
    let cover_edges = vec![
        ((4, 0), context.logical_id()),
        ((40, 0), ambient.logical_id()),
        ((40, 1), empty.logical_id()),
        ((41, 0), union.logical_id()),
    ];
    let cases = [
        (
            Statement::Cover {
                domain: union.clone(),
                members: vec![empty.clone(), ambient.clone()],
            },
            cover_edges.clone(),
        ),
        (
            Statement::Cover {
                domain: union.clone(),
                members: vec![ambient.clone(), empty.clone()],
            },
            cover_edges,
        ),
        (
            Statement::Subset {
                subset: empty.clone(),
                superset: ambient.clone(),
            },
            vec![
                ((4, 0), context.logical_id()),
                ((39, 0), empty.logical_id()),
                ((39, 1), ambient.logical_id()),
            ],
        ),
    ];
    for (conclusion, expected) in cases {
        let step = ApplicationDraft {
            rule: Primitive::RegionConstruct,
            context: context.clone(),
            conclusion,
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::None,
        };
        let witness = cas.check_proof(context, &[step], &mut default_budget()?)?;
        let Vertex::Proof(application) = cas.state.node(witness.proof().logical_id())? else {
            return Err("missing checked application".into());
        };
        let index = *cas
            .state
            .indices
            .get(application)
            .ok_or("application index")?;
        let mut actual = BTreeMap::new();
        for edge in cas.state.graph.edges(index) {
            let role = match *edge.weight() {
                Edge::InContext => (4, 0),
                Edge::ComparedRegion(position) => (39, position),
                Edge::CoverMember(position) => (40, position),
                Edge::ArgumentRegion(position) => (41, position),
                _ => return Err("unexpected theorem edge role".into()),
            };
            assert!(actual.insert(role, edge.target()).is_none());
        }
        let mut expected_indices = BTreeMap::new();
        for (role, id) in expected {
            expected_indices.insert(role, *cas.state.indices.get(&id).ok_or("target index")?);
        }
        assert_eq!(actual, expected_indices);
    }
    Ok(())
}
