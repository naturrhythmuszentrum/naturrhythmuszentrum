// SPDX-License-Identifier: AGPL-3.0-only
//! Equal derivative premises cannot be replaced, omitted or applied to different primitives.

use crate::domain_model::Layer;
use crate::model::Operator;
use crate::proof_model::Statement;
use crate::task::Task;
use crate::testing::common::TestResult;
use crate::testing::distribution_native::{Fixture, evidence};
use crate::testing::native::budget;

/// The uniqueness checker requires both exact first-derivative theorems in the same space.
/// ´test:crate:primitive-unique-premise-tampering´
#[test]
fn primitive_unique_premise_tampering() -> TestResult {
    let mut fixture = Fixture::new()?;
    let left = fixture.node(Operator::RegularEmbed, &[fixture.base.one.clone()])?;
    let right = fixture.node(Operator::DistributionZero, &[])?;
    let result = fixture.route(
        Task::PrimitiveUnique {
            space: fixture.space.clone(),
        },
        &[left, right],
        false,
    )?;
    let (original, premises) = evidence(&fixture, result.witness.proof())?;
    assert_eq!(result.witness.judgment().layer, Layer::Distribution);
    for mutation in 0..3 {
        let mut draft = original.clone();
        let mut altered = premises.clone();
        match mutation {
            0 => {
                let _removed = altered.pop();
            }
            1 => {
                let statement = &mut altered
                    .first_mut()
                    .ok_or("derivative premise")?
                    .judgment
                    .statement;
                let Statement::Distribution(value) = statement else {
                    return Err("distribution premise".into());
                };
                let crate::distribution_model::DistributionStatement::Derivative { order, .. } =
                    value.as_mut()
                else {
                    return Err("derivative".into());
                };
                *order = 2;
            }
            _ => {
                let Statement::Distribution(value) = &mut draft.conclusion else {
                    return Err("uniqueness".into());
                };
                let crate::distribution_model::DistributionStatement::PrimitiveUnique {
                    left, ..
                } = value.as_mut()
                else {
                    return Err("constant difference".into());
                };
                *left = fixture.node(Operator::Delta { order: 0 }, &[fixture.base.zero.clone()])?;
            }
        }
        assert!(
            crate::scoped_check::check_application(
                &mut fixture.base.cas,
                &draft,
                &altered,
                &mut budget()?
            )
            .is_err()
        );
    }
    Ok(())
}
