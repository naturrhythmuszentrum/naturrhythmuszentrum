// SPDX-License-Identifier: AGPL-3.0-only
//! Simultaneous free substitution with explicit de Bruijn lifting.

use std::collections::BTreeMap;

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Draft, ExpressionDraft, Handle, LogicalId, Reference, size};
use crate::model::{Operator, Scope};

/// A declared free key and the exact expression inserted for it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Substitution {
    /// Declared free-variable identity, never a binder display name.
    pub key: String,
    /// Replacement expression in the original root scope and semantic header.
    pub replacement: Handle,
}

/// Staged substitution output and its monotonically measured draft payload.
struct Plan {
    /// Child-before-parent expression drafts.
    drafts: Vec<Draft>,
    /// Accounted pending bytes.
    bytes: u64,
}

impl Plan {
    /// Append one bounded draft without publishing it.
    fn push(&mut self, draft: ExpressionDraft, budget: &mut Budget) -> Result<Reference, Fault> {
        let at = Location::new("substitution draft");
        let node = Draft::Expression(draft);
        let bytes = crate::validation::draft_payload(std::slice::from_ref(&node), budget)?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?;
        budget.check(BudgetKind::StagedPayloadBytes, self.bytes, at)?;
        let index = u32::try_from(self.drafts.len())
            .map_err(|_| Fault::capacity(BudgetKind::StagedVertices, at))?;
        budget.check(
            BudgetKind::StagedVertices,
            u64::from(index)
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedVertices, at))?,
            at,
        )?;
        self.drafts
            .try_reserve(1)
            .map_err(|_| Fault::allocation(at))?;
        self.drafts.push(node);
        Ok(Reference::Local(index))
    }
}

