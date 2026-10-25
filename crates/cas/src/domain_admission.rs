// SPDX-License-Identifier: AGPL-3.0-only
//! Kind, scope and header validation for exact domain syntax.

use crate::budget::{Budget, BudgetKind};
use crate::domain_model::{
    AmbientDomain, Boundary, BoundaryDraft, CaseArmDraft, Region, RegionDraft, RegionExpr, Space,
    SpaceDraft,
};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Edge, LogicalId, State, Vertex, position_u32, size, vertex_key};
use crate::model::{AtomicSort, Field, Scope, Sort};
use crate::source_model::{SourceForm, SourceFormDraft, SourceFormKind};

/// Location of exact domain admission errors.
const AT: Location = Location::new("domain admission");

/// Require exact ambient and scope agreement and an explicit header embedding.
pub(crate) fn region_context(
    state: &State,
    id: LogicalId,
    header: LogicalId,
    scope: &Scope,
    ambient: &AmbientDomain,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let value = state.region(id)?;
    if value.scope != *scope
        || value.ambient != *ambient
        || !crate::header::embeds(state, value.header, header, budget)?
    {
        return Err(Fault::invalid(InvalidCode::Context, AT));
    }
    Ok(())
}

/// Read the header and scope of an admitted Boolean expression or condition.
pub(crate) fn predicate_context(
    state: &State,
    id: LogicalId,
) -> Result<(LogicalId, &Scope), Fault> {
    match state.node(id)? {
        Vertex::Condition(value) => Ok((value.header, &value.scope)),
        Vertex::Expression(value) if value.sort == Sort::Value(AtomicSort::Boolean) => {
            Ok((value.header, &value.scope))
        }
        _ => Err(Fault::invalid(InvalidCode::Sort, AT)),
    }
}

/// Check that the ambient coordinate has the exact declared scalar category.
pub(crate) fn ambient_header(
    state: &State,
    header: LogicalId,
    ambient: &AmbientDomain,
) -> Result<(), Fault> {
    let environment = &state.header(header)?.environment;
    let (key, expected) = match ambient {
        AmbientDomain::Product { coordinates } => {
            let product = crate::solving::graph::coordinates(state, coordinates.logical_id())?;
            return if product.header.logical_id() == header {
                Ok(())
            } else {
                Err(Fault::invalid(InvalidCode::Scope, AT))
            };
        }
        AmbientDomain::Assignments => return Ok(()),
        AmbientDomain::Real(key) => (key, Some(Sort::Value(AtomicSort::Scalar(Field::Real)))),
        AmbientDomain::Integer(key) => (key, Some(Sort::Value(AtomicSort::Integer))),
        AmbientDomain::Opaque(key) => (key, None),
    };
    if !environment
        .declarations
        .iter()
        .any(|item| item.key == *key && expected.as_ref().is_none_or(|sort| item.sort == *sort))
    {
        return Err(Fault::invalid(InvalidCode::Scope, AT));
    }
    Ok(())
}

