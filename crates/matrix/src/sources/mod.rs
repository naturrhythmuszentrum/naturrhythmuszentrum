// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable source declarations and sufficient covariance witnesses.
//!
//! # Source declarations and covariance admission
//!
//! `Source` declares an immutable process-local identity, positive revision and coherent continuous difference schema for a zero-mean fluctuation with finite second moments. Clones retain the same identity. `redeclare` refuses changed schemas or revisions. `JointModel::from_factor` admits a complete ordered source vector and explicit square `Known`/`DeclaredZero`/`Unknown` table. Unknown blocks refuse requested contractions; distinct identities imply no independence. Zero declarations need nonempty assumption text and a structural zero witness. Reusing any source in a different ordered model refuses conservatively, including equivalent alternative factor formulas; exact repeated admissions share the original model identity. New physical declarations require new source identities.
//!
//! `SourceFactor` defines the mathematical covariance as `L diag(d) Lᵀ`, with finite represented column-ordered coefficients and nonnegative finite weights. Rectangular, singular, rank-one and zero-column factors are supported without a positive-pivot requirement. Its outward entry evaluator bounds covariance parameters, never physical source excursions. `JointLimits` bounds source count, representation entries and numerical work. Factor admission charges coefficients plus weights; entry evaluation charges factor columns; complete model admission charges the square source count times the greater of one and the factor column count.
//!
//! `DenseCovariance` admits exact represented symmetry and proves PSD by outward nonnegative diagonal dominance or interval symmetric LDL elimination with strictly positive pivot lower bounds. `with_enclosures` applies the sufficient test to the entire symmetric parameter family, including both endpoint matrices and all intervening members. Both tests can refuse a valid singular PSD matrix; `CannotValidateCovariance` reports insufficient proof, never indefiniteness. A factor is the alternative for those cases. No eigenvalue clipping, repair or inferred symmetry occurs. Dense admission budgets a conservative cubic-plus-quadratic work count, and retains the specific sufficient certificate.

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

mod dense;
mod representation;
pub use dense::{DenseCovariance, PsdCertificate};
pub use representation::CovarianceRepresentation;
