// SPDX-License-Identifier: AGPL-3.0-only
//! Finite orbit witnesses independently replay in an empty mathematical store.

use crate::testing::common::TestResult;
use crate::testing::solve_orbits::Fixture;
use nrz_cas::Cas;
use nrz_cas::limits::{Budget, Limits};
use nrz_cas::proof::Statement;
use nrz_cas::solving::SolveStatement;

/// Every orientation/power witness replays from bytes alone, then matches its complete expected judgment.
/// ´test:integration:orbit-empty-store-replay´
#[test]
fn orbit_empty_store_replay() -> TestResult {
    for orientation in [1, 2] {
        for (exponent, cardinality) in [(1, 6), (2, 6), (3, 2)] {
            let mut fixture = Fixture::new(orientation, exponent)?;
            let step = fixture.step();
            let witness = fixture.base.cas.check_proof(
                &fixture.base.context,
                std::slice::from_ref(&step),
                &mut Budget::new(Limits::blind_cubic())?,
            )?;
            let bytes = fixture
                .base
                .cas
                .export_witness(&witness, &mut Budget::new(Limits::blind_cubic())?)?;
            let mut empty = Cas::new(Limits::blind_cubic())?;
            let imported = empty.inspect(&bytes, &mut Budget::new(Limits::blind_cubic())?)?;
            assert_eq!(imported.hypotheses(), []);
            assert_eq!(imported.judgment().residuals, []);
            let Statement::Solve(statement) = &imported.judgment().statement else {
                return Err("wrong statement".into());
            };
            let SolveStatement::Orbit(orbit) = statement.as_ref() else {
                return Err("wrong orbit".into());
            };
            assert_eq!(orbit.members.len(), cardinality);
            let replayed = empty.replay(
                &bytes,
                imported.judgment(),
                &mut Budget::new(Limits::blind_cubic())?,
            )?;
            assert_eq!(replayed.judgment(), imported.judgment());
            assert_eq!(
                empty.export_witness(&replayed, &mut Budget::new(Limits::blind_cubic())?)?,
                bytes
            );
        }
    }
    Ok(())
}
