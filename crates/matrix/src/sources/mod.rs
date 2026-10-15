// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable source declarations and sufficient covariance witnesses.

mod error;
mod factor;
mod limits;
mod model;
mod source;

pub use error::JointError;
pub use factor::SourceFactor;
pub use limits::JointLimits;
pub use model::{CovarianceBlock, JointModel, ModelId};
pub use source::{Source, SourceId};
