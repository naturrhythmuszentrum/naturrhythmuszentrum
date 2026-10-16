// SPDX-License-Identifier: AGPL-3.0-only
//! Generic resolvent discovery; scratch candidates never assert a solution theorem.

use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::State;
use crate::solving::coefficients::model::Element;
use crate::solving::coefficients::storage::{invalid, reserve, vector};
use crate::solving::orbits::produce::{Orbit, enumerate};
use crate::solving::symmetric::algebra::Polynomial;
use crate::solving::symmetric::cursor::{Cursor, Seed};
use crate::solving::symmetric::resolvent::{Resolvent, smaller};
use crate::solving::symmetric::seeds::generate;

/// One attempted seed retains its actual orbit size, including rejected larger orbits.
#[derive(Debug)]
pub(crate) struct Attempt {
    /// Degree-generic cursor output, not a named formula.
    pub(crate) seed: Seed,
    /// Complete finite orbit size, before resolvent expansion.
    pub(crate) cardinality: usize,
}

/// A strictly smaller unsealed target and the finite evidence from which it was computed.
#[derive(Debug)]
pub(crate) struct Candidate {
    /// Exact orbit proposal, to be replayed before using any invariant claim.
    pub(crate) orbit: Orbit,
    /// Symmetric coefficient expansions, to be replayed before coefficient substitution.
    pub(crate) resolvent: Resolvent,
}

/// Continuation owns only cursor state and diagnostic history; polynomial authority remains external.
#[derive(Debug)]
pub(crate) struct Discovery {
    /// Next degree-generic seed; no result-cache identity is hidden here.
    cursor: Cursor,
    /// Pending seed intents use the shared cost/progress quotas.
    schedule: crate::solving::solve_search::schedule::Schedule<Seed>,
    /// Original positive degree that every resolvent must strictly reduce.
    degree: u32,
    /// Current strictly decreasing recursion depth.
    depth: u64,
    /// Every visited seed, in deterministic enumeration order.
    pub(crate) attempts: Vec<Attempt>,
}

impl Discovery {
    /// Start with an empty inventory and no formula or lemma lookup.
    pub(crate) fn new(degree: u32, depth: u64, budget: &mut Budget) -> Result<Self, Fault> {
        Ok(Self {
            cursor: Cursor::new(degree, budget)?,
            schedule: crate::solving::solve_search::schedule::Schedule::new(budget)?,
            degree,
            depth,
            attempts: vector(0, budget)?,
        })
    }

    /// Resume until the next strict degree descent, retaining the cursor after the returned seed.
    pub(crate) fn next(
        &mut self,
        frame: &Polynomial,
        unity: &Element,
        state: &State,
        budget: &mut Budget,
    ) -> Result<Option<Candidate>, Fault> {
        if usize::try_from(self.degree).map_err(|_| invalid())? != frame.variables.len() {
            return Err(invalid());
        }
        while let Some(seed) = self.cursor.next(budget)? {
            let key = seed_key(&seed, budget)?;
            self.schedule
                .push(0, [1, u64::from(self.degree), 0, 0], key, seed, budget)?;
            let seed = self.schedule.pop(budget)?.ok_or_else(invalid)?;
            let polynomial = generate(frame, &seed, Some(unity), state, budget)?;
            let orbit = enumerate(&polynomial, state, budget)?;
            reserve(&mut self.attempts, 1, budget)?;
            self.attempts.push(Attempt {
                seed,
                cardinality: orbit.members.len(),
            });
            if let Some(resolvent) = smaller(&orbit, self.degree, self.depth, state, budget)? {
                return Ok(Some(Candidate { orbit, resolvent }));
            }
        }
        Ok(None)
    }
}

/// Seed descriptors preserve the declared generic index/power and restricted-growth order.
fn seed_key(seed: &Seed, budget: &mut Budget) -> Result<Vec<u8>, Fault> {
    let width = match seed {
        Seed::Cyclic { .. } => 9,
        Seed::Partition(parts) => parts
            .len()
            .checked_mul(4)
            .and_then(|width| width.checked_add(1))
            .ok_or_else(invalid)?,
    };
    let mut bytes = vector(width, budget)?;
    match seed {
        Seed::Cyclic { index, power } => {
            bytes.push(0);
            bytes.extend(index.to_be_bytes());
            bytes.extend(power.to_be_bytes());
        }
        Seed::Partition(parts) => {
            bytes.push(1);
            for part in parts {
                bytes.extend(part.to_be_bytes());
            }
        }
    }
    Ok(bytes)
}
