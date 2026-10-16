// SPDX-License-Identifier: AGPL-3.0-only
//! Exact affine moments with source-preserving, independently verified contractions.

mod access;
mod affine;
mod allocation;
mod capture;
mod contraction;
mod cross;
mod error;
mod evaluate;
mod matrix;
mod model;
mod plan;
mod publish;
mod store;
mod structure;

pub use access::{MomentView, ParameterView};
pub use error::StatisticsError;
pub(crate) use plan::Plan;
pub(crate) use publish::{PreparedMoments, prepare};
pub use store::{MomentAssurance, MomentRef};
pub(crate) use store::{MomentPayload, Store};

pub(crate) use capture::refuse;
