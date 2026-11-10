// SPDX-License-Identifier: AGPL-3.0-only
//! Complete client computations and portable proof lifecycles.

mod trig_work;

mod graph;
mod numbers;
mod proofs;
mod search;
mod square_free;

mod cases;
mod contexts;
mod endpoint_hypotheses;
mod scalar_transport;
mod series;
mod series_compose_golden;
mod series_compose_limits;
mod series_residual_golden;
mod series_residual_limits;
mod series_residual_wire;

/// Share the independently authored refused import with its public conformance control.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Sibling conformance tests need crate visibility; public visibility violates unreachable_pub in this private suite"
)]
pub(super) fn nonzero_compose_import() -> crate::testing::common::TestResult<Vec<u8>> {
    series_compose_golden::nonzero_import()
}
mod series_exclusions;
mod series_golden;
mod series_limits;
mod series_mutations;
mod series_reciprocal_golden;
mod series_source_golden;
mod series_source_limits;
mod series_source_mutations;
mod series_work;

mod boundary;
mod distributions;

mod parser;
mod split;

mod germs;

mod primitive_unique;
mod pullbacks;

mod solve_numeric;

mod solve_graph;

mod solve_coefficients;

mod solve_elementary;

mod solve_orbits;

mod solve_blind;

mod solve_rational;

mod solve_linear;

mod solve_local;

mod sorts;

mod coefficient_evaluation;

mod family;

mod wire_answer;

mod exact_constants;

mod readings;

mod dependencies;
mod dependency_domains;

mod map_inverses;

mod normalization;

mod jacobians;

mod jacobian_matrices;

mod jacobian_composition;

mod half_pi;
mod half_pi_work;
mod strict_sign;
mod trig;
mod trig_values;
mod trig_values_work;

mod trig_defined;
mod trig_parity;

mod trig_defined_work;

mod trig_minus_one;
mod trig_minus_one_work;
mod trig_parity_work;
