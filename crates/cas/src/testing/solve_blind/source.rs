// SPDX-License-Identifier: AGPL-3.0-only
//! Original source syntax only; no candidate roots, formula identities or expected answers.

use crate::test_api::domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
use crate::test_api::limits::{Budget, Limits};
use crate::test_api::solving::{
    CoefficientDomainData, Coordinate, CoordinateRole, CoordinateSpaceData, SolveDraft,
    SolveProblemData,
};
use crate::test_api::syntax::{
    AtomicSort, Branch, Declaration, Draft, Environment, ExpressionDraft, Field, Operator,
    PredicateKind, Sort,
};
use crate::test_api::{Cas, Handle, Rational, Reference};
use crate::testing::common::TestResult;

/// Explicit graph setup for public theorem and replay tests.
pub(crate) struct Fixture {
    /// Owning graph store.
    pub(crate) cas: Cas,
    /// Fixed declaration header.
    pub(crate) header: Handle,
    /// Ordered coefficient product.
    pub(crate) coefficients: Handle,
    /// Coefficient region.
    pub(crate) region: Handle,
    /// Root context.
    pub(crate) context: Handle,
    /// Scalar ambient of this fixture.
    pub(crate) field: Field,
    /// Rational coefficient algebra for formal identities.
    pub(crate) algebra: Handle,
}

/// Setup permits a bounded degree-five tower; ordinary proofs retain their own smaller budgets.
pub(crate) fn admit(cas: &mut Cas, draft: &Draft) -> TestResult<Handle> {
    cas.admit(
        std::slice::from_ref(draft),
        &mut Budget::new(Limits {
            tower_degree: 32,
            ..Limits::blind_cubic()
        })?,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "missing admission".into())
}

impl Fixture {
    /// Construct an explicit declaration frame with no root or formula data.
    pub(crate) fn with_parameters(
        field: &Field,
        keys: &[&str],
        limits: Limits,
    ) -> TestResult<Self> {
        let mut cas = Cas::new(limits)?;
        let declarations = keys
            .iter()
            .copied()
            .chain(std::iter::once("x"))
            .map(|key| Declaration {
                key: key.into(),
                display_name: key.into(),
                sort: Sort::Value(AtomicSort::Scalar(field.clone())),
            })
            .collect();
        let header = admit(
            &mut cas,
            &Draft::Header(HeaderDraft::Core(Environment {
                field: field.clone(),
                branch: if *field == Field::Complex {
                    Branch::PrincipalComplex
                } else {
                    Branch::StrictReal
                },
                declarations,
            })),
        )?;
        let coefficients =
            coordinate_product(&mut cas, &header, keys, CoordinateRole::Coefficient, field)?;
        let region = product(&mut cas, &header, &coefficients, Vec::new())?;
        let context = admit(
            &mut cas,
            &Draft::Context(ContextDraft::Root {
                header: Reference::Existing(header.clone()),
                admissible: Reference::Existing(region.clone()),
                assumptions: Vec::new(),
            }),
        )?;
        let algebra = admit(
            &mut cas,
            &Draft::Solve(SolveDraft::CoefficientDomain(
                CoefficientDomainData::Rational,
            )),
        )?;
        Ok(Self {
            cas,
            header,
            coefficients,
            region,
            context,
            algebra,
            field: field.clone(),
        })
    }

    /// Build scalar syntax with an explicit binder scope and ordered children.
    pub(crate) fn expression(
        &mut self,
        operator: Operator,
        children: &[Handle],
        bound: bool,
    ) -> TestResult<Handle> {
        admit(
            &mut self.cas,
            &Draft::Expression(ExpressionDraft {
                header: Reference::Existing(self.header.clone()),
                scope: if bound {
                    vec![vec![AtomicSort::Scalar(self.field.clone())]]
                } else {
                    Vec::new()
                },
                operator,
                sort: Sort::Value(AtomicSort::Scalar(self.field.clone())),
                operands: children.iter().cloned().map(Reference::Existing).collect(),
                space: None,
                binding: None,
            }),
        )
    }

    /// Exact small integer with no external arithmetic dependency.
    pub(crate) fn integer(&mut self, value: i64, bound: bool) -> TestResult<Handle> {
        let value = Rational::from_ratio(value, 1, &mut Budget::new(Limits::blind_cubic())?)?;
        self.expression(Operator::Rational(value), &[], bound)
    }

    /// One original scalar declaration.
    pub(crate) fn free(&mut self, key: &str) -> TestResult<Handle> {
        self.expression(Operator::Free(key.into()), &[], false)
    }

    /// An original equation is retained as a condition, separate from its eventual solutions.
    pub(crate) fn equation(&mut self, left: &Handle, right: &Handle) -> TestResult<Handle> {
        admit(
            &mut self.cas,
            &Draft::Condition {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                kind: PredicateKind::Equal,
                arguments: vec![
                    Reference::Existing(left.clone()),
                    Reference::Existing(right.clone()),
                ],
            },
        )
    }

    /// Construct one original univariate problem without discarding any denominator.
    pub(crate) fn problem(
        &mut self,
        key: &str,
        predicate: &Handle,
        denominators: &[Handle],
    ) -> TestResult<Handle> {
        let unknowns = coordinate_product(
            &mut self.cas,
            &self.header,
            &[key],
            CoordinateRole::Unknown,
            &self.field,
        )?;
        let domain = product(&mut self.cas, &self.header, &unknowns, Vec::new())?;
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::SolveProblem(SolveProblemData {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                predicate: Reference::Existing(predicate.clone()),
                coefficients: Reference::Existing(self.coefficients.clone()),
                unknowns: Reference::Existing(unknowns),
                region: Reference::Existing(self.region.clone()),
                domain: Reference::Existing(domain),
                coefficient_domain: Reference::Existing(self.algebra.clone()),
                ambient: self.field.clone(),
                denominators: denominators
                    .iter()
                    .cloned()
                    .map(Reference::Existing)
                    .collect(),
            })),
        )
    }
}

/// Ordered products are graph objects; roles never come from display labels.
fn coordinate_product(
    cas: &mut Cas,
    header: &Handle,
    keys: &[&str],
    role: CoordinateRole,
    field: &Field,
) -> TestResult<Handle> {
    admit(
        cas,
        &Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
            scalar_spaces: Vec::new(),
            header: Reference::Existing(header.clone()),
            coordinates: keys
                .iter()
                .map(|key| Coordinate {
                    key: (*key).into(),
                    field: field.clone(),
                    role,
                })
                .collect(),
        })),
    )
}

/// Complete products have explicit scope, including a family binder when requested.
fn product(
    cas: &mut Cas,
    header: &Handle,
    coordinates: &Handle,
    scope: Vec<Vec<AtomicSort>>,
) -> TestResult<Handle> {
    admit(
        cas,
        &Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope,
            ambient: AmbientDomain::Product {
                coordinates: coordinates.clone(),
            },
            expression: RegionExpr::Ambient,
        }),
    )
}
