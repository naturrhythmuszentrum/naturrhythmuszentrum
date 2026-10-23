// SPDX-License-Identifier: AGPL-3.0-only
//! Public proof vocabulary and checked service contracts.
//!
//! # Closed checking, visible domains and portable witnesses
//!
//! `Cas::check_proof(context, steps, budget)` checks a supplied child-before-parent proof DAG and returns a sealed `VerifiedWitness`. Its final `Judgment` names the full context, layer, optional space, typed statement and canonical guarded residuals. The closed `Primitive` vocabulary has 59 executable native schemas and 21 registered solving schemas. The executable families are: equality structure and ordered congruence, scalar field laws, exact rational arithmetic, finite polynomial/rational coefficient identities and ordered affine reading identities, elementary scalar derivative rules, equality transport, simultaneous substitution, ordered Jacobian assembly, certified square-free layers, `ctx.hypothesis`, `ctx.weaken`, `ctx.close`, `region.decide`, `region.construct`, `case.ordered`, `case.domain`, `case.select`, `case.congruence`, `case.eliminate`, `jet.polynomial`, `diff.case-interior`, `diff.case-boundary`, the twelve distribution schemas documented in `src/distribution_model.rs` and `src/primitive_unique.rs`, and the four split/germ schemas. `region.decide` version 2 checks explicit empty-region DAGs, and `solve.split` version 2 converts an empty region to complete empty coverage. Other schemas retain version 1. The case certificates retain the complete reconstructed region and association inventory and exact cell evidence. `PRIMITIVES` exposes keys, semantic versions, conclusion/premise/certificate metadata and positive weights under the current `logical` policy version 10. Callers cannot register a trusted callback or label an arbitrary equation primitive. The schema matchers reconstruct conclusions and check each ordered premise; derivative relations cannot be reversed or composed as equality.
//!
//! The graph owns checked Application and Proof records, their context/endpoints, ordered premise links and typed domain conditions. Scoped application edges reconstruct each statement's region, boundary, side, function and ordered value roles; unordered cover members use canonical local identity order. Admission is atomic, including failed proof imports and substitutions. A proof references only earlier proofs; unreachable or duplicate canonical proof records are rejected. `register_derived` replays a sealed witness in an independent empty store, compares its exact expected claim, and admits only a positive strict cost improvement over known routes for the same context/endpoints/relation/residual set. A shortcut retains its older proof dependency; it never replaces that evidence. The compiled registry stays closed and versioned. `register_derived_with_policy` replays an equality witness and requires strict improvement over the finite known equality routes under its complete pinned table; shortcuts registered under another policy cannot supply a stale improvement record.
//!
//! The kernel derives requirements from both complete endpoints and every premise. Rational facts, exact named hypotheses, positivity implying nonzero, and nonzero products/integer powers discharge guards through bounded checks. Unknown requirements remain explicit `GuardedObligation` tuples naming home context, scope, layer, space, region and predicate; a refuted requirement blocks admission. The condition set acts as conjunction introduction, while conjunction hypotheses are projected with their original hypothesis key. Cancellation of x/x retains x nonzero; multiplication by zero retains the definedness of its other operand. Zero powers follow the strict nonzero-base convention, and the exponent-one derivative uses the child derivative directly. Simultaneous theorem substitution transports every requirement, including previously assumed ones, so an x nonzero hypothesis cannot justify substituting zero into x/x.
//!
//! `is_discharged()` means no residual remains. `hypotheses()` still identifies every context assumption used, and `context_consistency()` remains `Unknown`. Open binder requirements retain their checked term scope; the scalar checker cannot turn one bound instance into a universal fact. No proof or witness supplies a physical validation receipt. Field identities follow the scalar field axioms; coefficient equality compares finite exact maps after reconstructing both endpoints, and rational equality retains original guards before cross multiplication. The mathematical arguments and derivative formulas are in `docs/design/cas-design.md`. Coefficient maps are temporary checked views of graph syntax, bounded by total degree, the union of variables across monomials, nonzero term count, measured scratch storage and exact arithmetic work.
//!
//! `export_witness` writes only `NRZCAS05`: the used primitive manifest, canonical typed dependency records and complete conclusion. Headers, regions, spaces, expressions, arms, boundaries, source forms, predicates, contexts, guarded obligations, statements and applications have distinct record schemas. Kahn ordering chooses ready structural descriptors with already assigned child indices; unordered semantic sets use portable structural order. Runtime handles, graph serials, search costs and original receipts are absent. `NRZCAS01`, `NRZCAS02`, `NRZCAS03`, `NRZCAS04` and every other magic are refused as `UnsupportedVersion` before replay allocation. There are no external consumers or stored artifacts requiring a compatibility reader.
//!
//! `replay(bytes, expected_judgment, budget)` reconstructs all mathematical tables through ordinary admission, checks every primitive and condition again, compares the complete canonical re-encoding, and compares the caller's expected full context, statement, domain, layer, space and guarded residual set. Expected terms cannot be reinterpreted under a different semantic header. `inspect` performs the same checking while deliberately reporting what the bytes prove without an acceptance target. A different valid theorem is rejected by `replay`; a different valid proof of the intended theorem is permitted. Truncated/extended streams, forward references, unused records, unknown rule versions, nonminimal numbers, changed guards and mismatched claims refuse without publishing a decoded prefix. The separate replay allowance covers decoding, construction, checking, publication and expected-claim comparison before work occurs.

pub use crate::conditions::{ConditionStatus, GuardKind};
pub use crate::formal_register::{PRIMITIVE_SCHEMAS, PrimitiveSchema};
pub use crate::hypothesis_kinds::{
    HYPOTHESIS_KINDS, HYPOTHESIS_SCHEMAS, HypothesisKind, QualifiedHypothesis,
};
pub use crate::lemma_cache::{ANONYMOUS_LEMMA_SCHEMA, AnonymousLemmaCache};
pub use crate::multiplicity::{GcdCertificate, SquareFreeCertificate, SquareFreeLayer};
pub use crate::proof_citations::{PremiseCitation, ProofCitation};
pub use crate::rule::{
    CertificateSchema, ConclusionKind, PRIMITIVES, PremiseShape, Primitive, RuleSpec,
};
pub use crate::schema_standing::{LemmaProvenance, Standing};

pub use crate::case_model::CaseCertificate;
pub use crate::proof_model::{
    ApplicationDraft, Certificate, ConstructorLaw, DomainContract, GuardedObligation, Judgment,
    Ownership, Statement, VerifiedWitness,
};
pub use crate::region::RegionCertificate;

pub use crate::boundary_model::{
    BoundaryCertificate, BoundaryPartition, CaseDerivative, NonexistenceReason, PointDerivative,
    PointNonexistence, TraceValue,
};

pub use crate::distribution_model::{
    DistributionCertificate, DistributionStatement, RegularCertificate, RegularityKind,
};

pub use crate::split_model::{
    SplitCertificate, SplitKind, SplitPiece, SplitPoint, SplitRange, SplitValue,
};

pub use crate::germ_model::{GermAt, GermCertificate, GermMode, GermValue, SeriesAt};

pub use crate::pullback_model::{
    PullbackCertificate, PullbackKind, PullbackMode, PullbackValue, PullbackZero,
};

pub use crate::solving::{SolveCertificate, SolveStatement};

pub use crate::emptiness::{
    EMPTY_SCHEMAS, EmptinessDag, EmptyEvidence, EmptyFact, EmptyFactor, EmptyFactorIdentity,
    EmptyLeaf, EmptyNode, LemmaSchema,
};
