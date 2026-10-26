// SPDX-License-Identifier: AGPL-3.0-only
//! Public heap import contracts, refusal twins and type boundaries.

mod coordinates;
mod expansion;
mod factor_shapes;
mod imports;
mod layouts;
mod lifecycle;
mod outputs;
mod shapes;

mod binding;

mod transactions;

mod joint_sources;

mod joint_zero;

mod conversion;
mod joint_repeated;

mod conversion_twins;

mod precision_conversion;

mod kernel_limits;
mod kernel_linear;
mod kernel_statistics;
mod kernel_twins;

mod kernel_cancellation;
mod kernel_products;

mod kernel_domains;
mod kernel_nonlinear;

mod solve;

mod solve_statistics;

mod solve_family;

mod solve_budget;

mod solve_capacity;
mod statistics_affine;
mod statistics_structure;

mod statistics_limits;

mod statistics_refusals;

mod statistics_nonlinear;

mod statistics_conversion;

mod solve_recovery;

mod solve_factor_reuse;

mod solve_records;

mod dependencies;

mod reciprocal_operator;

mod spectral;

mod spectral_publication;

mod decision_snapshots;

mod decision_fixture;
mod decision_frozen;
