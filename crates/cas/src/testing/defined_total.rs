// SPDX-License-Identifier: AGPL-3.0-only
//! Independent syntax and exact claims for structural definedness.

use super::common::{TestResult, default_budget, real};
use super::series_residual::admit;
use crate::test_api::domain::{
    AmbientDomain, ContextDraft, HeaderDraft, Layer, RegionDraft, RegionExpr,
};
use crate::test_api::limits::Limits;
use crate::test_api::proof::{ApplicationDraft, Certificate, Judgment, Primitive, Statement};
use crate::test_api::solving::{Coordinate, CoordinateRole, CoordinateSpaceData, SolveDraft};
use crate::test_api::syntax::{
    AtomicSort, Branch, Declaration, Draft, Environment, ExpressionDraft, Field, Operator,
    PredicateKind, Scope, Sort,
};
use crate::test_api::{Cas, Handle, Rational, Reference};

pub(crate) struct Fixture {
    pub(crate) cas: Cas,
    pub(crate) header: Handle,
    pub(crate) region: Handle,
    pub(crate) context: Handle,
    pub(crate) scope: Scope,
}

impl Fixture {
    pub(crate) fn new(
        branch: Branch,
        scoped: bool,
        ambient: u8,
        reordered: bool,
    ) -> TestResult<Self> {
        let mut cas = Cas::new(Limits::default())?;
        let header = admit(
            &mut cas,
            Draft::Header(HeaderDraft::Core(Environment {
                field: Field::Real,
                branch,
                declarations: vec![
                    Declaration {
                        key: "x".into(),
                        display_name: "same".into(),
                        sort: real(),
                    },
                    Declaration {
                        key: "y".into(),
                        display_name: "same".into(),
                        sort: real(),
                    },
                    Declaration {
                        key: "i".into(),
                        display_name: "i".into(),
                        sort: Sort::Value(AtomicSort::Integer),
                    },
                    Declaration {
                        key: "opaque".into(),
                        display_name: "opaque".into(),
                        sort: Sort::Function {
                            inputs: vec![AtomicSort::Scalar(Field::Real)],
                            output: AtomicSort::Scalar(Field::Real),
                        },
                    },
                ],
            })),
        )?;
        let scope = if scoped {
            vec![vec![AtomicSort::Scalar(Field::Real)]]
        } else {
            Vec::new()
        };
        if reordered {
            admit(
                &mut cas,
                Draft::Expression(ExpressionDraft {
                    header: Reference::Existing(header.clone()),
                    space: None,
                    scope: scope.clone(),
                    binding: None,
                    sort: real(),
                    operator: Operator::Rational(Rational::one()),
                    operands: Vec::new(),
                }),
            )?;
        }
        let ambient = match ambient {
            0 => AmbientDomain::Assignments,
            1 => AmbientDomain::Real("x".into()),
            2 => AmbientDomain::Integer("i".into()),
            3 => AmbientDomain::Opaque("opaque".into()),
            _ => AmbientDomain::Product {
                coordinates: admit(
                    &mut cas,
                    Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
                        header: Reference::Existing(header.clone()),
                        scalar_spaces: Vec::new(),
                        coordinates: vec![Coordinate {
                            key: "x".into(),
                            field: Field::Real,
                            role: CoordinateRole::Coefficient,
                        }],
                    })),
                )?,
            },
        };
        let region = admit(
            &mut cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(header.clone()),
                scope: scope.clone(),
                ambient,
                expression: RegionExpr::Ambient,
            }),
        )?;
        let context = admit(
            &mut cas,
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(header.clone()),
                admissible: Reference::Existing(region.clone()),
                assumptions: Vec::new(),
            }),
        )?;
        Ok(Self {
            cas,
            header,
            region,
            context,
            scope,
        })
    }

    pub(crate) fn node(
        &mut self,
        operator: Operator,
        sort: Sort,
        operands: &[Handle],
    ) -> TestResult<Handle> {
        admit(
            &mut self.cas,
            Draft::Expression(ExpressionDraft {
                header: Reference::Existing(self.header.clone()),
                space: None,
                scope: self.scope.clone(),
                binding: None,
                sort,
                operator,
                operands: operands.iter().cloned().map(Reference::Existing).collect(),
            }),
        )
    }

    pub(crate) fn term(&mut self, operator: Operator, operands: &[Handle]) -> TestResult<Handle> {
        self.node(operator, real(), operands)
    }

    pub(crate) fn polynomial(&mut self) -> TestResult<Handle> {
        let x = self.term(
            if self.scope.is_empty() {
                Operator::Free("x".into())
            } else {
                Operator::Bound { depth: 0, slot: 0 }
            },
            &[],
        )?;
        let a = self.term(
            Operator::Rational(Rational::parse("1/3", &mut default_budget()?)?),
            &[],
        )?;
        let b = self.term(
            Operator::Rational(Rational::parse("1/45", &mut default_budget()?)?),
            &[],
        )?;
        let linear = self.term(Operator::Mul, &[a, x.clone()])?;
        let cube = self.term(Operator::IntegerPower(3), &[x])?;
        let cubic = self.term(Operator::Mul, &[b, cube])?;
        self.term(Operator::Sub, &[linear, cubic])
    }

    pub(crate) fn expected(&mut self, function: &Handle) -> TestResult<Judgment> {
        let predicate = admit(
            &mut self.cas,
            Draft::Condition {
                header: Reference::Existing(self.header.clone()),
                scope: self.scope.clone(),
                kind: PredicateKind::Defined,
                arguments: vec![Reference::Existing(function.clone())],
            },
        )?;
        Ok(Judgment {
            context: self.context.clone(),
            layer: Layer::Logic,
            space: None,
            statement: Statement::PredicateFact {
                region: self.region.clone(),
                predicate,
            },
            residuals: Vec::new(),
        })
    }

    pub(crate) fn draft(&mut self, function: &Handle) -> TestResult<ApplicationDraft> {
        Ok(ApplicationDraft {
            rule: Primitive::DefinedTotal,
            context: self.context.clone(),
            conclusion: self.expected(function)?.statement,
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::None,
        })
    }
}
