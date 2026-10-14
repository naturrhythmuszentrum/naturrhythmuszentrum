// SPDX-License-Identifier: AGPL-3.0-only
//! Simultaneous capture-safe transformation of complete typed syntax dependencies.

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Draft, Handle, LogicalId, Reference, Vertex, size};
use crate::model::{Operator, Scope, Sort};
use std::collections::BTreeMap;

/// A declared free key and the exact expression inserted for it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Substitution {
    /// Declared free-variable identity, never a binder display name.
    pub key: String,
    /// Replacement expression in the original root scope and semantic header.
    pub replacement: Handle,
}

/// Transformation accounting location.
const AT: Location = Location::new("typed substitution");
/// Memo identity includes kind, immutable original scope and lifting depth.
type MemoKey = (LogicalId, Scope, u32);
/// Completed transformations from one fixed substitution environment.
type Mappings = BTreeMap<MemoKey, Reference>;

/// One bounded transformation environment; lifting never re-applies free replacements.
enum Mode {
    /// Simultaneous replacement of declared free variables.
    Free {
        /// Original scope of every replacement.
        initial: Scope,
        /// Exact simultaneous replacements.
        replacements: BTreeMap<String, LogicalId>,
    },
    /// Remove one explicit frame while keeping nested frames local.
    Bound {
        /// Scope outside the removed frame.
        outer: Scope,
        /// Replacements in slot order.
        replacements: Vec<LogicalId>,
    },
    /// Insert frames between the caller scope and a replacement's own binders.
    Lift {
        /// Scope outside the replacement's local binders.
        initial: Scope,
        /// Scope including the inserted frames.
        target: Scope,
    },
}

/// Complete transaction plan, shared with nested replacement lifting.
struct Plan {
    /// Child-before-parent typed drafts.
    drafts: Vec<Draft>,
    /// Retained draft payload, charged before insertion.
    bytes: u64,
    /// Ordinary substitution must preserve each exact space identity.
    spaces: Vec<(LogicalId, Reference)>,
}

impl Plan {
    /// Append one metered typed draft, returning only a backward local reference.
    fn push(&mut self, node: Draft, budget: &mut Budget) -> Result<Reference, Fault> {
        let bytes = crate::validation::draft_payload(std::slice::from_ref(&node), budget)?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, self.bytes, AT)?;
        let index = u32::try_from(self.drafts.len())
            .map_err(|_| Fault::capacity(BudgetKind::StagedVertices, AT))?;
        budget.check(
            BudgetKind::StagedVertices,
            u64::from(index).saturating_add(1),
            AT,
        )?;
        self.drafts
            .try_reserve(1)
            .map_err(|_| Fault::allocation(AT))?;
        self.drafts.push(node);
        Ok(Reference::Local(index))
    }
}

impl Cas {
    /// Simultaneously substitute every reachable free term position, preserving semantic axes.
    ///
    /// # Errors
    /// Refuses foreign identities, duplicate keys, incompatible scopes/sorts and spent limits.
    /// Inserted replacements are not substituted again; failure publishes no prefix.
    pub fn substitute(
        &mut self,
        root: &Handle,
        substitutions: &[Substitution],
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        self.prepay_transform(budget)?;
        let root = self.checked(root)?;
        let initial = crate::descriptor::metadata(&self.state, root)?.1.clone();
        let replacements = self.free_replacements(root, substitutions, budget)?;
        self.transform_root(
            root,
            &Mode::Free {
                initial,
                replacements,
            },
            budget,
        )
    }

    /// Instantiate the root's innermost frame throughout its typed dependency graph.
    ///
    /// # Errors
    /// Refuses missing frames, wrong slot counts/sorts, invalid spaces or exhausted limits.
    pub fn instantiate_bound(
        &mut self,
        root: &Handle,
        replacements: &[Handle],
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        self.prepay_transform(budget)?;
        let root = self.checked(root)?;
        let (outer, replacements) = self.bound_replacements(root, replacements, budget)?;
        self.transform_root(
            root,
            &Mode::Bound {
                outer,
                replacements,
            },
            budget,
        )
    }

