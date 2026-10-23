// SPDX-License-Identifier: AGPL-3.0-only
//! Per-entry parameter declarations retain exact value identities and formal physical pairings.

use super::{AdmissionError, QuantitySymbol};
use crate::{Descriptor, EvaluationDescriptor, PairingWitness};
use nrz_cas::{
    Cas, Handle, Location,
    limits::{Budget, BudgetKind},
};
use std::rc::Rc;

/// One exact logical coordinate of a declared physical value or source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParameterTarget {
    /// Actual immutable quantity or source declaration.
    symbol: QuantitySymbol,
    /// Exact logical coordinate within the declaration.
    index: u64,
}

impl ParameterTarget {
    /// Resolve an in-range original coordinate before constructing any parameter meaning.
    /// # Errors
    /// Refuses an entry outside the actual symbol's declared rectangle.
    pub fn new(symbol: QuantitySymbol, index: u64) -> Result<Self, AdmissionError> {
        let [rows, columns] = symbol.shape();
        if index >= u64::from(rows) * u64::from(columns) {
            return Err(AdmissionError::RegionInventory);
        }
        Ok(Self { symbol, index })
    }
    /// Actual retained symbol identity, including its graph owner.
    #[must_use]
    pub const fn symbol(&self) -> &QuantitySymbol {
        &self.symbol
    }
    /// Exact column-ordered logical coordinate.
    #[must_use]
    pub const fn index(&self) -> u64 {
        self.index
    }
}

/// Complete interpretation of one parameter coordinate.
#[derive(Clone, Debug)]
enum Meaning {
    /// A value-like parameter, such as a declared mean or factor coefficient.
    Value(ParameterTarget),
    /// Checked covariance, radius, variance or formal sensitivity with actual targets.
    Paired {
        /// Checked physical relation, retained without a scalar-kind surrogate.
        pairing: Box<PairingWitness>,
        /// Exact first value or source coordinate.
        first: ParameterTarget,
        /// Exact second covariance or sensitivity coordinate.
        other: Option<ParameterTarget>,
    },
    /// Numerical error on the parameter itself, retaining its exact symbol and entry.
    Evaluation {
        /// Actual parameter declaration targeted by this error.
        parameter: ParameterSymbol,
        /// Exact logical coordinate within that declaration.
        index: usize,
    },
}

/// A sealed physical parameter entry; equal units never substitute its targets.
#[derive(Clone, Debug)]
pub struct ParameterEntry(Meaning);

impl ParameterEntry {
    /// Declare a value-like coordinate with the exact target's physical meaning.
    #[must_use]
    pub const fn value(target: ParameterTarget) -> Self {
        Self(Meaning::Value(target))
    }
    /// Bind a checked physical pairing to both actual ordered target coordinates.
    /// # Errors
    /// Refuses substituted target descriptors or a missing/extra covariance or sensitivity side.
    pub fn paired(
        pairing: PairingWitness,
        first: ParameterTarget,
        other: Option<ParameterTarget>,
    ) -> Result<Self, AdmissionError> {
        pairing
            .validate_targets(
                first.symbol.descriptor(),
                other.as_ref().map(|target| target.symbol.descriptor()),
            )
            .map_err(|_| AdmissionError::CompanionMismatch)?;
        Ok(Self(Meaning::Paired {
            pairing: Box::new(pairing),
            first,
            other,
        }))
    }
    /// Declare an evaluation-radius entry on a complete exact parameter coordinate.
    /// # Errors
    /// Refuses a missing coordinate or nesting one evaluation-error declaration inside another.
    pub fn evaluation(parameter: ParameterSymbol, index: usize) -> Result<Self, AdmissionError> {
        let entry = parameter
            .entries()
            .get(index)
            .ok_or(AdmissionError::RegionInventory)?;
        if matches!(entry.0, Meaning::Evaluation { .. }) {
            return Err(AdmissionError::CompanionMismatch);
        }
        Ok(Self(Meaning::Evaluation { parameter, index }))
    }
    /// Value-like physical descriptor, when this entry has an ordinary scalar meaning.
    #[must_use]
    pub fn descriptor(&self) -> Option<&Descriptor> {
        match &self.0 {
            Meaning::Value(target) => Some(target.symbol.descriptor()),
            _ => None,
        }
    }
    /// Formal paired meaning, with no invented scalar kind for a sensitivity quotient.
    #[must_use]
    pub const fn pairing(&self) -> Option<&PairingWitness> {
        match &self.0 {
            Meaning::Paired { pairing, .. } => Some(pairing),
            _ => None,
        }
    }
    /// Exact ordered value/source coordinates, distinct from a parameter-error target.
    #[must_use]
    pub const fn targets(&self) -> Option<(&ParameterTarget, Option<&ParameterTarget>)> {
        match &self.0 {
            Meaning::Value(target) => Some((target, None)),
            Meaning::Paired { first, other, .. } => Some((first, other.as_ref())),
            Meaning::Evaluation { .. } => None,
        }
    }
    /// Exact parameter identity and entry targeted by numerical evaluation error.
    #[must_use]
    pub const fn evaluation_target(&self) -> Option<(&ParameterSymbol, usize)> {
        match &self.0 {
            Meaning::Evaluation { parameter, index } => Some((parameter, *index)),
            _ => None,
        }
    }
    /// Rebuild the existing formal evaluation descriptor when the target is paired.
    #[must_use]
    pub fn evaluation_descriptor(&self) -> Option<EvaluationDescriptor> {
        self.evaluation_target()
            .and_then(|(parameter, index)| parameter.entries().get(index))
            .and_then(Self::pairing)
            .map(EvaluationDescriptor::new)
    }
}

