// SPDX-License-Identifier: AGPL-3.0-only
//! Both primitives of delta at zero retain their complete expected constant-difference theorem.

use crate::testing::budgets::derivation_budget as budget;
use crate::testing::common::TestResult;
use crate::testing::distributions::{Fixture, output};
use crate::testing::primitives::{expected, pair};
use nrz_cas::request::{Outcome, Task};

/// Step and step plus seven both differentiate to delta zero and satisfy primitive uniqueness.
/// ´test:integration:distribution-primitive-unique-lifecycle´
#[test]
fn distribution_primitive_unique_lifecycle() -> TestResult {
    let mut fixture = Fixture::new()?;
    let (left, right) = pair(&mut fixture)?;
    let delta = fixture.delta("0", 0)?;
    assert_eq!(output(&fixture.derivative(&left, 1)?)?, delta);
    assert_eq!(output(&fixture.derivative(&right, 1)?)?, delta);
    let (bytes, work) = unique_bytes(false)?;
    assert_eq!(bytes, unique_bytes(true)?.0);
    let mut receiver = Fixture::new()?;
    let (left, right) = pair(&mut receiver)?;
    let claim = expected(&receiver, left, right);
    let mut ledger = budget()?;
    let proof = receiver.base.cas.replay(&bytes, &claim, &mut ledger)?;
    assert_eq!(
        receiver.base.cas.export_witness(&proof, &mut budget()?)?,
        bytes
    );
    assert!(ledger.usage().work <= work.saturating_mul(10));
    Ok(())
}

/// Irrelevant store allocations cannot affect complete uniqueness proof bytes.
fn unique_bytes(extra: bool) -> TestResult<(Vec<u8>, u64)> {
    let mut fixture = Fixture::new()?;
    if extra {
        fixture.base.number("92")?;
    }
    let inputs: [_; 2] = pair(&mut fixture)?.into();
    let request = fixture.base.request(
        Task::PrimitiveUnique {
            space: fixture.space.clone(),
        },
        &inputs,
    )?;
    let Outcome::Proved { result, usage, .. } = fixture.base.cas.run(&request) else {
        return Err("primitive uniqueness".into());
    };
    assert!(result.replay_usage.work <= usage.work.saturating_mul(10));
    Ok((
        fixture
            .base
            .cas
            .export_witness(&result.witness, &mut budget()?)?,
        usage.work,
    ))
}
