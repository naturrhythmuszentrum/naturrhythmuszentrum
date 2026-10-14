// SPDX-License-Identifier: AGPL-3.0-only
//! Complete typed syntax dependencies shared by coverage and substitution.

use crate::budget::{Budget, BudgetKind};
use crate::domain_model::RegionExpr;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, LogicalId, State, Vertex, size};
use crate::model::{Binding, Scope};

/// Semantic role of an ordered dependency in a complete syntax descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxRole {
    /// An ordinary expression operand or predicate argument.
    Operand(u32),
    /// The stated domain of a form, case or space.
    Domain,
    /// An unordered native association in canonical local order.
    Arm(u32),
    /// One original clause selector in written order.
    Selector(u32),
    /// One original clause value in written order.
    Value(u32),
    /// The explicit final otherwise value.
    Otherwise,
    /// The base of a guard or relative difference.
    Base,
    /// The predicate of a guard.
    Predicate,
    /// An unordered intersection or union member.
    Member(u32),
    /// The excluded region of a relative difference.
    Excluded,
    /// The membership region of one association.
    Region,
    /// A boundary location expression.
    Location,
    /// The exact space of a space-dependent expression.
    Space,
}

/// An earlier syntax object together with its exact semantic edge role.
#[derive(Clone, Debug)]
pub struct SyntaxDependency {
    /// Role corresponding to the same index in an occurrence's children.
    pub role: SyntaxRole,
    /// Referenced syntax object, including its runtime store identity.
    pub target: Handle,
}

/// Complete syntax metadata; it is descriptive and supplies no theorem or receipt.
#[derive(Clone, Debug)]
pub struct SyntaxDescriptor {
    /// Complete constructor payload, distinct from ordered semantic dependencies.
    pub node: crate::descriptor_model::SyntaxNode,
    /// Actual semantic header, retaining declarations and their space identities.
    pub header: Handle,
    /// Explicit outer-to-inner bound frames.
    pub scope: Scope,
    /// An expression's binding frame, if any; cases introduce no binder.
    pub binding: Option<Binding>,
    /// Every semantic child, including inactive source clauses.
    pub dependencies: Vec<SyntaxDependency>,
}

/// Empty scope used only by globally declared spaces.
static EMPTY_SCOPE: Scope = Vec::new();
/// Descriptor accounting location.
const AT: Location = Location::new("typed syntax descriptor");

/// Borrow metadata without interpreting a non-expression as scalar syntax.
pub(crate) fn metadata(state: &State, id: LogicalId) -> Result<(LogicalId, &Scope), Fault> {
    match state.node(id)? {
        Vertex::Expression(v) => Ok((v.header, &v.scope)),
        Vertex::Condition(v) => Ok((v.header, &v.scope)),
        Vertex::Region(v) => Ok((v.header, &v.scope)),
        Vertex::CaseArm(v) => Ok((v.header, &v.scope)),
        Vertex::Boundary(v) => Ok((v.header, &v.scope)),
        Vertex::SourceForm(v) => Ok((v.header, &v.scope)),
        Vertex::Space(v) => Ok((v.header, &EMPTY_SCOPE)),
        _ => Err(Fault::invalid(InvalidCode::Reference, AT)),
    }
}

/// Borrow a binder only from an expression that actually introduces it.
pub(crate) fn binding(state: &State, id: LogicalId) -> Result<Option<&Binding>, Fault> {
    Ok(match state.node(id)? {
        Vertex::Expression(v) => v.binding.as_ref(),
        _ => None,
    })
}

/// Append only after accounting for the complete dependency record.
fn push(
    out: &mut Vec<(SyntaxRole, LogicalId)>,
    role: SyntaxRole,
    id: LogicalId,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let next = size(out.len(), AT)?
        .checked_add(1)
        .ok_or_else(|| Fault::capacity(BudgetKind::Edges, AT))?;
    budget.check(BudgetKind::Edges, next, AT)?;
    budget.work(64, AT)?;
    out.try_reserve(1).map_err(|_| Fault::allocation(AT))?;
    out.push((role, id));
    Ok(())
}

