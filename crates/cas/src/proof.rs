// SPDX-License-Identifier: AGPL-3.0-only
//! Public proof vocabulary and checked service contracts.

pub use crate::conditions::{ConditionStatus, GuardKind};
pub use crate::multiplicity::{GcdCertificate, SquareFreeCertificate, SquareFreeLayer};
pub use crate::rule::{
    CertificateSchema, ConclusionKind, PRIMITIVES, PremiseShape, Primitive, RuleSpec,
};

pub use crate::case_model::CaseCertificate;
pub use crate::proof_model::{
    ApplicationDraft, Certificate, ConstructorLaw, DomainContract, GuardedObligation, Judgment,
    Ownership, Statement, VerifiedWitness,
};
pub use crate::region::RegionCertificate;
