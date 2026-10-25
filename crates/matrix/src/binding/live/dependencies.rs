// SPDX-License-Identifier: AGPL-3.0-only
//! Extract immutable reading and proof closures once and reuse them for every write.

use super::{
    DependencyPolicy, Group, LiveBindingError, ProofDependency, RegionTicket, admission::charge,
};
use crate::{ReadingCoverage, SymbolState};
use nrz_bindings::{
    Budget,
    readings::algebra::{Cas, readings::DependencyInventory},
};
use std::{cell::Cell, rc::Rc};

/// Retained exact admission evidence or the flattened actual operand closure.
#[derive(Debug)]
pub(crate) enum ReadDependencies {
    /// Any write in the shared graph invalidates every reading.
    WholeBase,
    /// One authenticated dependency inventory per admitted reading.
    Entries(Vec<DependencyInventory>),
    /// Every published reading depends on all selected antecedents of its solve.
    Operands,
}

/// Prepare all exact closures before storage allocation or result evaluation.
pub(crate) fn prepare(
    graph: Rc<Cas>,
    coverage: ReadingCoverage<usize>,
    companions: Vec<crate::CompanionAdapter<usize>>,
    tickets: Vec<RegionTicket>,
    policy: DependencyPolicy,
    format: (u64, nrz_scalar::Precision),
    budget: &mut Budget,
) -> Result<Rc<Group>, LiveBindingError> {
    charge::<DependencyInventory>(coverage.readings().len(), budget)?;
    let mut reads = Vec::new();
    let mut proofs = Vec::new();
    if policy == DependencyPolicy::Entries {
        for (index, adapter) in coverage.readings().iter().enumerate() {
            reads.push(graph.reading_dependencies(adapter.reading().node(), budget)?);
            proofs.push(ProofDependency {
                inventory: graph
                    .witness_dependencies(adapter.representation().witness(), budget)?,
                readers: vec![index],
            });
        }
        for relation in coverage.overlaps() {
            let readers = coverage
                .readings()
                .iter()
                .enumerate()
                .filter_map(|(index, adapter)| {
                    let left = adapter.reading() == relation.left().reading();
                    let right = adapter.reading() == relation.right().reading();
                    (left || right).then_some(index)
                })
                .collect();
            proofs.push(ProofDependency {
                inventory: graph.witness_dependencies(relation.witness(), budget)?,
                readers,
            });
        }
    }
    let homogeneous = homogeneous(&graph, &coverage, budget)?;
    let mut companion_reads = Vec::new();
    // Whole-base writes already invalidate every reader before these fine inventories
    // could be queried. Extract them only for the policy that consumes them.
    if policy == DependencyPolicy::Entries {
        for companion in &companions {
            companion_reads.push(graph.reading_dependencies(companion.companion().node(), budget)?);
        }
    }
    let states = (0..coverage.readings().len())
        .map(|_| Cell::new(SymbolState::Pending))
        .collect();
    Ok(Rc::new(Group {
        scope: format.0,
        precision: format.1,
        graph,
        coverage,
        tickets,
        states,
        reads: match policy {
            DependencyPolicy::Entries => ReadDependencies::Entries(reads),
            DependencyPolicy::WholeBase => ReadDependencies::WholeBase,
        },
        proofs,
        homogeneous,
        companions,
        companion_reads,
        native: Vec::new(),
        antecedents: Vec::new(),
        result_definition: None,
        phasor_inputs: None,
        copies: Vec::new(),
    }))
}

/// Cache the closed homogeneous restriction once without discarding unsupported general readings.
fn homogeneous(
    graph: &Rc<Cas>,
    coverage: &ReadingCoverage<usize>,
    budget: &mut Budget,
) -> Result<Vec<Option<nrz_bindings::BindingSymbols>>, LiveBindingError> {
    let mut result = Vec::new();
    for adapter in coverage.readings() {
        match nrz_bindings::BindingSymbols::from_reading(
            Rc::clone(graph),
            adapter.reading(),
            budget,
        ) {
            Ok(symbols) => result.push(Some(symbols)),
            // Noncoherent parameter measures remain valid general readings; only the
            // optional coherent homogeneous recipe is unavailable for them.
            Err(
                nrz_bindings::AdmissionError::UnsupportedRecipe
                | nrz_bindings::AdmissionError::ValueMismatch,
            ) => result.push(None),
            Err(cause) => return Err(crate::HeapError::from(cause).into()),
        }
    }
    Ok(result)
}
