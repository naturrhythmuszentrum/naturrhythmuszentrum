// SPDX-License-Identifier: AGPL-3.0-only
//! Checks that need crate-visible access across implementation modules.

mod frontier;
mod frozen;
mod snapshot;

mod cases;
mod conditions;

mod boundary;
mod distributions;
mod scalar_transport;
mod series_live;
mod series_object;
mod series_residual;
mod trig_minus_one_transport;

mod region_cache;

mod germs;
mod parser;

mod primitive_unique;

mod admission;
mod interning;

mod solve_numeric;

mod solve_graph;

mod solve_coefficients;

mod solve_elementary;

mod solve_orbits;

mod solve_blind;

mod solve_rational;

mod solve_linear;

mod solve_local;

mod sorts_wire;

mod solve_real;

mod family;

mod emptiness_registry;
mod family_solver;

mod wire_answer;

mod tower_closure;

mod dependencies;

mod work_trace;

mod wire_memo;

mod half_pi;
mod half_pi_work;
mod reading_memo;
mod trig;
mod trig_wire_work;
mod trig_work;

mod defined_total;
