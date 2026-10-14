// SPDX-License-Identifier: AGPL-3.0-only
//! Small crate-visible case fixture for kernel evidence mutation tests.

use super::common::{TestResult, real};
use crate::budget::{Budget, Limits};
use crate::domain_model::{
    AmbientDomain, CaseArmDraft, CasesDraft, ContextDraft, HeaderDraft, RegionDraft, RegionExpr,
};
use crate::graph::{Cas, Draft, ExpressionDraft, Handle, Reference};
use crate::model::{Declaration, Environment, Operator, PredicateKind};
use crate::number::Rational;

/// Ample ordinary work leaves the explicitly tested limit as the only changed dimension.
pub(crate) fn budget() -> TestResult<Budget> {
    Ok(Budget::new(Limits {
        work: 2_000_000_000,
        replay_work: 2_000_000_000,
        ..Limits::default()
    })?)
}

/// Independent real-axis syntax with one closed zero-valued case.
pub(crate) struct Fixture {
    /// Store under test.
    pub(crate) cas: Cas,
    /// Semantic signature.
    pub(crate) header: Handle,
    /// Root context.
    pub(crate) context: Handle,
    /// Whole real axis.
    pub(crate) domain: Handle,
    /// Real coordinate.
    pub(crate) x: Handle,
    /// Exact zero.
    pub(crate) zero: Handle,
    /// Exact one.
    pub(crate) one: Handle,
    /// Native constant case.
    pub(crate) cases: Handle,
}

impl Fixture {
    /// Construct independently chosen leaves, association and ambient context.
    pub(crate) fn new() -> TestResult<Self> {
        let mut cas = Cas::new(budget()?.limits().clone())?;
        let (header, domain, context) = roots(&mut cas)?;
        let x = term(&mut cas, &header, Operator::Free("x".into()), &[])?;
        let zero = term(&mut cas, &header, Operator::Rational(Rational::zero()), &[])?;
        let one = term(&mut cas, &header, Operator::Rational(Rational::one()), &[])?;
        let arm = admit(
            &mut cas,
            Draft::CaseArm(CaseArmDraft {
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                region: Reference::Existing(domain.clone()),
                value: Reference::Existing(zero.clone()),
            }),
        )?;
        let cases = admit(
            &mut cas,
            Draft::Cases(CasesDraft {
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                domain: Reference::Existing(domain.clone()),
                arms: vec![Reference::Existing(arm)],
            }),
        )?;
        Ok(Self {
            cas,
            header,
            context,
            domain,
            x,
            zero,
            one,
            cases,
        })
    }
}

/// Admit one untrusted node, with no unchecked fixture indexing.
pub(crate) fn admit(cas: &mut Cas, draft: Draft) -> TestResult<Handle> {
    cas.admit(&[draft], &mut budget()?)?
        .first()
        .cloned()
        .ok_or_else(|| "missing fixture node".into())
}

/// Exact ordered scalar operands.
pub(crate) fn term(
    cas: &mut Cas,
    header: &Handle,
    operator: Operator,
    operands: &[Handle],
) -> TestResult<Handle> {
    admit(
        cas,
        Draft::Expression(ExpressionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            sort: real(),
            operator,
            binding: None,
            operands: operands.iter().cloned().map(Reference::Existing).collect(),
        }),
    )
}

/// Typed guard arguments.
pub(crate) fn predicate(
    cas: &mut Cas,
    header: &Handle,
    kind: PredicateKind,
    arguments: &[Handle],
) -> TestResult<Handle> {
    admit(
        cas,
        Draft::Condition {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            kind,
            arguments: arguments.iter().cloned().map(Reference::Existing).collect(),
        },
    )
}

/// Build an explicit real-axis header, domain and root context without mathematical production.
fn roots(cas: &mut Cas) -> TestResult<(Handle, Handle, Handle)> {
    let environment = Environment {
        declarations: vec![Declaration {
            key: "x".into(),
            display_name: "x".into(),
            sort: real(),
        }],
        ..Environment::default()
    };
    let header = admit(cas, Draft::Header(HeaderDraft::Core(environment)))?;
    let domain = admit(
        cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Real("x".into()),
            expression: RegionExpr::Ambient,
        }),
    )?;
    let context = admit(
        cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header.clone()),
            admissible: Reference::Existing(domain.clone()),
            assumptions: Vec::new(),
        }),
    )?;
    Ok((header, domain, context))
}

/// Build a native case and boundary sharing one explicit outer scalar binder frame.
pub(crate) fn scoped_cases(f: &mut Fixture) -> TestResult<(Handle, Handle)> {
    let scope = vec![vec![crate::model::AtomicSort::Scalar(
        crate::model::Field::Real,
    )]];
    let (bound, domain, predicate) = scoped_inputs(f, &scope)?;
    let region = admit(
        &mut f.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(f.header.clone()),
            scope: scope.clone(),
            ambient: AmbientDomain::Real("x".into()),
            expression: RegionExpr::GuardTrue {
                base: Reference::Existing(domain.clone()),
                predicate: Reference::Existing(predicate),
            },
        }),
    )?;
    let arm = admit(
        &mut f.cas,
        Draft::CaseArm(CaseArmDraft {
            header: Reference::Existing(f.header.clone()),
            scope: scope.clone(),
            region: Reference::Existing(region),
            value: Reference::Existing(bound.clone()),
        }),
    )?;
    let cases = admit(
        &mut f.cas,
        Draft::Cases(CasesDraft {
            header: Reference::Existing(f.header.clone()),
            scope: scope.clone(),
            domain: Reference::Existing(domain),
            arms: vec![Reference::Existing(arm)],
        }),
    )?;
    let boundary = admit(
        &mut f.cas,
        Draft::Boundary(crate::domain_model::BoundaryDraft {
            header: Reference::Existing(f.header.clone()),
            scope,
            ambient: AmbientDomain::Real("x".into()),
            location: Reference::Existing(bound),
        }),
    )?;
    Ok((cases, boundary))
}

/// The scalar, ambient region and predicate carry exactly the same explicit binder scope.
fn scoped_inputs(
    f: &mut Fixture,
    scope: &crate::model::Scope,
) -> TestResult<(Handle, Handle, Handle)> {
    let bound = admit(
        &mut f.cas,
        Draft::Expression(ExpressionDraft {
            header: Reference::Existing(f.header.clone()),
            scope: scope.clone(),
            sort: real(),
            operator: Operator::Bound { depth: 0, slot: 0 },
            binding: None,
            operands: Vec::new(),
        }),
    )?;
    let domain = admit(
        &mut f.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(f.header.clone()),
            scope: scope.clone(),
            ambient: AmbientDomain::Real("x".into()),
            expression: RegionExpr::Ambient,
        }),
    )?;
    let predicate = admit(
        &mut f.cas,
        Draft::Condition {
            header: Reference::Existing(f.header.clone()),
            scope: scope.clone(),
            kind: PredicateKind::NonZero,
            arguments: vec![Reference::Existing(bound.clone())],
        },
    )?;
    Ok((bound, domain, predicate))
}
