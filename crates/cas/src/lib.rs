// SPDX-License-Identifier: AGPL-3.0-only
#![doc = include_str!("../README.md")]
#![expect(
    clippy::multiple_crate_versions,
    reason = "petgraph 0.8.3 depends on hashbrown 0.15 while indexmap \
        uses hashbrown 0.17, so no single-version resolution exists"
)]

#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod budget;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod conditions;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod context;
/// Exact rational cut questions for existing native case associations.
pub mod decision;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the pure cut implementation stays private"
)]
mod decision_cut;
mod decision_enclosure;
mod decision_family_wire;
mod decision_integer_selection;
mod decision_integer_wire;
mod decision_introduction;
mod decision_kernel;
mod decision_pi;
mod decision_selection;
mod decision_sign;
mod decision_wire;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod derivative_check;
/// Certified rational endpoint arithmetic and original-scope enclosure evidence.
pub mod enclosure;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod encoding;
mod error;
#[expect(
    clippy::redundant_pub_crate,
    reason = "closed constant proof helpers stay inside private CAS modules"
)]
mod exact_constant;
mod formal_register;

#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod graph;
mod hypothesis_kinds;
/// Checked exact affine lattice basis admission.
pub mod lattice;
mod model;
/// Independently checked rational bounds for mathematical pi.
pub mod pi_bounds;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod polynomial;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod proof_check;
mod proof_citations;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod proof_storage;
#[expect(
    clippy::redundant_pub_crate,
    reason = "request memo stays internal to the proof producer"
)]
mod reading_memo;
mod rule;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod rule_check;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod scalar_model;
mod schema_standing;
#[expect(
    clippy::redundant_pub_crate,
    reason = "mixed roots share the bounded sign producer"
)]
mod strict_sign;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod wire;
#[expect(
    clippy::redundant_pub_crate,
    reason = "request-local witness records are shared by the encoder and ledger"
)]
mod wire_memo;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod wire_read;

mod axis_map;
#[expect(
    clippy::redundant_pub_crate,
    reason = "exact case calculations stay private"
)]
mod case_cells;
#[expect(
    clippy::redundant_pub_crate,
    reason = "closed case checkers stay private"
)]
mod case_check;
#[expect(
    clippy::redundant_pub_crate,
    reason = "cover elimination stays private"
)]
mod case_eliminate;
mod case_model;
#[expect(
    clippy::redundant_pub_crate,
    reason = "scoped case requirements stay private"
)]
mod case_obligations;
#[expect(
    clippy::redundant_pub_crate,
    reason = "ordered construction stays private"
)]
mod case_ordered;
#[expect(clippy::redundant_pub_crate, reason = "case proposals stay private")]
mod case_producer;
#[expect(
    clippy::redundant_pub_crate,
    reason = "case reach masks are private kernel support"
)]
mod case_reach;
#[expect(
    clippy::redundant_pub_crate,
    reason = "private module with crate-wide case recipes"
)]
mod case_recipes;
#[expect(
    clippy::redundant_pub_crate,
    reason = "refinement scheduling is crate-internal"
)]
mod case_refine;
#[expect(clippy::redundant_pub_crate, reason = "local selection stays private")]
mod case_select;
#[expect(
    clippy::redundant_pub_crate,
    reason = "native request dispatch is crate-internal"
)]
mod case_service;
#[expect(
    clippy::redundant_pub_crate,
    reason = "case syntax helpers stay private"
)]
mod case_syntax;
#[expect(
    clippy::redundant_pub_crate,
    reason = "typed helpers stay inside the private module"
)]
mod descriptor;
mod descriptor_model;
#[expect(
    clippy::redundant_pub_crate,
    reason = "empty-region checking stays inside the kernel"
)]
mod emptiness;
mod lemma_cache;
mod number;
mod parse_error;
mod parser;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod request_model;
mod snapshot;
#[expect(
    clippy::redundant_pub_crate,
    reason = "typed builders stay inside the private module"
)]
mod source_build;
mod source_factory;
mod substitution;
#[expect(
    clippy::redundant_pub_crate,
    reason = "typed rebuilding stays inside the private module"
)]
mod substitution_draft;
mod traversal;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod validation;

#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod calculus;
mod cost;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod frontier;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod producer;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod rewrite;

#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod frozen;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod known;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod search;
#[expect(
    clippy::redundant_pub_crate,
    reason = "boundary implementation is shared only within the crate"
)]
mod service;
mod shortcut;

