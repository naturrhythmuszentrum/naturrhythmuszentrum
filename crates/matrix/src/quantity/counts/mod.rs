// SPDX-License-Identifier: AGPL-3.0-only
//! Exact finite counts share their checked source owner and never enter a floating arena.

mod arithmetic;
mod ratio;

pub use ratio::ExactCountRatio;

use super::QuantityContext;
use nrz_bindings::readings::algebra::{Fault, Location, Rational, limits::BudgetKind};
use nrz_bindings::{Budget, CountDescriptor, Descriptor};
use std::rc::Rc;

/// An exact finite count, backed by its original bindings value or one checked integer result.
#[derive(Clone, Debug)]
pub struct CountQuantity(Rc<Data>);

/// One immutable count node, retaining its source descriptor without copying it.
#[derive(Debug)]
struct Data {
    /// Original checked counted-kind authority and exact source evidence.
    source: CountDescriptor,
    /// Only computed nodes need another exact integer; imports borrow the source's value.
    value: Option<u64>,
    /// Original operands and exact operation, with no numerical certificate or wire format.
    history: CountHistory,
    /// Conservative complete retained lineage envelope, including shared parents in full.
    bytes: u64,
    /// Maximum immutable parent depth, bounded by the caller's existing depth policy.
    depth: u64,
}

/// Exact operation provenance, separate from statistical or floating evidence.
#[derive(Clone, Debug)]
pub enum CountHistory {
    /// The original bindings count owns the only stored source value.
    Source,
    /// Compatible counted things were added with checked integer arithmetic.
    Add(CountQuantity, CountQuantity),
    /// Compatible counted things were subtracted within nonnegative finite storage.
    Subtract(CountQuantity, CountQuantity),
    /// A whole-number multiplicity scales the original count.
    Scale(CountQuantity, u64),
    /// An exact whole-number partition preserves counted meaning.
    Quotient(CountQuantity, u64),
}

/// A count refusal has no partially published integer or floating result.
#[derive(Debug, thiserror::Error)]
pub enum CountError {
    /// The original exact source did not satisfy bindings admission.
    #[error("count source: {0}")]
    Source(Box<nrz_bindings::DecisionError>),
    /// Counted kind or the actual integer result's domain was incompatible.
    #[error("count meaning: {0}")]
    Units(#[from] nrz_bindings::UnitError),
    /// Exact construction or immutable lineage exceeded caller resources.
    #[error("count resources: {0}")]
    Resources(#[from] Fault),
    /// The exact integer result cannot be represented by unsigned 64-bit count storage.
    #[error("exact count exceeds unsigned 64-bit storage")]
    Overflow,
    /// The result exceeds the caller's explicit executable count capacity.
    #[error("exact count exceeds caller capacity")]
    Capacity,
    /// A count quotient is not a whole number.
    #[error("count quotient is fractional")]
    Fractional,
    /// Division or an explicit ratio has a zero original divisor.
    #[error("count divisor is zero")]
    ZeroDivisor,
}

impl From<nrz_bindings::DecisionError> for CountError {
    fn from(fault: nrz_bindings::DecisionError) -> Self {
        Self::Source(Box::new(fault))
    }
}

impl QuantityContext {
    /// Retain an exact checked count declaration without allocating a numerical region.
    /// # Errors
    /// Refuses nonconstant/noncount declarations, fractional values, capacity and caller resources.
    pub fn count(
        &self,
        name: &str,
        capacity: u64,
        budget: &mut Budget,
    ) -> Result<CountQuantity, CountError> {
        let source =
            CountDescriptor::declared(Rc::clone(&self.declarations), name, capacity, budget)?;
        CountQuantity::capture(source, budget)
    }
}

impl CountQuantity {
    /// Retain an already admitted declared, proved or decided count without copying its value.
    /// # Errors
    /// Refuses exhausted Work or retained lineage capacity.
    pub fn capture(source: CountDescriptor, budget: &mut Budget) -> Result<Self, CountError> {
        charge(budget)?;
        let bytes = envelope(source.retained_payload_bytes(), 1, budget)?;
        Ok(Self(Rc::new(Data {
            source,
            value: None,
            history: CountHistory::Source,
            bytes,
            depth: 1,
        })))
    }

    /// Original counted kind, actual mathematical domain and coherent integer measure.
    #[must_use]
    pub fn descriptor(&self) -> &Descriptor {
        self.0.source.descriptor()
    }

    /// The exact whole-number value, never rounded, wrapped or clamped.
    #[must_use]
    pub fn value(&self) -> u64 {
        self.0.value.unwrap_or_else(|| self.0.source.value())
    }

    /// Original exact source owner, including its declaration, proof or decision evidence.
    #[must_use]
    pub fn source(&self) -> &CountDescriptor {
        &self.0.source
    }

    /// Immutable exact operands and operation retained with this value.
    #[must_use]
    pub fn history(&self) -> &CountHistory {
        &self.0.history
    }

    /// Conservative logical lineage bytes, excluding allocator capacity and shared source owners.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }

    /// Check the actual known result rather than assuming whole-domain arithmetic closure.
    fn result(
        &self,
        value: u64,
        capacity: u64,
        history: CountHistory,
        budget: &mut Budget,
    ) -> Result<Self, CountError> {
        if value > capacity {
            return Err(CountError::Capacity);
        }
        let exact = integer(value, budget)?;
        self.descriptor()
            .kind()
            .count_domain()
            .ok_or(nrz_bindings::UnitError::CountDomain)?
            .check_with_budget(&exact, budget)?;
        let (parents, depth) = history.envelope()?;
        let bytes = envelope(parents, depth, budget)?;
        Ok(Self(Rc::new(Data {
            source: self.0.source.clone(),
            value: Some(value),
            history,
            bytes,
            depth,
        })))
    }
}

impl CountHistory {
    /// Conservatively retain each parent envelope and bound the immutable chain depth.
    fn envelope(&self) -> Result<(u64, u64), CountError> {
        let (bytes, depth) = match self {
            Self::Source => (0, 0),
            Self::Add(left, right) | Self::Subtract(left, right) => (
                left.0
                    .bytes
                    .checked_add(right.0.bytes)
                    .ok_or(CountError::Capacity)?,
                left.0.depth.max(right.0.depth),
            ),
            Self::Scale(source, _) | Self::Quotient(source, _) => (source.0.bytes, source.0.depth),
        };
        Ok((bytes, depth.checked_add(1).ok_or(CountError::Capacity)?))
    }
}

/// Count arithmetic consumes its elementary operation before any refusal or result allocation.
fn charge(budget: &mut Budget) -> Result<(), Fault> {
    budget.work(1, Location::new("exact quantity count"))
}

/// Existing exact rational parsing admits the entire unsigned representation without a lossy cast.
fn integer(value: u64, budget: &mut Budget) -> Result<Rational, Fault> {
    Rational::parse(&value.to_string(), budget)
}

/// Bound retained operation metadata and immutable parent depth with existing caller policies.
fn envelope(parents: u64, depth: u64, budget: &Budget) -> Result<u64, CountError> {
    let bytes = parents
        .checked_add(
            u64::try_from(size_of::<Data>() + 2 * size_of::<usize>())
                .map_err(|_| CountError::Capacity)?,
        )
        .ok_or(CountError::Capacity)?;
    let at = Location::new("exact count lineage");
    budget.check(BudgetKind::Depth, depth, at)?;
    for kind in [BudgetKind::StagedPayloadBytes, BudgetKind::OutputBytes] {
        budget.check(kind, bytes, at)?;
    }
    Ok(bytes)
}
