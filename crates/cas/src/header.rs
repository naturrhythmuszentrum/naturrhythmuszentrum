// SPDX-License-Identifier: AGPL-3.0-only
//! Acyclic headers and explicit conservative semantic embeddings.

use std::collections::BTreeSet;

use crate::budget::{Budget, BudgetKind};
use crate::domain_model::{DeclarationSpaceDraft, Header, HeaderDraft};
use crate::encoding::{self, Encoder};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Edge, LogicalId, State, Vertex, position_u32, size};
use crate::model::{Declaration, Operator, Sort};

/// Header admission and embedding location.
const AT: Location = Location::new("semantic header");

impl Cas {
    /// Validate fresh signatures before publishing a header.
    pub(crate) fn admit_header(
        &self,
        state: &mut State,
        draft: &HeaderDraft,
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let (parent, environment, declarations, spaces) = match draft {
            HeaderDraft::Core(environment) => {
                if environment
                    .declarations
                    .iter()
                    .any(|item| space_sort(&item.sort))
                {
                    return Err(Fault::invalid(InvalidCode::Context, AT));
                }
                let environment = crate::validation::environment(environment, budget)?;
                let declarations = environment.declarations.clone();
                (None, environment, declarations, Vec::new())
            }
            HeaderDraft::Extend {
                parent,
                declarations,
                spaces,
            } => {
                let parent = self.resolve(state, parent, locals)?;
                let inherited = state.header(parent)?;
                let mut environment = inherited.environment.clone();
                for declaration in declarations {
                    budget.work(size(environment.declarations.len(), AT)?, AT)?;
                    if environment
                        .declarations
                        .iter()
                        .any(|item| item.key == declaration.key)
                    {
                        return Err(Fault::invalid(InvalidCode::Context, AT));
                    }
                    environment
                        .declarations
                        .try_reserve(1)
                        .map_err(|_| Fault::allocation(AT))?;
                    environment.declarations.push(declaration.clone());
                }
                let environment = crate::validation::environment(&environment, budget)?;
                let spaces =
                    self.declaration_spaces(state, declarations, spaces, locals, budget)?;
                let mut declarations = declarations.clone();
                declarations.sort_by(|left, right| left.key.cmp(&right.key));
                (Some(parent), environment, declarations, spaces)
            }
        };
        let mut out = Encoder::new(budget, AT);
        encoding::environment(&environment, &mut out)?;
        let bytes = out.finish();
        let scalar_spaces = self.registered_scalar_spaces(state, pending, budget)?;
        let header = Header {
            scalar_spaces,
            environment,
            bytes,
            parent,
            declarations,
            spaces,
        };
        let edges = header_edges(&header)?;
        self.insert(state, Vertex::Header(header), &edges, pending, budget)
    }

    /// Resolve exactly one space for each fresh space-dependent declaration.
    fn declaration_spaces(
        &self,
        state: &State,
        declarations: &[Declaration],
        drafts: &[DeclarationSpaceDraft],
        locals: &[LogicalId],
        budget: &mut Budget,
    ) -> Result<Vec<(String, LogicalId)>, Fault> {
        let count = size(declarations.len(), AT)?
            .checked_add(size(drafts.len(), AT)?)
            .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, AT))?;
        budget.work(
            count
                .checked_mul(count)
                .and_then(|n| n.checked_mul(budget.limits().name_bytes))
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(drafts.len())
            .map_err(|_| Fault::allocation(AT))?;
        let mut keys = BTreeSet::new();
        for draft in drafts {
            if !keys.insert(draft.key.clone())
                || !declarations
                    .iter()
                    .any(|item| item.key == draft.key && space_sort(&item.sort))
            {
                return Err(Fault::invalid(InvalidCode::Sort, AT));
            }
            let space = self.resolve(state, &draft.space, locals)?;
            state.space(space)?;
            result.push((draft.key.clone(), space));
        }
        if declarations
            .iter()
            .filter(|item| space_sort(&item.sort))
            .count()
            != result.len()
        {
            return Err(Fault::invalid(InvalidCode::Sort, AT));
        }
        result.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(result)
    }

    /// Read a context's actual header for explicit syntax construction.
    ///
    /// # Errors
    /// Refuses a foreign or non-context handle.
    pub fn context_header(
        &self,
        context: &crate::graph::Handle,
    ) -> Result<crate::graph::Handle, Fault> {
        Ok(self.handle(self.state.context(self.checked(context)?)?.header_id))
    }
}

/// Whether a sort requires an exact external space reference.
pub(crate) const fn space_sort(sort: &Sort) -> bool {
    matches!(sort, Sort::Test | Sort::Distribution)
}

/// Check an actual earlier header's exact conservative embedding into a later one.
pub(crate) fn embeds(
    state: &State,
    actual: LogicalId,
    target: LogicalId,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    state.header(actual)?;
    let mut current = target;
    let mut depth = 0_u64;
    loop {
        budget.work(1, AT)?;
        budget.check(BudgetKind::Depth, depth, AT)?;
        if current == actual {
            return Ok(true);
        }
        let Some(parent) = state.header(current)?.parent else {
            return Ok(false);
        };
        current = parent;
        depth = depth
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::Depth, AT))?;
    }
}

