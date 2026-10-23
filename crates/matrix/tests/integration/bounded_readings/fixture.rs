// SPDX-License-Identifier: AGPL-3.0-only
//! Public physical declarations for a native bounded result, independent of matrix internals.

use crate::binding_testing::TestResult;
use nrz_bindings::readings::algebra::{
    Cas, Handle, Reference,
    domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr},
    readings::{AffineMap, SelectionSpec},
    syntax::{
        AtomicSort, Declaration, Draft, Environment, ExpressionDraft, Extent, Field, Operator, Sort,
    },
};
use nrz_bindings::readings::{
    CoordinateFamily, EntryMeaning, PhysicalReading, ReadingDefinition, Representation,
    RepresentationDefinition,
};
use nrz_bindings::{
    Budget, Descriptor, Dimension, Kind, Limits, Measure, ReferenceDefinition, Role,
};
use nrz_matrix::{EntryTicket, ReadingAdapter, ReadingCoverage};
use std::rc::Rc;

/// Original voltage meaning stays attached to every numerical result and companion.
pub(super) fn voltage() -> TestResult<Descriptor> {
    Ok(Descriptor::new(
        Kind::Named(
            "voltage".into(),
            Box::new(Dimension::integers([2, 1, -3, -1, 0, 0, 0])?),
        ),
        Measure::coherent(),
        Role::Difference,
    )?)
}

/// Declare a two-entry voltage reading through public bindings and representation APIs.
pub(super) fn plan(quantity: &Descriptor) -> TestResult<(Rc<Cas>, ReadingCoverage<usize>)> {
    let mut budget = Budget::new(Limits::default())?;
    let mut cas = Cas::new(Limits::default())?;
    let (context, family_symbol, output) = declarations(&mut cas, &mut budget)?;
    let reference = ReferenceDefinition::unit("voltage reference", quantity)?;
    let meaning = EntryMeaning::new(quantity.clone(), reference, None, &mut budget)?;
    let entries = vec![meaning; 2];
    let family =
        CoordinateFamily::new(&cas, &context, &family_symbol, entries.clone(), &mut budget)?;
    let reading = PhysicalReading::define(
        &mut cas,
        &family,
        ReadingDefinition {
            symbol: output,
            selection: SelectionSpec::Column(0),
            map: AffineMap::identity(2, &mut budget)?,
            entries,
        },
        &mut budget,
    )?;
    let representation = Representation::prove(
        &mut cas,
        &reading,
        RepresentationDefinition::Defined {
            symbol: family_symbol,
            expansion: AffineMap::identity(2, &mut budget)?,
        },
        &mut budget,
    )?;
    let tickets = vec![
        EntryTicket {
            region: 0,
            index: 0,
        },
        EntryTicket {
            region: 0,
            index: 1,
        },
    ];
    let adapter = ReadingAdapter::new(representation, tickets.clone(), &mut budget)?;
    let coverage = ReadingCoverage::new(tickets, vec![adapter], Vec::new(), &mut budget)?;
    Ok((Rc::new(cas), coverage))
}

/// All mathematical declarations and the ambient context precede numerical allocation.
fn declarations(cas: &mut Cas, budget: &mut Budget) -> TestResult<(Handle, Handle, Handle)> {
    let sort = Sort::Value(AtomicSort::Matrix {
        field: Field::Real,
        rows: Extent::Literal(2),
        columns: Extent::Literal(1),
    });
    let environment = Environment {
        declarations: ["source", "result"]
            .into_iter()
            .map(|key| Declaration {
                key: key.into(),
                display_name: key.into(),
                sort: sort.clone(),
            })
            .collect(),
        ..Environment::default()
    };
    let mut drafts = vec![
        Draft::Header(HeaderDraft::Core(environment)),
        Draft::Region(RegionDraft {
            header: Reference::Local(0),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::Ambient,
        }),
        Draft::Context(ContextDraft::Root {
            header: Reference::Local(0),
            admissible: Reference::Local(1),
            assumptions: Vec::new(),
        }),
    ];
    for key in ["source", "result"] {
        drafts.push(Draft::Expression(ExpressionDraft {
            operator: Operator::Free(key.into()),
            sort: sort.clone(),
            header: Reference::Local(0),
            scope: Vec::new(),
            binding: None,
            space: None,
            operands: Vec::new(),
        }));
    }
    let nodes = cas.admit(&drafts, budget)?;
    Ok((
        nodes.get(2).ok_or("context")?.clone(),
        nodes.get(3).ok_or("family")?.clone(),
        nodes.get(4).ok_or("result")?.clone(),
    ))
}
