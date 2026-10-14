// SPDX-License-Identifier: AGPL-3.0-only
//! Whole-tuple condition identity across exact region and context boundaries.

use crate::domain::{AmbientDomain, ContextDraft, HeaderDraft, Layer, RegionDraft, RegionExpr};
use crate::proof::GuardedObligation;
use crate::syntax::{Draft, Environment, ExpressionDraft, Operator, PredicateKind};
use crate::testing::common::{TestResult, default_budget, real};
use crate::{Cas, Rational, Reference};

/// Condition union keeps equal predicates separate when their home, region or layer differs.
/// ´test:crate:guarded-tuple-identity´
#[test]
fn guarded_tuple_identity() -> TestResult {
    let mut cas = Cas::new(crate::limits::Limits::default())?;
    let values = cas.admit(
        &[
            Draft::Header(HeaderDraft::Core(Environment::default())),
            Draft::Region(RegionDraft {
                header: Reference::Local(0),
                scope: Vec::new(),
                ambient: AmbientDomain::Assignments,
                expression: RegionExpr::Ambient,
            }),
            Draft::Region(RegionDraft {
                header: Reference::Local(0),
                scope: Vec::new(),
                ambient: AmbientDomain::Assignments,
                expression: RegionExpr::Empty,
            }),
            Draft::Context(ContextDraft::Root {
                header: Reference::Local(0),
                admissible: Reference::Local(1),
                assumptions: Vec::new(),
            }),
            Draft::Context(ContextDraft::Root {
                header: Reference::Local(0),
                admissible: Reference::Local(2),
                assumptions: Vec::new(),
            }),
            Draft::Expression(ExpressionDraft {
                space: None,
                header: Reference::Local(0),
                scope: Vec::new(),
                sort: real(),
                operator: Operator::Rational(Rational::one()),
                binding: None,
                operands: Vec::new(),
            }),
            Draft::Condition {
                header: Reference::Local(0),
                scope: Vec::new(),
                kind: PredicateKind::NonZero,
                arguments: vec![Reference::Local(5)],
            },
        ],
        &mut default_budget()?,
    )?;
    let base = GuardedObligation {
        home: values.get(3).ok_or("missing root")?.clone(),
        scope: Vec::new(),
        layer: Layer::Pointwise,
        space: None,
        region: values.get(1).ok_or("missing region")?.clone(),
        predicate: values.get(6).ok_or("missing predicate")?.clone(),
    };
    let different_region = GuardedObligation {
        region: values.get(2).ok_or("missing empty region")?.clone(),
        ..base.clone()
    };
    let different_home = GuardedObligation {
        home: values.get(4).ok_or("missing second root")?.clone(),
        ..base.clone()
    };
    let different_layer = GuardedObligation {
        layer: Layer::Logic,
        ..base.clone()
    };
    let input = vec![
        base.clone(),
        different_region,
        different_home,
        different_layer,
        base,
    ];
    let result = crate::statement::union(&cas, &input, &mut default_budget()?)?;
    assert_eq!(result.len(), 4);
    for expected in input {
        assert!(result.contains(&expected));
    }
    let mut reversed = result.clone();
    reversed.reverse();
    assert_eq!(
        crate::statement::union(&cas, &reversed, &mut default_budget()?)?,
        result
    );
    Ok(())
}
