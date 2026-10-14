// SPDX-License-Identifier: AGPL-3.0-only
//! Exact local value agreement on independently reconstructed rational cells.

use crate::budget::Budget;
use crate::case_syntax::{self, Domains, invalid};
use crate::domain_model::AmbientDomain;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Draft, ExpressionDraft, LogicalId, Reference};
use crate::model::Operator;
use crate::proof_model::DomainContract;
use crate::region::RegionCertificate;
use crate::substitution::Substitution;
use std::collections::BTreeMap;

/// One finite expression interpretation on a complete exact cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Value {
    /// No value exists throughout this cell.
    Undefined,
    /// A strict scalar expression valid throughout the cell.
    Defined(LogicalId),
    /// Two reached associations have incompatible values or domains.
    Conflict,
}

/// All exact region rows required to interpret two endpoints locally.
pub(crate) struct Analysis {
    /// Stable structural root order of the partition rows.
    pub(crate) regions: Vec<LogicalId>,
    /// Independently reconstructed rational cuts and memberships.
    pub(crate) cells: RegionCertificate,
    /// Child-before-parent value dependencies.
    order: Vec<LogicalId>,
    /// Strict definedness of each expression inside the requested domain.
    domains: Domains,
    /// Applicability is restricted to each case's actual local reach.
    active: BTreeMap<(LogicalId, LogicalId), LogicalId>,
}

/// Build the exact partition without accepting any value equivalence.
pub(crate) fn analyze(
    cas: &mut Cas,
    left: LogicalId,
    right: LogicalId,
    within: LogicalId,
    budget: &mut Budget,
) -> Result<Analysis, Fault> {
    let order = case_syntax::order(cas, &[left, right], budget)?;
    let reaches = crate::case_reach::regions(cas, &[left, right], &order, within, budget)?;
    let mut domains = case_syntax::domains(cas, &order, within, budget)?;
    for (id, domain) in &mut domains {
        *domain = crate::case_reach::masked_meet(
            cas,
            *reaches.get(id).ok_or_else(invalid)?,
            &[*domain],
            budget,
        )?;
    }
    let mut active = BTreeMap::new();
    let mut roots = vec![within];
    roots.extend(domains.values().copied());
    for id in &order {
        let value = cas.state.expression(*id)?.clone();
        for arm in &value.arms {
            let domain = value.domain.ok_or_else(invalid)?;
            let region = cas.state.arm(*arm)?.region;
            let region = crate::case_reach::masked_meet(
                cas,
                *reaches.get(id).ok_or_else(invalid)?,
                &[domain, region],
                budget,
            )?;
            roots.push(region);
            active.insert((*id, *arm), region);
        }
    }
    let regions = case_syntax::canonical(cas, &roots, budget)?;
    let cells = crate::region_cache::partition(cas, &regions, budget)?;
    Ok(Analysis {
        regions,
        cells,
        order,
        domains,
        active,
    })
}

impl Analysis {
    /// Read a cell only through its exact declared region row.
    fn contains(&self, region: LogicalId, cell: usize) -> Result<bool, Fault> {
        let index = self
            .regions
            .iter()
            .position(|id| *id == region)
            .ok_or_else(invalid)?;
        self.cells
            .membership
            .get(index)
            .and_then(|row| row.get(cell))
            .copied()
            .ok_or_else(invalid)
    }

