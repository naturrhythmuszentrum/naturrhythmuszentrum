// SPDX-License-Identifier: AGPL-3.0-only
//! Public import and ownership lifecycles.

mod aliases;
mod compaction;
mod coordinates;
mod imports;
mod layouts;
mod lifecycle;
mod shapes;

mod binding;

mod transactions;

mod joint_companions;

mod conversion;
mod joint_dense;

mod precision_conversion;
mod schema_conversion;

mod kernel_lifecycle;

mod kernel_product;

mod kernel_nonlinear;

mod solve;

mod solve_bounded;

mod solve_inverse;

mod solve_restoration;

mod solve_coverage;

mod solve_adversarial;

mod solve_physical;
mod statistics_affine;

mod statistics_nonlinear;
mod statistics_physical;

mod statistics_product;

mod statistics_conversion;
mod statistics_precision;

mod statistics_structured;

mod solve_sensitivity;

mod solve_mixed_precision;

mod solve_factor_reuse;

mod acceptance;

mod exact_scale_boundaries;
mod symbol_lifecycle;

mod conversion_errors;

mod bounded_readings;

mod complex_copies;

mod reciprocal_operator;

mod reciprocal_sources;

mod reciprocal_derivatives;