/// Check an explicit expression space and any exact declaration binding.
pub(crate) fn expression_space(
    state: &State,
    header: LogicalId,
    operator: &Operator,
    sort: &Sort,
    explicit: Option<LogicalId>,
    operands: &[LogicalId],
    budget: &mut Budget,
) -> Result<Option<LogicalId>, Fault> {
    if !(space_sort(sort)
        || crate::distribution_syntax::scalar_action(operator)
        || explicit.is_some() && crate::distribution_syntax::action_arithmetic(operator))
    {
        if operands
            .iter()
            .any(|id| state.expression(*id).is_ok_and(|v| v.space.is_some()))
        {
            return Err(Fault::invalid(InvalidCode::Sort, AT));
        }
        return if explicit.is_none() {
            Ok(None)
        } else {
            Err(Fault::invalid(InvalidCode::Sort, AT))
        };
    }
    let declared = if let Operator::Free(key) = operator {
        declaration_space(state, header, key, budget)?
    } else {
        None
    };
    let space = explicit
        .or(declared)
        .ok_or_else(|| Fault::invalid(InvalidCode::Sort, AT))?;
    let value = state.space(space)?;
    if declared.is_some_and(|expected| expected != space)
        || !embeds(state, value.header, header, budget)?
    {
        return Err(Fault::invalid(InvalidCode::Sort, AT));
    }
    for operand in operands {
        if state
            .expression(*operand)?
            .space
            .is_some_and(|actual| actual != space)
        {
            return Err(Fault::invalid(InvalidCode::Sort, AT));
        }
    }
    crate::distribution_syntax::space(state, operator, sort, space, operands, budget)?;
    Ok(Some(space))
}

/// Resolve the exact declaration binding without inferring a space from display spelling.
fn declaration_space(
    state: &State,
    header: LogicalId,
    key: &str,
    budget: &mut Budget,
) -> Result<Option<LogicalId>, Fault> {
    let mut current = header;
    let mut depth = 0_u64;
    loop {
        budget.check(BudgetKind::Depth, depth, AT)?;
        depth = depth
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::Depth, AT))?;
        budget.work(1, AT)?;
        let value = state.header(current)?;
        if let Some((_, space)) = value.spaces.iter().find(|(name, _)| name == key) {
            return Ok(Some(*space));
        }
        current = value
            .parent
            .ok_or_else(|| Fault::invalid(InvalidCode::Sort, AT))?;
    }
}

impl Cas {
    /// Read the actual header of a mathematical expression without borrowing a proof context.
    ///
    /// # Errors
    /// Refuses foreign handles and non-expression objects.
    pub fn expression_header(
        &self,
        expression: &crate::graph::Handle,
    ) -> Result<crate::graph::Handle, Fault> {
        Ok(self.handle(self.state.expression(self.checked(expression)?)?.header))
    }
}

impl Cas {
    /// Read the explicit binder scope carried by one expression.
    ///
    /// # Errors
    /// Refuses foreign handles and non-expression objects.
    pub fn expression_scope(
        &self,
        expression: &crate::graph::Handle,
    ) -> Result<&crate::model::Scope, Fault> {
        Ok(&self.state.expression(self.checked(expression)?)?.scope)
    }
}

impl Cas {
    /// Inspect the exact predicate constructor carried by a guarded obligation.
    ///
    /// # Errors
    /// Refuses foreign handles and non-predicate graph objects.
    pub fn predicate_kind(
        &self,
        predicate: &crate::graph::Handle,
    ) -> Result<&crate::model::PredicateKind, Fault> {
        let crate::graph::Vertex::Condition(value) = self.state.node(self.checked(predicate)?)?
        else {
            return Err(Fault::invalid(InvalidCode::Reference, AT));
        };
        Ok(&value.kind)
    }

    /// Copy the ordered predicate arguments under the caller's allocation and work limits.
    ///
    /// # Errors
    /// Refuses foreign predicates and exhausted resources.
    pub fn predicate_arguments(
        &self,
        predicate: &crate::graph::Handle,
        budget: &mut Budget,
    ) -> Result<Vec<crate::graph::Handle>, Fault> {
        let crate::graph::Vertex::Condition(value) = self.state.node(self.checked(predicate)?)?
        else {
            return Err(Fault::invalid(InvalidCode::Reference, AT));
        };
        budget.check(
            BudgetKind::ConditionAtoms,
            size(value.arguments.len(), AT)?,
            AT,
        )?;
        budget.work(size(value.arguments.len(), AT)?, AT)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(value.arguments.len())
            .map_err(|_| Fault::allocation(AT))?;
        result.extend(value.arguments.iter().map(|id| self.handle(*id)));
        Ok(result)
    }
}

/// Retain parent, distribution-space and scalar-registry edges in their exact roles.
fn header_edges(header: &Header) -> Result<Vec<(LogicalId, Edge)>, Fault> {
    let mut edges = Vec::new();
    let capacity = header
        .spaces
        .len()
        .checked_add(header.scalar_spaces.len())
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| Fault::capacity(BudgetKind::Edges, AT))?;
    edges
        .try_reserve(capacity)
        .map_err(|_| Fault::allocation(AT))?;
    if let Some(parent) = header.parent {
        edges.push((parent, Edge::ParentHeader));
    }
    for (position, (_, space)) in header.spaces.iter().enumerate() {
        edges.push((*space, Edge::DeclarationSpace(position_u32(position)?)));
    }
    for (position, space) in header.scalar_spaces.iter().enumerate() {
        edges.push((
            *space,
            Edge::Scalar(crate::graph::ScalarEdge::DeclarationScalarSpace(
                position_u32(position)?,
            )),
        ));
    }
    Ok(edges)
}