/// Copy an indexed dependency family in its retained semantic order.
fn family(
    out: &mut Vec<(SyntaxRole, LogicalId)>,
    values: &[LogicalId],
    role: fn(u32) -> SyntaxRole,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for (index, id) in values.iter().enumerate() {
        let index = u32::try_from(index).map_err(|_| Fault::capacity(BudgetKind::Edges, AT))?;
        push(out, role(index), *id, budget)?;
    }
    Ok(())
}

/// Describe each constructor without dropping domain or predicate dependencies.
pub(crate) fn children(
    state: &State,
    id: LogicalId,
    budget: &mut Budget,
) -> Result<Vec<(SyntaxRole, LogicalId)>, Fault> {
    let mut out = Vec::new();
    match state.node(id)? {
        Vertex::Expression(v) => {
            if let Some(domain) = v.domain {
                push(&mut out, SyntaxRole::Domain, domain, budget)?;
            }
            family(&mut out, &v.arms, SyntaxRole::Arm, budget)?;
            family(&mut out, &v.operands, SyntaxRole::Operand, budget)?;
            if let Some(space) = v.space {
                push(&mut out, SyntaxRole::Space, space, budget)?;
            }
        }
        Vertex::Condition(v) => family(&mut out, &v.arguments, SyntaxRole::Operand, budget)?,
        Vertex::Region(v) => region_children(&v.expression, &mut out, budget)?,
        Vertex::CaseArm(v) => {
            push(&mut out, SyntaxRole::Region, v.region, budget)?;
            push(&mut out, SyntaxRole::Value(0), v.value, budget)?;
        }
        Vertex::Boundary(v) => push(&mut out, SyntaxRole::Location, v.location, budget)?,
        Vertex::Space(v) => push(&mut out, SyntaxRole::Domain, v.domain, budget)?,
        Vertex::SourceForm(v) => source_children(v, &mut out, budget)?,
        _ => return Err(Fault::invalid(InvalidCode::Reference, AT)),
    }
    Ok(out)
}

/// Region constructors keep guard truth separate from total Boolean negation.
fn region_children(
    value: &RegionExpr<LogicalId>,
    out: &mut Vec<(SyntaxRole, LogicalId)>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    match value {
        RegionExpr::Ambient | RegionExpr::Empty => {}
        RegionExpr::GuardTrue { base, predicate } | RegionExpr::GuardFalse { base, predicate } => {
            push(out, SyntaxRole::Base, *base, budget)?;
            push(out, SyntaxRole::Predicate, *predicate, budget)?;
        }
        RegionExpr::Meet(values) | RegionExpr::Union(values) => {
            family(out, values, SyntaxRole::Member, budget)?;
        }
        RegionExpr::Difference { base, excluded } => {
            push(out, SyntaxRole::Base, *base, budget)?;
            push(out, SyntaxRole::Excluded, *excluded, budget)?;
        }
    }
    Ok(())
}

/// Source clauses retain every selector/value pair in written order.
fn source_children(
    value: &crate::source_model::SourceForm,
    out: &mut Vec<(SyntaxRole, LogicalId)>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    push(out, SyntaxRole::Domain, value.domain, budget)?;
    for (index, (selector, branch)) in value.clauses.iter().enumerate() {
        let index = u32::try_from(index).map_err(|_| Fault::capacity(BudgetKind::Edges, AT))?;
        push(out, SyntaxRole::Selector(index), *selector, budget)?;
        push(out, SyntaxRole::Value(index), *branch, budget)?;
    }
    if let Some(otherwise) = value.otherwise {
        push(out, SyntaxRole::Otherwise, otherwise, budget)?;
    }
    Ok(())
}

impl Cas {
    /// Inspect all typed dependencies before constructing an occurrence tree.
    ///
    /// # Errors
    /// Refuses foreign or nonsyntax handles and exhausted descriptor limits.
    pub fn describe_syntax(
        &self,
        root: &Handle,
        budget: &mut Budget,
    ) -> Result<SyntaxDescriptor, Fault> {
        let id = self.checked(root)?;
        let (header, scope) = metadata(&self.state, id)?;
        let children = children(&self.state, id, budget)?;
        let mut dependencies = Vec::new();
        dependencies
            .try_reserve_exact(children.len())
            .map_err(|_| Fault::allocation(AT))?;
        budget.work(size(scope.len(), AT)?, AT)?;
        for (role, target) in children {
            dependencies.push(SyntaxDependency {
                role,
                target: self.handle(target),
            });
        }
        Ok(SyntaxDescriptor {
            node: payload(&self.state, id, budget)?,
            header: self.handle(header),
            scope: scope.clone(),
            binding: binding(&self.state, id)?.cloned(),
            dependencies,
        })
    }
}

