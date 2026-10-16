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
use crate::solving::symmetric::resolvent::{Resolvent, smaller_observed};
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

    /// Refill one quota epoch, keeping the remaining intents alive when a descent returns early.
    fn scheduled(&mut self, budget: &mut Budget) -> Result<Option<Seed>, Fault> {
        if let Some(seed) = self.schedule.pop(budget)? {
            return Ok(Some(seed));
        }
        let count = budget
            .limits()
            .cost_quota
            .checked_add(budget.limits().progress_quota)
            .ok_or_else(invalid)?;
        for _ in 0..count {
            let Some(seed) = self.cursor.next(budget)? else {
                break;
            };
            let key = seed_key(&seed, budget)?;
            self.schedule
                .push(0, [1, u64::from(self.degree), 0, 0], key, seed, budget)?;
        }
        self.schedule.pop(budget)
    }

    /// Resume until the next strict degree descent, retaining the cursor after the returned seed.
    pub(crate) fn next(
        &mut self,
        frame: &Polynomial,
        unity: &Element,
        state: &State,
        budget: &mut Budget,
        mut observe: impl FnMut((&'static str, Option<usize>), &Seed, &mut Budget) -> Result<(), Fault>,
    ) -> Result<Option<Candidate>, Fault> {
        if usize::try_from(self.degree).map_err(|_| invalid())? != frame.variables.len() {
            return Err(invalid());
        }
        while let Some(seed) = self.scheduled(budget)? {
            observe(("seed-construction-start", None), &seed, budget)?;
            let polynomial = generate(frame, &seed, Some(unity), state, budget)?;
            observe(("seed-construction-end", None), &seed, budget)?;
            observe(("orbit-enumeration-start", None), &seed, budget)?;
            let orbit = enumerate(&polynomial, state, budget)?;
            observe(("orbit-enumeration-end", None), &seed, budget)?;
            observe(("smaller-resolvent-start", None), &seed, budget)?;
            reserve(&mut self.attempts, 1, budget)?;
            let result = smaller_observed(
                &orbit,
                self.degree,
                self.depth,
                state,
                budget,
                |phase, index, budget| observe((phase, Some(index)), &seed, budget),
            )?;
            observe(("smaller-resolvent-end", None), &seed, budget)?;
            self.attempts.push(Attempt {
                seed,
                cardinality: orbit.members.len(),
            });
            if let Some(resolvent) = result {
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