    /// Reserve graph-owned payload before any descriptor or scope cloning.
    fn prepay_transform(&self, budget: &mut Budget) -> Result<(), Fault> {
        let bytes = self
            .counts()
            .payload_bytes
            .checked_mul(4)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        budget.work(bytes, AT)
    }

    /// Publish all rebuilt syntax only after the complete dependency walk succeeds.
    fn transform_root(
        &mut self,
        root: LogicalId,
        mode: &Mode,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let mut plan = Plan {
            drafts: Vec::new(),
            bytes: 0,
            spaces: Vec::new(),
        };
        let output = self.transform(root, mode, &mut plan, budget)?;
        let mut candidate = self.fork(budget)?;
        let admitted = candidate.admit(&plan.drafts, budget)?;
        for (original, transformed) in &plan.spaces {
            if resolve(transformed, &admitted)?.logical_id() != *original {
                return Err(invalid());
            }
        }
        let result = resolve(&output, &admitted)?;
        self.state = candidate.state;
        Ok(result)
    }

    /// Transform a DAG iteratively; nested calls only lift replacements without substitution.
    fn transform(
        &self,
        root: LogicalId,
        mode: &Mode,
        plan: &mut Plan,
        budget: &mut Budget,
    ) -> Result<Reference, Fault> {
        let mut mapped = Mappings::new();
        let mut pending = vec![(root, false)];
        while let Some((id, expanded)) = pending.pop() {
            budget.work(size(mapped.len(), AT)?.saturating_add(1), AT)?;
            let key = memo_key(self, id, mode)?;
            if mapped.contains_key(&key) {
                continue;
            }
            if let Some(output) = self.replacement(id, mode, plan, budget)? {
                mapped.insert(key, output);
                continue;
            }
            let children = crate::descriptor::children(&self.state, id, budget)?;
            if !expanded {
                pending
                    .try_reserve(children.len().saturating_add(1))
                    .map_err(|_| Fault::allocation(AT))?;
                pending.push((id, true));
                pending.extend(children.into_iter().rev().map(|(_, child)| (child, false)));
                continue;
            }
            let scope = mode.scope(crate::descriptor::metadata(&self.state, id)?.1)?;
            let operator = self.transformed_operator(id, mode)?;
            let mut resolve = |child| {
                mapped
                    .get(&memo_key(self, child, mode)?)
                    .cloned()
                    .ok_or_else(invalid)
            };
            let draft =
                crate::substitution_draft::rebuild(self, id, scope, operator, &mut resolve)?;
            let output = plan.push(draft, budget)?;
            if matches!(self.state.node(id)?, Vertex::Space(_)) {
                plan.spaces
                    .try_reserve(1)
                    .map_err(|_| Fault::allocation(AT))?;
                plan.spaces.push((id, output.clone()));
            }
            mapped.insert(key, output);
        }
        mapped
            .get(&memo_key(self, root, mode)?)
            .cloned()
            .ok_or_else(invalid)
    }

    /// Resolve a simultaneous replacement before traversing the replaced node's children.
    fn replacement(
        &self,
        id: LogicalId,
        mode: &Mode,
        plan: &mut Plan,
        budget: &mut Budget,
    ) -> Result<Option<Reference>, Fault> {
        let (_, scope) = crate::descriptor::metadata(&self.state, id)?;
        if let (Vertex::Space(space), Mode::Free { replacements, .. }) =
            (self.state.node(id)?, mode)
            && let Some(replacement) = replacements.get(&space.coordinate)
            && self.state.expression(*replacement)?.operator
                != Operator::Free(space.coordinate.clone())
        {
            return Err(invalid());
        }
        if matches!(self.state.node(id)?, Vertex::Space(_)) && !matches!(mode, Mode::Free { .. }) {
            return Ok(Some(Reference::Existing(self.handle(id))));
        }
        let Vertex::Expression(value) = self.state.node(id)? else {
            return Ok(None);
        };
        if let Operator::Rational(number) = &value.operator {
            number.check(budget, AT)?;
        }
        let replacement = match (mode, &value.operator) {
            (
                Mode::Free {
                    initial,
                    replacements,
                },
                Operator::Free(key),
            ) => replacements.get(key).map(|id| (*id, initial)),
            (
                Mode::Bound {
                    outer,
                    replacements,
                },
                Operator::Bound { depth, slot },
            ) if *depth == mode.depth(scope)? => {
                let index = usize::try_from(*slot).map_err(|_| invalid())?;
                Some((*replacements.get(index).ok_or_else(invalid)?, outer))
            }
            _ => None,
        };
        let Some((replacement, initial)) = replacement else {
            return Ok(None);
        };
        let target = mode.scope(scope)?;
        if *initial == target {
            return Ok(Some(Reference::Existing(self.handle(replacement))));
        }
        let lift = Mode::Lift {
            initial: initial.clone(),
            target,
        };
        self.transform(replacement, &lift, plan, budget).map(Some)
    }

