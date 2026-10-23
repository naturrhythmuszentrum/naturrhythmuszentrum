// SPDX-License-Identifier: AGPL-3.0-only
//! The complete policy manifest is independent expected data.

use crate::testing::common::TestResult;
use nrz_cas::proof::PRIMITIVES;
use nrz_cas::request::CostPolicy;

/// Complete independent expected weights for all registered semantic-version-one schemas.
const EXPECTED: [(&str, u64); 80] = [
    ("case.congruence", 4),
    ("case.domain", 2),
    ("case.eliminate", 4),
    ("case.ordered", 4),
    ("case.select", 2),
    ("coeff.evaluate", 4),
    ("coeff.identity", 4),
    ("complex.conjugate", 4),
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
    ("dist.ae", 4),
    ("dist.affine-pullback", 12),
    ("dist.delta-multiply", 8),
    ("dist.delta-pair", 4),
    ("dist.derivative", 2),
    ("dist.jump", 8),
    ("dist.linear", 2),
    ("dist.primitive-unique", 8),
    ("dist.regular", 8),
    ("dist.simple-zero-pullback", 12),
    ("dist.smooth", 8),
    ("dist.support-action", 8),
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
    ("extension.adjoin", 8),
    ("extension.split", 12),
    ("extension.unit", 8),
    ("integral.case-split", 8),
    ("jet.polynomial", 4),
    ("limit.case-restrict", 2),
    ("linear.equivalent", 8),
    ("orbit.cover", 8),
    ("poly.jets-equal", 4),
    ("poly.rational-equal", 4),
    ("poly.square-free", 8),
    ("real.cell-exists", 2),
    ("real.root-count", 8),
    ("real.sign", 2),
    ("region.construct", 2),
    ("region.decide", 4),
    ("root.binomial", 8),
    ("root.isolate", 12),
    ("root.select", 8),
    ("series.case-restrict", 4),
    ("solve.family", 8),
    ("solve.local-branch", 16),
    ("solve.member", 4),
    ("solve.multiplicity", 8),
    ("solve.product", 8),
    ("solve.split", 4),
    ("solve.substitute", 4),
    ("sum.case-split", 4),
];

/// The current logical policy includes the closed registered schemas, with availability separate.
/// ´test:integration:active-policy-complete-manifest´
#[test]
fn active_policy_complete_manifest() -> TestResult {
    let policy = CostPolicy::default();
    assert_eq!(policy.version, 10);
    assert_eq!(PRIMITIVES.len(), EXPECTED.len());
    let actual: Vec<_> = PRIMITIVES
        .iter()
        .map(|primitive| {
            let rule = primitive.specification();
            assert_eq!(
                rule.version,
                if *primitive == nrz_cas::proof::Primitive::RegionDecide {
                    2
                } else {
                    1
                }
            );
            assert_eq!(
                rule.available,
                match primitive {
                    nrz_cas::proof::Primitive::Solve(schema) => matches!(
                        schema.availability(),
                        nrz_cas::solving::Availability::Available
                    ),
                    _ => true,
                }
            );
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
    search.costs.version = 4;
    assert!(matches!(
        fixture.cas.run_with_policy(&request, &search),
        nrz_cas::request::Outcome::Invalid { .. }
    ));
    Ok(())
}
