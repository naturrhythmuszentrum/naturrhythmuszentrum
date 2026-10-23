// SPDX-License-Identifier: AGPL-3.0-only
//! Acyclic headers and explicit conservative semantic embeddings.

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
                    if declaration_index(
                        &inherited.environment.declarations,
                        &declaration.key,
                        budget,
                        AT,
                    )?
                    .is_some()
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
                let mut declarations = declarations.clone();
                sort_declarations(&mut declarations, budget, AT)?;
                let spaces =
                    self.declaration_spaces(state, &declarations, spaces, locals, budget)?;
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
        let count = size(declarations.len(), AT)?;
        let bytes = count
            .checked_mul(size(std::mem::size_of::<Option<LogicalId>>(), AT)?)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        let unit = budget
            .limits()
            .name_bytes
            .checked_add(2)
            .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?;
        budget.work(
            count
                .checked_mul(unit)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
            AT,
        )?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(declarations.len())
            .map_err(|_| Fault::allocation(AT))?;
        slots.resize(declarations.len(), None);
        for draft in drafts {
            let index = declaration_index(declarations, &draft.key, budget, AT)?
                .ok_or_else(|| Fault::invalid(InvalidCode::Sort, AT))?;
            if slots[index].is_some() || !space_sort(&declarations[index].sort) {
                return Err(Fault::invalid(InvalidCode::Sort, AT));
            }
            let space = self.resolve(state, &draft.space, locals)?;
            state.space(space)?;
            slots[index] = Some(space);
        }
        let mut result = Vec::new();
        result
            .try_reserve_exact(drafts.len())
            .map_err(|_| Fault::allocation(AT))?;
        for (declaration, space) in declarations.iter().zip(slots) {
            if space_sort(&declaration.sort) != space.is_some() {
                return Err(Fault::invalid(InvalidCode::Sort, AT));
            }
            if let Some(space) = space {
                result.push((declaration.key.clone(), space));
            }
        }
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

/// Binary lookup prepays ceil(log2(n))+1 complete key comparisons, including singleton tables.
pub(crate) fn declaration_index(
    declarations: &[Declaration],
    key: &str,
    budget: &mut Budget,
    at: Location,
) -> Result<Option<usize>, Fault> {
    let comparisons = if declarations.is_empty() {
        0
    } else {
        u64::from((declarations.len() - 1).bit_width()) + 1
    };
    budget.work(
        comparisons
            .checked_mul(budget.limits().name_bytes)
            .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?,
        at,
    )?;
    let mut lower = 0;
    let mut upper = declarations.len();
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        match declarations[middle].key.as_str().cmp(key) {
            std::cmp::Ordering::Less => lower = middle + 1,
            std::cmp::Ordering::Greater => upper = middle,
            std::cmp::Ordering::Equal => return Ok(Some(middle)),
        }
    }
    Ok(None)
}

/// Bottom-up stable merging prepays n*ceil(log2(n)) key comparisons and index moves explicitly.
pub(crate) fn sort_declarations(
    declarations: &mut [Declaration],
    budget: &mut Budget,
    at: Location,
) -> Result<(), Fault> {
    let count = declarations.len();
    let levels = if count < 2 {
        0
    } else {
        u64::from((count - 1).bit_width())
    };
    let count_u64 = size(count, at)?;
    let unit = budget
        .limits()
        .name_bytes
        .checked_add(1)
        .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?;
    let work = levels
        .checked_mul(unit)
        .and_then(|value| value.checked_add(12))
        .and_then(|value| value.checked_mul(count_u64))
        .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?;
    let bytes = count_u64
        .checked_mul(size(2 * std::mem::size_of::<usize>(), at)?)
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?;
    budget.with_retained_payload(bytes, at, |budget| {
        budget.work(work, at)?;
        let mut source = indices(count, at)?;
        let mut scratch = indices(count, at)?;
        let mut width = 1;
        while width < count {
            merge_indices(declarations, &source, &mut scratch, width);
            std::mem::swap(&mut source, &mut scratch);
            width = width.saturating_mul(2);
        }
        for (position, original) in source.into_iter().enumerate() {
            scratch[original] = position;
        }
        for position in 0..count {
            while scratch[position] != position {
                let destination = scratch[position];
                declarations.swap(position, destination);
                scratch.swap(position, destination);
            }
        }
        Ok(())
    })
}

/// Allocate both prepaid index buffers before changing a declaration's position.
fn indices(count: usize, at: Location) -> Result<Vec<usize>, Fault> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Fault::allocation(at))?;
    values.extend(0..count);
    Ok(values)
}