    /// Shift only references outside the preserved nested binder suffix.
    fn transformed_operator(&self, id: LogicalId, mode: &Mode) -> Result<Option<Operator>, Fault> {
        let Vertex::Expression(value) = self.state.node(id)? else {
            return Ok(None);
        };
        let mut operator = value.operator.clone();
        if let Operator::Bound { depth, .. } = &mut operator {
            let nested = mode.depth(&value.scope)?;
            match mode {
                Mode::Bound { .. } if *depth > nested => {
                    *depth = depth.checked_sub(1).ok_or_else(invalid)?;
                }
                Mode::Lift { initial, target } if *depth >= nested => {
                    let added = target
                        .len()
                        .checked_sub(initial.len())
                        .ok_or_else(invalid)?;
                    *depth = depth
                        .checked_add(u32::try_from(added).map_err(|_| invalid())?)
                        .ok_or_else(invalid)?;
                }
                _ => {}
            }
        }
        Ok(Some(operator))
    }

    /// Check the simultaneous map against the exact header and original root scope.
    fn free_replacements(
        &self,
        root: LogicalId,
        substitutions: &[Substitution],
        budget: &mut Budget,
    ) -> Result<BTreeMap<String, LogicalId>, Fault> {
        let (header_id, scope) = crate::descriptor::metadata(&self.state, root)?;
        let header = self.state.header(header_id)?;
        let mut result = BTreeMap::new();
        for substitution in substitutions {
            budget.work(size(header.bytes.len(), AT)?, AT)?;
            let id = self.checked(&substitution.replacement)?;
            let value = self.state.expression(id)?;
            let declared = header
                .environment
                .declarations
                .iter()
                .find(|declaration| declaration.key == substitution.key)
                .ok_or_else(invalid)?;
            if value.scope != *scope
                || value.sort != declared.sort
                || value.header != header_id
                || value.space
                    != crate::header::expression_space(
                        &self.state,
                        header_id,
                        &Operator::Free(substitution.key.clone()),
                        &declared.sort,
                        budget,
                    )?
                || result.insert(substitution.key.clone(), id).is_some()
            {
                return Err(invalid());
            }
        }
        Ok(result)
    }

    /// Validate every removed slot before any transformed draft is staged.
    fn bound_replacements(
        &self,
        root: LogicalId,
        replacements: &[Handle],
        budget: &mut Budget,
    ) -> Result<(Scope, Vec<LogicalId>), Fault> {
        let (header, scope) = crate::descriptor::metadata(&self.state, root)?;
        let frame = scope.last().ok_or_else(invalid)?;
        if frame.len() != replacements.len() {
            return Err(invalid());
        }
        let length = scope.len().checked_sub(1).ok_or_else(invalid)?;
        let outer = scope.get(..length).ok_or_else(invalid)?.to_vec();
        let header_bytes = &self.state.header(header)?.bytes;
        let mut result = Vec::new();
        result
            .try_reserve_exact(replacements.len())
            .map_err(|_| Fault::allocation(AT))?;
        for (handle, sort) in replacements.iter().zip(frame) {
            budget.work(size(header_bytes.len(), AT)?, AT)?;
            let id = self.checked(handle)?;
            let value = self.state.expression(id)?;
            if value.scope != outer
                || value.sort != Sort::Value(sort.clone())
                || value.header != header
            {
                return Err(invalid());
            }
            result.push(id);
        }
        Ok((outer, result))
    }
}

