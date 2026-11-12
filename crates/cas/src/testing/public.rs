// SPDX-License-Identifier: AGPL-3.0-only
//! Shared public-API fixtures for conformance and integration tests.
//!
//! Compiled only in the public test target.
//!
//! Inline unit tests never import this harness. Crate-only fixture sources are not included.

#[expect(clippy::redundant_pub_crate, reason = "shared formal object fixtures")]
pub(crate) mod series;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared complete-operation finite boundaries"
)]
pub(crate) mod series_boundaries;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared independent calculus fixtures"
)]
pub(crate) mod series_calculus;
#[expect(
    clippy::redundant_pub_crate,
    reason = "Composition fixtures are shared by the public test suites"
)]
pub(crate) mod series_compose;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared finite arithmetic fixtures"
)]
pub(crate) mod series_proof;
#[expect(
    clippy::redundant_pub_crate,
    reason = "public residual fixtures are shared by client tests"
)]
#[expect(
    dead_code,
    reason = "exact ancestor-frame mutation uses the internal shared checker"
)]
pub(crate) mod series_residual;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared source evidence fixtures"
)]
pub(crate) mod series_source;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared complete source measurements"
)]
pub(crate) mod series_source_work;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared formal object boundaries"
)]
pub(crate) mod series_work;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared public totality fixtures"
)]
#[path = "trig_defined.rs"]
pub(crate) mod trig_defined;

#[expect(clippy::redundant_pub_crate, reason = "shared public replay fixtures")]
#[path = "trig_replay.rs"]
pub(crate) mod trig_replay;

#[expect(clippy::redundant_pub_crate, reason = "shared graph resource ceilings")]
#[path = "trig_limits.rs"]
pub(crate) mod trig_limits;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "budgets.rs"]
pub(crate) mod budgets;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "builders.rs"]
pub(crate) mod builders;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared exact-space map fixtures"
)]
#[path = "case_spaces.rs"]
pub(crate) mod case_spaces;
#[expect(clippy::redundant_pub_crate, reason = "shared public case fixtures")]
#[path = "cases.rs"]
pub(crate) mod cases;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "common.rs"]
pub(crate) mod common;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "graph.rs"]
pub(crate) mod graph;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "pendulum.rs"]
pub(crate) mod pendulum;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "proofs.rs"]
pub(crate) mod proofs;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "representation.rs"]
pub(crate) mod representation;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "requests.rs"]
pub(crate) mod requests;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "square_free.rs"]
pub(crate) mod square_free;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "wire.rs"]
pub(crate) mod wire;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared test helpers stay crate-visible inside the private harness"
)]
#[path = "witness.rs"]
pub(crate) mod witness;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared boundary syntax fixtures"
)]
#[path = "boundary.rs"]
pub(crate) mod boundary;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared public distribution fixtures"
)]
#[path = "distributions.rs"]
pub(crate) mod distributions;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared public split calculus fixtures"
)]
#[path = "split.rs"]
pub(crate) mod split;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared public germ calculus fixtures"
)]
#[path = "germs.rs"]
pub(crate) mod germs;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared public pullback fixtures"
)]
#[path = "pullbacks.rs"]
pub(crate) mod pullbacks;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared public primitive fixtures"
)]
#[path = "primitives.rs"]
pub(crate) mod primitives;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared admission fixtures stay crate-visible"
)]
#[path = "admission.rs"]
pub(crate) mod admission;

#[path = "solve_numeric.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared solving fixtures remain crate-visible"
)]
pub(crate) mod solve_numeric;

#[path = "solve_graph.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared solving fixtures remain crate-visible"
)]
pub(crate) mod solve_graph;

#[path = "solve_coefficients_public.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared solving fixtures remain crate-visible"
)]
pub(crate) mod solve_coefficients;

#[path = "solve_elementary_public.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared solving fixtures remain crate-visible"
)]
pub(crate) mod solve_elementary;

