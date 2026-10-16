// SPDX-License-Identifier: AGPL-3.0-only
//! Visible polynomial expressions and exact binder lifting for the scoped Vieta derivation.

use super::construction::admit;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Draft, ExpressionDraft, Handle, Reference, Vertex};
use crate::model::{AtomicSort, Field, Operator, Scope, Sort};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::forms::Variable;
use crate::solving::coefficients::proof::fault;
use crate::solving::coefficients::storage::{copied, invalid, reserve, vector};
use crate::solving::symmetric::algebra::Polynomial;
use crate::solving::towers::materialize;
use crate::solving::{CoefficientMapData, MapTarget, SolveDraft};

/// The literal map supplies the exact scope of every displayed coefficient.
pub(crate) struct Rendering {
    /// Original scalar declaration header.
    pub(crate) header: Handle,
    /// Exact scalar interpretation with its region and localization.
    pub(crate) map: Handle,
    /// Complete formal root frame, or the original empty scope.
    pub(crate) scope: Scope,
}

impl Rendering {
    /// An explicitly typed operation retains the complete formal root frame.
    pub(crate) fn operation(
        &self,
        cas: &mut Cas,
        operator: Operator,
        operands: &[Handle],
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let mut arguments = vector(operands.len(), budget)?;
        arguments.extend(operands.iter().cloned().map(Reference::Existing));
        admit(
            cas,
            &Draft::Expression(ExpressionDraft {
                header: Reference::Existing(self.header.clone()),
                scope: self.scope.clone(),
                operator,
                sort: Sort::Value(AtomicSort::Scalar(Field::Complex)),
                operands: arguments,
                space: None,
                binding: None,
            }),
            budget,
        )
    }

    /// Coefficient literals are scoped through the explicit map, never invented free root names.
    fn coefficient(
        &self,
        cas: &mut Cas,
        value: &crate::solving::coefficients::model::Element,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let value = materialize::value(cas, value, budget)?;
        admit(
            cas,
            &Draft::ExactCoefficient {
                header: Reference::Existing(self.header.clone()),
                value: Reference::Existing(value),
                map: Reference::Existing(self.map.clone()),
            },
            budget,
        )
    }

    /// Keep the unreduced nesting when elementary expressions replace the formal polynomial slots.
    pub(crate) fn polynomial(
        &self,
        cas: &mut Cas,
        source: &Polynomial,
        images: &[Handle],
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        if source.variables.len() != images.len() {
            return Err(invalid());
        }
        let mut result = None;
        for term in &source.form.terms {
            let mut product = self.coefficient(cas, &term.coefficient, budget)?;
            for (variable, exponent) in &term.powers {
                let Variable::Slot(slot) = variable else {
                    return Err(invalid());
                };
                let image = images
                    .get(usize::try_from(*slot).map_err(|_| invalid())?)
                    .ok_or_else(invalid)?;
                let power = if *exponent == 1 {
                    image.clone()
                } else {
                    self.operation(
                        cas,
                        Operator::IntegerPower(i64::from(*exponent)),
                        std::slice::from_ref(image),
                        budget,
                    )?
                };
                product = self.operation(cas, Operator::Mul, &[product, power], budget)?;
            }
            result = Some(if let Some(previous) = result {
                self.operation(cas, Operator::Add, &[previous, product], budget)?
            } else {
                product
            });
        }
        if let Some(result) = result {
            return Ok(result);
        }
        let engine = Engine {
            state: &cas.state,
            domain: &source.domain,
        };
        let zero = engine
            .constant(false, budget)
            .map_err(|error| fault(&error))?;
        self.coefficient(cas, &zero, budget)
    }
}

/// Lift only ordinary parameter syntax; attached roots cannot be silently rebound.
pub(crate) fn lift(
    cas: &mut Cas,
    root: &Handle,
    scope: &Scope,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let mut pending = vector(1, budget)?;
    pending.push((cas.checked(root)?, false));
    let mut known = std::collections::BTreeMap::new();
    while let Some((id, expanded)) = pending.pop() {
        budget.work(
            crate::graph::size(known.len(), super::AT)?.max(1),
            super::AT,
        )?;
        if known.contains_key(&id) {
            continue;
        }
        let (_, original) = crate::descriptor::metadata(&cas.state, id)?;
        if !original.is_empty() {
            return Err(invalid());
        }
        let operator = match cas.state.node(id)? {
            Vertex::Expression(value) if value.attachment.is_none() && value.binding.is_none() => {
                Some(value.operator.clone())
            }
            Vertex::Condition(_) | Vertex::Region(_) => None,
            _ => return Err(invalid()),
        };
        let children = crate::descriptor::children(&cas.state, id, budget)?;
        if expanded {
            let draft = crate::substitution_draft::rebuild(
                cas,
                id,
                scope.clone(),
                operator,
                &mut |child| {
                    known
                        .get(&child)
                        .cloned()
                        .map(Reference::Existing)
                        .ok_or_else(invalid)
                },
            )?;
            known.insert(id, admit(cas, &draft, budget)?);
        } else {
            reserve(
                &mut pending,
                children.len().checked_add(1).ok_or_else(invalid)?,
                budget,
            )?;
            pending.push((id, true));
            pending.extend(children.into_iter().map(|(_, child)| (child, false)));
        }
    }
    known.remove(&root.logical_id()).ok_or_else(invalid)
}

/// Exact coefficient maps retain the original domain, ordered images and every denominator.
pub(crate) fn mapping(
    cas: &mut Cas,
    source: &Handle,
    region: &Handle,
    scope: &Scope,
    budget: &mut Budget,
) -> Result<Handle, Fault> {
    let SolveDraft::CoefficientMap(data) =
        crate::solving::graph::object(&cas.state, cas.checked(source)?)?
    else {
        return Err(invalid());
    };
    let MapTarget::ScalarAmbient {
        header,
        field: Field::Complex,
    } = &data.target
    else {
        return Err(invalid());
    };
    let (domain, header, mode) = (data.source.clone(), header.clone(), data.mode);
    let originals = copied(&data.images, budget)?;
    let mut denominators = vector(data.denominators.len(), budget)?;
    denominators.extend(data.denominators.iter().cloned().map(Reference::Existing));
    let mut images = vector(originals.len(), budget)?;
    for image in originals {
        images.push(Reference::Existing(lift(cas, &image, scope, budget)?));
    }
    materialize::admit(
        cas,
        SolveDraft::CoefficientMap(CoefficientMapData {
            source: Reference::Existing(domain),
            target: MapTarget::ScalarAmbient {
                header: Reference::Existing(header),
                field: Field::Complex,
            },
            mode,
            region: Reference::Existing(region.clone()),
            images,
            denominators,
        }),
        budget,
    )
}