impl Cas {
    /// Resolve and intern an exact region without claiming its truth set is known.
    pub(crate) fn admit_region(
        &self,
        state: &mut State,
        draft: &RegionDraft,
        projection: Option<&crate::solving::LiesOver<crate::graph::Reference>>,
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        crate::validation::scope_limit(&draft.scope, budget)?;
        let header = self.resolve(state, &draft.header, locals)?;
        if let AmbientDomain::Product { coordinates } = &draft.ambient {
            self.resolve(
                state,
                &crate::graph::Reference::Existing(coordinates.clone()),
                locals,
            )?;
        }
        ambient_header(state, header, &draft.ambient)?;
        let mut edges = vec![(header, Edge::InHeader)];
        if let AmbientDomain::Product { coordinates } = &draft.ambient {
            edges.push((coordinates.logical_id(), Edge::Coordinates));
        }
        let expression = match &draft.expression {
            RegionExpr::Ambient => RegionExpr::Ambient,
            RegionExpr::Empty => RegionExpr::Empty,
            RegionExpr::GuardTrue { base, predicate }
            | RegionExpr::GuardFalse { base, predicate } => {
                let base = self.resolve(state, base, locals)?;
                region_context(state, base, header, &draft.scope, &draft.ambient, budget)?;
                let predicate = self.resolve(state, predicate, locals)?;
                let (actual, scope) = predicate_context(state, predicate)?;
                if scope != &draft.scope || !crate::header::embeds(state, actual, header, budget)? {
                    return Err(Fault::invalid(InvalidCode::Scope, AT));
                }
                edges.push((base, Edge::RegionInput(0)));
                edges.push((predicate, Edge::MembershipPredicate));
                if matches!(&draft.expression, RegionExpr::GuardTrue { .. }) {
                    RegionExpr::GuardTrue { base, predicate }
                } else {
                    RegionExpr::GuardFalse { base, predicate }
                }
            }
            RegionExpr::Meet(items) | RegionExpr::Union(items) => {
                let mut members = self.resolve_all(state, items, locals, budget)?;
                let count = size(members.len(), AT)?;
                budget.work(
                    count
                        .checked_mul(count)
                        .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
                    AT,
                )?;
                members.sort_unstable();
                members.dedup();
                for (position, member) in members.iter().enumerate() {
                    region_context(state, *member, header, &draft.scope, &draft.ambient, budget)?;
                    edges.push((*member, Edge::RegionInput(position_u32(position)?)));
                }
                if matches!(&draft.expression, RegionExpr::Meet(_)) {
                    RegionExpr::Meet(members)
                } else {
                    RegionExpr::Union(members)
                }
            }
            RegionExpr::Difference { base, excluded } => {
                let base = self.resolve(state, base, locals)?;
                let excluded = self.resolve(state, excluded, locals)?;
                for id in [base, excluded] {
                    region_context(state, id, header, &draft.scope, &draft.ambient, budget)?;
                }
                edges.push((base, Edge::RegionInput(0)));
                edges.push((excluded, Edge::RegionInput(1)));
                RegionExpr::Difference { base, excluded }
            }
        };
        let lies_over = crate::solving::sorts::projection(
            self,
            state,
            projection,
            header,
            &draft.ambient,
            locals,
            &mut edges,
            budget,
        )?;
        let value = Region {
            lies_over,
            header,
            scope: draft.scope.clone(),
            ambient: draft.ambient.clone(),
            expression,
        };
        self.retain_region(state, value, &edges, pending, budget)
    }

    /// Charge a new region exactly once, including regions introduced by local frames.
    pub(crate) fn retain_region(
        &self,
        state: &mut State,
        region: Region,
        edges: &[(LogicalId, Edge)],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let vertex = Vertex::Region(region);
        let (_, key) = vertex_key(&vertex, state, budget)?;
        if state.interned(&key, budget)?.is_none() {
            budget.specialized(BudgetKind::RegionNodes, 1, true, AT)?;
        }
        self.insert(state, vertex, edges, pending, budget)
    }

    /// Admit a declared real test space without granting analytic premises.
    pub(crate) fn admit_space(
        &self,
        state: &mut State,
        draft: &SpaceDraft,
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let header = self.resolve(state, &draft.header, locals)?;
        let domain = self.resolve(state, &draft.domain, locals)?;
        region_context(
            state,
            domain,
            header,
            &Vec::new(),
            &AmbientDomain::Real(draft.coordinate.clone()),
            budget,
        )?;
        if state.header(header)?.parent.is_some()
            || draft.field != Field::Real
            || draft.test_family != "CompactSmooth"
        {
            return Err(Fault::invalid(InvalidCode::Sort, AT));
        }
        crate::region::open_interval(state, domain, budget)?;
        let value = Space {
            header,
            domain,
            coordinate: draft.coordinate.clone(),
            field: draft.field.clone(),
            test_family: draft.test_family.clone(),
        };
        self.insert(
            state,
            Vertex::Space(value),
            &[(header, Edge::InHeader), (domain, Edge::SpaceDomain)],
            pending,
            budget,
        )
    }

    /// Admit a scalar association; overlap agreement belongs to the kernel.
    pub(crate) fn admit_arm(
        &self,
        state: &mut State,
        draft: &CaseArmDraft,
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let header = self.resolve(state, &draft.header, locals)?;
        let region = self.resolve(state, &draft.region, locals)?;
        let value = self.resolve(state, &draft.value, locals)?;
        let arm = crate::solving::sorts::arm(
            self,
            state,
            draft,
            (header, region, value),
            locals,
            budget,
        )?;
        let mut edges = vec![
            (header, Edge::InHeader),
            (region, Edge::ArmRegion),
            (value, Edge::ArmValue),
        ];
        for (position, pair) in arm.placements.iter().enumerate() {
            let position = position_u32(position)?;
            edges.push((
                pair.operand,
                Edge::Scalar(crate::graph::ScalarEdge::PlacedOperand(position)),
            ));
            edges.push((
                pair.piece,
                Edge::Scalar(crate::graph::ScalarEdge::PlacementPiece(position)),
            ));
        }
        self.insert(state, Vertex::CaseArm(arm), &edges, pending, budget)
    }

