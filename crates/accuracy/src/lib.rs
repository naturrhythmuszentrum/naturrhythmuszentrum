// SPDX-License-Identifier: AGPL-3.0-only
#![doc = include_str!("../README.md")]

mod budget;
mod error;
mod nonnegative;
mod policy;
mod recovery;
mod vector;
mod work;

pub use budget::{Budget, BudgetAssessment, BudgetDecision, assess_budget};
pub use error::AccuracyFailure;
pub use nonnegative::Nonnegative;
pub use policy::{Promotion, RecoveryPolicy};
pub use recovery::{Recovery, RecoveryEvidence, RecoveryStatus, StopReason};
pub use vector::{BoundVectorRef, NormKind, NormSummary};
pub use work::{Limits, ResourceLimit, WorkBudget};

#[cfg(any(doc, doctest))]
#[path = "../tests/conformance/type_twins.rs"]
mod type_twins;
