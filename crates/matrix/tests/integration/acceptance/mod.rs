// SPDX-License-Identifier: AGPL-3.0-only
//! Integrated acceptance uses the public facade and independently decoded rational expectations.

mod adversarial;
mod lifecycles;
mod oracle;
mod physical_boundary;
#[expect(
    clippy::redundant_pub_crate,
    reason = "fixtures are shared by private acceptance modules and the measurement executable"
)]
mod support;
mod symbolic_budget;

mod transcripts;