    /// Admit only a location, never function-specific ownership or jets.
    pub(crate) fn admit_boundary(
        &self,
        state: &mut State,
        draft: &BoundaryDraft,
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let header = self.resolve(state, &draft.header, locals)?;
        ambient_header(state, header, &draft.ambient)?;
        let location = self.resolve(state, &draft.location, locals)?;
        let point = state.expression(location)?;
        let expected = match &draft.ambient {
            AmbientDomain::Real(_) => Sort::Value(AtomicSort::Scalar(Field::Real)),
            AmbientDomain::Integer(_) => Sort::Value(AtomicSort::Integer),
            _ => return Err(Fault::invalid(InvalidCode::UnsupportedFragment, AT)),
        };
        if point.sort != expected
            || point.scope != draft.scope
            || !crate::header::embeds(state, point.header, header, budget)?
        {
            return Err(Fault::invalid(InvalidCode::Sort, AT));
        }
        let boundary = Boundary {
            header,
            scope: draft.scope.clone(),
            ambient: draft.ambient.clone(),
            location,
        };
        let vertex = Vertex::Boundary(boundary);
        let (_, key) = vertex_key(&vertex, state, budget)?;
        if state.interned(&key, budget)?.is_none() {
            budget.specialized(BudgetKind::BoundaryNodes, 1, true, AT)?;
        }
        self.insert(
            state,
            vertex,
            &[(header, Edge::InHeader), (location, Edge::BoundaryLocation)],
            pending,
            budget,
        )
    }

    /// Retain all original clauses in order, without creating an occurrence receipt.
    pub(crate) fn admit_source_form(
        &self,
        state: &mut State,
        draft: &SourceFormDraft,
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        budget.specialized(
            BudgetKind::CaseArms,
            size(draft.clauses.len(), AT)?,
            false,
            AT,
        )?;
        let header = self.resolve(state, &draft.header, locals)?;
        let domain = self.resolve(state, &draft.domain, locals)?;
        let region = state.region(domain)?;
        region_context(state, domain, header, &draft.scope, &region.ambient, budget)?;
        let mut clauses = Vec::new();
        clauses
            .try_reserve_exact(draft.clauses.len())
            .map_err(|_| Fault::allocation(AT))?;
        let mut values = Vec::new();
        let mut edges = vec![(header, Edge::InHeader), (domain, Edge::CaseDomain)];
        for clause in &draft.clauses {
            let selector = self.resolve(state, &clause.selector, locals)?;
            let value = self.resolve(state, &clause.value, locals)?;
            if draft.kind == SourceFormKind::OrderedCases {
                let (actual, scope) = predicate_context(state, selector)?;
                if scope != &draft.scope || !crate::header::embeds(state, actual, header, budget)? {
                    return Err(Fault::invalid(InvalidCode::Scope, AT));
                }
            } else {
                region_context(
                    state,
                    selector,
                    header,
                    &draft.scope,
                    &region.ambient,
                    budget,
                )?;
            }
            for id in [selector, value] {
                let position = edges
                    .len()
                    .checked_sub(2)
                    .ok_or_else(|| Fault::capacity(BudgetKind::Edges, AT))?;
                edges.push((id, Edge::SourceArgument(position_u32(position)?)));
            }
            clauses.push((selector, value));
            values.push(value);
        }
        let otherwise = draft
            .otherwise
            .as_ref()
            .map(|item| self.resolve(state, item, locals))
            .transpose()?;
        if let Some(value) = otherwise {
            if draft.kind == SourceFormKind::CaseLiteral {
                return Err(Fault::invalid(InvalidCode::Sort, AT));
            }
            values.push(value);
            let position = edges
                .len()
                .checked_sub(2)
                .ok_or_else(|| Fault::capacity(BudgetKind::Edges, AT))?;
            edges.push((value, Edge::SourceArgument(position_u32(position)?)));
        }
        check_values(state, header, &draft.scope, &values, budget)?;
        self.insert(
            state,
            Vertex::SourceForm(SourceForm {
                header,
                scope: draft.scope.clone(),
                kind: draft.kind,
                domain,
                clauses,
                otherwise,
            }),
            &edges,
            pending,
            budget,
        )
    }
}

