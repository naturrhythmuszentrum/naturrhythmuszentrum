// SPDX-License-Identifier: AGPL-3.0-only
//! Domain-only dependencies remain live even for disjoint and exact-only readings.

use super::*;
use nrz_bindings::readings::{
    CoordinateFamily,
    algebra::{
        Handle, Reference,
        domain::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr},
        syntax::{Draft, PredicateKind},
    },
};

/// Admit one independent original node without a proof cache or hidden numerical value.
fn admit(graph: &mut Cas, draft: Draft) -> TestResult<Handle> {
    graph
        .admit(&[draft], &mut ledger()?)?
        .pop()
        .ok_or_else(|| "node".into())
}

/// Restrict the complete family domain by the otherwise disjoint current entry.
fn guard(fixture: &mut Fixture) -> TestResult {
    let base = fixture.symbol("z")?;
    let selected = fixture.cas.select_entries(
        &base,
        &SelectionSpec::Entry(EntryIndex { row: 1, column: 0 }),
        &mut ledger()?,
    )?;
    let header = fixture.cas.context_header(&fixture.context)?;
    let domain = fixture.cas.admissible_region(&fixture.context)?;
    let predicate = admit(
        &mut fixture.cas,
        Draft::Condition {
            kind: PredicateKind::NonNegative,
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            arguments: vec![Reference::Existing(selected)],
        },
    )?;
    let domain = admit(
        &mut fixture.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::GuardTrue {
                base: Reference::Existing(domain),
                predicate: Reference::Existing(predicate),
            },
        }),
    )?;
    fixture.context = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header),
            admissible: Reference::Existing(domain),
            assumptions: Vec::new(),
        }),
    )?;
    Ok(())
}

/// Complete value and no-storage inventories share one immutable guarded graph.
type GuardedPlans = (Plan, ReadingCoverage<usize>);

/// Declare all readings on the guarded family before constructing any numerical ticket.
fn guarded_plans() -> TestResult<GuardedPlans> {
    let mut case = state_case()?;
    guard(&mut case.fixture)?;
    let meanings = case.whole.entries().to_vec();
    let family = CoordinateFamily::new(
        &case.fixture.cas,
        &case.fixture.context,
        &case.fixture.symbol("z")?,
        meanings.clone(),
        &mut ledger()?,
    )?;
    case.whole = case.fixture.define(
        &family,
        "x",
        SelectionSpec::Column(0),
        AffineMap::identity(2, &mut ledger()?)?,
        meanings,
    )?;
    case.component = case.fixture.define(
        &family,
        "v",
        SelectionSpec::Entry(EntryIndex { row: 0, column: 0 }),
        AffineMap::identity(1, &mut ledger()?)?,
        case.component.entries().to_vec(),
    )?;
    let fixed = case.fixture.define(
        &family,
        "v",
        SelectionSpec::Column(0),
        crate::binding::adapters::fixtures::map(2, 1, &[0, 0], &[7])?,
        case.component.entries().to_vec(),
    )?;
    let representation = Representation::prove(
        &mut case.fixture.cas,
        &fixed,
        RepresentationDefinition::Exact,
        &mut ledger()?,
    )?;
    let exact = ReadingAdapter::new(representation, Vec::new(), &mut ledger()?)?;
    let none = ReadingCoverage::new(Vec::new(), vec![exact], Vec::new(), &mut ledger()?)?;
    Ok((plan(case)?, none))
}

/// A write to a predicate-only dependency invalidates disjoint projections and exact-only proofs.
/// ´test:unit:live-domain-premise-invalidates-disjoint-and-none´
#[test]
fn live_domain_premise_invalidates_disjoint_and_none() -> TestResult {
    let ((graph, coverage), none) = guarded_plans()?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let exact = heap.import_readings::<f64>(
        Rc::clone(&graph),
        none,
        Vec::new(),
        &[],
        DependencyPolicy::Entries,
        &mut ledger()?,
    )?;
    let regions = [ReadingRegion {
        extent: Extent::new(2, 1)?,
        values: vec![1.0_f64; 2],
    }];
    let imported = heap.import_readings(
        graph,
        coverage,
        Vec::new(),
        &regions,
        DependencyPolicy::Entries,
        &mut ledger()?,
    )?;
    let voltage = &imported.bindings()[1];
    let current = &imported.bindings()[2];
    assert_eq!(heap.reading_entries(voltage)?, vec![1.0]);
    assert_eq!(heap.reading_entries(&exact.bindings()[0])?, [] as [f64; 0]);
    heap.replace_reading_entries(current, &[0.0], &mut ledger()?)?;
    assert_eq!(voltage.state(), SymbolState::Unavailable);
    assert_eq!(exact.bindings()[0].state(), SymbolState::Unavailable);
    assert!(heap.reading_entries(voltage).is_err());
    assert!(heap.reading_entries(&exact.bindings()[0]).is_err());
    Ok(())
}