#[path = "solve_orbits_public.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared solving fixtures remain crate-visible"
)]
pub(crate) mod solve_orbits;

#[path = "solve_blind_public.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared solving fixtures remain crate-visible"
)]
pub(crate) mod solve_blind;

#[path = "solve_rational.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared solving fixtures remain crate-visible"
)]
pub(crate) mod solve_rational;

#[path = "solve_linear.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared solving fixtures remain crate-visible"
)]
pub(crate) mod solve_linear;

#[path = "solve_local.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared solving fixtures remain crate-visible"
)]
pub(crate) mod solve_local;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared scalar-space admission fixtures"
)]
pub(crate) mod sorts;

#[expect(
    clippy::redundant_pub_crate,
    reason = "public harness shares original routing fixtures"
)]
pub(crate) mod sort_routing;

#[expect(
    clippy::redundant_pub_crate,
    reason = "public harness shares supplied evaluation fixtures"
)]
pub(crate) mod coefficient_evaluation;

#[expect(
    clippy::redundant_pub_crate,
    reason = "public harness shares supplied Real witnesses"
)]
pub(crate) mod solve_real;

#[expect(
    clippy::redundant_pub_crate,
    reason = "public replay fixtures share original expected claims"
)]
#[path = "answer_public.rs"]
pub(crate) mod answer;

#[path = "readings.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared reading fixtures stay crate-visible"
)]
pub(crate) mod readings;

#[path = "map_inverses.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared exact-map fixtures stay crate-visible"
)]
pub(crate) mod map_inverses;

#[path = "jacobians.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared public Jacobian fixtures"
)]
pub(crate) mod jacobians;

#[path = "jacobian_composition.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared reading Jacobian fixtures"
)]
pub(crate) mod jacobian_composition;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared public strict sign fixtures"
)]
pub(crate) mod strict_sign;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared structural trigonometric fixtures"
)]
#[path = "trig.rs"]
pub(crate) mod trig;

#[expect(clippy::redundant_pub_crate, reason = "shared half-Pi fixtures")]
pub(crate) mod half_pi;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared exact trigonometric value fixtures"
)]
pub(crate) mod trig_values;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared exact value boundary fixtures"
)]
pub(crate) mod trig_values_work;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared half-Pi resource fixtures"
)]
pub(crate) mod half_pi_limits;
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared half-Pi resource fixtures"
)]
pub(crate) mod half_pi_work;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared totality resource fixtures"
)]
#[path = "trig_defined_work.rs"]
pub(crate) mod trig_defined_work;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared original parity fixtures"
)]
#[path = "trig_parity.rs"]
pub(crate) mod trig_parity;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared measured parity fixtures"
)]
#[path = "trig_parity_work.rs"]
pub(crate) mod trig_parity_work;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared endpoint provenance fixtures"
)]
#[path = "endpoint_hypotheses.rs"]
pub(crate) mod endpoint_hypotheses;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared original minus-one recipe fixtures"
)]
#[path = "trig_minus_one.rs"]
pub(crate) mod trig_minus_one;

#[path = "trig_minus_one_work.rs"]
#[expect(clippy::redundant_pub_crate, reason = "shared public test fixtures")]
pub(crate) mod trig_minus_one_work;

#[path = "scalar_transport.rs"]
#[expect(clippy::redundant_pub_crate, reason = "shared public test fixtures")]
pub(crate) mod scalar_transport;

#[path = "scalar_transport_work.rs"]
#[expect(clippy::redundant_pub_crate, reason = "shared public test fixtures")]
pub(crate) mod scalar_transport_work;

#[expect(
    clippy::redundant_pub_crate,
    reason = "shared structural totality fixtures"
)]
#[path = "defined_total.rs"]
pub(crate) mod defined_total;

#[expect(
    clippy::redundant_pub_crate,
    reason = "independent public resource inventory"
)]
#[path = "defined_total_minimal.rs"]
pub(crate) mod defined_total_minimal;
