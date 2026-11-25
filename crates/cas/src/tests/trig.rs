// SPDX-License-Identifier: AGPL-3.0-only
//! Canonical identities and binder-safe reconstruction of closed Real nodes.

use crate::Reference;
use crate::syntax::{AtomicSort, Binding, Draft, Field, Operator, Substitution};
use crate::testing::common::{TestResult, default_budget, real};
use crate::testing::trig::{Fixture, operators};

/// Complete canonical keys distinguish constructors, opaque calls, and declaration contexts.
/// ´test:crate:trig-canonical-identity´
#[test]
fn trig_canonical_identity() -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    let mut f = Fixture::new()?;
    let mut keys = Vec::new();
    let projection = f.term(Operator::RealPart, &[f.z.clone()])?;
    assert_ne!(projection, f.z);
    for (operator, name) in operators() {
        let node = f.term(operator.clone(), &[f.x.clone()])?;
        let again = f.term(operator.clone(), &[f.x.clone()])?;
        assert_eq!(node, again);
        let opaque = f.term(Operator::Call(name.into()), &[f.x.clone()])?;
        for handle in [&node, &opaque] {
            let vertex = f.cas.state.node(handle.logical_id(), &mut budget)?;
            let key = crate::graph::vertex_key(vertex, &f.cas.state, &mut default_budget()?)?.1;
            assert!(!keys.contains(&key));
            keys.push(key);
        }
        let original = &f
            .cas
            .state
            .header(f.header.logical_id(), &mut budget)?
            .environment;
        let mut environment = crate::syntax::Environment {
            field: original.field.clone(),
            branch: original.branch.clone(),
            declarations: original.declarations.clone(),
        };
        environment.declarations.push(crate::syntax::Declaration {
            key: "extra".into(),
            display_name: "extra".into(),
            sort: real(),
        });
        let header = f
            .cas
            .admit(
                &[Draft::Header(crate::domain::HeaderDraft::Core(environment))],
                &mut default_budget()?,
            )?
            .pop()
            .ok_or("other header")?;
        let mut other = f.draft(Operator::Free("x".into()), &[]);
        other.header = Reference::Existing(header.clone());
        let x = f
            .cas
            .admit(&[Draft::Expression(other)], &mut default_budget()?)?
            .pop()
            .ok_or("other x")?;
        let mut other = f.draft(operator, &[x]);
        other.header = Reference::Existing(header);
        let other = f
            .cas
            .admit(&[Draft::Expression(other)], &mut default_budget()?)?
            .pop()
            .ok_or("other node")?;
        assert_ne!(node, other);
    }
    Ok(())
}

/// A replacement bound outside two integrals is lifted twice and never captured.
/// ´test:crate:trig-capture-safe-substitution´
#[test]
fn trig_capture_safe_substitution() -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    let mut f = Fixture::new()?;
    let scalar = AtomicSort::Scalar(Field::Real);
    let outer = vec![vec![scalar.clone()]];
    let middle = vec![vec![scalar.clone()]; 2];
    let inner = vec![vec![scalar]; 3];
    let replacement = f.scoped(Operator::Bound { depth: 0, slot: 0 }, outer.clone(), &[])?;
    for (operator, _) in operators() {
        let x = f.scoped(Operator::Free("x".into()), inner.clone(), &[])?;
        let body = f.scoped(operator.clone(), inner.clone(), std::slice::from_ref(&x))?;
        let mut root = body.clone();
        for scope in [&middle, &outer] {
            root = integral(&mut f, root, scope.clone())?;
        }
        let source = f
            .cas
            .admit(
                &[Draft::Source {
                    key: format!("{operator:?}"),
                    text: "original".into(),
                }],
                &mut default_budget()?,
            )?
            .pop()
            .ok_or("source")?;
        let closed = integral(&mut f, root.clone(), Vec::new())?;
        let original = f
            .cas
            .materialize_original(&closed, &source, &mut default_budget()?)?;
        let walk = f.cas.walk_original(&original, &mut default_budget()?)?;
        let changed = f.cas.substitute(
            &root,
            &[Substitution {
                key: "x".into(),
                replacement: replacement.clone(),
            }],
            &mut default_budget()?,
        )?;
        let mut current = changed;
        for _ in 0..2 {
            current = f
                .cas
                .operands(&current, &mut default_budget()?)?
                .pop()
                .ok_or("body")?;
        }
        let expression = f.cas.state.expression(current.logical_id(), &mut budget)?;
        assert_eq!(expression.operator, operator);
        assert_eq!(expression.scope, inner);
        assert_eq!(expression.sort, real());
        assert_eq!(expression.header, f.header.logical_id());
        assert_eq!(
            expression.ambient,
            f.cas
                .state
                .expression(body.logical_id(), &mut budget)?
                .ambient
        );
        let child = f
            .cas
            .operands(&current, &mut default_budget()?)?
            .pop()
            .ok_or("lifted operand")?;
        assert_eq!(
            f.cas.expression(&child, &mut budget)?.0,
            &Operator::Bound { depth: 2, slot: 0 }
        );
        assert_eq!(f.cas.operands(&body, &mut default_budget()?)?, [x]);
        let after = f.cas.walk_original(&original, &mut default_budget()?)?;
        assert_eq!(format!("{:?}", walk.events), format!("{:?}", after.events));
    }
    Ok(())
}

fn integral(
    f: &mut Fixture,
    body: crate::Handle,
    scope: crate::syntax::Scope,
) -> TestResult<crate::Handle> {
    let mut draft = f.draft(Operator::Integral { definite: false }, &[body]);
    draft.scope = scope;
    draft.binding = Some(Binding {
        sorts: vec![AtomicSort::Scalar(Field::Real)],
        body_operands: vec![0],
    });
    Ok(f.cas
        .admit(&[Draft::Expression(draft)], &mut default_budget()?)?
        .pop()
        .ok_or("integral")?)
}
