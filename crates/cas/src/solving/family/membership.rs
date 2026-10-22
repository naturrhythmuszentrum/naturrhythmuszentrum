// SPDX-License-Identifier: AGPL-3.0-only
//! Placement outcomes are derived from existing component proofs, without a new statement shape.

use super::invalid;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::{AtomicSort, Field, Sort};
use crate::proof_model::VerifiedWitness;
use crate::solving::SolveDraft;

/// A query distinguishes a value on the piece, a witnessed exclusion and missing membership.
#[derive(Clone, Debug)]
pub enum PlacementOutcome {
    /// Both registered incidence equations have ordinary component proofs.
    Inside {
        /// Exact original placement instance retained in both proof closures.
        piece: Handle,
        /// Same-Complex query for the piece's separate local query binder.
        location: Handle,
        /// Same-Real local argument; no global declaration was changed.
        argument: Handle,
        /// Existing equality proofs, in real-component then imaginary-component order.
        incidence: Box<[VerifiedWitness; 2]>,
    },
    /// The strictly defined reciprocal proves exclusion from the sole registered piece.
    Refused {
        /// Exact original placement instance retained in the proof closure.
        piece: Handle,
        /// Original Complex query.
        location: Handle,
        /// The specific mathematical exclusion.
        reason: crate::request_model::RefusalReason,
        /// Existing strict equality Im(q)/Im(q)=1 on the exact query region.
        exclusion: Box<VerifiedWitness>,
    },
    /// Neither membership nor exclusion was proved.
    Unresolved {
        /// The missing mathematical obligation.
        reason: crate::solving::SolveReason,
    },
}

/// Query substitution preserves the registered Complex slot's exact scalar sort and scope.
pub(super) fn validate(
    cas: &Cas,
    piece: &Handle,
    location: &Handle,
    domain: &Handle,
) -> Result<(), Fault> {
    let SolveDraft::ScalarPiece(piece) =
        crate::solving::graph::object(&cas.state, cas.checked(piece)?)?
    else {
        return Err(invalid());
    };
    let home = cas.state.region(piece.region.logical_id())?;
    let actual = cas.state.region(cas.checked(domain)?)?;
    let query = cas.state.expression(cas.checked(location)?)?;
    if home.header != actual.header
        || home.scope != actual.scope
        || home.ambient != actual.ambient
        || query.sort != Sort::Value(AtomicSort::Scalar(Field::Complex))
        || query.header != actual.header
        || query.scope != actual.scope
        || query.space.is_some()
    {
        return Err(invalid());
    }
    Ok(())
}
