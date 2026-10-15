// SPDX-License-Identifier: AGPL-3.0-only
//! Checks that need crate-visible access across implementation modules.

mod frontier;
mod frozen;
mod snapshot;

mod cases;
mod conditions;

mod boundary;
mod distributions;

mod region_cache;

mod germs;
mod parser;

mod primitive_unique;

mod admission;

mod solve_numeric;

mod solve_graph;

mod solve_coefficients;

mod solve_elementary;

mod solve_orbits;

mod solve_blind;

mod solve_rational;

mod solve_linear;

mod solve_local;
