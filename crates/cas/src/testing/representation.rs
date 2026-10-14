// SPDX-License-Identifier: AGPL-3.0-only
//! Typed scalar, binder and matrix representation setup.
//!
//! Compiled only in the public test target.

use crate::test_api::{
    AtomicSort, Binding, Cas, Declaration, Draft, Environment, Field, Handle, Limits, Operator,
    Rational, Reference, Scope, Sort,
};

use super::budgets::graph_budget as budget;
use super::builders;
use super::common::{TestResult, real};

/// A local constructor with the complete semantic context kept explicit.
#[derive(Debug)]
pub(crate) struct Fixture {
    /// Store with default permanent ceilings.
    pub(crate) cas: Cas,
    /// Immutable declaration context.
    pub(crate) context: Handle,
}

impl Fixture {
    /// Declare one scalar and one unary function without assigning numerical behavior.
    pub(crate) fn new() -> TestResult<Self> {
        let mut cas = Cas::new(Limits::default())?;
        let environment = Environment {
            declarations: vec![
                Declaration {
                    key: "x".into(),
                    display_name: "x".into(),
                    sort: real(),
                },
                Declaration {
                    key: "f".into(),
                    display_name: "f".into(),
                    sort: Sort::Function {
                        inputs: vec![AtomicSort::Scalar(Field::Real)],
                        output: AtomicSort::Scalar(Field::Real),
                    },
                },
            ],
            ..Environment::default()
        };
        let context = builders::admit(
            &mut cas,
            Draft::Context {
                environment,
                assumptions: Vec::new(),
            },
            &mut budget()?,
        )?;
        Ok(Self { cas, context })
    }

    /// Construct a scoped or ordinary expression without a parser.
    pub(crate) fn node(
        &mut self,
        operator: Operator,
        sort: Sort,
        scope: Scope,
        binding: Option<Binding>,
        operands: &[Handle],
    ) -> TestResult<Handle> {
        builders::admit(
            &mut self.cas,
            builders::expression_draft(
                operator,
                sort,
                Reference::Existing(self.context.clone()),
                scope,
                binding,
                operands,
            ),
            &mut budget()?,
        )
    }

    /// Construct a real scalar constant in a specified scope.
    pub(crate) fn one(&mut self, scope: Scope) -> TestResult<Handle> {
        self.node(
            Operator::Rational(Rational::one()),
            real(),
            scope,
            None,
            &[],
        )
    }
}
