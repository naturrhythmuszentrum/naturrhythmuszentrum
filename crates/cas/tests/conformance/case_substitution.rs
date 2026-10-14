// SPDX-License-Identifier: AGPL-3.0-only
//! Complete source substitution and explicit semantic-axis renaming contracts.

use crate::testing::budgets::derivation_budget as budget;
use crate::testing::builders::admit;
use crate::testing::cases::Fixture;
use crate::testing::common::{TestResult, real};
use nrz_cas::domain::{AmbientDomain, HeaderDraft};
use nrz_cas::source::{AxisMap, SyntaxNode, SyntaxRole};
use nrz_cas::syntax::{Declaration, Draft, Environment, Operator, PredicateKind, Substitution};

/// Simultaneous substitution reaches selectors and values without substituting inside replacements.
/// ´test:integration:case-source-simultaneous-substitution´
#[test]
fn case_source_simultaneous_substitution() -> TestResult {
    let mut f = Fixture::new()?;
    let guard = f.predicate(PredicateKind::NonZero, &[f.x.clone()])?;
    let form = f.ordered(&[(guard, f.x.clone())], Some(f.zero.clone()))?;
    let replacement = f.term(Operator::Add, &[f.x.clone(), f.one.clone()])?;
    let output = f.cas.substitute(
        &form.syntax,
        &[Substitution {
            key: "x".into(),
            replacement: replacement.clone(),
        }],
        &mut budget()?,
    )?;
    let descriptor = f.cas.describe_syntax(&output, &mut budget()?)?;
    let selector = descriptor
        .dependencies
        .iter()
        .find(|child| child.role == SyntaxRole::Selector(0))
        .ok_or("missing source selector")?
        .target
        .clone();
    let value = descriptor
        .dependencies
        .iter()
        .find(|child| child.role == SyntaxRole::Value(0))
        .ok_or("missing source value")?
        .target
        .clone();
    assert_eq!(value, replacement);
    assert_eq!(
        f.cas.predicate_arguments(&selector, &mut budget()?)?,
        vec![replacement]
    );
    let root = f
        .cas
        .materialize_derived(&output, &form.syntax, &mut budget()?)?;
    assert!(
        !f.cas
            .walk_derived(&root, &mut budget()?)?
            .receipt
            .is_original()
    );
    assert!(f.cas.walk_original(&root, &mut budget()?).is_err());
    Ok(())
}

/// Renaming an axis changes its exact header, source guards, values and domain together.
/// ´test:integration:case-exact-axis-map´
#[test]
fn case_exact_axis_map() -> TestResult {
    let mut f = Fixture::new()?;
    let guard = f.predicate(PredicateKind::NonZero, &[f.x.clone()])?;
    let form = f.ordered(&[(guard, f.x.clone())], Some(f.zero.clone()))?;
    let header = admit(
        &mut f.cas,
        Draft::Header(HeaderDraft::Core(Environment {
            declarations: vec![Declaration {
                key: "u".into(),
                display_name: "x".into(),
                sort: real(),
            }],
            ..Environment::default()
        })),
        &mut budget()?,
    )?;
    let map = AxisMap {
        coordinates: vec![("x".into(), "u".into())],
        headers: vec![(f.header.clone(), header.clone())],
        spaces: Vec::new(),
    };
    let transformed = f.cas.rename_axes(&form.syntax, &map, &mut budget()?)?;
    let descriptor = f.cas.describe_syntax(&transformed, &mut budget()?)?;
    assert_eq!(descriptor.header, header);
    for child in descriptor.dependencies {
        match child.role {
            SyntaxRole::Domain => assert!(
                matches!(f.cas.describe_syntax(&child.target, &mut budget()?)?.node,
                SyntaxNode::Region(AmbientDomain::Real(ref key), _) if key == "u")
            ),
            SyntaxRole::Value(0) => assert_eq!(
                *f.cas.expression(&child.target)?.0,
                Operator::Free("u".into())
            ),
            _ => {}
        }
    }
    let before = f.cas.canonical_snapshot(&mut budget()?)?;
    let missing = AxisMap {
        headers: Vec::new(),
        ..map
    };
    assert!(
        f.cas
            .rename_axes(&form.syntax, &missing, &mut budget()?)
            .is_err()
    );
    assert_eq!(f.cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// Descriptor roles include predicate arguments and boundaries; receipts cannot skip either.
/// ´test:integration:case-typed-descriptor-roles´
#[test]
fn case_typed_descriptor_roles() -> TestResult {
    let mut f = Fixture::new()?;
    let predicate = f.predicate(PredicateKind::Equal, &[f.x.clone(), f.zero.clone()])?;
    let descriptor = f.cas.describe_syntax(&predicate, &mut budget()?)?;
    assert!(matches!(
        descriptor.node,
        SyntaxNode::Condition(PredicateKind::Equal)
    ));
    assert_eq!(
        descriptor
            .dependencies
            .iter()
            .map(|child| child.role)
            .collect::<Vec<_>>(),
        vec![SyntaxRole::Operand(0), SyntaxRole::Operand(1)]
    );
    let boundary = f.boundary(&f.x.clone())?;
    let transformed = f.cas.substitute(
        &boundary,
        &[Substitution {
            key: "x".into(),
            replacement: f.one.clone(),
        }],
        &mut budget()?,
    )?;
    let descriptor = f.cas.describe_syntax(&transformed, &mut budget()?)?;
    assert_eq!(
        descriptor
            .dependencies
            .first()
            .ok_or("missing boundary location")?
            .target,
        f.one
    );
    let root = f
        .cas
        .materialize_original(&predicate, &f.source, &mut budget()?)?;
    assert_eq!(
        f.cas
            .walk_original(&root, &mut budget()?)?
            .receipt
            .visited(),
        3
    );
    Ok(())
}

/// Exact space maps rename the interval and refuse an ordinary functional coordinate substitution.
/// ´test:integration:case-exact-space-map´
#[test]
fn case_exact_space_map() -> TestResult {
    use crate::testing::case_spaces::{axis, space};
    let mut f = Fixture::new()?;
    let (old_space, old_header, phi) = space(&mut f.cas, &f.header, &f.domain, "x")?;
    let (core, domain) = axis(&mut f.cas, "u")?;
    let (new_space, new_header, expected) = space(&mut f.cas, &core, &domain, "u")?;
    let map = AxisMap {
        coordinates: vec![("x".into(), "u".into())],
        headers: vec![(f.header.clone(), core), (old_header.clone(), new_header)],
        spaces: vec![(old_space, new_space)],
    };
    assert_eq!(f.cas.rename_axes(&phi, &map, &mut budget()?)?, expected);
    let one = admit(
        &mut f.cas,
        crate::testing::builders::expression_draft(
            Operator::Rational(nrz_cas::Rational::one()),
            real(),
            nrz_cas::Reference::Existing(old_header),
            Vec::new(),
            None,
            &[],
        ),
        &mut budget()?,
    )?;
    let before = f.cas.canonical_snapshot(&mut budget()?)?;
    assert!(
        f.cas
            .substitute(
                &phi,
                &[Substitution {
                    key: "x".into(),
                    replacement: one
                }],
                &mut budget()?
            )
            .is_err()
    );
    let missing = AxisMap {
        spaces: Vec::new(),
        ..map
    };
    assert!(f.cas.rename_axes(&phi, &missing, &mut budget()?).is_err());
    assert_eq!(f.cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}
