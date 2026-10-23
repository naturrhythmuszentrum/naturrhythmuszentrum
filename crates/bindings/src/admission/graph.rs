// SPDX-License-Identifier: AGPL-3.0-only
//! Closed CAS association edges join independently declared symbols without growing headers.

use super::{AdmissionError, BindingSymbols, CompanionKind, QuantitySymbol, SymbolKind};
use crate::{Descriptor, UncertaintyMode, uncertainty};
use nrz_cas::syntax::{
    AtomicSort, Declaration, Draft, Environment, ExpressionDraft, Extent, Field, Operator, Sort,
    SymbolRelation as CasRelation,
};
use nrz_cas::{Cas, Handle, Reference, domain::HeaderDraft, limits::Budget};
use std::{cell::RefCell, rc::Rc};

/// Mutable construction state; previously admitted graph nodes remain immutable.
#[derive(Debug)]
pub(crate) struct Store {
    /// The single exact graph authority.
    pub(crate) cas: Cas,
    /// Fresh declaration sequence, never reused after successful admission.
    next: u64,
}

/// Exact graph authority, either construction-owned or an immutable declared reading graph.
#[derive(Clone, Debug)]
pub(crate) enum Authority {
    /// Shared construction graph for newly declared homogeneous quantities.
    Local(Rc<RefCell<Store>>),
    /// Immutable authority of an already defined general physical reading.
    Reading(Rc<Cas>),
}

/// Immutable checked declaration and its actual graph relation.
#[derive(Debug)]
pub(crate) struct SymbolData {
    /// Retained graph owner.
    pub(crate) graph: Authority,
    /// Exact source record retained even across immutable graph authorities.
    pub(crate) lineage: Option<QuantitySymbol>,
    /// General physical definition when this symbol selects a derived reading.
    pub(crate) reading: Option<crate::readings::PhysicalReading>,
    /// Actual free-symbol expression.
    pub(crate) node: Handle,
    /// Mathematical declaration sort.
    pub(crate) sort: Sort,
    /// Logical rectangle, independent of physical coordinate identities.
    pub(crate) shape: [u32; 2],
    /// Physical quantity coordinates.
    pub(crate) quantity: Descriptor,
    /// Value or distinct companion interpretation.
    pub(crate) kind: SymbolKind,
    /// Complete companion presence fixed when this bundle was declared.
    pub(crate) companions: [bool; 2],
    /// Checked ordered Boolean relation application.
    pub(crate) relation: Option<Handle>,
    /// Exact source or target symbol of that relation.
    pub(crate) target: Option<Handle>,
}

/// Fixed-size declarations to admit together, before any heap allocation.
struct Plan {
    /// Immutable required companion inventory of the declared value.
    companions: [bool; 2],
    /// Ordered value, optional bound and optional error kinds.
    kinds: Vec<SymbolKind>,
    /// Single atomic graph admission batch.
    drafts: Vec<Draft>,
    /// Optional relation node positions for the corresponding symbols.
    relations: Vec<Option<usize>>,
}

/// Construct one bounded symbol bundle using one caller-owned CAS ledger.
pub(crate) fn declare(
    quantity: &Descriptor,
    shape: [u32; 2],
    companions: [bool; 2],
    source: Option<&QuantitySymbol>,
    budget: &mut Budget,
) -> Result<BindingSymbols, AdmissionError> {
    if companions[1] && !companions[0] {
        return Err(AdmissionError::CompanionMismatch);
    }
    let radius = companions[0]
        .then(|| uncertainty(UncertaintyMode::Radius, quantity, None, None))
        .transpose()?;
    let graph = match source {
        Some(symbol) if matches!(symbol.0.graph, Authority::Local(_)) => {
            let Authority::Local(graph) = &symbol.0.graph else {
                return Err(AdmissionError::ResourceLimit);
            };
            Rc::clone(graph)
        }
        _ => Rc::new(RefCell::new(Store {
            cas: Cas::new(budget.limits().clone())?,
            next: 0,
        })),
    };
    let mut state = graph
        .try_borrow_mut()
        .map_err(|_| AdmissionError::ResourceLimit)?;
    let next = state
        .next
        .checked_add(1)
        .ok_or(AdmissionError::ResourceLimit)?;
    let sort = atomic(shape);
    let local_source = source.filter(|symbol| matches!(symbol.0.graph, Authority::Local(_)));
    let plan = plan(&sort, companions, local_source, state.next);
    let handles = state.cas.admit(&plan.drafts, budget)?;
    state.next = next;
    drop(state);
    assemble(
        &graph,
        quantity,
        radius.as_ref(),
        shape,
        source,
        &plan,
        &handles,
    )
}

