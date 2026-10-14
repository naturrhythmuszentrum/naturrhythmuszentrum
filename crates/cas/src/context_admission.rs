// SPDX-License-Identifier: AGPL-3.0-only
//! Atomic root and local frame admission with explicit restriction provenance.

use std::collections::{BTreeMap, BTreeSet};

use crate::budget::{Budget, BudgetKind};
use crate::domain_model::{ContextDraft, Region, RegionExpr};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{
    AssumptionDraft, Cas, Context, Edge, LogicalId, State, Vertex, position_u32, size, vertex_key,
};

/// Context admission and scoped ancestry location.
const AT: Location = Location::new("context frame");

impl Cas {
    /// Build a root frame or an exact local restriction in the candidate graph.
    pub(crate) fn admit_context_frame(
        &self,
        state: &mut State,
        draft: &ContextDraft,
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let value = match draft {
            ContextDraft::Root {
                header,
                admissible,
                assumptions,
            } => {
                let header_id = self.resolve(header, locals)?;
                let admissible = self.resolve(admissible, locals)?;
                let region = state.region(admissible)?;
                if !region.scope.is_empty()
                    || !crate::header::embeds(state, region.header, header_id, budget)?
                {
                    return Err(Fault::invalid(InvalidCode::Context, AT));
                }
                let declared =
                    self.frame_predicates(state, header_id, assumptions, locals, budget)?;
                let assumptions: Vec<_> = declared
                    .into_iter()
                    .collect::<BTreeMap<_, _>>()
                    .into_iter()
                    .collect();
                let canonical_assumptions = crate::context::canonical(state, &assumptions, budget)?;
                let header = state.header(header_id)?;
                Context {
                    header_id,
                    admissible,
                    parent: None,
                    restriction: None,
                    frame: Vec::new(),
                    depth: 0,
                    environment: header.environment.clone(),
                    header: header.bytes.clone(),
                    assumptions,
                    canonical_assumptions,
                }
            }
            ContextDraft::Local {
                parent,
                region,
                hypotheses,
            } => {
                let parent = self.resolve(parent, locals)?;
                let region = self.resolve(region, locals)?;
                self.local_context(state, parent, region, hypotheses, locals, pending, budget)?
            }
        };
        let mut edges = vec![
            (value.header_id, Edge::InHeader),
            (value.admissible, Edge::AdmissibleRegion),
        ];
        if let Some(parent) = value.parent {
            edges.push((parent, Edge::ParentContext));
        }
        let hypotheses = if value.parent.is_some() {
            &value.frame
        } else {
            &value.assumptions
        };
        for (position, (_, condition)) in hypotheses.iter().enumerate() {
            let position = position_u32(position)?;
            let slot = if value.parent.is_some() {
                position
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::ConditionAtoms, AT))?
            } else {
                position
            };
            edges.push((*condition, Edge::Assumption(slot)));
        }
        let local = value.parent.is_some();
        let vertex = Vertex::Context(value);
        if local {
            let (_, key) = vertex_key(&vertex, state, budget)?;
            if !state.intern.contains_key(&key) {
                budget.specialized(BudgetKind::LocalContexts, 1, true, AT)?;
            }
        }
        self.insert(state, vertex, &edges, pending, budget)
    }

    /// Validate predicate slots before copying them, without reordering local frames.
    fn frame_predicates(
        &self,
        state: &State,
        header: LogicalId,
        drafts: &[AssumptionDraft],
        locals: &[LogicalId],
        budget: &mut Budget,
    ) -> Result<Vec<(String, LogicalId)>, Fault> {
        budget.check(BudgetKind::ConditionAtoms, size(drafts.len(), AT)?, AT)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(drafts.len())
            .map_err(|_| Fault::allocation(AT))?;
        let mut names = BTreeSet::new();
        for draft in drafts {
            budget.condition(size(draft.key.len(), AT)?, AT)?;
            if draft.key.is_empty() || !names.insert(draft.key.clone()) {
                return Err(Fault::invalid(InvalidCode::Context, AT));
            }
            let id = self.resolve(&draft.condition, locals)?;
            let Vertex::Condition(value) = state.node(id)? else {
                return Err(Fault::invalid(InvalidCode::Reference, AT));
            };
            if !value.scope.is_empty()
                || !crate::header::embeds(state, value.header, header, budget)?
            {
                return Err(Fault::invalid(InvalidCode::Scope, AT));
            }
            result.push((draft.key.clone(), id));
        }
        Ok(result)
    }

    /// Retain the exact meet and reject recognized contradictions along the ancestry.
    #[expect(
        clippy::too_many_arguments,
        reason = "all transaction resources are explicit"
    )]
    fn local_context(
        &self,
        state: &mut State,
        parent: LogicalId,
        restriction: LogicalId,
        drafts: &[AssumptionDraft],
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<Context, Fault> {
        let enclosing = state.context(parent)?.clone();
        let depth = enclosing
            .depth
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::HypothesisDepth, AT))?;
        budget.specialized(BudgetKind::HypothesisDepth, depth, false, AT)?;
        let base = state.region(enclosing.admissible)?.clone();
        crate::domain_admission::region_context(
            state,
            restriction,
            enclosing.header_id,
            &base.scope,
            &base.ambient,
            budget,
        )?;
        let frame = self.frame_predicates(state, enclosing.header_id, drafts, locals, budget)?;
        let mut hypotheses = enclosing.assumptions.clone();
        let mut ancestor = Some(parent);
        while let Some(id) = ancestor {
            budget.condition(1, AT)?;
            let context = state.context(id)?;
            hypotheses
                .try_reserve(context.frame.len())
                .map_err(|_| Fault::allocation(AT))?;
            hypotheses.extend_from_slice(&context.frame);
            ancestor = context.parent;
        }
        hypotheses
            .try_reserve(frame.len())
            .map_err(|_| Fault::allocation(AT))?;
        hypotheses.extend_from_slice(&frame);
        let canonical_assumptions = crate::context::canonical(state, &hypotheses, budget)?;
        let mut members = vec![enclosing.admissible, restriction];
        members.sort_unstable();
        members.dedup();
        let mut edges = vec![(enclosing.header_id, Edge::InHeader)];
        for (position, member) in members.iter().enumerate() {
            edges.push((*member, Edge::RegionInput(position_u32(position)?)));
        }
        let admissible = self.retain_region(
            state,
            Region {
                header: enclosing.header_id,
                scope: base.scope,
                ambient: base.ambient,
                expression: RegionExpr::Meet(members),
            },
            &edges,
            pending,
            budget,
        )?;
        Ok(Context {
            header_id: enclosing.header_id,
            admissible,
            parent: Some(parent),
            restriction: Some(restriction),
            frame,
            depth,
            environment: enclosing.environment,
            header: enclosing.header,
            assumptions: enclosing.assumptions,
            canonical_assumptions,
        })
    }

    /// Read the exact admissible domain of a checked context.
    ///
    /// # Errors
    /// Refuses a foreign handle or a handle of another kind.
    pub fn admissible_region(
        &self,
        context: &crate::graph::Handle,
    ) -> Result<crate::graph::Handle, Fault> {
        Ok(self.handle(self.state.context(self.checked(context)?)?.admissible))
    }
}

impl Cas {
    /// Inspect the provenance name of a qualified hypothesis.
    ///
    /// The implicit local membership binder has no user-supplied name.
    ///
    /// # Errors
    /// Refuses foreign contexts, incorrect context kinds and out-of-range slots.
    pub fn hypothesis_name(
        &self,
        reference: &crate::domain_model::HypothesisRef,
    ) -> Result<Option<&str>, Fault> {
        let frame = self.state.context(self.checked(&reference.context)?)?;
        let slot = usize::try_from(reference.slot)
            .map_err(|_| Fault::invalid(InvalidCode::Reference, AT))?;
        let (entries, slot) = if frame.parent.is_some() {
            let Some(slot) = slot.checked_sub(1) else {
                return Ok(None);
            };
            (&frame.frame, slot)
        } else {
            (&frame.assumptions, slot)
        };
        let entry = entries
            .get(slot)
            .ok_or_else(|| Fault::invalid(InvalidCode::Reference, AT))?;
        Ok(Some(entry.0.as_str()))
    }
}
