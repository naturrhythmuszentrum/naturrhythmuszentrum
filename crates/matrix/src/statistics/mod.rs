// SPDX-License-Identifier: AGPL-3.0-only
//! Exact affine moments with source-preserving, independently verified contractions.

mod access;
mod affine;
mod allocation;
mod capture;
pub(crate) mod contraction;
mod cross;
mod derivative;
mod error;
mod evaluate;
mod matrix;
pub(crate) use affine::Affine;
pub(crate) use matrix::{Matrix, enclosed, singleton};
mod model;
mod nonlinear;
mod operating;
mod plan;
mod product;
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
mod conversion;
mod precision;

mod structured;
pub(crate) use structured::lower_definition;