/// Keep matrix shape in the declared sort without selecting a storage representation.
const fn atomic(shape: [u32; 2]) -> AtomicSort {
    AtomicSort::Matrix {
        field: Field::Real,
        rows: Extent::Literal(shape[0]),
        columns: Extent::Literal(shape[1]),
    }
}

/// Build the complete header and fixed ordered relation applications.
fn plan(
    sort: &AtomicSort,
    companions: [bool; 2],
    source: Option<&QuantitySymbol>,
    serial: u64,
) -> Plan {
    let mut plan = Plan {
        companions,
        kinds: vec![SymbolKind::Value],
        drafts: Vec::new(),
        relations: Vec::new(),
    };
    for (present, kind) in companions.into_iter().zip([
        CompanionKind::DeterministicBound,
        CompanionKind::NumericalError,
    ]) {
        if present {
            plan.kinds.push(SymbolKind::Companion(kind));
        }
    }
    let declarations = (0..plan.kinds.len())
        .map(|index| {
            let key = format!("q{serial:020}_{index}");
            Declaration {
                display_name: key.clone(),
                key,
                sort: Sort::Value(sort.clone()),
            }
        })
        .collect();
    plan.drafts
        .push(Draft::Header(HeaderDraft::Core(Environment {
            declarations,
            ..Environment::default()
        })));
    for index in 0..plan.kinds.len() {
        plan.drafts.push(expression(
            Operator::Free(format!("q{serial:020}_{index}")),
            Sort::Value(sort.clone()),
            Vec::new(),
        ));
    }
    relation_nodes(&mut plan, source);
    plan
}

/// These constructors declare associations and never grant arithmetic proof authority.
const fn relation_kind(kind: SymbolKind) -> CasRelation {
    match kind {
        SymbolKind::Value => CasRelation::Copy,
        SymbolKind::Companion(CompanionKind::DeterministicBound) => CasRelation::DeterministicBound,
        SymbolKind::Companion(CompanionKind::NumericalError) => CasRelation::NumericalError,
    }
}

/// The checked relation application retains both actual symbol nodes in order.
fn relation_nodes(plan: &mut Plan, source: Option<&QuantitySymbol>) {
    for (&kind, position) in plan.kinds.iter().zip(1_u32..) {
        let target = if kind == SymbolKind::Value {
            let Some(symbol) = source else {
                plan.relations.push(None);
                continue;
            };
            Reference::Existing(symbol.node().clone())
        } else {
            Reference::Local(1)
        };
        plan.relations.push(Some(plan.drafts.len()));
        plan.drafts.push(expression(
            Operator::SymbolRelation(relation_kind(kind)),
            Sort::Value(AtomicSort::Boolean),
            vec![Reference::Local(position), target],
        ));
    }
}

/// No original operand is normalized or eliminated during relation construction.
const fn expression(operator: Operator, sort: Sort, operands: Vec<Reference>) -> Draft {
    Draft::Expression(ExpressionDraft {
        space: None,
        operator,
        sort,
        header: Reference::Local(0),
        scope: Vec::new(),
        binding: None,
        operands,
    })
}

/// Seal declarations only after the full graph batch has passed CAS admission.
fn assemble(
    graph: &Rc<RefCell<Store>>,
    quantity: &Descriptor,
    radius: Option<&Descriptor>,
    shape: [u32; 2],
    source: Option<&QuantitySymbol>,
    plan: &Plan,
    handles: &[Handle],
) -> Result<BindingSymbols, AdmissionError> {
    let mut symbols = Vec::new();
    for (index, &kind) in plan.kinds.iter().enumerate() {
        let descriptor = if kind == SymbolKind::Value {
            quantity
        } else {
            radius.ok_or(AdmissionError::CompanionMismatch)?
        };
        let target = if kind == SymbolKind::Value {
            source.map(|symbol| symbol.node().clone())
        } else {
            Some(handle(handles, 1)?)
        };
        symbols.push(QuantitySymbol(Rc::new(SymbolData {
            graph: Authority::Local(Rc::clone(graph)),
            lineage: source.filter(|_| kind == SymbolKind::Value).cloned(),
            reading: None,
            node: handle(handles, index + 1)?,
            sort: Sort::Value(atomic(shape)),
            shape,
            quantity: descriptor.clone(),
            kind,
            companions: plan.companions,
            relation: plan.relations[index]
                .map(|position| handle(handles, position))
                .transpose()?,
            target,
        })));
    }
    let mut symbols = symbols.into_iter();
    BindingSymbols::check(
        symbols.next().ok_or(AdmissionError::ResourceLimit)?,
        symbols.next(),
        symbols.next(),
    )
}

/// Missing admitted positions are typed internal-construction refusals.
fn handle(handles: &[Handle], index: usize) -> Result<Handle, AdmissionError> {
    handles
        .get(index)
        .cloned()
        .ok_or(AdmissionError::ResourceLimit)
}
