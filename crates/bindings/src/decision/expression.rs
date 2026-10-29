// SPDX-License-Identifier: AGPL-3.0-only
//! Frozen physical originals provide closed exact targets without losing source or map identity.

use super::{DecisionError, storage};
use crate::{CheckedDeclarations, Kind, Role};
use nrz_cas::{ExactConstant, Handle, limits::Budget};
use std::{collections::BTreeMap, rc::Rc};

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
        Self::closed(owner, name, true, budget)
    }

    /// Share closed-source checking while leaving the scalar route's charges unchanged.
    fn closed(
        owner: Rc<CheckedDeclarations>,
        name: &str,
        ratio: bool,
        budget: &mut Budget,
    ) -> Result<Self, DecisionError> {
        let length = u64::try_from(name.len()).map_err(|_| DecisionError::Capacity)?;
        budget.work(length.saturating_add(4), storage::AT)?;
        let checked = owner.checked(name)?;
        let descriptor = checked.descriptor();
        if ratio {
            if descriptor.kind() != &Kind::Ratio || descriptor.role() != Role::Difference {
                return Err(DecisionError::Target);
            }
        } else if !matches!(
            descriptor.kind(),
            Kind::Ratio | Kind::Angle | Kind::Named(..)
        ) {
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

    /// Original checked physical kind, unit map, role and dependency inventory.
    /// # Errors
    /// Refuses an unavailable declaration rather than manufacturing a descriptor.
    pub fn checked(&self) -> Result<&crate::CheckedExpression, DecisionError> {
        Ok(self.owner().checked(self.name())?)
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

/// A continuous physical coordinate retaining its original kind, unit and affine role.
///
/// This distinct type cannot be passed as the scalar decision's ratio source.
#[derive(Clone, Debug)]
pub struct DecisionCoordinate(DecisionExpression);

impl DecisionCoordinate {
    /// Reuse closed exact source checking for a continuous physical coordinate.
    /// # Errors
    /// Refuses counts, uncertainty kinds, open or undefined expressions and caller limits.
    pub fn declared(
        owner: Rc<CheckedDeclarations>,
        name: &str,
        budget: &mut Budget,
    ) -> Result<Self, DecisionError> {
        Ok(Self(DecisionExpression::closed(
            owner, name, false, budget,
        )?))
    }

    /// Prepare a batch within one operation, retaining one check per original source identity.
    ///
    /// Repeated requests share their checked constant and owner. Independent frozen
    /// sources never share an entry, even when their exact values are equal.
    /// # Errors
    /// Refuses the same inputs as `Self::declared` and exhausted cumulative resources.
    /// A failed batch returns no coordinates; all earlier Work remains spent.
    pub fn declared_many(
        sources: &[(Rc<CheckedDeclarations>, &str)],
        budget: &mut Budget,
    ) -> Result<Vec<Self>, DecisionError> {
        let bytes = u64::try_from(sources.len())
            .map_err(|_| DecisionError::Capacity)?
            .saturating_mul(256);
        storage::retained(bytes, budget)?;
        let mut cache = BTreeMap::new();
        let mut retained = 0_u64;
        let mut result = Vec::new();
        result
            .try_reserve_exact(sources.len())
            .map_err(|_| DecisionError::Capacity)?;
        budget.with_retained_payload(bytes, storage::AT, |budget| {
            for (owner, name) in sources {
                let original = &owner.checked(name)?.original().syntax;
                let key = (Rc::as_ptr(owner).addr(), original.logical_id(), *name);
                let value = if let Some(value) = cache.get(&key) {
                    budget.work(1, storage::AT)?;
                    Self::clone(value)
                } else {
                    let value = budget.with_retained_payload(retained, storage::AT, |budget| {
                        Self::declared(Rc::clone(owner), name, budget)
                    })?;
                    retained = retained
                        .saturating_add(storage::constant_bytes(value.exact()))
                        .saturating_add(
                            u64::try_from(name.len()).map_err(|_| DecisionError::Capacity)?,
                        )
                        .saturating_add(256);
                    storage::retained(retained, budget)?;
                    cache.insert(key, value.clone());
                    value
                };
                result.push(value);
            }
            Ok::<_, DecisionError>(())
        })?;
        Ok(result)
    }

    /// Original checked kind, unit map, affine role and dependency inventory.
    /// # Errors
    /// Refuses an unavailable declaration rather than manufacturing a descriptor.
    pub fn checked(&self) -> Result<&crate::CheckedExpression, DecisionError> {
        self.0.checked()
    }

    /// Exact coherent value, without a dimensionless reinterpretation.
    #[must_use]
    pub fn exact(&self) -> &ExactConstant {
        self.0.exact()
    }

    /// Frozen original declarations and semantic revision.
    #[must_use]
    pub fn owner(&self) -> &CheckedDeclarations {
        self.0.owner()
    }

    /// Original declaration key within its retained owner.
    #[must_use]
    pub fn name(&self) -> &str {
        self.0.name()
    }

    /// Share input association internals without exposing a scalar ratio conversion.
    pub(super) const fn source(&self) -> &DecisionExpression {
        &self.0
    }
}
