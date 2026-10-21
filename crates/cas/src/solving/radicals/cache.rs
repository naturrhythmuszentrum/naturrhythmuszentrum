// SPDX-License-Identifier: AGPL-3.0-only
//! Request-local reuse of computed formal discovery; every regional proof is still reconstructed.

use super::discovery::Candidate;
use crate::solving::coefficients::{
    arithmetic::Engine,
    model::{CoefficientFailure, Element},
    storage::{copied, invalid, reserve, vector},
};
use crate::solving::{
    orbits::produce::Orbit,
    symmetric::{algebra::Polynomial, reduction::Reduction, resolvent::Resolvent},
};
use crate::{budget::Budget, error::Fault, graph::State};

/// Computed scratch has no context, region, selected-root theorem or proof authority.
struct Entry {
    /// Exact primitive element in its actual quotient algebra.
    unity: Element,
    /// Complete finite candidate generated earlier in this request.
    candidate: Candidate,
    /// Cyclic character index and power offered by the cursor.
    seed: (u32, u32),
}

/// Empty at request entry and never shared between requests or coefficient algebras.
#[derive(Default)]
pub(crate) struct Cache {
    /// Bounded candidates whose coefficient and formal-root frames are immutable.
    entries: Vec<Entry>,
    /// Universal cross-character products, independent of regional evaluation maps.
    pairings: Vec<PairingEntry>,
}

/// Copy all finite combinatorial evidence with explicit storage accounting.
fn rows(source: &[Vec<u32>], budget: &mut Budget) -> Result<Vec<Vec<u32>>, Fault> {
    let mut result = vector(source.len(), budget)?;
    for row in source {
        result.push(copied(row, budget)?);
    }
    Ok(result)
}

/// Every retained coefficient is copied through its actual algebra's metered arithmetic.
fn polynomials(
    source: &[Polynomial],
    state: &State,
    budget: &mut Budget,
) -> Result<Vec<Polynomial>, Fault> {
    let mut result = vector(source.len(), budget)?;
    for value in source {
        result.push(value.copy(state, budget)?);
    }
    Ok(result)
}

/// Scratch copying preserves the original expansion obligations and the complete finite orbit.
fn candidate(source: &Candidate, state: &State, budget: &mut Budget) -> Result<Candidate, Fault> {
    let mut coefficients = vector(source.resolvent.coefficients.len(), budget)?;
    for reduction in &source.resolvent.coefficients {
        coefficients.push(Reduction {
            source: reduction.source.copy(state, budget)?,
            expression: reduction.expression.copy(state, budget)?,
            basis: polynomials(&reduction.basis, state, budget)?,
        });
    }
    Ok(Candidate {
        orbit: Orbit {
            members: polynomials(&source.orbit.members, state, budget)?,
            generators: rows(&source.orbit.generators, budget)?,
            actions: rows(&source.orbit.actions, budget)?,
            words: rows(&source.orbit.words, budget)?,
        },
        resolvent: Resolvent {
            coefficients,
            depth: source.resolvent.depth,
        },
    })
}

impl Cache {
    /// Reuse requires the same full algebra, ordered formal frame, primitive orientation and depth.
    pub(crate) fn lookup(
        &self,
        frame: &Polynomial,
        unity: &Element,
        depth: u64,
        state: &State,
        budget: &mut Budget,
    ) -> Result<Option<(Candidate, u32, u32)>, CoefficientFailure> {
        for entry in &self.entries {
            budget.work(1, super::AT)?;
            let seed = entry.candidate.orbit.members.first().ok_or_else(invalid)?;
            if seed.domain != frame.domain
                || seed.variables != frame.variables
                || entry.candidate.resolvent.depth != depth.checked_add(1).ok_or_else(invalid)?
            {
                continue;
            }
            let engine = Engine {
                state,
                domain: &unity.domain,
            };
            if !engine.formal_equal(unity, &entry.unity, budget)? {
                continue;
            }
            return Ok(Some((
                candidate(&entry.candidate, state, budget)?,
                entry.seed.0,
                entry.seed.1,
            )));
        }
        Ok(None)
    }

    /// Remember only already-computed finite data, after charging its complete owned copy.
    pub(crate) fn remember(
        &mut self,
        found: &Candidate,
        unity: &Element,
        seed: (u32, u32),
        state: &State,
        budget: &mut Budget,
    ) -> Result<(), CoefficientFailure> {
        let engine = Engine {
            state,
            domain: &unity.domain,
        };
        let entry = Entry {
            unity: engine.copy(unity, budget)?,
            candidate: candidate(found, state, budget)?,
            seed,
        };
        reserve(&mut self.entries, 1, budget)?;
        self.entries.push(entry);
        Ok(())
    }
}

/// One discovered formal invariant and its exact primitive orientation.
struct PairingEntry {
    /// Primitive element in the actual coefficient algebra.
    unity: Element,
    /// Enumeration bound that produced this first invariant.
    maximum: u32,
    /// Computed polynomial data; no regional theorem is retained.
    pairing: super::pairing::Pairing,
}

/// Copy a complete expansion obligation under the same coefficient and storage ledger.
fn pairing(
    source: &super::pairing::Pairing,
    state: &State,
    budget: &mut Budget,
) -> Result<super::pairing::Pairing, Fault> {
    let reduction = &source.reduction;
    Ok(super::pairing::Pairing {
        powers: copied(&source.powers, budget)?,
        reduction: Reduction {
            source: reduction.source.copy(state, budget)?,
            expression: reduction.expression.copy(state, budget)?,
            basis: polynomials(&reduction.basis, state, budget)?,
        },
    })
}

impl Cache {
    /// Discover once per exact formal frame; every dependent cell still certifies its own substitution.
    pub(crate) fn pairing(
        &mut self,
        frame: &Polynomial,
        unity: &Element,
        maximum: u32,
        state: &State,
        budget: &mut Budget,
    ) -> Result<Option<super::pairing::Pairing>, CoefficientFailure> {
        let engine = Engine {
            state,
            domain: &unity.domain,
        };
        for entry in &self.pairings {
            budget.work(1, super::AT)?;
            let source = &entry.pairing.reduction.source;
            if source.domain != frame.domain
                || source.variables != frame.variables
                || entry.maximum != maximum
            {
                continue;
            }
            if engine.formal_equal(unity, &entry.unity, budget)? {
                return Ok(Some(pairing(&entry.pairing, state, budget)?));
            }
        }
        let Some(found) = super::pairing::discover(frame, unity, maximum, state, budget)? else {
            return Ok(None);
        };
        let retained = PairingEntry {
            unity: engine.copy(unity, budget)?,
            maximum,
            pairing: pairing(&found, state, budget)?,
        };
        reserve(&mut self.pairings, 1, budget)?;
        self.pairings.push(retained);
        Ok(Some(found))
    }
}
