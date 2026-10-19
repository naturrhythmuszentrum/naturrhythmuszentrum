// SPDX-License-Identifier: AGPL-3.0-only
//! Exact bijective coordinate renaming with explicit header and space identities.

use crate::budget::{Budget, BudgetKind};
use crate::domain_model::AmbientDomain;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Draft, Handle, LogicalId, Reference, Vertex, size};
use crate::model::{AtomicSort, Extent, Operator, Scope, Sort};
use std::collections::{BTreeMap, BTreeSet};

/// An exact renaming, never a general coordinate pullback or a theorem transport.
#[derive(Clone, Debug, Default)]
pub struct AxisMap {
    /// Coordinate declaration keys, with distinct sources and distinct targets.
    pub coordinates: Vec<(String, String)>,
    /// Every reached source header and its exact renamed target header.
    pub headers: Vec<(Handle, Handle)>,
    /// Every reached source space and its exact renamed target space.
    pub spaces: Vec<(Handle, Handle)>,
}

/// Validated explicit identities used by one atomic transformation.
struct Mapping {
    /// Scalar coordinate key bijection.
    coordinates: BTreeMap<String, String>,
    /// Exact source-to-target header identities.
    headers: BTreeMap<LogicalId, LogicalId>,
    /// Exact source-to-target space identities.
    spaces: BTreeMap<LogicalId, LogicalId>,
}

/// Axis transformation location.
const AT: Location = Location::new("exact axis map");

impl Cas {
    /// Rename typed syntax through an explicit exact coordinate, header and space map.
    ///
    /// # Errors
    /// Refuses missing maps, changed signatures or intervals, capture and exhausted resources.
    /// No proof, context assumption, ownership fact or traversal receipt is transported.
    pub fn rename_axes(
        &mut self,
        root: &Handle,
        map: &AxisMap,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let root = self.checked(root)?;
        crate::descriptor::metadata(&self.state, root)?;
        let mapping = Mapping::validate(self, map, budget)?;
        self.transaction(budget, |candidate, budget| {
            let output = mapping.transform(candidate, root, budget)?;
            Ok(output)
        })
    }
}

impl Mapping {
    /// Validate the whole explicit map before traversing any transformed syntax.
    fn validate(cas: &Cas, map: &AxisMap, budget: &mut Budget) -> Result<Self, Fault> {
        let mut coordinates = BTreeMap::new();
        let mut targets = BTreeSet::new();
        for (left, right) in &map.coordinates {
            budget.work(size(left.len().saturating_add(right.len()), AT)?, AT)?;
            if coordinates.insert(left.clone(), right.clone()).is_some()
                || !targets.insert(right.clone())
            {
                return Err(invalid());
            }
        }
        let headers = identities(cas, &map.headers, budget)?;
        let spaces = identities(cas, &map.spaces, budget)?;
        let result = Self {
            coordinates,
            headers,
            spaces,
        };
        for (left, right) in &result.headers {
            result.header(cas, *left, *right, budget)?;
        }
        for (left, right) in &result.spaces {
            result.space(cas, *left, *right)?;
        }
        Ok(result)
    }

    /// A renamed header preserves every signature, display spelling, parent and space binding.
    fn header(
        &self,
        cas: &Cas,
        left: LogicalId,
        right: LogicalId,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let old = cas.state.header(left)?;
        let new = cas.state.header(right)?;
        let mut expected = old.environment.clone();
        for declaration in &mut expected.declarations {
            budget.work(1, AT)?;
            if let Some(key) = self.coordinates.get(&declaration.key) {
                if !matches!(
                    declaration.sort,
                    Sort::Value(AtomicSort::Integer | AtomicSort::Scalar(_))
                ) {
                    return Err(invalid());
                }
                declaration.key.clone_from(key);
            }
            declaration.sort = self.sort(&declaration.sort);
        }
        expected
            .declarations
            .sort_by(|left, right| left.key.cmp(&right.key));
        let parent = old
            .parent
            .map(|id| self.headers.get(&id).copied().ok_or_else(invalid))
            .transpose()?;
        if expected != new.environment
            || parent != new.parent
            || old.scalar_spaces != new.scalar_spaces
            || old.spaces.len() != new.spaces.len()
        {
            return Err(invalid());
        }
        for (key, space) in &old.spaces {
            let mapped = self.spaces.get(space).ok_or_else(invalid)?;
            if !new.spaces.contains(&(self.key(key), *mapped)) {
                return Err(invalid());
            }
        }
        Ok(())
    }

    /// A space keeps its declared family and field; its transformed interval is checked later.
    fn space(&self, cas: &Cas, left: LogicalId, right: LogicalId) -> Result<(), Fault> {
        let old = cas.state.space(left)?;
        let new = cas.state.space(right)?;
        if self.headers.get(&old.header) != Some(&new.header)
            || self.key(&old.coordinate) != new.coordinate
            || old.field != new.field
            || old.test_family != new.test_family
        {
            return Err(invalid());
        }
        Ok(())
    }

