// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical snapshots across crate-internal physical graph repacking.

use std::collections::BTreeMap;

use petgraph::graph::DiGraph;
use petgraph::visit::EdgeRef;

use crate::testing::common::{TestResult, default_budget, real};
use crate::{Cas, Draft, Environment, ExpressionDraft, Limits, Operator, Rational, Reference};

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
            Draft::Context {
                environment: Environment::default(),
                assumptions: Vec::new(),
            },
            Draft::Expression(ExpressionDraft {
                operator: Operator::Rational(Rational::one()),
                sort: real(),
                context: Reference::Local(1),
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
