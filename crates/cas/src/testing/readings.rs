// SPDX-License-Identifier: AGPL-3.0-only
//! Public reading fixtures, compiled only in the public test target.

use super::builders;
use super::common::TestResult;
use crate::test_api::limits::{Budget, Limits};
use crate::test_api::readings::AffineMap;
use crate::test_api::syntax::{
    AtomicSort, Declaration, Environment, Extent, Field, Operator, Sort,
};
use crate::test_api::{Cas, Fault, Handle, Rational};

/// A fresh unchanged production ledger for each independent setup operation.
pub(crate) fn ledger() -> Result<Budget, Fault> {
    Budget::new(Limits::default())
}

/// Mathematical Real rectangle in logical column coordinates.
pub(crate) const fn matrix(rows: u32, columns: u32) -> Sort {
    Sort::Value(AtomicSort::Matrix {
        field: Field::Real,
        rows: Extent::Literal(rows),
        columns: Extent::Literal(columns),
    })
}

/// Exact integer coefficients supplied independently of map arithmetic.
pub(crate) fn numbers(values: &[i64]) -> TestResult<Vec<Rational>> {
    values
        .iter()
        .map(|value| Ok(Rational::from_ratio(*value, 1, &mut ledger()?)?))
        .collect()
}

/// A small explicit map with separate rational setup and construction ledgers.
pub(crate) fn map(
    inputs: u32,
    outputs: u32,
    coefficients: &[i64],
    offsets: &[i64],
) -> TestResult<AffineMap> {
    Ok(AffineMap::new(
        inputs,
        outputs,
        numbers(coefficients)?,
        numbers(offsets)?,
        &mut ledger()?,
    )?)
}

/// Independently reconstructed declaration snapshot and original symbols.
pub(crate) struct Fixture {
    /// Independent graph with no checked proof at setup.
    pub(crate) cas: Cas,
    /// Exact source declaration and domain context.
    pub(crate) context: Handle,
    /// Original symbol keys and their admitted identities.
    pub(crate) symbols: Vec<(String, Handle)>,
}

impl Fixture {
    /// Keep setup free of proof records and numerical payloads.
    pub(crate) fn new(declarations: &[(&str, Sort)]) -> TestResult<Self> {
        let mut cas = Cas::new(Limits::default())?;
        let environment = Environment {
            declarations: declarations
                .iter()
                .map(|(key, sort)| Declaration {
                    key: (*key).into(),
                    display_name: (*key).into(),
                    sort: sort.clone(),
                })
                .collect(),
            ..Environment::default()
        };
        let context = builders::core_context(&mut cas, environment, Vec::new(), &mut ledger()?)?;
        let mut symbols = Vec::new();
        for (key, sort) in declarations {
            let value = builders::expression(
                &mut cas,
                &context,
                Operator::Free((*key).into()),
                sort.clone(),
                &[],
                &mut ledger()?,
            )?;
            symbols.push(((*key).into(), value));
        }
        Ok(Self {
            cas,
            context,
            symbols,
        })
    }

    /// Resolve only the explicitly declared original symbol.
    pub(crate) fn symbol(&self, key: &str) -> TestResult<Handle> {
        self.symbols
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
            .ok_or_else(|| "missing fixture symbol".into())
    }

    /// Build a complete original operator under the exact fixture header.
    pub(crate) fn term(
        &mut self,
        operator: Operator,
        sort: Sort,
        operands: &[Handle],
    ) -> TestResult<Handle> {
        builders::expression(
            &mut self.cas,
            &self.context,
            operator,
            sort,
            operands,
            &mut ledger()?,
        )
    }
}
