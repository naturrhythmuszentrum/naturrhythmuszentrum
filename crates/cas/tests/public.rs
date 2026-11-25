// SPDX-License-Identifier: AGPL-3.0-only
//! Public-API conformance and client integration tests share one test binary.

use nrz_cas as test_api;

#[path = "../src/testing/public.rs"]
#[expect(
    clippy::items_after_statements,
    reason = "legacy fixture-local imports stay next to public test setup"
)]
mod testing;

mod conformance;
mod integration;
