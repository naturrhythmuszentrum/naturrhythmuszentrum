// SPDX-License-Identifier: AGPL-3.0-only
//! Borrowed known-owner partitions, never another reservation or a backend inventory.
#[cfg(test)]
use super::{LcOmegaCandidate, LcShapeError};
use nrz_bindings::{
    AdmissionError, Budget, LcOmegaCorrespondence, LcOmegaDeclarationToken,
    readings::algebra::enclosure::lc_root::Certificate,
};

/// Scope of a disjoint owning-layer increment, not complete physical residence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::loan) enum Scope {
    /// Captured token allocation and pair capacity, excluding the initial token slot.
    CapturedToken,
    /// Inline carrier, store allowance and logical graph/law payload, including its token edge.
    Correspondence,
    /// Certificate header and actual buffer capacity, excluding temporary wire scratch.
    Certificate,
    /// Native outer Rc control words; its carrier is already in the correspondence allowance.
    NativeControlBlock,
}

/// Owner borrow bounds the lifetime during which the allocation identity is usable.
#[derive(Debug)]
enum Owner<'a> {
    /// Original captured token allocation, compared by its existing charged predicate.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the owning comparison preserves the independently tested token and captured-token identity relation"
        )
    )]
    Token(&'a LcOmegaDeclarationToken),
    /// Same captured allocation after its initial token slot has been dropped.
    Captured(&'a LcOmegaCorrespondence),
    /// Native facade allocation identity, distinct from the receiving store identity.
    Native(&'a std::rc::Rc<LcOmegaCorrespondence>),
    /// Frozen receiving-store identity, independent of the carrier location.
    Correspondence(&'a LcOmegaCorrespondence),
    /// Owned byte buffer; a borrowed alias does not allocate another copy.
    Certificate(&'a Certificate),
}

/// Logical allowance and measured certificate residence remain separate units.
#[derive(Debug)]
pub(in crate::loan) struct Observation<'a> {
    /// Borrowed authority for the owning-layer identity comparison.
    owner: Owner<'a>,
    /// Coverage attached to this identity, not transitive physical ownership.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the scoped projection preserves its independently tested coverage classification"
        )
    )]
    pub(in crate::loan) scope: Scope,
    /// Existing owning-layer allowance, excluding the token's initial slot when applicable.
    pub(in crate::loan) logical_allowance: Option<u64>,
    /// Only the certificate supplies its header plus actual buffer capacity here.
    pub(in crate::loan) observed_residence: Option<u64>,
}
impl<'a> Observation<'a> {
    /// Observe the captured owner without its separately located initial slot.
    #[cfg(test)]
    pub(in crate::loan) fn token(
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

    /// Observe the captured allocation through the correspondence that still owns it.
    pub(in crate::loan) fn captured(
        value: &'a LcOmegaCorrespondence,
    ) -> Result<Self, AdmissionError> {
        Ok(Self {
            owner: Owner::Captured(value),
            scope: Scope::CapturedToken,
            logical_allowance: Some(value.captured_declaration_bytes()?),
            observed_residence: None,
        })
    }

    /// Observe the existing logical allowance; backend capacity remains unavailable.
    pub(in crate::loan) fn correspondence(value: &'a LcOmegaCorrespondence) -> Self {
        Self {
            owner: Owner::Correspondence(value),
            scope: Scope::Correspondence,
            logical_allowance: Some(value.retained_bytes()),
            observed_residence: None,
        }
    }

    /// The native Rc adds only its control block beside the already counted carrier.
    pub(in crate::loan) const fn native(value: &'a std::rc::Rc<LcOmegaCorrespondence>) -> Self {
        Self {
            owner: Owner::Native(value),
            scope: Scope::NativeControlBlock,
            logical_allowance: None,
            observed_residence: Some(2 * size_of::<usize>() as u64),
        }
    }

    /// Observe actual certificate residence without exporting or copying its bytes.
    pub(in crate::loan) const fn certificate(value: &'a Certificate) -> Self {
        Self {
            owner: Owner::Certificate(value),
            scope: Scope::Certificate,
            logical_allowance: None,
            observed_residence: Some(value.retained_bytes()),
        }
    }

    /// Only the owning correspondence can identify payload already reserved by this ledger.
    pub(in crate::loan) fn persistent_overlap(
        &self,
        budget: &mut Budget,
    ) -> Result<u64, AdmissionError> {
        match self.owner {
            Owner::Correspondence(value) => {
                Ok(value.retained_bytes() - value.staged_retained_bytes(budget)?)
            }
            _ => Ok(0),
        }
    }

    /// Existing token comparison keeps its one-visit charge and propagates exhaustion.
    /// Other comparisons are fixed owning-layer identity observations. This never
    /// identifies carriers by address, clones owners or reserves their payload again.
    pub(in crate::loan) fn same_allocation(
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
            (Owner::Captured(left), Owner::Captured(right)) => {
                left.shares_declaration(right, budget)
            }
            (Owner::Captured(value), Owner::Token(token))
            | (Owner::Token(token), Owner::Captured(value)) => {
                value.captures_declaration(token, budget)
            }
            (Owner::Correspondence(left), Owner::Correspondence(right)) => {
                Ok(left.shares_allocation(right))
            }
            (Owner::Native(left), Owner::Native(right)) => Ok(std::rc::Rc::ptr_eq(left, right)),
            (Owner::Certificate(left), Owner::Certificate(right)) => {
                // Empty certificates have no buffer identity. Their observed inline
                // header still belongs to one borrowed original record, not every alias.
                Ok(left.shares_allocation(right)
                    || (left.retained_bytes() == size_of::<Certificate>() as u64
                        && std::ptr::eq(*left, *right)))
            }
            _ => Ok(false),
        }
    }
}

/// An aggregate alternative to its disjoint children, never an additional addend.
#[derive(Debug)]
#[cfg(test)]
pub(in crate::loan) struct Candidate<'a> {
    /// Captured allocation child, excluding the initial slot.
    pub(in crate::loan) captured: Observation<'a>,
    /// Receiving-store allowance child, including its inline token edge.
    pub(in crate::loan) correspondence: Observation<'a>,
    /// Initial slot belongs to this by-value candidate, not the shared captured allocation.
    pub(in crate::loan) initial_slot: u64,
    /// Owning Candidate receipt; replaces rather than adds to the children.
    pub(in crate::loan) logical_allowance: u64,
}
#[cfg(test)]
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
#[cfg(test)]
pub(in crate::loan) enum Exclusion {
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
#[cfg(test)]
pub(in crate::loan) const EXCLUSIONS: [Exclusion; 5] = [
    Exclusion::OriginalBaseline,
    Exclusion::NativeClosureFacadeAndTables,
    Exclusion::CasBackendCapacity,
    Exclusion::PrivateCheckedRecordRegistrations,
    Exclusion::ExternalCanonicalKeyAndProofCopies,
];
