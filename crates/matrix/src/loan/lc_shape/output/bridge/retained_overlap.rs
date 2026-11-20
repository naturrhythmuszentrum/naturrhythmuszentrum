// SPDX-License-Identifier: AGPL-3.0-only
//! Borrowed known-owner partitions, never another reservation or a backend inventory.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the later graph adapter consumes this private projection"
    )
)]
use super::{LcOmegaCandidate, LcShapeError};
use nrz_bindings::{
    AdmissionError, Budget, LcOmegaCorrespondence, LcOmegaDeclarationToken,
    readings::algebra::enclosure::lc_root::Certificate,
};

/// Scope of a disjoint owning-layer increment, not complete physical residence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::loan::lc_shape) enum Scope {
    /// Captured token allocation and pair capacity, excluding the initial token slot.
    CapturedToken,
    /// Inline carrier, store allowance and logical graph/law payload, including its token edge.
    Correspondence,
    /// Certificate header and actual buffer capacity, excluding temporary wire scratch.
    Certificate,
}

/// Owner borrow bounds the lifetime during which the allocation identity is usable.
#[derive(Debug)]
enum Owner<'a> {
    /// Original captured token allocation, compared by its existing charged predicate.
    Token(&'a LcOmegaDeclarationToken),
    /// Frozen receiving-store identity, independent of the carrier location.
    Correspondence(&'a LcOmegaCorrespondence),
    /// Owned byte buffer; a borrowed alias does not allocate another copy.
    Certificate(&'a Certificate),
}

/// Logical allowance and measured certificate residence remain separate units.
#[derive(Debug)]
pub(in crate::loan::lc_shape) struct Observation<'a> {
    /// Borrowed authority for the owning-layer identity comparison.
    owner: Owner<'a>,
    /// Coverage attached to this identity, not transitive physical ownership.
    pub(in crate::loan::lc_shape) scope: Scope,
    /// Existing owning-layer allowance, excluding the token's initial slot when applicable.
    pub(in crate::loan::lc_shape) logical_allowance: Option<u64>,
    /// Only the certificate supplies its header plus actual buffer capacity here.
    pub(in crate::loan::lc_shape) observed_residence: Option<u64>,
}
impl<'a> Observation<'a> {
    /// Observe the captured owner without its separately located initial slot.
    pub(in crate::loan::lc_shape) fn token(
        token: &'a LcOmegaDeclarationToken,
    ) -> Result<Self, AdmissionError> {
        Ok(Self {
            owner: Owner::Token(token),
            scope: Scope::CapturedToken,
            logical_allowance: Some(
                token.retained_bytes()? - size_of::<LcOmegaDeclarationToken>() as u64,
            ),
            observed_residence: None,
        })
    }

    /// Observe the existing logical allowance; backend capacity remains unavailable.
    pub(in crate::loan::lc_shape) fn correspondence(value: &'a LcOmegaCorrespondence) -> Self {
        Self {
            owner: Owner::Correspondence(value),
            scope: Scope::Correspondence,
            logical_allowance: Some(value.retained_bytes()),
            observed_residence: None,
        }
    }

    /// Observe actual certificate residence without exporting or copying its bytes.
    pub(in crate::loan::lc_shape) const fn certificate(value: &'a Certificate) -> Self {
        Self {
            owner: Owner::Certificate(value),
            scope: Scope::Certificate,
            logical_allowance: None,
            observed_residence: Some(value.retained_bytes()),
        }
    }

    /// Existing token comparison keeps its one-visit charge and propagates exhaustion.
    /// Other comparisons are fixed owning-layer identity observations. This never
    /// identifies carriers by address, clones owners or reserves their payload again.
    pub(in crate::loan::lc_shape) fn same_allocation(
        &self,
        other: &Self,
        budget: &mut Budget,
    ) -> Result<bool, AdmissionError> {
        match (&self.owner, &other.owner) {
            (Owner::Token(left), Owner::Token(right)) => match left.check_same(right, budget) {
                Ok(()) => Ok(true),
                Err(AdmissionError::ValueMismatch) => Ok(false),
                Err(error) => Err(error),
            },
            (Owner::Correspondence(left), Owner::Correspondence(right)) => {
                Ok(left.shares_allocation(right))
            }
            (Owner::Certificate(left), Owner::Certificate(right)) => {
                Ok(left.shares_allocation(right))
            }
            _ => Ok(false),
        }
    }
}

/// An aggregate alternative to its disjoint children, never an additional addend.
#[derive(Debug)]
pub(in crate::loan::lc_shape) struct Candidate<'a> {
    /// Captured allocation child, excluding the initial slot.
    pub(in crate::loan::lc_shape) captured: Observation<'a>,
    /// Receiving-store allowance child, including its inline token edge.
    pub(in crate::loan::lc_shape) correspondence: Observation<'a>,
    /// Initial slot belongs to this by-value candidate, not the shared captured allocation.
    pub(in crate::loan::lc_shape) initial_slot: u64,
    /// Owning Candidate receipt; replaces rather than adds to the children.
    pub(in crate::loan::lc_shape) logical_allowance: u64,
}
impl<'a> Candidate<'a> {
    /// Borrow the actual sealed candidate and its disjoint owning-layer partitions.
    pub(in crate::loan::lc_shape) fn new(
        value: &'a LcOmegaCandidate,
    ) -> Result<Self, LcShapeError> {
        Ok(Self {
            captured: Observation::token(&value.token)?,
            correspondence: Observation::correspondence(&value.correspondence),
            initial_slot: size_of::<LcOmegaDeclarationToken>() as u64,
            logical_allowance: value.retained_bytes()?,
        })
    }
}

/// Coverage absent from this projection must not be inferred from a known partition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::loan::lc_shape) enum Exclusion {
    /// Original questions, symbolic stores, provenance and Registry scopes.
    OriginalBaseline,
    /// Native captured closure, facade, reference fields and debit tables.
    NativeClosureFacadeAndTables,
    /// Receiving CAS collection capacity beyond existing logical allowances.
    CasBackendCapacity,
    /// CAS-private registration metadata, capacity, refresh and rollback.
    PrivateCheckedRecordRegistrations,
    /// Additional canonical, key or proof copies outside the named partitions.
    ExternalCanonicalKeyAndProofCopies,
}

/// Unknown and separately inventoried scopes prevent a complete residence total.
pub(in crate::loan::lc_shape) const EXCLUSIONS: [Exclusion; 5] = [
    Exclusion::OriginalBaseline,
    Exclusion::NativeClosureFacadeAndTables,
    Exclusion::CasBackendCapacity,
    Exclusion::PrivateCheckedRecordRegistrations,
    Exclusion::ExternalCanonicalKeyAndProofCopies,
];
