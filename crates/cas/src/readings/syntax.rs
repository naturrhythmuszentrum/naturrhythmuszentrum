// SPDX-License-Identifier: AGPL-3.0-only
//! Typed definitions retain their original declared symbol and complete source expression.

use super::{AT, AffineMap, EntrySelection, SelectionSpec, invalid};
use crate::budget::Budget;
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Draft, Expression, ExpressionDraft, Handle, Reference};
use crate::model::{AtomicSort, Extent, Field, Operator, Sort};

/// Count Real coordinates without changing their declared component order.
pub(crate) fn coordinates(sort: &Sort) -> Result<u32, Fault> {
    match sort {
        Sort::Value(AtomicSort::Scalar(Field::Real)) => Ok(1),
        Sort::Value(AtomicSort::Scalar(Field::Complex)) => Ok(2),
        Sort::Value(AtomicSort::Matrix {
            field: Field::Real,
            rows: Extent::Literal(rows),
            columns: Extent::Literal(columns),
        }) => rows
            .checked_mul(*columns)
            .ok_or_else(|| invalid(InvalidCode::ReadingShape)),
        _ => Err(invalid(InvalidCode::ReadingShape)),
    }
}

/// Validate all new syntax before graph publication, including original operands under zero.
pub(crate) fn validate(
    operator: &Operator,
    result: &Sort,
    children: &[&Expression],
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(1, AT)?;
    match operator {
        Operator::Select(selection) => {
            let [source] = children else {
                return Err(invalid(InvalidCode::SelectionShape));
            };
            declared(source)?;
            selection.validate(&source.sort, budget)?;
            if selection.result_sort(&source.sort, budget)? != *result {
                return Err(invalid(InvalidCode::SelectionShape));
            }
        }
        Operator::AffineReading(map) => {
            let [declaration, source] = children else {
                return Err(invalid(InvalidCode::ReadingDefinition));
            };
            if !matches!(declaration.operator, Operator::Free(_)) || declaration.sort != *result {
                return Err(invalid(InvalidCode::ReadingDefinition));
            }
            declared(declaration)?;
            map.validate(budget)?;
            if map.inputs != coordinates(&source.sort)? || map.outputs != coordinates(result)? {
                return Err(invalid(InvalidCode::ReadingShape));
            }
        }
        _ => return Err(invalid(InvalidCode::ReadingDefinition)),
    }
    Ok(())
}

/// A selection names one closed declared symbol or its transparent reading definition.
const fn declared(value: &Expression) -> Result<(), Fault> {
    let named = matches!(
        value.operator,
        Operator::Free(_) | Operator::AffineReading(_)
    );
    if !named || !value.scope.is_empty() || value.space.is_some() {
        return Err(invalid(InvalidCode::ReadingDefinition));
    }
    Ok(())
}

impl Cas {
    /// Select entries of a closed declared matrix or a defined matrix reading.
    ///
    /// The returned expression retains its exact source and declaration snapshot.
    /// # Errors
    /// Refuses invalid selections, foreign handles, nonsymbol sources and exhausted resources.
    pub fn select_entries(
        &mut self,
        source: &Handle,
        spec: &SelectionSpec,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let value = self.state.expression(self.checked(source)?)?;
        declared(value)?;
        let selection = EntrySelection::new(&value.sort, spec, budget)?;
        let sort = selection.result_sort(&value.sort, budget)?;
        let header = self.handle(value.header);
        self.reading_expression(
            &header,
            Operator::Select(Box::new(selection)),
            sort,
            std::slice::from_ref(source),
            budget,
        )
    }

    /// Define a reading of an exact source under a declared result symbol.
    ///
    /// This creates the transparent expression `symbol := map(source)`. The bare
    /// `Free` symbol remains a declaration, not an independently asserted equation.
    /// Bindings must retain the returned definition as the symbol's reading identity.
    /// # Errors
    /// Refuses incompatible declarations, scopes, shapes, foreign sources and exhausted resources.
    pub fn define_reading(
        &mut self,
        symbol: &Handle,
        source: &Handle,
        map: &AffineMap,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let declaration = self.state.expression(self.checked(symbol)?)?;
        if !matches!(declaration.operator, Operator::Free(_)) {
            return Err(invalid(InvalidCode::ReadingDefinition));
        }
        declared(declaration)?;
        coordinates(&declaration.sort)?;
        map.validate(budget)?;
        let header = self.handle(declaration.header);
        let sort = declaration.sort.clone();
        self.reading_expression(
            &header,
            Operator::AffineReading(Box::new(map.clone())),
            sort,
            &[symbol.clone(), source.clone()],
            budget,
        )
    }

    /// Ordinary atomic admission is the sole publication path for both constructors.
    fn reading_expression(
        &mut self,
        header: &Handle,
        operator: Operator,
        sort: Sort,
        operands: &[Handle],
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let draft = Draft::Expression(ExpressionDraft {
            operator,
            sort,
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            binding: None,
            space: None,
            operands: operands.iter().cloned().map(Reference::Existing).collect(),
        });
        self.admit(&[draft], budget)?
            .pop()
            .ok_or_else(|| invalid(InvalidCode::ReadingDefinition))
    }
}
