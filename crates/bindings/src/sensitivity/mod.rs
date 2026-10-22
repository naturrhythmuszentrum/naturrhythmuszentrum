// SPDX-License-Identifier: AGPL-3.0-only
//! Closed native schedules backed by original checks and replayed graph evidence.
//!
//! # Native symbolic sensitivity contracts
//!
//! `Context::check_sensitivity` consumes a builder after checking every declaration and every supplied original operand. `SensitivityRecipe::Square` accepts a square of one declared continuous input; `Reciprocal` accepts its negative first power or a literal-one quotient. Arbitrary equivalent formulas and user-supplied derivatives are not executable recipes. Rows retain the caller's original order and columns retain distinct declared input keys, including requested independent coordinates with formal zero partials. Derived symbols, discrete counts, metadata, missing operands and duplicate coordinates refuse. Original physical checking still rejects unregistered kind operations, point multiplication and hidden invalid operands before any symbolic transformation.
//!
//! The adapter submits the checked coherent-coordinate expressions through the public CAS Jacobian request on an explicit context domain. It inspects the replayed theorem's context, layer, domain, coordinate order, source column and result shape, then converts the CAS row enumeration to logical column order. Every partial is additionally proved equal to the fixed native coefficient recipe. `PairingWitness::sensitivity` supplies the output-over-input descriptor, including formal derivative zeros; an original untyped physical zero receives no new exemption. Input measure scales participate in the chain rule, and outputs use coherent coordinates. Square evaluates its derivative as `(2*c)*s` and reciprocal as `-s/(c*c)`, where `c=s*x` and `s` is the input's exact measure scale.
//!
//! `CheckedSensitivity` privately owns the frozen declarations, original checks, graph witnesses, ordered descriptors and `SensitivityKernel` entries. It exposes read-only evidence and unresolved `SensitivityObligation` values, with no numerical evaluation API or arbitrary checked-state constructor. Reciprocal proofs use explicit symbolic nonzero hypotheses; their qualified provenance remains in the witnesses, and numerical nonzero/separated-divisor obligations remain even when a declaration already claims nonzero. Only complete discharged CAS results can enter the schedule; conditional, unfinished, unsupported and exhausted outcomes remain typed refusals with their outcome retained. A symbolic premise is never a numerical certificate, and the numerical owner must enclose coefficient evaluation before using a sensitivity (see `docs/design/matrix-core-design.md`).
//!
//! For larger closed recipes, `Context::with_cas_limits` permits explicit per-store CAS ceilings without changing global defaults or the ordinary constructor. Each derivative and equality request still requires explicit `LimitsOverride` work and replay allowances, capped by that store. Physical checks retain their existing ledger. Sensitivity graph construction resolves the symbolic policy into a separate ledger shared across the lowering phase; each service request has its own generation and replay ledger. `JacobianEvidence::work` reports derivative-phase construction work, per-request generation/replay peaks and completed request count; it grants no additional work or numerical assurance.
//!
//! The closed-recipe conformance profile explicitly allows 16,000,000 construction/generation work units and 16,000,000 replay work units, with all other CAS ceilings unchanged. The square and reciprocal fixture costs reported in the seed README at `51105aeb4927f5e2d0bb76a607d87aae1d40c00b`, inspected on 2026-10-22, were respectively 1,002,411 / 4,517,336 and 762,132 / 4,317,129 generation/replay units. Tests measure each recipe afresh, require at least twofold profile headroom, succeed at the exact observed replay limit and refuse one unit below it. This profile is an explicit client choice for the closed fixtures, not a promise that arbitrary larger schedules fit; size or work exhaustion remains a typed refusal.

mod evidence;
mod graph;
mod implicit;
mod native;
mod replay;
mod root;
mod solve;

pub use solve::SolveSymbols;

use nrz_cas::{Fault, request::Outcome};

use crate::{Diagnostic, PairingError, UnitError};

pub use evidence::{
    CheckedSensitivity, JacobianEntry, JacobianEvidence, SensitivityEntry, SensitivityWork,
};

