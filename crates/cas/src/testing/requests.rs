// SPDX-License-Identifier: AGPL-3.0-only
//! Declared real coordinates and covered generated-operation requests.
//!
//! Compiled only in the public test target.

use crate::test_api::{Cas, Handle};

use crate::test_api::request::{Request, Task};
use crate::test_api::syntax::{Field, Operator};

use super::budgets::derivation_budget as budget;
use super::builders::{self};
use super::common::{TestResult, real};

/// A minimal independent request builder, with a complete generated occurrence walk.
#[derive(Debug)]
pub(crate) struct Fixture {
    /// Store under test.
    pub(crate) cas: Cas,
    /// Shared mathematical context.
    pub(crate) context: Handle,
    /// Explicit domain of the fixture context.
    pub(crate) domain: Handle,
}

impl Fixture {
    /// Declare only the named real coordinates.
    pub(crate) fn new(keys: &[&str]) -> TestResult<Self> {
        let mut cas = Cas::new(budget()?.limits().clone())?;
        let context = builders::core_context(
            &mut cas,
            builders::environment(Field::Real, keys),
            Vec::new(),
            &mut budget()?,
        )?;
        let domain = cas.admissible_region(&context)?;
        Ok(Self {
            cas,
            context,
            domain,
        })
    }

    /// Retain the caller's exact ordered expression spelling.
    pub(crate) fn term(&mut self, operator: Operator, operands: &[Handle]) -> TestResult<Handle> {
        builders::expression(
            &mut self.cas,
            &self.context,
            operator,
            real(),
            operands,
            &mut budget()?,
        )
    }

    /// Exact scalar literal setup.
    pub(crate) fn number(&mut self, text: &str) -> TestResult<Handle> {
        builders::number(&mut self.cas, &self.context, real(), text, budget)
    }

    /// Distinct declared free identity, unaffected by display names.
    pub(crate) fn variable(&mut self, key: &str) -> TestResult<Handle> {
        self.term(Operator::Free(key.into()), &[])
    }

    /// Each request receives all its occurrence receipts before execution.
    pub(crate) fn request(&mut self, task: Task, expressions: &[Handle]) -> TestResult<Request> {
        builders::request(&mut self.cas, &self.context, task, expressions)
    }
}