pub use error::{Exhaustion, Fault, FaultKind, InvalidCode, Location};
pub use exact_constant::{
    ConstantIdentity, ConstantIdentityKind, ExactConstant, PositiveConstant, PreparedConstant,
};
pub use graph::{Cas, Handle, LogicalId, Reference};
pub use number::Rational;

#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod euclidean;
mod multiplicity;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod square_free;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod square_free_check;
#[expect(
    clippy::redundant_pub_crate,
    reason = "items shared across private modules stay crate-visible because the workspace denies \
        unreachable_pub"
)]
mod univariate;

#[cfg(test)]
use crate as test_api;

#[cfg(test)]
mod testing;
#[cfg(test)]
mod tests;

pub mod domain;
pub mod jacobians;
pub mod limits;
pub mod proof;
#[expect(
    clippy::redundant_pub_crate,
    reason = "reading syntax and wire helpers stay within CAS"
)]
pub mod readings;
pub mod request;
pub mod source;
pub mod syntax;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared implementation items stay crate-visible inside private modules"
)]
mod domain_model;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared implementation items stay crate-visible inside private modules"
)]
mod source_model;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared implementation items stay crate-visible inside private modules"
)]
mod domain_encoding;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared implementation items stay crate-visible inside private modules"
)]
mod domain_admission;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared implementation items stay crate-visible inside private modules"
)]
mod header;

mod context_admission;

#[expect(
    clippy::redundant_pub_crate,
    reason = "typed proof records are shared only among private kernel modules"
)]
mod proof_model;
#[expect(
    clippy::redundant_pub_crate,
    reason = "exact region compilation is a private shared kernel component"
)]
mod region;

#[expect(
    clippy::redundant_pub_crate,
    reason = "scoped key helpers remain private implementation"
)]
mod statement;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared private kernel implementation"
)]
mod context_check;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared private kernel implementation"
)]
mod proof_view;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared private kernel implementation"
)]
mod region_check;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared private kernel implementation"
)]
mod scoped_check;
mod wire_answer;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared private kernel implementation"
)]
mod wire_build;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared private kernel implementation"
)]
mod wire_decode;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared private kernel implementation"
)]
mod wire_objects;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared private kernel implementation"
)]
mod wire_records;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared private kernel implementation"
)]
mod wire_statement;

#[expect(
    clippy::redundant_pub_crate,
    reason = "task encoding is shared privately"
)]
mod task;

mod proof_service;

#[expect(
    clippy::redundant_pub_crate,
    reason = "typed role validation is shared only within the private kernel"
)]
mod statement_check;

#[expect(
    clippy::redundant_pub_crate,
    reason = "boundary implementation is shared only within the crate"
)]
mod boundary_check;
#[expect(
    clippy::redundant_pub_crate,
    reason = "boundary implementation is shared only within the crate"
)]
mod boundary_decode;
mod boundary_model;
#[expect(
    clippy::redundant_pub_crate,
    reason = "boundary implementation is shared only within the crate"
)]
mod boundary_partition;
#[expect(
    clippy::redundant_pub_crate,
    reason = "boundary implementation is shared only within the crate"
)]
mod boundary_producer;
#[expect(
    clippy::redundant_pub_crate,
    reason = "boundary implementation is shared only within the crate"
)]
mod boundary_regions;
#[expect(
    clippy::redundant_pub_crate,
    reason = "boundary implementation is shared only within the crate"
)]
mod boundary_roles;
#[expect(
    clippy::redundant_pub_crate,
    reason = "boundary implementation is shared only within the crate"
)]
mod boundary_runner;
#[expect(
    clippy::redundant_pub_crate,
    reason = "boundary implementation is shared only within the crate"
)]
mod boundary_wire;
#[expect(
    clippy::redundant_pub_crate,
    reason = "boundary implementation is shared only within the crate"
)]
mod case_calculus;
mod derivative_model;
#[expect(
    clippy::redundant_pub_crate,
    reason = "boundary implementation is shared only within the crate"
)]
mod jet_polynomial;
#[expect(
    clippy::redundant_pub_crate,
    reason = "boundary implementation is shared only within the crate"
)]
mod named_case;

#[expect(
    clippy::redundant_pub_crate,
    reason = "boundary implementation is shared only within the crate"
)]
mod case_point;