/// The closed native operation families admitted by sensitivity checking.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SensitivityRecipe {
    /// Elementwise square of a declared continuous difference input.
    Square,
    /// Elementwise reciprocal of a declared continuous difference input.
    Reciprocal,
    /// Positive square root established through its polynomial relation.
    PositiveRoot,
    /// One nonempty square operator shared by all right-hand sides.
    Solve {
        /// Positive square operator order.
        order: usize,
        /// Positive right-hand-side column count.
        right_hand_sides: usize,
    },
}

/// One entry in a fixed numerical schedule, never an arbitrary expression evaluator.
///
/// Input coordinates use their declared measures; outputs use coherent measures.
/// Let `s` be the input's exact measure scale and `c = s*x`. Each indicated
/// operation must be evaluated with numerical error bounds by the numerical owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SensitivityKernel {
    /// Formal derivative zero with the entry's output/input pairing descriptor.
    Zero,
    /// Evaluate `(2*c)*s` in that order for the named input coordinate.
    Square {
        /// Index in the retained ordered coordinate inventory.
        coordinate: usize,
    },
    /// Evaluate `-s/(c*c)` in that order for the named input coordinate.
    Reciprocal {
        /// Index in the retained ordered coordinate inventory.
        coordinate: usize,
    },
    /// Evaluate `s/(2*t)` using the enclosure of the exact positive output root `t`.
    PositiveRoot {
        /// Input coordinate whose exact measure scale is `s`.
        coordinate: usize,
        /// Root in the retained logical output order.
        output: usize,
    },
    /// Select an entry of the certified differential solve for this input column.
    ///
    /// In increasing coordinate order, enclose `delta_B - delta_A*X_0`, including
    /// the selected input's measure scale, then solve with the same verified `A`.
    /// `X_0` denotes the exact nominal solution, never an uncorrected candidate.
    Solve {
        /// Independent coordinate in `(vec(A), vec(B))` order.
        coordinate: usize,
        /// Selected entry of the resulting differential solution in column order.
        output: usize,
    },
}

/// Numerical premises retained after symbolic proof; none is discharged by bindings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SensitivityObligation {
    /// Establish a nonzero operating coordinate and a separated divisor enclosure.
    Nonzero {
        /// Index in the retained ordered coordinate inventory.
        coordinate: usize,
    },
    /// Enclose the exact root satisfying `t*t = c`, with `t > 0` and a smooth neighborhood.
    PositiveRoot {
        /// Input coordinate with coherent value `c`.
        coordinate: usize,
        /// Exact positive root in output order.
        output: usize,
    },
    /// Establish invertibility of the one common nominal operator numerically.
    Invertible,
    /// Enclose the exact nominal `X_0` satisfying every retained `A*X_0-B=0` relation.
    NominalSolution,
}

/// An honest refusal retaining original diagnostics or the complete CAS outcome.
#[derive(Debug, thiserror::Error)]
pub enum SensitivityError {
    /// Every original declaration and operand must pass physical checking first.
    #[error(transparent)]
    Original(#[from] Diagnostic),
    /// Descriptor algebra refused the proposed operation or relation.
    #[error(transparent)]
    Units(#[from] UnitError),
    /// A formal sensitivity pairing could not be derived.
    #[error(transparent)]
    Pairing(#[from] PairingError),
    /// Graph construction or bounded inspection failed.
    #[error(transparent)]
    Algebra(#[from] Fault),
    /// A request did not produce a complete, discharged replayed theorem.
    #[error("CAS did not prove the requested sensitivity relation")]
    Evidence(Box<Outcome>),
    /// Only the specified native original syntax is executable.
    #[error("original expression is outside the selected native recipe")]
    UnsupportedRecipe,
    /// Coordinates must be distinct declared continuous inputs, covering every operand.
    #[error("invalid or incomplete ordered sensitivity coordinates")]
    Coordinates,
    /// A replayed theorem did not refer to the requested inputs, domain or order.
    #[error("replayed sensitivity evidence does not match its requested relation")]
    WitnessMismatch,
    /// Checked extents or the caller's finite schedule allowance were exceeded.
    #[error("sensitivity schedule exceeds its extent or work allowance")]
    ResourceLimit,
}
