// SPDX-License-Identifier: AGPL-3.0-only
//! Frozen physical originals provide closed exact targets without losing source or map identity.

use super::{DecisionError, storage};
use crate::{CheckedDeclarations, Kind, Role};
use nrz_cas::{ExactConstant, Handle, limits::Budget};
use std::rc::Rc;

/// A closed exact ratio expression with its original physical declaration owner.
///
/// This subset covers exact nominal declarations. Free input families and numerical
/// enclosure origins require their separate correspondence and authenticity contracts.
#[derive(Clone, Debug)]
pub struct DecisionExpression(Rc<Data>);

/// Shared frozen source and its independently checked exact coherent coordinates.
#[derive(Debug)]
struct Data {
    /// Complete source, unit maps, original operations and semantic revision.
    owner: Rc<CheckedDeclarations>,
    /// Exact declaration key within the retained owner.
    name: String,
    /// Original occurrence identity, before coherent-coordinate normalization.
    original: Handle,
    /// Independently checked source-free exact coherent value.
    exact: ExactConstant,
}

impl DecisionExpression {
    /// Bind an already checked declared ratio to its closed exact coherent expression.
    ///
    /// No source assumption is inferred from an equal numerical center. The frozen
    /// owner retains original physical checks and every declaration and unit map.
    /// # Errors
    /// Refuses non-ratios, metadata, open expressions, undefined constants and caller limits.
    pub fn declared(
        owner: Rc<CheckedDeclarations>,
        name: &str,
        budget: &mut Budget,
    ) -> Result<Self, DecisionError> {
        let length = u64::try_from(name.len()).map_err(|_| DecisionError::Capacity)?;
        budget.work(length.saturating_add(4), storage::AT)?;
        let checked = owner.checked(name)?;
        if checked.descriptor().kind() != &Kind::Ratio
            || checked.descriptor().role() != Role::Difference
        {
            return Err(DecisionError::Target);
        }
        let original = checked.original().syntax.clone();
        let exact = {
            let graph = owner.cas()?;
            ExactConstant::from_expression(&graph, &checked.coordinates, budget)?
        };
        storage::retained(
            length
                .saturating_add(storage::constant_bytes(&exact))
                .saturating_add(256),
            budget,
        )?;
        Ok(Self(Rc::new(Data {
            owner,
            name: name.into(),
            original,
            exact,
        })))
    }

    /// Frozen physical declarations, including original source and exact unit maps.
    #[must_use]
    pub fn owner(&self) -> &CheckedDeclarations {
        &self.0.owner
    }
    /// Original declaration key within that exact owner.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.0.name
    }
    /// Original source syntax, never replaced by a selected value or center.
    #[must_use]
    pub fn original(&self) -> &Handle {
        &self.0.original
    }
    /// Exact coherent value admitted by the closed CAS constant checker.
    #[must_use]
    pub fn exact(&self) -> &ExactConstant {
        &self.0.exact
    }
}
