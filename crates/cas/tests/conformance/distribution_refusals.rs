// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit refusal categories remain distinct from missing analytic premises.

use crate::testing::budgets::derivation_budget as budget;
use crate::testing::builders::admit;
use crate::testing::common::TestResult;
use crate::testing::distributions::Fixture;
use nrz_cas::domain::{ContextDraft, Layer};
use nrz_cas::request::{ActionMode, Outcome, RefusalReason, Task};
use nrz_cas::syntax::{Branch, CaseName, Draft, NamedCase, Operator, PredicateKind, Sort};
use nrz_cas::{Cas, Reference};

/// Point evaluation and distribution products are refused before constructing invalid syntax.
/// ´test:integration:distribution-named-refusals´
#[test]
fn distribution_named_refusals() -> TestResult {
    let mut fixture = Fixture::new()?;
    let delta = fixture.delta("0", 0)?;
    let point = fixture.base.request(
        Task::ApplyAt {
            coordinate: "x".into(),
            point: fixture.base.zero.clone(),
        },
        std::slice::from_ref(&delta),
    )?;
    assert!(matches!(
        fixture.base.cas.run(&point),
        Outcome::Refused {
            reason: RefusalReason::DistributionPointValue,
            ..
        }
    ));
    for task in [
        Task::Product {
            requested_sort: Sort::Distribution,
        },
        Task::SmoothMultiply {
            space: fixture.space.clone(),
        },
    ] {
        let product = fixture
            .base
            .request(task, &[delta.clone(), delta.clone()])?;
        assert!(matches!(
            fixture.base.cas.run(&product),
            Outcome::Refused {
                reason: RefusalReason::DistributionProductUndefined,
                ..
            }
        ));
    }
    let step = fixture.base.cas.named_case(
        &fixture.base.domain,
        &NamedCase {
            name: CaseName::Step,
            coordinate: "x".into(),
            parameter: Some(nrz_cas::Rational::zero()),
        },
        &mut budget()?,
    )?;
    let product = fixture.base.request(
        Task::Product {
            requested_sort: Sort::Distribution,
        },
        &[delta, step],
    )?;
    assert!(matches!(
        fixture.base.cas.run(&product),
        Outcome::Refused {
            reason: RefusalReason::DistributionProductUndefined,
            ..
        }
    ));
    Ok(())
}

/// Identical display names do not turn a positive-half-axis test into a whole-axis test.
/// ´test:integration:distribution-test-space-refusal´
#[test]
fn distribution_test_space_refusal() -> TestResult {
    let mut fixture = Fixture::new()?;
    let delta = fixture.delta("2", 0)?;
    let positive = fixture
        .base
        .predicate(PredicateKind::Positive, &[fixture.base.x.clone()])?;
    let domain = fixture.base.region(&positive, true)?;
    let (_, header, test) = crate::testing::case_spaces::space(
        &mut fixture.base.cas,
        &fixture.base.header,
        &domain,
        "x",
    )?;
    let context = admit(
        &mut fixture.base.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header),
            admissible: Reference::Existing(fixture.base.domain.clone()),
            assumptions: Vec::new(),
        }),
        &mut budget()?,
    )?;
    fixture.base.context = context;
    let request = fixture.base.request(
        Task::Pair {
            space: fixture.space.clone(),
            action_mode: ActionMode::Test,
        },
        &[delta, test],
    )?;
    assert!(matches!(
        fixture.base.cas.run(&request),
        Outcome::Refused {
            reason: RefusalReason::TestSpaceMismatch,
            ..
        }
    ));
    Ok(())
}

/// Lack of a polynomial smoothness certificate does not prove discontinuity.
/// ´test:integration:distribution-unknown-smoothness-pending´
#[test]
fn distribution_unknown_smoothness_pending() -> TestResult {
    let mut fixture = Fixture::new()?;
    let delta = fixture.delta("2", 0)?;
    let function = fixture.base.term(
        Operator::GeneralPower(Branch::StrictReal),
        &[fixture.base.x.clone(), fixture.base.x.clone()],
    )?;
    let request = fixture.base.request(
        Task::Product {
            requested_sort: Sort::Distribution,
        },
        &[function, delta],
    )?;
    assert!(matches!(
        fixture.base.cas.run(&request),
        Outcome::PendingPremise { .. }
    ));
    Ok(())
}

/// The full expected judgment prevents relabeling a scalar pairing as a classical equality.
/// ´test:integration:distribution-layer-and-space-tampering´
#[test]
fn distribution_layer_and_space_tampering() -> TestResult {
    let mut fixture = Fixture::new()?;
    let delta = fixture.delta("2", 0)?;
    let polynomial = fixture.base.x.clone();
    let result = fixture.pair(&delta, &polynomial)?;
    let bytes = fixture
        .base
        .cas
        .export_witness(&result.witness, &mut budget()?)?;
    let mut receiver = Cas::new(budget()?.limits().clone())?;
    let imported = receiver.inspect(&bytes, &mut budget()?)?;
    let mut expected = imported.judgment().clone();
    expected.layer = Layer::Pointwise;
    assert!(receiver.replay(&bytes, &expected, &mut budget()?).is_err());
    expected.clone_from(imported.judgment());
    expected.space = None;
    assert!(receiver.replay(&bytes, &expected, &mut budget()?).is_err());
    Ok(())
}