impl Cas {
    /// Reserve a full-graph upper bound before cloning any reachable expression payload.
    fn prepay_substitution(&self, budget: &mut Budget, at: Location) -> Result<(), Fault> {
        let bytes = self
            .counts()
            .payload_bytes
            .checked_mul(2)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, at))?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, at)?;
        budget.work(bytes, at)
    }

    /// Instantiate every slot of the root's innermost frame and remove that frame.
    ///
    /// # Errors
    /// Refuses a missing frame, wrong slot count/sort, foreign handles or exhausted budgets.
    /// Nested local binders remain local; references outside the removed frame shift down.
    pub fn instantiate_bound(
        &mut self,
        root: &Handle,
        replacements: &[Handle],
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let at = Location::new("bound instantiation");
        self.prepay_substitution(budget, at)?;
        let root_id = self.checked(root)?;
        let (outer, replacement_ids) = self.bound_replacements(root_id, replacements, budget)?;
        let outer_len = outer.len();
        let mut plan = Plan {
            drafts: Vec::new(),
            bytes: 0,
        };
        let mut mapped = BTreeMap::new();
        let mut pending = vec![(root_id, false)];
        while let Some((id, expanded)) = pending.pop() {
            budget.work(
                size(mapped.len(), at)?
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?,
                at,
            )?;
            if mapped.contains_key(&id) {
                continue;
            }
            let value = self.state.expression(id)?;
            if let Operator::Rational(number) = &value.operator {
                number.check(budget, at)?;
            }
            let prefix = outer_len
                .checked_add(1)
                .ok_or_else(|| Fault::invalid(InvalidCode::Scope, at))?;
            let suffix = value
                .scope
                .get(prefix..)
                .ok_or_else(|| Fault::invalid(InvalidCode::Scope, at))?;
            let nested =
                u32::try_from(suffix.len()).map_err(|_| Fault::invalid(InvalidCode::Scope, at))?;
            let mut scope = outer.clone();
            scope
                .try_reserve(suffix.len())
                .map_err(|_| Fault::allocation(at))?;
            scope.extend_from_slice(suffix);
            if let Operator::Bound { depth, slot } = &value.operator
                && *depth == nested
            {
                let index =
                    usize::try_from(*slot).map_err(|_| Fault::invalid(InvalidCode::Scope, at))?;
                let replacement = replacement_ids
                    .get(index)
                    .copied()
                    .ok_or_else(|| Fault::invalid(InvalidCode::Scope, at))?;
                let result =
                    self.lift_replacement(replacement, &outer, &scope, &mut plan, budget)?;
                mapped.insert(id, result);
                continue;
            }
            if !expanded {
                pending
                    .try_reserve(
                        value
                            .operands
                            .len()
                            .checked_add(1)
                            .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, at))?,
                    )
                    .map_err(|_| Fault::allocation(at))?;
                pending.push((id, true));
                pending.extend(value.operands.iter().rev().map(|child| (*child, false)));
                continue;
            }
            let mut operator = value.operator.clone();
            if let Operator::Bound { depth, .. } = &mut operator
                && *depth > nested
            {
                *depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| Fault::invalid(InvalidCode::Scope, at))?;
            }
            let operands = mapped_operands(&value.operands, &mapped, budget)?;
            let result = plan.push(
                ExpressionDraft {
                    operator,
                    sort: value.sort.clone(),
                    context: Reference::Existing(self.handle(value.context)),
                    scope,
                    binding: value.binding.clone(),
                    operands,
                },
                budget,
            )?;
            mapped.insert(id, result);
        }
        let root = mapped
            .get(&root_id)
            .cloned()
            .ok_or_else(|| Fault::invalid(InvalidCode::Reference, at))?;
        self.publish_substitution(&plan.drafts, root, budget)
    }

    /// Validate the removed frame and each replacement before building any drafts.
    fn bound_replacements(
        &self,
        root: LogicalId,
        replacements: &[Handle],
        budget: &mut Budget,
    ) -> Result<(Scope, Vec<LogicalId>), Fault> {
        let at = Location::new("bound instantiation");
        let value = self.state.expression(root)?;
        let frame = value
            .scope
            .last()
            .ok_or_else(|| Fault::invalid(InvalidCode::Scope, at))?;
        if frame.len() != replacements.len() {
            return Err(Fault::invalid(InvalidCode::Scope, at));
        }
        let outer_len = value
            .scope
            .len()
            .checked_sub(1)
            .ok_or_else(|| Fault::invalid(InvalidCode::Scope, at))?;
        let outer = value
            .scope
            .get(..outer_len)
            .ok_or_else(|| Fault::invalid(InvalidCode::Scope, at))?
            .to_vec();
        let header = &self.state.context(value.context)?.header;
        let mut replacement_ids = Vec::new();
        replacement_ids
            .try_reserve_exact(replacements.len())
            .map_err(|_| Fault::allocation(at))?;
        for (handle, sort) in replacements.iter().zip(frame) {
            let id = self.checked(handle)?;
            let replacement = self.state.expression(id)?;
            budget.work(size(header.len(), at)?, at)?;
            if replacement.scope != outer
                || replacement.sort != crate::model::Sort::Value(sort.clone())
                || self.state.context(replacement.context)?.header != *header
            {
                return Err(Fault::invalid(InvalidCode::Scope, at));
            }
            replacement_ids.push(id);
        }
        Ok((outer, replacement_ids))
    }

    /// Simultaneously substitute free declaration keys without capturing bound references.
    ///
    /// # Errors
    /// Refuses foreign handles, duplicate keys, incompatible scopes/sorts or resource limits.
    /// Inserted replacements are not recursively substituted, and failure publishes nothing.
    pub fn substitute(
        &mut self,
        root: &Handle,
        substitutions: &[Substitution],
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let at = Location::new("free substitution");
        self.prepay_substitution(budget, at)?;
        let root_id = self.checked(root)?;
        let expression = self.state.expression(root_id)?;
        let initial_scope = expression.scope.clone();
        let replacements = self.free_replacements(root_id, substitutions, budget)?;
        let mut plan = Plan {
            drafts: Vec::new(),
            bytes: 0,
        };
        let mut mapped = BTreeMap::new();
        let mut pending = vec![(root_id, false)];
        while let Some((id, expanded)) = pending.pop() {
            budget.work(
                size(mapped.len(), at)?
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?,
                at,
            )?;
            if mapped.contains_key(&id) {
                continue;
            }
            let value = self.state.expression(id)?;
            if let Operator::Rational(number) = &value.operator {
                number.check(budget, at)?;
            }
            if let Operator::Free(key) = &value.operator
                && let Some(replacement) = replacements.get(key)
            {
                let result = self.lift_replacement(
                    *replacement,
                    &initial_scope,
                    &value.scope,
                    &mut plan,
                    budget,
                )?;
                mapped.insert(id, result);
                continue;
            }
            if !expanded {
                pending
                    .try_reserve(
                        value
                            .operands
                            .len()
                            .checked_add(1)
                            .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, at))?,
                    )
                    .map_err(|_| Fault::allocation(at))?;
                pending.push((id, true));
                pending.extend(value.operands.iter().rev().map(|child| (*child, false)));
                continue;
            }
            let operands = mapped_operands(&value.operands, &mapped, budget)?;
            let result = plan.push(
                ExpressionDraft {
                    operator: value.operator.clone(),
                    sort: value.sort.clone(),
                    context: Reference::Existing(self.handle(value.context)),
                    scope: value.scope.clone(),
                    binding: value.binding.clone(),
                    operands,
                },
                budget,
            )?;
            mapped.insert(id, result);
        }
        let root = mapped
            .get(&root_id)
            .cloned()
            .ok_or_else(|| Fault::invalid(InvalidCode::Reference, at))?;
        self.publish_substitution(&plan.drafts, root, budget)
    }

    /// Check simultaneous replacements against the root's declaration snapshot and scope.
    fn free_replacements(
        &self,
        root: LogicalId,
        substitutions: &[Substitution],
        budget: &mut Budget,
    ) -> Result<BTreeMap<String, LogicalId>, Fault> {
        let at = Location::new("free substitution");
        let expression = self.state.expression(root)?;
        let initial_scope = &expression.scope;
        let context = self.state.context(expression.context)?;
        let mut replacements = BTreeMap::new();
        let count = size(substitutions.len(), at)?;
        budget.work(
            count
                .checked_mul(count)
                .and_then(|n| n.checked_mul(budget.limits().name_bytes))
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?,
            at,
        )?;
        for substitution in substitutions {
            let replacement = self.checked(&substitution.replacement)?;
            let value = self.state.expression(replacement)?;
            budget.work(size(context.header.len(), at)?, at)?;
            let declared = context
                .environment
                .declarations
                .iter()
                .find(|declaration| declaration.key == substitution.key)
                .ok_or_else(|| Fault::invalid(InvalidCode::Scope, at))?;
            if value.scope != *initial_scope
                || value.sort != declared.sort
                || self.state.context(value.context)?.header != context.header
                || replacements
                    .insert(substitution.key.clone(), replacement)
                    .is_some()
            {
                return Err(Fault::invalid(InvalidCode::Scope, at));
            }
        }
        Ok(replacements)
    }

    /// Commit the complete plan before resolving its resulting root handle.
    fn publish_substitution(
        &mut self,
        drafts: &[Draft],
        root: Reference,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let admitted = self.admit(drafts, budget)?;
        match root {
            Reference::Existing(handle) => Ok(handle),
            Reference::Local(index) => usize::try_from(index)
                .ok()
                .and_then(|index| admitted.get(index))
                .cloned()
                .ok_or_else(|| {
                    Fault::invalid(
                        InvalidCode::Reference,
                        Location::new("substitution publication"),
                    )
                }),
        }
    }

    /// Lift only references to the replacement's outer frames, preserving its own binders.
    fn lift_replacement(
        &self,
        root: LogicalId,
        initial: &Scope,
        target: &Scope,
        plan: &mut Plan,
        budget: &mut Budget,
    ) -> Result<Reference, Fault> {
        let at = Location::new("bound-reference lifting");
        self.prepay_substitution(budget, at)?;
        let added = target
            .len()
            .checked_sub(initial.len())
            .ok_or_else(|| Fault::invalid(InvalidCode::Scope, at))?;
        if target.get(..initial.len()) != Some(initial.as_slice()) {
            return Err(Fault::invalid(InvalidCode::Scope, at));
        }
        if added == 0 {
            return Ok(Reference::Existing(self.handle(root)));
        }
        let added = u32::try_from(added).map_err(|_| Fault::invalid(InvalidCode::Scope, at))?;
        let mut mapped = BTreeMap::new();
        let mut pending = vec![(root, false)];
        while let Some((id, expanded)) = pending.pop() {
            budget.work(
                size(mapped.len(), at)?
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?,
                at,
            )?;
            if mapped.contains_key(&id) {
                continue;
            }
            let value = self.state.expression(id)?;
            if let Operator::Rational(number) = &value.operator {
                number.check(budget, at)?;
            }
            if !expanded {
                pending
                    .try_reserve(
                        value
                            .operands
                            .len()
                            .checked_add(1)
                            .ok_or_else(|| Fault::capacity(BudgetKind::Vertices, at))?,
                    )
                    .map_err(|_| Fault::allocation(at))?;
                pending.push((id, true));
                pending.extend(value.operands.iter().rev().map(|child| (*child, false)));
                continue;
            }
            let suffix = value
                .scope
                .get(initial.len()..)
                .ok_or_else(|| Fault::invalid(InvalidCode::Scope, at))?;
            let mut scope = target.clone();
            scope
                .try_reserve(suffix.len())
                .map_err(|_| Fault::allocation(at))?;
            scope.extend_from_slice(suffix);
            budget.check(BudgetKind::Depth, size(scope.len(), at)?, at)?;
            let mut operator = value.operator.clone();
            if let Operator::Bound { depth, .. } = &mut operator
                && u64::from(*depth) >= size(suffix.len(), at)?
            {
                *depth = depth
                    .checked_add(added)
                    .ok_or_else(|| Fault::invalid(InvalidCode::Scope, at))?;
            }
            let operands = mapped_operands(&value.operands, &mapped, budget)?;
            let result = plan.push(
                ExpressionDraft {
                    operator,
                    sort: value.sort.clone(),
                    context: Reference::Existing(self.handle(value.context)),
                    scope,
                    binding: value.binding.clone(),
                    operands,
                },
                budget,
            )?;
            mapped.insert(id, result);
        }
        mapped
            .get(&root)
            .cloned()
            .ok_or_else(|| Fault::invalid(InvalidCode::Reference, at))
    }
}

/// Read only completed child mappings in deterministic operand order.
fn mapped_operands(
    operands: &[LogicalId],
    mapped: &BTreeMap<LogicalId, Reference>,
    budget: &mut Budget,
) -> Result<Vec<Reference>, Fault> {
    let at = Location::new("substitution operands");
    let charge = size(operands.len(), at)?
        .checked_mul(
            size(mapped.len(), at)?
                .checked_add(1)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?,
        )
        .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?;
    budget.work(charge, at)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(operands.len())
        .map_err(|_| Fault::allocation(at))?;
    for operand in operands {
        result.push(
            mapped
                .get(operand)
                .cloned()
                .ok_or_else(|| Fault::invalid(InvalidCode::Reference, at))?,
        );
    }
    Ok(result)
}