impl Mode {
    /// Count local binders inside the root frame.
    fn depth(&self, scope: &Scope) -> Result<u32, Fault> {
        let prefix = match self {
            Self::Free { initial, .. } | Self::Lift { initial, .. } => initial.len(),
            Self::Bound { outer, .. } => outer.len().checked_add(1).ok_or_else(invalid)?,
        };
        u32::try_from(scope.len().checked_sub(prefix).ok_or_else(invalid)?).map_err(|_| invalid())
    }

    /// Reconstruct each explicit scope with the same nested binder suffix.
    fn scope(&self, original: &Scope) -> Result<Scope, Fault> {
        let (initial, target, prefix) = match self {
            Self::Free { .. } => return Ok(original.clone()),
            Self::Bound { outer, .. } => (
                outer,
                outer,
                outer.len().checked_add(1).ok_or_else(invalid)?,
            ),
            Self::Lift { initial, target } => (initial, target, initial.len()),
        };
        if original.get(..initial.len()) != Some(initial.as_slice())
            || target.get(..initial.len()) != Some(initial.as_slice())
        {
            return Err(invalid());
        }
        let suffix = original.get(prefix..).ok_or_else(invalid)?;
        let mut scope = target.clone();
        scope
            .try_reserve(suffix.len())
            .map_err(|_| Fault::allocation(AT))?;
        scope.extend_from_slice(suffix);
        Ok(scope)
    }
}

/// Memoize kind, original scope and lifting depth under one immutable environment.
fn memo_key(cas: &Cas, id: LogicalId, mode: &Mode) -> Result<MemoKey, Fault> {
    let scope = crate::descriptor::metadata(&cas.state, id)?.1;
    let depth =
        if matches!(cas.state.node(id)?, Vertex::Space(_)) || matches!(mode, Mode::Free { .. }) {
            0
        } else {
            mode.depth(scope)?
        };
    Ok((id, scope.clone(), depth))
}

/// Ill-scoped transformations cannot become published syntax.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Scope, AT)
}

/// Resolve a staged output only after complete transactional admission.
fn resolve(reference: &Reference, admitted: &[Handle]) -> Result<Handle, Fault> {
    match reference {
        Reference::Existing(handle) => Ok(handle.clone()),
        Reference::Local(index) => usize::try_from(*index)
            .ok()
            .and_then(|index| admitted.get(index))
            .cloned()
            .ok_or_else(invalid),
    }
}

/// Frame transformations exercise private scope arithmetic without a fixture harness.
#[cfg(test)]
mod tests {
    use super::Mode;
    use crate::Fault;
    use crate::model::{AtomicSort, Field};

    /// Removing the caller frame preserves the nested Boolean binder and outer real frame.
    /// ´test:unit:typed-bound-frame-removal´
    #[test]
    fn typed_bound_frame_removal() -> Result<(), Fault> {
        let real = vec![AtomicSort::Scalar(Field::Real)];
        let boolean = vec![AtomicSort::Boolean];
        let mode = Mode::Bound {
            outer: vec![real.clone()],
            replacements: Vec::new(),
        };
        let scope = vec![real.clone(), vec![AtomicSort::Integer], boolean.clone()];
        assert_eq!(mode.depth(&scope)?, 1);
        assert_eq!(mode.scope(&scope)?, vec![real, boolean]);
        Ok(())
    }

    /// Lifting inserts frames before a replacement's local binders and rejects changed prefixes.
    /// ´test:unit:typed-replacement-scope-lifting´
    #[test]
    fn typed_replacement_scope_lifting() -> Result<(), Fault> {
        let real = vec![AtomicSort::Scalar(Field::Real)];
        let integer = vec![AtomicSort::Integer];
        let boolean = vec![AtomicSort::Boolean];
        let mode = Mode::Lift {
            initial: vec![real.clone()],
            target: vec![real.clone(), integer.clone()],
        };
        assert_eq!(
            mode.scope(&vec![real.clone(), boolean.clone()])?,
            vec![real, integer, boolean]
        );
        assert!(mode.scope(&vec![vec![AtomicSort::Integer]]).is_err());
        Ok(())
    }
}