/// Each merge pass writes n indices and compares keys at most n times.
fn merge_indices(
    declarations: &[Declaration],
    source: &[usize],
    target: &mut [usize],
    width: usize,
) {
    let mut start = 0;
    while start < source.len() {
        let middle = start.saturating_add(width).min(source.len());
        let end = middle.saturating_add(width).min(source.len());
        let mut left = start;
        let mut right = middle;
        for destination in &mut target[start..end] {
            let right_finished = right == end;
            let left_available = left < middle;
            let take_left = right_finished
                || left_available
                    && declarations[source[left]].key <= declarations[source[right]].key;
            let index = if take_left {
                let value = left;
                left += 1;
                value
            } else {
                let value = right;
                right += 1;
                value
            };
            *destination = source[index];
        }
        start = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        limits::Limits,
        model::{AtomicSort, Environment, Field},
    };
    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    /// Empty, singleton and full LC-sized declaration inventories retain canonical key order under defaults.
    /// ´test:unit:header-key-index-preserves-default-admission´
    #[test]
    fn header_key_index_preserves_default_admission() -> TestResult {
        for count in [0, 1, 2, 94] {
            let mut cas = Cas::new(Limits::default())?;
            let nodes = cas.admit(&[core(count)], &mut Budget::new(Limits::default())?)?;
            let header = cas
                .state
                .header(cas.checked(nodes.first().ok_or("header")?)?)?;
            assert_eq!(header.environment.declarations.len(), count);
            assert!(
                header
                    .environment
                    .declarations
                    .windows(2)
                    .all(|pair| pair[0].key < pair[1].key)
            );
        }
        Ok(())
    }

    /// Exact cumulative Work boundaries reject before publishing a partial header.
    /// ´test:unit:header-key-index-has-cumulative-boundaries´
    #[test]
    fn header_key_index_has_cumulative_boundaries() -> TestResult {
        let mut cas = Cas::new(Limits::default())?;
        let mut budget = Budget::new(Limits::default())?;
        cas.admit(&[core(94)], &mut budget)?;
        let needed = budget.usage().work;
        for (limit, prior, success) in [
            (needed, 0, true),
            (needed - 1, 0, false),
            (needed, 1, false),
        ] {
            let mut cas = Cas::new(Limits::default())?;
            let before = cas.counts();
            let mut budget = Budget::new(Limits {
                work: limit,
                ..Limits::default()
            })?;
            budget.work(prior, AT)?;
            assert_eq!(cas.admit(&[core(94)], &mut budget).is_ok(), success);
            if !success {
                assert_eq!(cas.counts(), before);
            }
        }
        Ok(())
    }

    /// A late canonical key keeps sort checks, unknown-key refusal and atomic cumulative limits.
    /// ´test:unit:indexed-symbols-preserve-validation-and-cumulative-boundaries´
    #[test]
    fn indexed_symbols_preserve_validation_and_cumulative_boundaries() -> TestResult {
        let mut cas = Cas::new(Limits::default())?;
        let header = cas
            .admit(&[core(94)], &mut Budget::new(Limits::default())?)?
            .pop()
            .ok_or("header")?;
        let real = Sort::Value(AtomicSort::Scalar(Field::Real));
        let valid = symbol(&header, "value_093", real.clone());
        let mut budget = Budget::new(Limits::default())?;
        cas.admit(std::slice::from_ref(&valid), &mut budget)?;
        let needed = budget.usage().work;
        for (limit, prior, success) in [
            (needed, 0, true),
            (needed - 1, 0, false),
            (needed, 1, false),
        ] {
            let mut cas = Cas::new(Limits::default())?;
            let header = cas
                .admit(&[core(94)], &mut Budget::new(Limits::default())?)?
                .pop()
                .ok_or("header")?;
            let before = cas.counts();
            let mut budget = Budget::new(Limits {
                work: limit,
                ..Limits::default()
            })?;
            budget.work(prior, AT)?;
            let result = cas.admit(&[symbol(&header, "value_093", real.clone())], &mut budget);
            assert_eq!(result.is_ok(), success);
            if !success {
                assert_eq!(cas.counts(), before);
            }
        }
        for draft in [
            symbol(&header, "missing", real),
            symbol(&header, "value_093", Sort::Value(AtomicSort::Integer)),
        ] {
            let before = cas.counts();
            assert!(
                cas.admit(&[draft], &mut Budget::new(Limits::default())?)
                    .is_err()
            );
            assert_eq!(cas.counts(), before);
        }
        Ok(())
    }

    /// The original header identity remains attached to every admitted or refused leaf.
    fn symbol(header: &crate::graph::Handle, key: &str, sort: Sort) -> crate::graph::Draft {
        crate::graph::Draft::Expression(crate::graph::ExpressionDraft {
            operator: Operator::Free(key.into()),
            sort,
            header: crate::graph::Reference::Existing(header.clone()),
            scope: Vec::new(),
            binding: None,
            space: None,
            operands: Vec::new(),
        })
    }

    /// Reversed input keys exercise canonical reordering without changing declaration meaning.
    fn core(count: usize) -> crate::graph::Draft {
        crate::graph::Draft::Header(HeaderDraft::Core(Environment {
            declarations: (0..count)
                .rev()
                .map(|index| Declaration {
                    key: format!("value_{index:03}"),
                    display_name: "value".into(),
                    sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
                })
                .collect(),
            ..Environment::default()
        }))
    }
}
