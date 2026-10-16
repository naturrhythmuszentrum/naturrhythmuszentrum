// SPDX-License-Identifier: AGPL-3.0-only
//! Kernel proof facts are observable but only the checked transaction can construct them.
//!
//! A published recipe exposes its closed law:
//! ```rust
//! use nrz_matrix::{KernelEvidence, KernelKind};
//! fn law(evidence: &KernelEvidence) -> KernelKind { evidence.kind() }
//! ```
//!
//! A caller cannot change that law while retaining the proof:
//! ```compile_fail
//! use nrz_matrix::{KernelEvidence, KernelKind};
//! fn substitute(evidence: &mut KernelEvidence) { evidence.kind = KernelKind::Subtract; }
//! ```
