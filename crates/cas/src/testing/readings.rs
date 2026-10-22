// SPDX-License-Identifier: AGPL-3.0-only
//! Public reading fixtures, compiled only in the public test target.

use super::builders;
use super::common::TestResult;
use crate::test_api::limits::{Budget, Limits};
use crate::test_api::readings::{AffineMap, EntryIndex, EntryOrder, SelectionSpec};
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

/// One independently reconstructed expected reading equality.
pub(crate) struct ReadingCase {
    /// Complete original syntax and context, initially without proofs.
    pub(crate) fixture: Fixture,
    /// The derived Real reading or selected state component.
    pub(crate) left: Handle,
    /// The independently composed component of the whole reading.
    pub(crate) right: Handle,
}

impl ReadingCase {
    /// Construct the full expected claim without accepting an inspected witness's claim.
    pub(crate) fn expected(&self) -> TestResult<crate::test_api::proof::Judgment> {
        use crate::test_api::domain::Layer;
        use crate::test_api::proof::{DomainContract, Judgment, Statement};
        Ok(Judgment {
            context: self.fixture.context.clone(),
            layer: Layer::Pointwise,
            space: None,
            statement: Statement::Equality {
                left: self.left.clone(),
                right: self.right.clone(),
                domain: self.fixture.cas.admissible_region(&self.fixture.context)?,
                domain_contract: DomainContract::DefinedAndEqual,
            },
            residuals: Vec::new(),
        })
    }

    /// Use the public producer with the caller's one cumulative ledger.
    pub(crate) fn prove(
        &mut self,
        budget: &mut Budget,
    ) -> Result<crate::test_api::proof::VerifiedWitness, Fault> {
        self.fixture.cas.prove_reading_equality(
            &self.fixture.context,
            &self.left,
            &self.right,
            budget,
        )
    }
}

/// A single semantic mutation in the Complex reading fixture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ComplexChange {
    /// Original whole-family definition and selected corner.
    None,
    /// The other stored copy of the real coordinate.
    Copy,
    /// Negate the Real reading.
    Sign,
    /// Swap the first two selected block coordinates.
    Order,
    /// Change the original free-coordinate declaration's extent.
    Extent,
    /// Change the derived symbol's original declaration key.
    Declaration,
    /// Omit the whole-family expansion and leave four unrelated coordinates.
    Unstructured,
    /// Omit the Complex reading's defining equation.
    MissingDefinition,
}

/// The complete block expansion and both exact readings are independently reconstructed.
pub(crate) fn complex_case(change: ComplexChange) -> TestResult<ReadingCase> {
    let real_name = if change == ComplexChange::Declaration {
        "corner"
    } else {
        "r"
    };
    let extent = if change == ComplexChange::Extent {
        3
    } else {
        2
    };
    let complex = Sort::Value(AtomicSort::Scalar(Field::Complex));
    let mut fixture = Fixture::new(&[
        ("z", matrix(extent, 1)),
        ("block", matrix(2, 2)),
        ("c", complex),
        (real_name, super::common::real()),
    ])?;
    let block = complex_block(&mut fixture, change)?;
    let value = complex_value(&mut fixture, &block, change)?;
    let corner = u32::from(matches!(
        change,
        ComplexChange::Copy | ComplexChange::Unstructured
    ));
    let selected = fixture.cas.select_entries(
        &block,
        &SelectionSpec::Entry(EntryIndex {
            row: corner,
            column: corner,
        }),
        &mut ledger()?,
    )?;
    let symbol = fixture.symbol(real_name)?;
    let sign = if change == ComplexChange::Sign { -1 } else { 1 };
    let left = fixture.cas.define_reading(
        &symbol,
        &selected,
        &map(1, 1, &[sign], &[0])?,
        &mut ledger()?,
    )?;
    let right = fixture.term(
        Operator::RealPart,
        super::common::real(),
        std::slice::from_ref(&value),
    )?;
    Ok(ReadingCase {
        fixture,
        left,
        right,
    })
}

/// Explicit expansion retains both repeated `a` entries and the opposite `b` signs.
fn complex_block(fixture: &mut Fixture, change: ComplexChange) -> TestResult<Handle> {
    let symbol = fixture.symbol("block")?;
    if change == ComplexChange::Unstructured {
        return Ok(symbol);
    }
    let source = fixture.symbol("z")?;
    let expansion = if change == ComplexChange::Extent {
        map(3, 4, &[1, 0, 0, 0, 1, 0, 0, -1, 0, 1, 0, 0], &[0, 0, 0, 0])?
    } else {
        map(2, 4, &[1, 0, 0, 1, 0, -1, 1, 0], &[0, 0, 0, 0])?
    };
    Ok(fixture
        .cas
        .define_reading(&symbol, &source, &expansion, &mut ledger()?)?)
}

/// The Complex reading retains the complete ordered block, even its unused coordinates.
fn complex_value(
    fixture: &mut Fixture,
    block: &Handle,
    change: ComplexChange,
) -> TestResult<Handle> {
    let symbol = fixture.symbol("c")?;
    if change == ComplexChange::MissingDefinition {
        return Ok(symbol);
    }
    let spec = if change == ComplexChange::Order {
        SelectionSpec::Entries {
            entries: vec![
                EntryIndex { row: 1, column: 0 },
                EntryIndex { row: 0, column: 0 },
                EntryIndex { row: 0, column: 1 },
                EntryIndex { row: 1, column: 1 },
            ],
            order: EntryOrder::Given,
        }
    } else {
        SelectionSpec::Block {
            row: 0,
            column: 0,
            rows: 2,
            columns: 2,
        }
    };
    let selected = fixture.cas.select_entries(block, &spec, &mut ledger()?)?;
    Ok(fixture.cas.define_reading(
        &symbol,
        &selected,
        &map(4, 2, &[1, 0, 0, 0, 0, 1, 0, 0], &[0, 0])?,
        &mut ledger()?,
    )?)
}

/// A state and its independently defined component share one ordered two-coordinate family.
pub(crate) fn state_case(component: u32) -> TestResult<ReadingCase> {
    let mut fixture = Fixture::new(&[
        ("z", matrix(2, 1)),
        ("x", matrix(2, 1)),
        ("v", super::common::real()),
        ("i", super::common::real()),
    ])?;
    let source = fixture.symbol("z")?;
    let state = fixture.symbol("x")?;
    let selected =
        fixture
            .cas
            .select_entries(&source, &SelectionSpec::Column(0), &mut ledger()?)?;
    let whole = fixture.cas.define_reading(
        &state,
        &selected,
        &AffineMap::identity(2, &mut ledger()?)?,
        &mut ledger()?,
    )?;
    let spec = SelectionSpec::Entry(EntryIndex {
        row: component,
        column: 0,
    });
    let selected = fixture.cas.select_entries(&source, &spec, &mut ledger()?)?;
    let symbol = fixture.symbol(if component == 0 { "v" } else { "i" })?;
    let left = fixture.cas.define_reading(
        &symbol,
        &selected,
        &AffineMap::identity(1, &mut ledger()?)?,
        &mut ledger()?,
    )?;
    let right = fixture.cas.select_entries(&whole, &spec, &mut ledger()?)?;
    Ok(ReadingCase {
        fixture,
        left,
        right,
    })
}
