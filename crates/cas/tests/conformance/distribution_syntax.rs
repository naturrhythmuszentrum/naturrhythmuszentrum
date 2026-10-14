// SPDX-License-Identifier: AGPL-3.0-only
//! Complete typed descriptors and substitutions preserve distribution action spaces.

use crate::testing::budgets::derivation_budget as budget;
use crate::testing::common::{TestResult, real};
use crate::testing::distributions::Fixture;
use nrz_cas::domain::Layer;
use nrz_cas::source::{SyntaxNode, SyntaxRole};
use nrz_cas::syntax::{Operator, Substitution};

/// A scalar-valued pairing carries both the distribution layer and an explicit space dependency.
/// ´test:integration:distribution-action-descriptor-space´
#[test]
fn distribution_action_descriptor_space() -> TestResult {
    let mut fixture = Fixture::new()?;
    let test = fixture.test()?;
    let delta = fixture.delta("2", 0)?;
    let pairing = fixture.node(Operator::Pair, &[delta, test])?;
    let descriptor = fixture.base.cas.describe_syntax(&pairing, &mut budget()?)?;
    let SyntaxNode::Expression(value) = descriptor.node else {
        return Err("expression".into());
    };
    assert_eq!(value.sort, real());
    assert_eq!(value.layer, Layer::Distribution);
    assert!(descriptor.dependencies.iter().any(
        |dependency| dependency.role == SyntaxRole::Space && dependency.target == fixture.space
    ));
    Ok(())
}

/// Replacing a declared test preserves the complete action, while ordinary substitution cannot move its coordinate.
/// ´test:integration:distribution-substitution-preserves-space´
#[test]
fn distribution_substitution_preserves_space() -> TestResult {
    let mut fixture = Fixture::new()?;
    let test = fixture.test()?;
    let delta = fixture.delta("2", 0)?;
    let pairing = fixture.node(Operator::Pair, &[delta.clone(), test.clone()])?;
    let differentiated = fixture.node(Operator::TestDerivative { order: 1 }, &[test])?;
    let actual = fixture.base.cas.substitute(
        &pairing,
        &[Substitution {
            key: "phi".into(),
            replacement: differentiated.clone(),
        }],
        &mut budget()?,
    )?;
    let expected = fixture.node(Operator::Pair, &[delta, differentiated])?;
    assert_eq!(actual, expected);
    let zero = fixture.base.number("0")?;
    assert!(
        fixture
            .base
            .cas
            .substitute(
                &pairing,
                &[Substitution {
                    key: "x".into(),
                    replacement: zero,
                }],
                &mut budget()?
            )
            .is_err()
    );
    Ok(())
}
