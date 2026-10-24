// SPDX-License-Identifier: AGPL-3.0-only
//! Derive inventory views from the canonical retained closure used by witness export.

use super::inventory_budget::{Meter, index, table_meter};
use super::inventory_model::{
    DependencyBase, DependencyContents, DependencyDeclaration, DependencyDefinition,
    DependencyInventory, DependencyPremise, DependencyPremiseKind, DependencySelection,
};
use super::{AT, bases, invalid};
use crate::budget::Budget;
use crate::encoding::Encoder;
use crate::error::{Fault, InvalidCode};
use crate::graph::{Cas, Handle, LogicalId, Vertex, size};
use crate::model::Operator;
use crate::proof_model::Statement;
use crate::wire_build::{Assignments, IndexedTable, Key};

/// Local base handles accompany portable data only until final publication.
pub(super) struct BaseRecord {
    /// Authenticated local base identity.
    pub(super) id: LogicalId,
    /// Portable declaration, definition and exact demanded entries.
    pub(super) value: DependencyBase,
}

/// One private construction session; every growing vector shares its retained-byte meter.
pub(super) struct Assembly<'a> {
    /// Exact source graph, read only throughout inventory construction.
    pub(super) cas: &'a Cas,
    /// Canonical record indices already computed by witness export machinery.
    pub(super) assigned: &'a Assignments,
    /// Complete portable record and view payload.
    pub(super) contents: DependencyContents,
    /// Authenticated local base identities with their portable views.
    pub(super) bases: Vec<BaseRecord>,
    /// Semantic roots whose coordinate demands must be propagated.
    roots: Vec<LogicalId>,
    /// Conservative simultaneously retained payload.
    pub(super) meter: Meter,
}

/// No dependency is inferred from the receiver's unrelated graph records.
pub(super) fn build(
    cas: &Cas,
    owner: &Handle,
    table: IndexedTable,
    budget: &mut Budget,
) -> Result<DependencyInventory, Fault> {
    let (records, root, _, assigned) = table;
    let meter = table_meter(&records, &assigned, budget)?;
    let mut assembly = Assembly {
        cas,
        assigned: &assigned,
        meter,
        bases: Vec::new(),
        roots: Vec::new(),
        contents: DependencyContents {
            records,
            root,
            declarations: Vec::new(),
            selections: Vec::new(),
            definitions: Vec::new(),
            bases: Vec::new(),
            premises: Vec::new(),
        },
    };
    assembly.describe(budget)?;
    if matches!(cas.state.node(cas.checked(owner)?)?, Vertex::Expression(_)) {
        assembly.root(cas.checked(owner)?, budget)?;
    }
    let roots = std::mem::take(&mut assembly.roots);
    for root in roots {
        assembly.trace(root, budget)?;
    }
    assembly.finish(owner, budget)
}