/// Copy complete payloads only after reserving their encoded structural size.
fn payload(
    state: &State,
    id: LogicalId,
    budget: &mut Budget,
) -> Result<crate::descriptor_model::SyntaxNode, Fault> {
    use crate::descriptor_model::{ExpressionSyntax, SpaceSyntax, SyntaxNode};
    let value = state.node(id)?;
    let (_, key) = crate::graph::vertex_key(value, state, budget)?;
    let bytes = size(key.len(), AT)?
        .checked_mul(4)
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.work(bytes, AT)?;
    Ok(match value {
        Vertex::Expression(v) => SyntaxNode::Expression(Box::new(ExpressionSyntax {
            sort: v.sort.clone(),
            operator: v.operator.clone(),
            layer: v.layer,
        })),
        Vertex::Condition(v) => SyntaxNode::Condition(v.kind.clone()),
        Vertex::Region(v) => {
            SyntaxNode::Region(v.ambient.clone(), region_constructor(&v.expression))
        }
        Vertex::CaseArm(_) => SyntaxNode::CaseArm,
        Vertex::Boundary(v) => SyntaxNode::Boundary(v.ambient.clone()),
        Vertex::Space(v) => SyntaxNode::Space(Box::new(SpaceSyntax {
            coordinate: v.coordinate.clone(),
            field: v.field.clone(),
            test_family: v.test_family.clone(),
        })),
        Vertex::SourceForm(v) => SyntaxNode::SourceForm(v.kind),
        _ => return Err(Fault::invalid(InvalidCode::Reference, AT)),
    })
}

/// Measure unfolded typed dependencies without hiding regions, predicates or association nodes.
pub(crate) fn measure(
    state: &State,
    root: LogicalId,
    budget: &mut Budget,
) -> Result<(u64, u64), Fault> {
    let mut known = std::collections::BTreeMap::new();
    let mut pending = vec![(root, false)];
    while let Some((id, expanded)) = pending.pop() {
        budget.work(size(known.len(), AT)?.saturating_add(1), AT)?;
        if known.contains_key(&id) {
            continue;
        }
        let children = children(state, id, budget)?;
        if !expanded {
            pending
                .try_reserve(children.len().saturating_add(1))
                .map_err(|_| Fault::allocation(AT))?;
            pending.push((id, true));
            pending.extend(children.into_iter().rev().map(|(_, id)| (id, false)));
            continue;
        }
        let (mut depth, mut count) = (1_u64, 1_u64);
        for (_, child) in children {
            let (child_depth, child_count): (u64, u64) = *known
                .get(&child)
                .ok_or_else(|| Fault::invalid(InvalidCode::Reference, AT))?;
            depth = depth.max(
                child_depth
                    .checked_add(1)
                    .ok_or_else(|| Fault::capacity(BudgetKind::Depth, AT))?,
            );
            count = count
                .checked_add(child_count)
                .ok_or_else(|| Fault::capacity(BudgetKind::PeakExpressionSize, AT))?;
        }
        budget.check(BudgetKind::Depth, depth, AT)?;
        budget.check(BudgetKind::PeakExpressionSize, count, AT)?;
        known.insert(id, (depth, count));
    }
    known
        .remove(&root)
        .ok_or_else(|| Fault::invalid(InvalidCode::Reference, AT))
}

/// Region payload tags never infer guard totality.
const fn region_constructor(
    value: &RegionExpr<LogicalId>,
) -> crate::descriptor_model::RegionConstructor {
    use crate::descriptor_model::RegionConstructor;
    match value {
        RegionExpr::Ambient => RegionConstructor::Ambient,
        RegionExpr::Empty => RegionConstructor::Empty,
        RegionExpr::GuardTrue { .. } => RegionConstructor::GuardTrue,
        RegionExpr::GuardFalse { .. } => RegionConstructor::GuardFalse,
        RegionExpr::Meet(_) => RegionConstructor::Meet,
        RegionExpr::Union(_) => RegionConstructor::Union,
        RegionExpr::Difference { .. } => RegionConstructor::Difference,
    }
}