    /// Rebuild the complete typed DAG in postorder under one shared ledger.
    fn transform(
        &self,
        cas: &mut Cas,
        root: LogicalId,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let mut pending = vec![(root, false)];
        let mut mapped = BTreeMap::new();
        while let Some((id, expanded)) = pending.pop() {
            budget.work(size(mapped.len(), AT)?.saturating_add(1), AT)?;
            if mapped.contains_key(&id) {
                continue;
            }
            let mut children = crate::descriptor::children(&cas.state, id, budget)?;
            if let Vertex::Region(region) = cas.state.node(id)?
                && let AmbientDomain::Product { coordinates } = &region.ambient
            {
                children.push((
                    crate::descriptor::SyntaxRole::Semantic(0),
                    coordinates.logical_id(),
                ));
            }
            if !expanded {
                budget.check(
                    BudgetKind::StagedVertices,
                    size(mapped.len().saturating_add(children.len()), AT)?,
                    AT,
                )?;
                pending
                    .try_reserve(children.len().saturating_add(1))
                    .map_err(|_| Fault::allocation(AT))?;
                pending.push((id, true));
                pending.extend(children.into_iter().rev().map(|(_, child)| (child, false)));
                continue;
            }
            mapped.insert(id, self.node(cas, id, &mapped, budget)?);
        }
        mapped.remove(&root).ok_or_else(invalid)
    }

    /// Rebuild one complete node after every dependency has its exact renamed identity.
    fn node(
        &self,
        cas: &mut Cas,
        id: LogicalId,
        mapped: &BTreeMap<LogicalId, Handle>,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        if let Vertex::Solve(value) = cas.state.node(id)? {
            let value = value.clone();
            return self.solve_node(cas, &value, mapped, budget);
        }
        let (header, scope) = crate::descriptor::metadata(&cas.state, id)?;
        let header = cas.handle(*self.headers.get(&header).ok_or_else(invalid)?);
        let scope: Scope = scope
            .iter()
            .map(|frame| frame.iter().map(|sort| self.atomic(sort)).collect())
            .collect();
        let operator = match cas.state.node(id)? {
            Vertex::Expression(value) => Some(self.operator(&value.operator)),
            _ => None,
        };
        let mut resolve = |child| {
            mapped
                .get(&child)
                .cloned()
                .map(Reference::Existing)
                .ok_or_else(invalid)
        };
        let mut draft = crate::substitution_draft::rebuild(cas, id, scope, operator, &mut resolve)?;
        self.reheader(&mut draft, &header)?;
        if let Draft::Region(region) | Draft::FamilyRegion { region, .. } = &mut draft
            && let AmbientDomain::Product { coordinates } = &mut region.ambient
        {
            *coordinates = mapped
                .get(&coordinates.logical_id())
                .cloned()
                .ok_or_else(invalid)?;
        }
        let output = crate::case_syntax::admit(cas, draft, budget)?;
        self.check_output(cas, id, output.logical_id())?;
        Ok(output)
    }

    /// Rebuild algebra and family references without manufacturing a header for registry nodes.
    fn solve_node(
        &self,
        cas: &mut Cas,
        value: &crate::solving::SolveDraft<Handle>,
        mapped: &BTreeMap<LogicalId, Handle>,
        budget: &mut Budget,
    ) -> Result<Handle, Fault> {
        let mut draft = value.resolve(&mut |handle| {
            let id = handle.logical_id();
            if id.kind == crate::graph::VertexKind::Header {
                Ok(Reference::Existing(
                    cas.handle(*self.headers.get(&id).ok_or_else(invalid)?),
                ))
            } else {
                mapped
                    .get(&id)
                    .cloned()
                    .map(Reference::Existing)
                    .ok_or_else(invalid)
            }
        })?;
        self.solve_axes(&mut draft);
        crate::case_syntax::admit(cas, Draft::Solve(draft), budget)
    }

    /// Coordinate spelling changes preserve every scalar sort and local dependency role.
    fn solve_axes(&self, value: &mut crate::solving::SolveDraft) {
        use crate::solving::SolveDraft;
        let scope = match value {
            SolveDraft::CoordinateSpace(value) => {
                for coordinate in &mut value.coordinates {
                    coordinate.key = self.key(&coordinate.key);
                }
                return;
            }
            SolveDraft::Tuple(value) => &mut value.scope,
            SolveDraft::FamilyMap(value) => &mut value.scope,
            SolveDraft::RootRelation(value) => &mut value.scope,
            SolveDraft::RootSelection(value) => &mut value.scope,
            SolveDraft::SolveProblem(value) => &mut value.scope,
            SolveDraft::RealCell(value) => &mut value.scope,
            SolveDraft::FamilyExpression(value) => {
                value.operator = self.operator(&value.operator);
                value.result = self.sort(&value.result);
                &mut value.scope
            }
            _ => return,
        };
        *scope = scope
            .iter()
            .map(|frame| frame.iter().map(|sort| self.atomic(sort)).collect())
            .collect();
    }