/// Require a nonempty set of same-sort scalar values under the actual source header.
fn check_values(
    state: &State,
    header: LogicalId,
    scope: &Scope,
    values: &[LogicalId],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let first = values
        .first()
        .copied()
        .ok_or_else(|| Fault::invalid(InvalidCode::Sort, AT))?;
    let sort = &state.expression(first)?.sort;
    if !matches!(sort, Sort::Value(AtomicSort::Scalar(_))) {
        return Err(Fault::invalid(InvalidCode::Sort, AT));
    }
    for id in values {
        let term = state.expression(*id)?;
        if &term.sort != sort
            || &term.scope != scope
            || !crate::header::embeds(state, term.header, header, budget)?
        {
            return Err(Fault::invalid(InvalidCode::Sort, AT));
        }
    }
    Ok(())
}

impl Cas {
    /// Resolve and canonicalize the finite unordered arm inventory under its own work charge.
    fn case_arms(
        &self,
        state: &State,
        draft: &crate::domain_model::CasesDraft,
        locals: &[LogicalId],
        budget: &mut Budget,
    ) -> Result<Vec<LogicalId>, Fault> {
        let at = Location::new("native case admission");
        let mut arms = self.resolve_all(state, &draft.arms, locals, budget)?;
        budget.work(
            size(arms.len(), at)?
                .checked_mul(size(arms.len(), at)?)
                .ok_or_else(|| Fault::capacity(BudgetKind::Work, at))?,
            at,
        )?;
        arms.sort_unstable();
        arms.dedup();
        Ok(arms)
    }

    /// Admit only well-scoped native arm syntax; totality and overlap remain proof obligations.
    pub(crate) fn admit_cases(
        &self,
        state: &mut State,
        draft: &crate::domain_model::CasesDraft,
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        let at = Location::new("native case admission");
        budget.specialized(BudgetKind::CaseArms, size(draft.arms.len(), at)?, false, at)?;
        let header = self.resolve(state, &draft.header, locals)?;
        state.header(header)?;
        crate::validation::scope_limit(&draft.scope, budget)?;
        let domain = self.resolve(state, &draft.domain, locals)?;
        let region = state.region(domain)?;
        region_context(state, domain, header, &draft.scope, &region.ambient, budget)?;
        let arms = self.case_arms(state, draft, locals, budget)?;
        if arms.is_empty() {
            return Err(Fault::invalid(InvalidCode::Sort, at));
        }
        let sort = draft.sort.clone();
        let (domain_depth, domain_size) = crate::descriptor::measure(state, domain, budget)?;
        let mut depth = domain_depth
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::Depth, at))?;
        let mut unfolded = domain_size
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::PeakExpressionSize, at))?;
        let mut edges = vec![(header, Edge::InHeader), (domain, Edge::CaseDomain)];
        edges
            .try_reserve(arms.len())
            .map_err(|_| Fault::allocation(at))?;
        for (position, arm) in arms.iter().enumerate() {
            let value = state.arm(*arm)?;
            if value.sort != sort {
                return Err(Fault::invalid(InvalidCode::SymbolSortMismatch, at));
            }
            if value.scope != draft.scope
                || !crate::header::embeds(state, value.header, header, budget)?
            {
                return Err(Fault::invalid(InvalidCode::Sort, at));
            }
            region_context(
                state,
                value.region,
                header,
                &draft.scope,
                &region.ambient,
                budget,
            )?;
            let (arm_depth, arm_size) = crate::descriptor::measure(state, *arm, budget)?;
            depth = depth.max(
                arm_depth
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::Depth, at))?,
            );
            unfolded = unfolded
                .checked_add(arm_size)
                .ok_or_else(|| Fault::capacity(BudgetKind::PeakExpressionSize, at))?;
            edges.push((
                *arm,
                Edge::CaseMember(
                    u32::try_from(position).map_err(|_| Fault::capacity(BudgetKind::Edges, at))?,
                ),
            ));
        }
        budget.check(BudgetKind::Depth, depth, at)?;
        budget.check(BudgetKind::PeakExpressionSize, unfolded, at)?;
        let ambient = crate::solving::sorts::ambient(state, header, &sort)?;
        if let Some(ambient) = ambient {
            edges.push((
                ambient,
                Edge::Scalar(crate::graph::ScalarEdge::CheckedAmbient),
            ));
        }
        self.insert(
            state,
            Vertex::Expression(crate::graph::Expression {
                ambient,
                attachment: None,
                header,
                scope: draft.scope.clone(),
                sort,
                operator: crate::model::Operator::Cases,
                binding: None,
                operands: Vec::new(),
                layer: crate::domain_model::Layer::Pointwise,
                space: None,
                domain: Some(domain),
                arms,
                depth,
                unfolded,
            }),
            &edges,
            pending,
            budget,
        )
    }
}