    /// Compare exact local values with either strict or two-direction definedness semantics.
    pub(crate) fn check(
        &self,
        cas: &mut Cas,
        left: LogicalId,
        right: LogicalId,
        within: LogicalId,
        contract: DomainContract,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let count = self.cells.membership.first().ok_or_else(invalid)?.len();
        for cell in 0..count {
            budget.condition(1, Location::new("local case agreement"))?;
            if !self.contains(within, cell)? {
                continue;
            }
            let values = self.specialize(cas, within, cell, budget)?;
            let left = *values.get(&left).ok_or_else(invalid)?;
            let right = *values.get(&right).ok_or_else(invalid)?;
            match (left, right) {
                (Value::Conflict, _) | (_, Value::Conflict) => return Err(conflict()),
                (Value::Undefined, Value::Undefined)
                    if contract == DomainContract::SameDefinedDomain => {}
                (Value::Defined(left), Value::Defined(right)) => {
                    if !equal_on_cell(cas, left, right, within, &self.cells, cell, budget)? {
                        return Err(conflict());
                    }
                }
                _ => {
                    return Err(Fault::invalid(
                        InvalidCode::RefutedDomain,
                        Location::new("local case definedness"),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Lower one exact cell without evaluating inactive or undefined values.
    fn specialize(
        &self,
        cas: &mut Cas,
        within: LogicalId,
        cell: usize,
        budget: &mut Budget,
    ) -> Result<BTreeMap<LogicalId, Value>, Fault> {
        let mut result = BTreeMap::new();
        for id in &self.order {
            budget.condition(1, Location::new("case cell specialization"))?;
            let domain = *self.domains.get(id).ok_or_else(invalid)?;
            let value = cas.state.expression(*id)?.clone();
            let output = if !self.contains(domain, cell)? {
                Value::Undefined
            } else if value.domain.is_some() {
                self.case_value(cas, *id, within, cell, &result, budget)?
            } else {
                scalar_value(cas, *id, &result, budget)?
            };
            result.insert(*id, output);
        }
        Ok(result)
    }

    /// Check every applicable association; order cannot turn an overlap into first-match semantics.
    fn case_value(
        &self,
        cas: &mut Cas,
        id: LogicalId,
        within: LogicalId,
        cell: usize,
        values: &BTreeMap<LogicalId, Value>,
        budget: &mut Budget,
    ) -> Result<Value, Fault> {
        let expression = cas.state.expression(id)?;
        let arms = case_syntax::canonical(cas, &expression.arms, budget)?;
        let mut selected = None;
        for arm in arms {
            let region = *self.active.get(&(id, arm)).ok_or_else(invalid)?;
            let arm = cas.state.arm(arm)?;
            if !self.contains(region, cell)? {
                continue;
            }
            let value = *values.get(&arm.value).ok_or_else(invalid)?;
            if let Some(previous) = selected {
                match (previous, value) {
                    (Value::Undefined, Value::Undefined) => {}
                    (Value::Defined(left), Value::Defined(right)) => {
                        if !equal_on_cell(cas, left, right, within, &self.cells, cell, budget)? {
                            return Ok(Value::Conflict);
                        }
                    }
                    _ => return Ok(Value::Conflict),
                }
            } else {
                selected = Some(value);
            }
        }
        Ok(selected.unwrap_or(Value::Undefined))
    }

    /// Propose one cell-local scalar germ; a separate equality proof checks the whole region.
    pub(crate) fn germ(
        &self,
        cas: &mut Cas,
        expression: LogicalId,
        within: LogicalId,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let count = self.cells.membership.first().ok_or_else(invalid)?.len();
        for cell in 0..count {
            if self.contains(within, cell)? {
                return match self
                    .specialize(cas, within, cell, budget)?
                    .get(&expression)
                    .ok_or_else(invalid)?
                {
                    Value::Defined(value) => Ok(*value),
                    Value::Conflict => Err(conflict()),
                    Value::Undefined => Err(Fault::invalid(
                        InvalidCode::PendingPremise,
                        Location::new("undefined boundary germ"),
                    )),
                };
            }
        }
        Err(invalid())
    }

    /// Interpret every open cell for the finite polynomial regularity checker.
    pub(crate) fn polynomial_cells(
        &self,
        cas: &mut Cas,
        expression: LogicalId,
        within: LogicalId,
        coordinate: &str,
        budget: &mut Budget,
    ) -> Result<Vec<Option<Vec<crate::number::Rational>>>, Fault> {
        let count = self.cells.membership.first().ok_or_else(invalid)?.len();
        let mut result = Vec::new();
        result
            .try_reserve_exact(count.div_ceil(2))
            .map_err(|_| Fault::allocation(Location::new("regular polynomial cells")))?;
        for cell in (0..count).step_by(2) {
            if !self.contains(within, cell)? {
                result.push(None);
                continue;
            }
            let values = self.specialize(cas, within, cell, budget)?;
            let value = match values.get(&expression).ok_or_else(invalid)? {
                Value::Defined(value) => *value,
                Value::Conflict => return Err(conflict()),
                Value::Undefined => {
                    return Err(Fault::invalid(
                        InvalidCode::PendingPremise,
                        Location::new("regular open cell"),
                    ));
                }
            };
            result.push(Some(crate::polynomial::univariate(
                &cas.state, value, coordinate, budget,
            )?));
        }
        Ok(result)
    }

    /// Expose exact endpoint definedness to the domain schema.
    pub(crate) fn domain(&self, expression: LogicalId) -> Result<LogicalId, Fault> {
        self.domains.get(&expression).copied().ok_or_else(invalid)
    }

    /// Evaluate the unique inhabited singleton without guessing ownership from missing evidence.
    pub(crate) fn point_value(
        &self,
        cas: &mut Cas,
        expression: LogicalId,
        singleton: LogicalId,
        budget: &mut Budget,
    ) -> Result<Option<LogicalId>, Fault> {
        let count = self.cells.membership.first().ok_or_else(invalid)?.len();
        for cell in 0..count {
            if !self.contains(singleton, cell)? {
                continue;
            }
            if cell & 1 != 1 {
                return Err(invalid());
            }
            let values = self.specialize(cas, singleton, cell, budget)?;
            return match values.get(&expression).ok_or_else(invalid)? {
                Value::Undefined => Ok(None),
                Value::Conflict => Err(conflict()),
                Value::Defined(value) => {
                    let point = self.cells.cuts.get(cell / 2).ok_or_else(invalid)?;
                    let coordinate = match &cas.state.region(singleton)?.ambient {
                        AmbientDomain::Real(key) | AmbientDomain::Integer(key) => key.clone(),
                        _ => return Err(invalid()),
                    };
                    let value = at_point(cas, *value, &coordinate, point, budget)?;
                    Ok(Some(crate::polynomial::normalize(cas, value, budget)?))
                }
            };
        }
        Err(invalid())
    }
}

/// Rebuild strict scalar syntax after each operand has a cell-local value.
fn scalar_value(
    cas: &mut Cas,
    id: LogicalId,
    values: &BTreeMap<LogicalId, Value>,
    budget: &mut Budget,
) -> Result<Value, Fault> {
    let value = cas.state.expression(id)?;
    if matches!(value.operator, Operator::NamedCase(_)) {
        let child = value.operands.first().ok_or_else(invalid)?;
        return values.get(child).copied().ok_or_else(invalid);
    }
    if value.operands.is_empty() {
        return Ok(Value::Defined(id));
    }
    let mut operands = Vec::new();
    operands
        .try_reserve_exact(value.operands.len())
        .map_err(|_| Fault::allocation(Location::new("case scalar operands")))?;
    for child in &value.operands {
        match values.get(child).ok_or_else(invalid)? {
            Value::Undefined => return Ok(Value::Undefined),
            Value::Conflict => return Ok(Value::Conflict),
            Value::Defined(id) => operands.push(Reference::Existing(cas.handle(*id))),
        }
    }
    let draft = Draft::Expression(ExpressionDraft {
        space: None,
        header: Reference::Existing(cas.handle(value.header)),
        scope: value.scope.clone(),
        sort: value.sort.clone(),
        operator: value.operator.clone(),
        binding: value.binding.clone(),
        operands,
    });
    Ok(Value::Defined(
        case_syntax::admit(cas, draft, budget)?.logical_id(),
    ))
}

/// Polynomial identity proves agreement on an open cell; singleton agreement is exact evaluation.
fn equal_on_cell(
    cas: &mut Cas,
    mut left: LogicalId,
    mut right: LogicalId,
    within: LogicalId,
    cells: &RegionCertificate,
    cell: usize,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    if left == right {
        return Ok(true);
    }
    if cell & 1 == 1 {
        let point = cells.cuts.get(cell / 2).ok_or_else(invalid)?;
        let coordinate = match &cas.state.region(within)?.ambient {
            AmbientDomain::Real(key) | AmbientDomain::Integer(key) => key.clone(),
            _ => return Err(invalid()),
        };
        left = at_point(cas, left, &coordinate, point, budget)?;
        right = at_point(cas, right, &coordinate, point, budget)?;
    }
    let equal = crate::polynomial::equal(&cas.state, left, right, true, budget)?;
    if !equal
        && cell & 1 == 0
        && matches!(cas.state.region(within)?.ambient, AmbientDomain::Integer(_))
    {
        return Err(Fault::invalid(
            InvalidCode::UnsupportedFragment,
            Location::new("integer local agreement"),
        ));
    }
    Ok(equal)
}

/// Substitute a rational location without borrowing a producer's numerical evaluator.
fn at_point(
    cas: &mut Cas,
    id: LogicalId,
    coordinate: &str,
    point: &crate::number::Rational,
    budget: &mut Budget,
) -> Result<LogicalId, Fault> {
    let value = cas.state.expression(id)?;
    let sort = cas
        .state
        .header(value.header)?
        .environment
        .declarations
        .iter()
        .find(|declaration| declaration.key == coordinate)
        .ok_or_else(invalid)?
        .sort
        .clone();
    let draft = Draft::Expression(ExpressionDraft {
        space: None,
        header: Reference::Existing(cas.handle(value.header)),
        scope: value.scope.clone(),
        sort,
        operator: Operator::Rational(point.clone()),
        binding: None,
        operands: Vec::new(),
    });
    let replacement = case_syntax::admit(cas, draft, budget)?;
    let term = cas.handle(id);
    Ok(cas
        .substitute(
            &term,
            &[Substitution {
                key: coordinate.into(),
                replacement,
            }],
            budget,
        )?
        .logical_id())
}

/// A witnessed nonempty cell refutes overlap agreement.
const fn conflict() -> Fault {
    Fault::invalid(
        InvalidCode::OverlapConflict,
        Location::new("exact case overlap"),
    )
}