impl Assembly<'_> {
    /// Process semantic objects in portable order, and retain synthetic premise identities.
    fn describe(&mut self, budget: &mut Budget) -> Result<(), Fault> {
        let mut objects = Vec::new();
        for (key, (node, _)) in self.assigned {
            budget.work(1, AT)?;
            match key {
                Key::Object(id) => self.meter.push(&mut objects, (*node, *id), budget)?,
                Key::Guard(_) => self.premise(*node, DependencyPremiseKind::Guard, budget)?,
                Key::Statement(_) => {
                    self.premise(*node, DependencyPremiseKind::Statement, budget)?;
                }
                Key::Conclusion(_) => {}
            }
        }
        let count = size(objects.len(), AT)?;
        budget.work(count.saturating_mul(count), AT)?;
        objects.sort_unstable_by_key(|(node, _)| *node);
        for (node, id) in objects {
            self.object(node, id, budget)?;
        }
        Ok(())
    }

    /// Record the actual retained declarations, selections, equations and exact premises.
    fn object(&mut self, node: u32, id: LogicalId, budget: &mut Budget) -> Result<(), Fault> {
        budget.work(1, AT)?;
        match self.cas.state.node(id)? {
            Vertex::Header(value) => {
                for declaration in &value.environment.declarations {
                    let bytes = self.meter.run(budget, |budget| {
                        let mut out = Encoder::new(budget, AT);
                        out.name(&declaration.key)?;
                        out.name(&declaration.display_name)?;
                        crate::encoding::sort(&declaration.sort, &mut out)?;
                        Ok(out.finish().len())
                    })?;
                    self.meter
                        .keep(size(bytes, AT)?.saturating_mul(4), budget)?;
                    self.meter.push(
                        &mut self.contents.declarations,
                        DependencyDeclaration {
                            header: node,
                            declaration: declaration.clone(),
                        },
                        budget,
                    )?;
                }
            }
            Vertex::Expression(value) => self.expression(node, value, budget)?,
            Vertex::Context(_) => self.premise(node, DependencyPremiseKind::Context, budget)?,
            Vertex::Region(_) => self.premise(node, DependencyPremiseKind::Domain, budget)?,
            Vertex::Condition(value) => {
                self.premise(node, DependencyPremiseKind::Predicate, budget)?;
                for argument in &value.arguments {
                    self.root(*argument, budget)?;
                }
            }
            Vertex::Proof(_) => self.proof(node, id, budget)?,
            _ => {}
        }
        Ok(())
    }

    /// Original selection order and every defining coefficient survive projection pruning.
    fn expression(
        &mut self,
        node: u32,
        value: &crate::graph::Expression,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        match &value.operator {
            Operator::Select(selection) => {
                let base = *value
                    .operands
                    .first()
                    .ok_or_else(|| invalid(InvalidCode::Reference))?;
                self.ensure_base(base, budget)?;
                let base = index(self.assigned, base, budget)?;
                self.meter.keep(
                    size(selection.indices().len(), AT)?.saturating_mul(8),
                    budget,
                )?;
                self.meter.push(
                    &mut self.contents.selections,
                    DependencySelection {
                        node,
                        base,
                        selection: selection.as_ref().clone(),
                    },
                    budget,
                )?;
            }
            Operator::AffineReading(map) => {
                let [symbol, source] = value.operands.as_slice() else {
                    return Err(invalid(InvalidCode::ReadingDefinition));
                };
                let symbol = index(self.assigned, *symbol, budget)?;
                let source = index(self.assigned, *source, budget)?;
                let bytes = self.meter.run(budget, |budget| map.validate(budget))?;
                self.meter.keep(bytes, budget)?;
                self.meter.push(
                    &mut self.contents.definitions,
                    DependencyDefinition {
                        node,
                        symbol,
                        source,
                        map: map.as_ref().clone(),
                    },
                    budget,
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Every equality in a composed proof contributes both original endpoint demands.
    fn proof(&mut self, node: u32, id: LogicalId, budget: &mut Budget) -> Result<(), Fault> {
        self.premise(node, DependencyPremiseKind::Proof, budget)?;
        let application = self.meter.run(budget, |budget| {
            crate::proof_view::application(self.cas, id, budget)
        })?;
        let endpoints =
            if let Statement::Equality { left, right, .. } = &application.judgment.statement {
                Some([self.cas.checked(left)?, self.cas.checked(right)?])
            } else {
                None
            };
        drop(application);
        if let Some(endpoints) = endpoints {
            for endpoint in endpoints {
                self.root(endpoint, budget)?;
            }
        }
        Ok(())
    }

    /// Repeated endpoint occurrences do not manufacture independent dependencies.
    fn root(&mut self, root: LogicalId, budget: &mut Budget) -> Result<(), Fault> {
        budget.work(size(self.roots.len(), AT)?, AT)?;
        if !self.roots.contains(&root) {
            self.meter.push(&mut self.roots, root, budget)?;
        }
        Ok(())
    }

    /// Premise roles are distinct while repeated references to one role are idempotent.
    pub(super) fn premise(
        &mut self,
        node: u32,
        kind: DependencyPremiseKind,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let premise = DependencyPremise { node, kind };
        budget.work(size(self.contents.premises.len(), AT)?, AT)?;
        if !self.contents.premises.contains(&premise) {
            self.meter
                .push(&mut self.contents.premises, premise, budget)?;
        }
        Ok(())
    }

    /// Compute portable base identity only from its retained declaration and full definition.
    pub(super) fn ensure_base(
        &mut self,
        id: LogicalId,
        budget: &mut Budget,
    ) -> Result<usize, Fault> {
        budget.work(size(self.bases.len(), AT)?, AT)?;
        if let Some(position) = self.bases.iter().position(|base| base.id == id) {
            return Ok(position);
        }
        let declaration = bases::declaration(self.cas, id)?;
        let Operator::Free(symbol) = &declaration.operator else {
            return Err(invalid(InvalidCode::ReadingDefinition));
        };
        let node = index(self.assigned, id, budget)?;
        let snapshot = self.snapshot(declaration.header, budget)?;
        self.meter.keep(size(snapshot.len(), AT)?, budget)?;
        let definition = self.meter.run(budget, |budget| {
            crate::wire_build::syntax_key(self.cas, id, budget)
        })?;
        self.meter.keep(size(definition.len(), AT)?, budget)?;
        self.meter.keep(size(symbol.len(), AT)?, budget)?;
        let position = self.bases.len();
        self.meter.push(
            &mut self.bases,
            BaseRecord {
                id,
                value: DependencyBase {
                    node,
                    symbol: symbol.clone(),
                    snapshot,
                    definition,
                    entries: Vec::new(),
                },
            },
            budget,
        )?;
        Ok(position)
    }

    /// Reuse the complete canonical declaration snapshot already retained by this read-only DAG.
    /// Exact graph identity includes every ancestor header and declaration; names alone never match.
    fn snapshot(&self, header: LogicalId, budget: &mut Budget) -> Result<Vec<u8>, Fault> {
        for base in &self.bases {
            budget.work(1, AT)?;
            if bases::declaration(self.cas, base.id)?.header == header {
                return self.meter.run(budget, |budget| {
                    let mut out = Encoder::witness(budget, AT);
                    out.raw(&base.value.snapshot)?;
                    Ok(out.finish())
                });
            }
        }
        self.meter.run(budget, |budget| {
            crate::wire_build::syntax_key(self.cas, header, budget)
        })
    }

    /// Portable sorting happens only after every source and premise has been traversed.
    fn finish(mut self, owner: &Handle, budget: &mut Budget) -> Result<DependencyInventory, Fault> {
        let count = size(self.bases.len(), AT)?;
        budget.work(count.saturating_mul(count), AT)?;
        self.bases.sort_unstable_by_key(|base| base.value.node);
        let count = size(self.contents.premises.len(), AT)?;
        budget.work(count.saturating_mul(count), AT)?;
        self.contents
            .premises
            .sort_unstable_by_key(|premise| (premise.node, premise.kind));
        let mut bases = Vec::new();
        for mut base in self.bases {
            let count = size(base.value.entries.len(), AT)?;
            budget.work(count.saturating_mul(count), AT)?;
            base.value
                .entries
                .sort_unstable_by_key(|entry| (entry.column, entry.row));
            self.meter.push(&mut bases, base.id, budget)?;
            self.meter
                .push(&mut self.contents.bases, base.value, budget)?;
        }
        Ok(DependencyInventory {
            owner: owner.clone(),
            contents: self.contents,
            bases,
        })
    }
}
