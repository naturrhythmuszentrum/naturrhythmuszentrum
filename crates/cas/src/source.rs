// SPDX-License-Identifier: AGPL-3.0-only
//! Public source vocabulary and checked service contracts.

pub use crate::traversal::{
    CompleteWalk, SourceLocation, TraversalReceipt, Visit, WalkEvent, WalkFailure,
};

pub use crate::axis_map::AxisMap;
pub use crate::descriptor::{SyntaxDependency, SyntaxDescriptor, SyntaxRole};
pub use crate::descriptor_model::{ExpressionSyntax, RegionConstructor, SpaceSyntax, SyntaxNode};
pub use crate::source_factory::{OriginalForm, case_literal, ordered_cases};
pub use crate::source_model::{SourceClauseDraft, SourceFormDraft, SourceFormKind};
