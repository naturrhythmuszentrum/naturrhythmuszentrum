// SPDX-License-Identifier: AGPL-3.0-only
//! Existence of a constant difference requires checked equality of actual first derivatives.

use crate::testing::common::TestResult;
use crate::testing::distributions::Fixture;
use crate::testing::primitives::pair;
use nrz_cas::request::{Outcome, Task};
use nrz_cas::syntax::Operator;

/// Step and step plus x have unequal derivatives, so the uniqueness rule cannot apply.
/// ´test:integration:primitive-unique-unequal-derivatives´
#[test]
fn primitive_unique_unequal_derivatives() -> TestResult {
    let mut fixture = Fixture::new()?;
    let (left, _) = pair(&mut fixture)?;
    let linear = fixture.node(Operator::RegularEmbed, &[fixture.base.x.clone()])?;
    let right = fixture.node(Operator::DistributionAdd, &[left.clone(), linear])?;
    let request = fixture.base.request(
        Task::PrimitiveUnique {
            space: fixture.space.clone(),
        },
        &[left, right],
    )?;
    assert!(!matches!(
        fixture.base.cas.run(&request),
        Outcome::Proved { .. } | Outcome::Exhausted { .. }
    ));
    Ok(())
}
