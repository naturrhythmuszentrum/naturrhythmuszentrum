// SPDX-License-Identifier: AGPL-3.0-only
//! Frozen physical originals provide closed exact targets without losing source or map identity.

use super::{DecisionError, family, storage};
use crate::{CheckedDeclarations, Kind, Role};
use nrz_cas::{ExactConstant, Handle, limits::Budget};
use std::{collections::BTreeMap, rc::Rc};

/// A checked ratio expression with its original physical declaration owner.
///
/// Closed declarations retain an exact value. Physical families instead retain their
/// explicit source boxes and qualified endpoint assumptions.
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
    exact: Option<Rc<ExactConstant>>,
    /// Explicit qualified source boxes for an open physical question.
    family: Option<family::Inputs>,
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
            exact: Some(Rc::new(exact)),
            family: None,
        })))
    }

    /// Declare a two-input physical quotient with explicit coherent source boxes and assumptions.
    ///
    /// Every varying declaration is matched to one original CAS source; its endpoint
    /// assumptions remain conditional. Constant leaves retain their declared exact maps.
    /// # Errors
    /// Refuses incomplete source inventories, unsupported originals, changed premises and limits.
    pub fn family(
        cas: &nrz_cas::Cas,
        owner: Rc<CheckedDeclarations>,
        name: &str,
        context: &Handle,
        inputs: Vec<(String, nrz_cas::decision::ScalarKernelFamily)>,
        budget: &mut Budget,
    ) -> Result<Self, DecisionError> {
        let family = family::Inputs::check(cas, &owner, name, context, inputs, budget)?;
        let original = owner.checked(name)?.original().syntax.clone();
        Ok(Self(Rc::new(Data {
            owner,
            name: name.into(),
            original,
            exact: None,
            family: Some(family),
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
    /// Exact coherent value for a closed declaration; an open family has no nominal substitute.
    #[must_use]
    pub fn exact(&self) -> Option<&ExactConstant> {
        self.0.exact.as_deref()
    }
    /// Complete declared physical source boxes; an empty list denotes a closed source.
    #[must_use]
    pub fn input_families(&self) -> &[(String, nrz_cas::decision::ScalarKernelFamily)] {
        self.0
            .family
            .as_ref()
            .map_or(&[], |family| family.inputs.as_slice())
    }
    /// Original scope of the explicitly retained source assumptions, when the source is open.
    #[must_use]
    pub fn family_context(&self) -> Option<&Handle> {
        self.0.family.as_ref().map(|family| &family.context)
    }
}

/// A continuous physical coordinate retaining its original kind, unit and affine role.
///
/// This distinct type cannot be passed as the scalar decision's ratio source.
#[derive(Clone, Debug)]
pub struct DecisionCoordinate {
    /// Original closed physical source and its immutable declaration owner.
    source: DecisionExpression,
    /// Shared closed exact meaning, guaranteed by the coordinate constructor.
    exact: Rc<ExactConstant>,
}

impl DecisionCoordinate {
    /// Reuse closed exact source checking for a continuous physical coordinate.
    /// # Errors
    /// Refuses counts, uncertainty kinds, open or undefined expressions and caller limits.
    pub fn declared(
        owner: Rc<CheckedDeclarations>,
        name: &str,
        budget: &mut Budget,
    ) -> Result<Self, DecisionError> {
        let source = DecisionExpression::closed(owner, name, false, budget)?;
        let exact = source.0.exact.as_ref().ok_or(DecisionError::Source)?;
        Ok(Self {
            exact: Rc::clone(exact),
            source,
        })
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
        self.source.checked()
    }

    /// Exact coherent value, without a dimensionless reinterpretation.
    #[must_use]
    pub fn exact(&self) -> &ExactConstant {
        &self.exact
    }

    /// Frozen original declarations and semantic revision.
    #[must_use]
    pub fn owner(&self) -> &CheckedDeclarations {
        self.source.owner()
    }

    /// Original declaration key within its retained owner.
    #[must_use]
    pub fn name(&self) -> &str {
        self.source.name()
    }

    /// Share input association internals without exposing a scalar ratio conversion.
    pub(super) const fn source(&self) -> &DecisionExpression {
        &self.source
    }
}
