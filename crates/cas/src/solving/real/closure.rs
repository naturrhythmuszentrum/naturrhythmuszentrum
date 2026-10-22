// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded semantic dependency inspection for genuinely Real claims and independent endpoints.

use super::{AT, invalid};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, Edge, Handle, Vertex};
use crate::model::{AtomicSort, Field, Sort};
use crate::solving::{MapTarget, SolveDraft};
use petgraph::visit::EdgeRef;
use std::collections::HashSet;

/// Reject actual Complex values and relation frames, including those hidden in exact scalar maps.
fn node(value: &Vertex) -> Result<(), Fault> {
    let complex = match value {
        Vertex::Expression(value) => value.sort == Sort::Value(AtomicSort::Scalar(Field::Complex)),
        Vertex::Solve(value) => match value.as_ref() {
            SolveDraft::RootRelation(value) => value.ambient != Field::Real,
            SolveDraft::Polynomial(value) => value.variables.contains(&Field::Complex),
            SolveDraft::CoefficientDomain(crate::solving::CoefficientDomainData::Gaussian) => true,
            SolveDraft::CoordinateSpace(value) => value
                .coordinates
                .iter()
                .any(|slot| slot.field == Field::Complex),
            SolveDraft::CoefficientMap(value) => matches!(
                &value.target,
                MapTarget::ScalarAmbient {
                    field: Field::Complex,
                    ..
                }
            ),
            _ => false,
        },
        _ => false,
    };
    if complex {
        return Err(invalid());
    }
    Ok(())
}

/// Header declaration catalogues are not expression dependencies; their parent scope remains visible.
#[expect(
    clippy::redundant_pub_crate,
    reason = "RealExact presentation shares the semantic closure check"
)]
pub(crate) fn check<'a>(
    cas: &Cas,
    roots: impl IntoIterator<Item = &'a Handle>,
    forbidden: Option<&Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut pending = crate::solving::coefficients::storage::vector(0, budget)?;
    let mut seen = HashSet::new();
    for root in roots {
        cas.checked(root)?;
        crate::solving::coefficients::storage::reserve(&mut pending, 1, budget)?;
        pending.push((
            *cas.state
                .indices
                .get(&root.logical_id())
                .ok_or_else(invalid)?,
            1_u64,
        ));
    }
    let forbidden = forbidden.and_then(|handle| cas.state.indices.get(&handle.logical_id()));
    while let Some((index, depth)) = pending.pop() {
        budget.work(1, AT)?;
        budget.check(BudgetKind::Depth, depth, AT)?;
        if Some(&index) == forbidden {
            return Err(invalid());
        }
        if seen.contains(&index) {
            continue;
        }
        budget.check(
            BudgetKind::Vertices,
            crate::graph::size(seen.len().saturating_add(1), AT)?,
            AT,
        )?;
        seen.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
        seen.insert(index);
        let value = cas.state.graph.node_weight(index).ok_or_else(invalid)?;
        node(value)?;
        if let Vertex::ScopedApplication(application) = value {
            for handle in super::closure_evidence::roots(cas, application, budget)? {
                crate::solving::coefficients::storage::reserve(&mut pending, 1, budget)?;
                pending.push((
                    *cas.state
                        .indices
                        .get(&handle.logical_id())
                        .ok_or_else(invalid)?,
                    depth.saturating_add(1),
                ));
            }
        }
        for edge in cas.state.graph.edges(index) {
            if matches!(value, Vertex::Header(_)) && !matches!(edge.weight(), Edge::ParentHeader) {
                continue;
            }
            crate::solving::coefficients::storage::reserve(&mut pending, 1, budget)?;
            pending.push((edge.target(), depth.saturating_add(1)));
        }
    }
    Ok(())
}

/// Presentation checks every supplied proof dependency, including intermediates absent from the final tuple.
#[expect(
    clippy::redundant_pub_crate,
    reason = "RealExact presentation checks its complete proof closure"
)]
pub(crate) fn evidence(
    cas: &Cas,
    set: &Handle,
    premises: &[crate::proof_model::CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut roots = crate::solving::coefficients::storage::vector(1, budget)?;
    roots.push(set.clone());
    for premise in premises {
        let additional = super::closure_evidence::roots(cas, premise, budget)?;
        crate::solving::coefficients::storage::reserve(&mut roots, additional.len(), budget)?;
        roots.extend(additional);
    }
    check(cas, &roots, None, budget)
}
