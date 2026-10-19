// SPDX-License-Identifier: AGPL-3.0-only
//! Checked operator ambients and direct registered placement requirements.

use super::SpacePlacementData;
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, Vertex, size};
use crate::model::{Field, Operator};
use crate::solving::{SolveDraft, SolveReason};

/// One original operand and its exact required directed definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmbientPlacement {
    /// Unchanged original operand.
    pub operand: Handle,
    /// Closed registered definition, ready for ordinary structural admission.
    pub definition: SpacePlacementData<Handle>,
}

/// A checked result or a specific unresolved choice, without constructing a scalar coercion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AmbientResolution {
    /// The signature and all direct operand placements agree.
    Resolved {
        /// Exact registered result space.
        ambient: Handle,
        /// Required operand associations in original operand order.
        placements: Vec<AmbientPlacement>,
    },
    /// No implicit path or preferred ambient is invented.
    Unresolved(SolveReason),
}

/// Exact accounting and refusal site.
const AT: Location = Location::new("registered operator ambient");

impl Cas {
    /// Resolve an operator from original operand spaces and an optional explicit result space.
    ///
    /// # Errors
    /// Refuses foreign references, non-scalar operands, unsupported signatures and exhausted limits.
    pub fn resolve_scalar_ambient(
        &self,
        header: &Handle,
        operator: &Operator,
        operands: &[Handle],
        requested: Option<&Handle>,
        budget: &mut Budget,
    ) -> Result<AmbientResolution, Fault> {
        self.state.header(self.checked(header)?)?;
        signature(operator, operands.len())?;
        let mut spaces = Vec::new();
        budget.check(
            BudgetKind::StagedPayloadBytes,
            size(operands.len(), AT)?.saturating_mul(256),
            AT,
        )?;
        spaces
            .try_reserve_exact(operands.len())
            .map_err(|_| Fault::allocation(AT))?;
        for operand in operands {
            budget.work(1, AT)?;
            spaces.push(self.operand_space(operand)?);
        }
        let explicit = requested
            .map(|space| {
                self.checked(space)?;
                super::field(&self.state, space)
            })
            .transpose()?;
        let Some(output) = choose(operator, &spaces, explicit.as_ref(), self)? else {
            return Ok(AmbientResolution::Unresolved(
                SolveReason::AmbientSpaceAmbiguous,
            ));
        };
        if explicit.as_ref().is_some_and(|field| *field != output) {
            return Ok(AmbientResolution::Unresolved(
                SolveReason::SpacePlacementUnavailable,
            ));
        }
        let ambient = requested
            .cloned()
            .map_or_else(|| self.registered(header, &output, budget), Ok)?;
        self.placements(header, operator, operands, &spaces, ambient, budget)
    }

    /// Read a scalar's checked node without retagging its original payload.
    fn operand_space(&self, operand: &Handle) -> Result<Handle, Fault> {
        match self.state.node(self.checked(operand)?)? {
            Vertex::Expression(value) => value
                .ambient
                .map(|id| self.handle(id))
                .ok_or_else(|| Fault::invalid(InvalidCode::SpaceRegistrationUnavailable, AT)),
            Vertex::Solve(value) => match value.as_ref() {
                SolveDraft::FamilyExpression(value) => Ok(value.ambient.clone()),
                _ => Err(Fault::invalid(InvalidCode::SymbolSortMismatch, AT)),
            },
            _ => Err(Fault::invalid(InvalidCode::SymbolSortMismatch, AT)),
        }
    }

    /// Find an installed specification by its registry identity, never by an encoded field tag.
    fn registered(
        &self,
        header: &Handle,
        field: &Field,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let expected = super::registry::specification(field)?;
        for id in &self.state.header(header.logical_id())?.scalar_spaces {
            budget.work(
                size(expected.specification.len(), AT)?.saturating_add(1),
                AT,
            )?;
            if let SolveDraft::ScalarSpace(value) = crate::solving::graph::object(&self.state, *id)?
                && *value == expected
            {
                return Ok(self.handle(*id));
            }
        }
        Err(Fault::invalid(
            InvalidCode::SpaceRegistrationUnavailable,
            AT,
        ))
    }

    /// Every cross-space operand requires the exact installed edge, with no path composition.
    fn placements(
        &self,
        header: &Handle,
        operator: &Operator,
        operands: &[Handle],
        spaces: &[Handle],
        ambient: Handle,
        budget: &mut Budget,
    ) -> Result<AmbientResolution, Fault> {
        let component = matches!(operator, Operator::RealPart | Operator::ImagPart);
        let destination = if component {
            self.registered(header, &Field::Complex, budget)?
        } else {
            ambient.clone()
        };
        let mut placements = Vec::new();
        placements
            .try_reserve_exact(operands.len())
            .map_err(|_| Fault::allocation(AT))?;
        for (operand, source) in operands.iter().zip(spaces) {
            budget.work(1, AT)?;
            if *source == destination {
                continue;
            }
            let definition = SpacePlacementData {
                source: source.clone(),
                ambient: destination.clone(),
                specification: "placement.real-in-complex".into(),
                version: 1,
            };
            if super::registry::placement(&self.state, &definition).is_err() {
                return Ok(AmbientResolution::Unresolved(
                    SolveReason::SpacePlacementUnavailable,
                ));
            }
            placements.push(AmbientPlacement {
                operand: operand.clone(),
                definition,
            });
        }
        Ok(AmbientResolution::Resolved {
            ambient,
            placements,
        })
    }
}

/// Closed signatures keep the existing Real and Complex arithmetic meanings exact.
fn choose(
    operator: &Operator,
    spaces: &[Handle],
    requested: Option<&Field>,
    cas: &Cas,
) -> Result<Option<Field>, Fault> {
    if matches!(operator, Operator::RealPart | Operator::ImagPart) {
        return Ok(Some(Field::Real));
    }
    if matches!(operator, Operator::Conjugate) {
        return Ok(Some(Field::Complex));
    }
    if let Some(field) = requested {
        return Ok(Some(field.clone()));
    }
    let mut selected = None;
    for space in spaces {
        let field = super::field(&cas.state, space)?;
        selected = match (selected, field) {
            (None, value) => Some(value),
            (Some(previous), value) if previous == value => Some(value),
            (Some(Field::Complex), Field::Real) | (Some(Field::Real), Field::Complex) => {
                Some(Field::Complex)
            }
            _ => return Ok(None),
        };
    }
    Ok(selected)
}

/// Refuse an operator without an installed scalar signature before proposing a placement.
const fn signature(operator: &Operator, count: usize) -> Result<(), Fault> {
    let arity = match operator {
        Operator::Rational(_) => 0,
        Operator::Neg
        | Operator::IntegerPower(_)
        | Operator::Conjugate
        | Operator::RealPart
        | Operator::ImagPart => 1,
        Operator::Add | Operator::Sub | Operator::Mul | Operator::Div => 2,
        _ => return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT)),
    };
    if count != arity {
        return Err(Fault::invalid(InvalidCode::SymbolSortMismatch, AT));
    }
    Ok(())
}
