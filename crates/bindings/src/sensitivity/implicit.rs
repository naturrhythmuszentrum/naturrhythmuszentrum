// SPDX-License-Identifier: AGPL-3.0-only
//! Shared inspection of fixed polynomial defining relations and their partials.

use std::collections::BTreeSet;

use nrz_cas::Handle;

use super::{JacobianEntry, JacobianEvidence, SensitivityError, graph::Graph};
use crate::{CheckedExpression, PairingWitness};

/// Auxiliary output placeholders cannot alias one another or any independent input.
pub(crate) fn distinct(inputs: &[String], auxiliary: &[String]) -> Result<(), SensitivityError> {
    let mut seen = inputs.iter().collect::<BTreeSet<_>>();
    for key in auxiliary {
        if !seen.insert(key) {
            return Err(SensitivityError::Coordinates);
        }
    }
    Ok(())
}

/// Check every coefficient against a fixed recipe, retaining zero descriptors from the relation.
pub(crate) fn partials(
    graph: &mut Graph<'_>,
    originals: Vec<CheckedExpression>,
    coordinates: &[String],
    inputs: &[CheckedExpression],
    expected: &[Handle],
) -> Result<JacobianEvidence, SensitivityError> {
    let count = graph.extent(originals.len(), coordinates.len())?;
    if count != expected.len() || inputs.len() != coordinates.len() {
        return Err(SensitivityError::WitnessMismatch);
    }
    let mut witnesses = Vec::new();
    let mut entries = Vec::new();
    for (key, input) in coordinates.iter().zip(inputs) {
        for original in &originals {
            let position = entries.len();
            let witness = graph.differentiate(original, key)?;
            let expression = super::replay::derivative(graph, &witness, original, key)?;
            let coefficient = expected
                .get(position)
                .ok_or(SensitivityError::WitnessMismatch)?;
            let identity = graph.equal(&expression, coefficient)?;
            let pairing =
                PairingWitness::sensitivity(&original.descriptor().coherent(), input.descriptor())?;
            entries.push(JacobianEntry {
                expression,
                witness_index: witnesses.len(),
                pairing,
                identity,
            });
            witnesses.push(witness);
        }
    }
    Ok(JacobianEvidence {
        originals,
        coordinates: coordinates.to_vec(),
        witnesses,
        work: graph.work,
        entries,
    })
}
