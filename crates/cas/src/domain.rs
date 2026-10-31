// SPDX-License-Identifier: AGPL-3.0-only
//! Public domain vocabulary and checked service contracts.
//!
//! # Headers, exact regions and qualified contexts
//!
//! Headers separate immutable declarations from assumptions. `Core` installs the closed scalar registry and has no distribution-space declarations; `Extend` adds fresh declarations referring only to earlier spaces and cannot shadow existing keys. Regions use exact ambient, empty, guard-true, guard-false, meet, union and difference constructors. The checker decides rational affine comparisons and integer cells, preserving singleton endpoints and strict definedness; an undefined guard is neither true nor false. Real distribution spaces currently require a nonempty connected open interval and the `CompactSmooth` test family.
//!
//! A local context records its parent, exact admissible meet and qualified frame slots. Hypothesis use names its introducing context and slot; siblings cannot borrow that identity. Closure removes only used local assumptions, using supplied parent evidence or retaining a guarded residual on the child region. Restriction requires an explicit checked subset premise. Predicate facts are checked on exact nonempty regions; empty-region facts hold vacuously without evaluating their predicate. An empty requested function domain refuses.

pub use crate::graph::AssumptionDraft;

pub use crate::header::FrozenEnvironment;

pub use crate::domain_model::{
    AmbientDomain, BoundaryDraft, CaseArmDraft, CasesDraft, ContextDraft, DeclarationSpaceDraft,
    HeaderDraft, HypothesisRef, Layer, RegionDraft, RegionExpr, Side, SpaceDraft,
};