#[expect(
    clippy::redundant_pub_crate,
    reason = "boundary implementation is shared only within the crate"
)]
mod boundary_derivative;
#[expect(
    clippy::redundant_pub_crate,
    reason = "boundary implementation is shared only within the crate"
)]
mod boundary_map;

#[expect(
    clippy::redundant_pub_crate,
    reason = "private boundary preflight checks"
)]
mod boundary_measure;

mod boundary_replay;

#[expect(
    clippy::redundant_pub_crate,
    reason = "partition memo helpers are shared only within the kernel"
)]
mod region_cache;

mod distribution_model;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution implementation is shared only within the crate"
)]
mod distribution_syntax;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution implementation is shared only within the crate"
)]
mod distribution_wire;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution implementation is shared only within the crate"
)]
mod distribution_decode;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution implementation is shared only within the crate"
)]
mod distribution_regular;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution implementation is shared only within the crate"
)]
mod distribution_check;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution implementation is shared only within the crate"
)]
mod distribution_linear;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution implementation is shared only within the crate"
)]
mod distribution_derivative;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution implementation is shared only within the crate"
)]
mod distribution_action;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution implementation is shared only within the crate"
)]
mod distribution_support;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution implementation is shared only within the crate"
)]
mod distribution_multiply;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution implementation is shared only within the crate"
)]
mod distribution_jump;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution implementation is shared only within the crate"
)]
mod distribution_ae;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution implementation is shared only within the crate"
)]
mod distribution_terms;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution implementation is shared only within the crate"
)]
mod distribution_producer;

mod distribution_pair_producer;

#[expect(
    clippy::redundant_pub_crate,
    reason = "private distribution preflight entry"
)]
mod distribution_preflight;
#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution implementation is shared only within the crate"
)]
mod distribution_search;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution implementation is shared only within the crate"
)]
mod distribution_refusal;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution limits are shared only within the crate"
)]
mod distribution_measure;

#[expect(
    clippy::redundant_pub_crate,
    reason = "native equality snapshots stay private"
)]
mod distribution_known;

#[expect(
    clippy::redundant_pub_crate,
    reason = "derived distribution recipes remain private"
)]
mod distribution_recipes;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution scheduling remains private"
)]
mod distribution_schedule;

mod split_model;

#[expect(
    clippy::redundant_pub_crate,
    reason = "split calculus helpers stay inside the kernel"
)]
mod split_polynomial;

#[expect(
    clippy::redundant_pub_crate,
    reason = "split calculus helpers stay inside the kernel"
)]
mod split_geometry;

#[expect(
    clippy::redundant_pub_crate,
    reason = "split calculus helpers stay inside the kernel"
)]
mod split_check;

#[expect(
    clippy::redundant_pub_crate,
    reason = "split calculus helpers stay inside the kernel"
)]
mod split_producer;

#[expect(
    clippy::redundant_pub_crate,
    reason = "split calculus helpers stay inside the kernel"
)]
mod split_wire;

mod germ_model;

#[expect(
    clippy::redundant_pub_crate,
    reason = "germ calculus helpers stay inside the kernel"
)]
mod germ_polynomial;

#[expect(
    clippy::redundant_pub_crate,
    reason = "germ calculus helpers stay inside the kernel"
)]
mod germ_check;

#[expect(
    clippy::redundant_pub_crate,
    reason = "germ calculus helpers stay inside the kernel"
)]
mod germ_producer;

#[expect(
    clippy::redundant_pub_crate,
    reason = "germ calculus helpers stay inside the kernel"
)]
mod germ_wire;

mod pullback_model;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution pullback helpers stay inside the kernel"
)]
mod pullback_geometry;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution pullback helpers stay inside the kernel"
)]
mod pullback_check;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution pullback helpers stay inside the kernel"
)]
mod pullback_output;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution pullback helpers stay inside the kernel"
)]
mod pullback_producer;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution pullback helpers stay inside the kernel"
)]
mod pullback_wire;

#[expect(
    clippy::redundant_pub_crate,
    reason = "distribution pullback helpers stay inside the kernel"
)]
mod primitive_unique;

#[expect(
    clippy::redundant_pub_crate,
    reason = "total polynomial map validation stays inside the kernel"
)]
mod pullback_polynomial;

pub mod solving;
