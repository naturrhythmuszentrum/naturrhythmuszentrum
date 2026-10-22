// SPDX-License-Identifier: AGPL-3.0-only
//! Public syntax vocabulary and checked service contracts.
//!
//! # Admitted syntax and symbol associations
//!
//! `Operator` represents scalar arithmetic, declarations and calls, derivatives, integral/sum/series/limit bodies, predicates and solution sets, matrices and comprehensions, matrix operations, spectral requests and selected algebraic roots. Admission checks arity, sort, matrix shape and explicit de Bruijn frames. `ExpressionDraft::space` carries an optional exact typed link: ordinary pointwise scalar syntax requires absence, scalar-valued distribution actions retain their space, declared test/distribution leaves must match their declaration, and nonleaf space-dependent syntax requires an explicit matching space. Portable records and typed transformations retain that dependency. Representation supplies no evaluation, identity or proof of a predicate. Context keys sort declarations and named hypotheses, flatten conjunctions and reject recognized literal falsehoods or an atom paired with its explicit negation; unrecognized consistency remains unknown.
//!
//! The closed `SymbolRelation` syntax declares `Copy`, `DeterministicBound` and `NumericalError` associations between two distinct already-admitted Free symbols in the same graph. It requires closed scopes, a Boolean result, ownership by the result symbol's exact header, literal Real matrix endpoints and equal companion rectangles. Copy selections retain their separately checked numerical entry map. These edges grant no theorem, equation, normalization, solving or numerical access. Their dedicated admission inspects only the two sealed endpoints, charging one dispatch unit and one unit per endpoint before the existing transaction, encoding and storage charges. Independent bounded headers keep the symbol inventory from accumulating in every relation; ordinary expression scope rules remain unchanged. Tests cover named endpoint refusals, all three canonical wire kinds and unknown-kind refusal, absence of equation authority, and exact N/N−1 Work and payload boundaries (´test:unit:symbol-relation-refuses-scope-sort-and-header´), (´test:unit:symbol-relation-refuses-invalid-endpoint-identities´), (´test:unit:symbol-relation-refuses-sorts-and-companion-extents´), (´test:unit:symbol-relation-wire-kinds´), (´test:unit:symbol-relation-has-no-equation-authority´), (´test:unit:symbol-relation-work-and-payload-boundaries´).

//!
//! `Operator::Select` retains a checked finite index list against its declared source rectangle.
//! `Operator::AffineReading` retains a declared result symbol, the complete source and an exact
//! rational ordered map. These transparent definitions preserve the original symbols and headers;
//! a bare `Free` declaration gains no equation. `Cas::select_entries`, `Cas::define_reading` and
//! `Cas::prove_reading_equality` expose checked construction and whole-family coefficient proof.
//! The `readings` module documents coordinate order, named refusals and replay identity.

pub use crate::graph::{Draft, ExpressionDraft, GraphCounts, OccurrenceDraft, VertexKind};
pub use crate::model::{
    Approach, AtomicSort, Binding, Branch, Constant, Declaration, Environment, Extent, Field,
    MatrixOperation, Operator, PredicateKind, RootSelection, ScalarOperation, Scope, Sort, Span,
    SpectralKind, SymbolRelation,
};
pub use crate::substitution::Substitution;

pub use crate::boundary_model::{CaseName, NamedCase};

pub use crate::solving::{Coordinate, CoordinateRole, SolveDraft, SolveView};
