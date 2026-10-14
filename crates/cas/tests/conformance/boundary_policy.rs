// SPDX-License-Identifier: AGPL-3.0-only
//! The complete policy manifest is independent expected data.

use crate::testing::common::TestResult;
use nrz_cas::proof::PRIMITIVES;
use nrz_cas::request::CostPolicy;

/// All forty existing weights plus exactly three semantic-version-one boundary checkers.
const EXPECTED: [(&str, u64); 43] = [
    ("case.congruence", 4),
    ("case.domain", 2),
    ("case.eliminate", 4),
    ("case.ordered", 4),
    ("case.select", 2),
    ("ctx.close", 2),
    ("ctx.hypothesis", 1),
    ("ctx.weaken", 1),
    ("diff.add", 2),
    ("diff.case-boundary", 4),
    ("diff.case-interior", 2),
    ("diff.constant", 2),
    ("diff.integer-power", 2),
    ("diff.jacobian", 2),
    ("diff.neg", 2),
    ("diff.product", 2),
    ("diff.quotient", 2),
    ("diff.sub", 2),
    ("diff.transport", 1),
    ("diff.variable", 2),
    ("eq.add-associate", 2),
    ("eq.add-commute", 2),
    ("eq.add-inverse", 2),
    ("eq.add-zero", 2),
    ("eq.cancel", 2),
    ("eq.congruence", 1),
    ("eq.distribute", 2),
    ("eq.integer-power", 2),
    ("eq.mul-associate", 2),
    ("eq.mul-commute", 2),
    ("eq.mul-one", 2),
    ("eq.mul-zero", 2),
    ("eq.refl", 1),
    ("eq.substitute", 1),
    ("eq.sym", 1),
    ("eq.trans", 1),
    ("exact.rational", 1),
    ("jet.polynomial", 4),
    ("poly.jets-equal", 4),
    ("poly.rational-equal", 4),
    ("poly.square-free", 8),
    ("region.construct", 2),
    ("region.decide", 4),
];

/// Logical version four has no distribution or reserved future primitive placeholders.
/// ´test:integration:boundary-policy-four-complete-manifest´
#[test]
fn boundary_policy_four_complete_manifest() -> TestResult {
    let policy = CostPolicy::default();
    assert_eq!(policy.version, 4);
    assert_eq!(PRIMITIVES.len(), EXPECTED.len());
    let actual: Vec<_> = PRIMITIVES
        .iter()
        .map(|primitive| {
            let rule = primitive.specification();
            assert_eq!(rule.version, 1);
            assert!(rule.available);
            (rule.key, rule.weight)
        })
        .collect();
    assert_eq!(actual, EXPECTED);
    assert_eq!(
        policy.weights,
        EXPECTED
            .iter()
            .map(|(_, weight)| *weight)
            .collect::<Vec<_>>()
    );
    let mut fixture = crate::testing::cases::Fixture::new()?;
    let one = fixture.one.clone();
    let request = fixture.request(nrz_cas::request::Task::Simplify, std::slice::from_ref(&one))?;
    let mut search = nrz_cas::request::SearchPolicy::default();
    search.costs.version = 3;
    assert!(matches!(
        fixture.cas.run_with_policy(&request, &search),
        nrz_cas::request::Outcome::Invalid { .. }
    ));
    Ok(())
}