    /// Change all semantic axis fields together with the exact header identity.
    fn reheader(&self, draft: &mut Draft, header: &Handle) -> Result<(), Fault> {
        let target = Reference::Existing(header.clone());
        match draft {
            Draft::Expression(value) => {
                value.header = target;
                value.sort = self.sort(&value.sort);
                if let Some(binding) = &mut value.binding {
                    binding.sorts = binding.sorts.iter().map(|sort| self.atomic(sort)).collect();
                }
            }
            Draft::Cases(value) => value.header = target,
            Draft::RootValue { header, .. }
            | Draft::ExactCoefficient { header, .. }
            | Draft::Condition { header, .. } => *header = target,
            Draft::Region(value) | Draft::FamilyRegion { region: value, .. } => {
                value.header = target;
                value.ambient = self.ambient(&value.ambient);
            }
            Draft::CaseArm(value) => value.header = target,
            Draft::Boundary(value) => {
                value.header = target;
                value.ambient = self.ambient(&value.ambient);
            }
            Draft::Space(value) => {
                value.header = target;
                value.coordinate = self.key(&value.coordinate);
            }
            Draft::SourceForm(value) => value.header = target,
            _ => return Err(invalid()),
        }
        Ok(())
    }

    /// Exact mapped intervals and expression spaces must match the supplied target identities.
    fn check_output(&self, cas: &Cas, old: LogicalId, new: LogicalId) -> Result<(), Fault> {
        match cas.state.node(old)? {
            Vertex::Space(_) if self.spaces.get(&old) != Some(&new) => return Err(invalid()),
            Vertex::Expression(value) => {
                let expected = value
                    .space
                    .map(|id| self.spaces.get(&id).copied().ok_or_else(invalid))
                    .transpose()?;
                if cas.state.expression(new)?.space != expected {
                    return Err(invalid());
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Identity applies to declarations not named in the coordinate map.
    fn key(&self, key: &str) -> String {
        self.coordinates
            .get(key)
            .map_or_else(|| key.to_owned(), Clone::clone)
    }

    /// Coordinate-bearing operator payloads are renamed without inferring a derivative identity.
    fn operator(&self, operator: &Operator) -> Operator {
        let mut result = operator.clone();
        match &mut result {
            Operator::Free(key) | Operator::Derivative(key) => *key = self.key(key),
            Operator::NamedCase(value) => value.coordinate = self.key(&value.coordinate),
            Operator::AlgebraicRoot { variable, .. } => *variable = self.key(variable),
            Operator::Comprehension { rows, columns } => {
                *rows = self.extent(rows);
                *columns = self.extent(columns);
            }
            _ => {}
        }
        result
    }

    /// Exact parameter identities in sorts follow the same bijection.
    fn extent(&self, extent: &Extent) -> Extent {
        match extent {
            Extent::Literal(value) => Extent::Literal(*value),
            Extent::Parameter(key) => Extent::Parameter(self.key(key)),
        }
    }

    /// Matrix shapes retain their exact renamed integer parameters.
    fn atomic(&self, sort: &AtomicSort) -> AtomicSort {
        match sort {
            AtomicSort::Matrix {
                field,
                rows,
                columns,
            } => AtomicSort::Matrix {
                field: field.clone(),
                rows: self.extent(rows),
                columns: self.extent(columns),
            },
            _ => sort.clone(),
        }
    }

    /// Function signatures, set elements and values share the same exact sort transport.
    fn sort(&self, sort: &Sort) -> Sort {
        match sort {
            Sort::Value(value) => Sort::Value(self.atomic(value)),
            Sort::Set(value) => Sort::Set(self.atomic(value)),
            Sort::Function { inputs, output } => Sort::Function {
                inputs: inputs.iter().map(|sort| self.atomic(sort)).collect(),
                output: self.atomic(output),
            },
            _ => sort.clone(),
        }
    }

    /// Semantic coordinates are renamed only through the explicit bijection.
    fn ambient(&self, ambient: &AmbientDomain) -> AmbientDomain {
        match ambient {
            AmbientDomain::Assignments => AmbientDomain::Assignments,
            AmbientDomain::Real(key) => AmbientDomain::Real(self.key(key)),
            AmbientDomain::Integer(key) => AmbientDomain::Integer(self.key(key)),
            AmbientDomain::Opaque(key) => AmbientDomain::Opaque(self.key(key)),
            AmbientDomain::Product { coordinates } => AmbientDomain::Product {
                coordinates: coordinates.clone(),
            },
        }
    }
}

/// Resolve a finite injective identity map, rejecting foreign handles before use.
fn identities(
    cas: &Cas,
    pairs: &[(Handle, Handle)],
    budget: &mut Budget,
) -> Result<BTreeMap<LogicalId, LogicalId>, Fault> {
    let mut result = BTreeMap::new();
    let mut targets = BTreeSet::new();
    for (left, right) in pairs {
        budget.work(size(pairs.len(), AT)?, AT)?;
        let left = cas.checked(left)?;
        let right = cas.checked(right)?;
        if result.insert(left, right).is_some() || !targets.insert(right) {
            return Err(invalid());
        }
    }
    Ok(result)
}

/// General functional coordinate changes are outside exact renaming.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Scope, AT)
}
