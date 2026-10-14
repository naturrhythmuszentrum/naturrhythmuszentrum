// SPDX-License-Identifier: AGPL-3.0-only
//! Named graph, proof and derivation profiles complement the shared default profile.
//!
//! Compiled only in the public test target.

use crate::test_api::{Budget, Limits, LimitsOverride};

use super::common::TestResult;

/// Graph setup raises work while retaining the default replay and storage ceilings.
fn graph_limits() -> Limits {
    Limits {
        work: 100_000_000,
        ..Limits::default()
    }
}

/// Proof setup raises both independent work allowances without changing storage.
fn proof_limits() -> Limits {
    Limits {
        work: 100_000_000,
        replay_work: 100_000_000,
        ..Limits::default()
    }
}

/// Generated calculus and multiplicity fixtures share this explicit work profile.
fn derivation_limits() -> Limits {
    Limits {
        work: 2_000_000_000,
        replay_work: 2_000_000_000,
        ..Limits::default()
    }
}

/// A fresh graph-construction ledger.
pub(crate) fn graph_budget() -> TestResult<Budget> {
    Ok(Budget::new(graph_limits())?)
}

/// A fresh proof-construction and replay ledger.
pub(crate) fn proof_budget() -> TestResult<Budget> {
    Ok(Budget::new(proof_limits())?)
}

/// A fresh ledger for generated derivatives, search and square-free certificates.
pub(crate) fn derivation_budget() -> TestResult<Budget> {
    Ok(Budget::new(derivation_limits())?)
}

/// Requests change only the same two work ceilings as their setup profile.
pub(crate) fn derivation_request_limits() -> LimitsOverride {
    let limits = derivation_limits();
    LimitsOverride {
        work: Some(limits.work),
        replay_work: Some(limits.replay_work),
        ..LimitsOverride::default()
    }
}