/// One actual mathematical parameter symbol with a complete sealed physical inventory.
#[derive(Clone, Debug)]
pub struct ParameterSymbol(Rc<Data>);

/// Immutable parameter graph and its complete physical entry inventory.
#[derive(Debug)]
struct Data {
    /// Actual mathematical graph authority.
    graph: Cas,
    /// Declared Real matrix expression.
    node: Handle,
    /// Literal rectangle in logical column order.
    shape: [u32; 2],
    /// Complete original per-entry target meanings.
    entries: Vec<ParameterEntry>,
}

impl ParameterSymbol {
    /// Declare the mathematical matrix and every physical parameter before numerical allocation.
    /// # Errors
    /// Refuses incomplete per-entry meanings or exhausted exact Work and payload ceilings.
    pub fn declare(
        shape: [u32; 2],
        entries: Vec<ParameterEntry>,
        budget: &mut Budget,
    ) -> Result<Self, AdmissionError> {
        let count = u64::from(shape[0]) * u64::from(shape[1]);
        let actual = u64::try_from(entries.len()).map_err(|_| AdmissionError::ResourceLimit)?;
        if count != actual {
            return Err(AdmissionError::RegionInventory);
        }
        let at = Location::new("parameter declaration");
        budget.check(BudgetKind::MatrixEntries, count, at)?;
        budget.check(
            BudgetKind::StagedPayloadBytes,
            count.saturating_mul(
                u64::try_from(std::mem::size_of::<ParameterEntry>())
                    .map_err(|_| AdmissionError::ResourceLimit)?,
            ),
            at,
        )?;
        budget.work(count, at)?;
        let (graph, node) = super::parameter_graph::declare(shape, budget)?;
        Ok(Self(Rc::new(Data {
            graph,
            node,
            shape,
            entries,
        })))
    }
    /// Actual mathematical CAS node; physical meaning remains in this sealed bindings record.
    #[must_use]
    pub fn node(&self) -> &Handle {
        &self.0.node
    }
    /// Exact graph authority, with no numerical payload or mutable escape.
    #[must_use]
    pub fn cas(&self) -> &Cas {
        &self.0.graph
    }
    /// Logical rectangle, independent of numerical precision and layout.
    #[must_use]
    pub fn shape(&self) -> [u32; 2] {
        self.0.shape
    }
    /// Every parameter coordinate in logical column order.
    #[must_use]
    pub fn entries(&self) -> &[ParameterEntry] {
        &self.0.entries
    }
}
