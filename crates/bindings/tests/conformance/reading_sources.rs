// SPDX-License-Identifier: AGPL-3.0-only
//! Joint source tables and indexed projections preserve exact shared identities.

use crate::testing::readings::{Fixture, TestResult, ledger, matrix, meaning, real};
use nrz_bindings::readings::{
    CompanionParameters, CompanionReading, CompanionReadingKind, CoordinateFamily, IndexedReading,
    JointSources, ReadingAxis, ReadingSource, SourceEdge, SourceError, SourceKnowledge,
};
use nrz_cas::{
    readings::{AffineMap, EntryIndex, SelectionSpec},
    syntax::{AtomicSort, Sort},
};

/// Original source declarations and a complete admitted covariance parameter inventory.
struct Sources {
    /// One shared original graph and domain.
    fixture: Fixture,
    /// Original voltage and current sources, each selecting its exact coordinate.
    declared: Vec<ReadingSource>,
    /// Complete triangular known-covariance edges.
    edges: Vec<SourceEdge>,
}

/// Known source covariance targets use actual CAS reading identities.
fn sources() -> TestResult<Sources> {
    let mut fixture = Fixture::new(&[
        ("z", matrix(2, 1)),
        ("v", real()),
        ("i", real()),
        ("parameters", matrix(2, 2)),
        ("vv", matrix(1, 1)),
        ("vi", matrix(1, 1)),
        ("ii", matrix(1, 1)),
        ("zero", Sort::Value(AtomicSort::Boolean)),
    ])?;
    let entries = vec![
        meaning("voltage", [2, 1, -3, -1, 0, 0, 0])?,
        meaning("current", [0, 0, 0, 1, 0, 0, 0])?,
    ];
    let family = CoordinateFamily::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol("z")?,
        entries.clone(),
        &mut ledger()?,
    )?;
    let mut sources = Vec::new();
    for (position, key) in [(0, "v"), (1, "i")] {
        let reading = fixture.define(
            &family,
            key,
            SelectionSpec::Entry(EntryIndex {
                row: position,
                column: 0,
            }),
            AffineMap::identity(1, &mut ledger()?)?,
            vec![entries[usize::try_from(position)?].clone()],
        )?;
        sources.push(ReadingSource::new(reading, &mut ledger()?)?);
    }
    let mut edges = Vec::new();
    for (left, right, key) in [(0, 0, "vv"), (0, 1, "vi"), (1, 1, "ii")] {
        let targets = [
            sources[left].reading().clone(),
            sources[right].reading().clone(),
        ];
        let parameters = CompanionParameters::new(
            &fixture.cas,
            &fixture.context,
            &fixture.symbol("parameters")?,
            CompanionReadingKind::Covariance,
            &targets,
            &mut ledger()?,
        )?;
        let symbol = fixture.symbol(key)?;
        let covariance = CompanionReading::define(
            &mut fixture.cas,
            &symbol,
            &parameters,
            &targets,
            &mut ledger()?,
        )?;
        edges.push(SourceEdge {
            left,
            right,
            knowledge: SourceKnowledge::Known(Box::new(covariance)),
        });
    }
    Ok(Sources {
        fixture,
        declared: sources,
        edges,
    })
}

/// Known, declared-zero and unknown cross blocks remain distinct explicit model inputs.
/// ´test:integration:joint-reading-sources-keep-explicit-knowledge´
#[test]
fn joint_reading_sources_keep_explicit_knowledge() -> TestResult {
    let case = sources()?;
    let known = JointSources::new(
        &case.fixture.cas,
        &case.fixture.context,
        case.declared.clone(),
        case.edges.clone(),
        &mut ledger()?,
    )?;
    assert!(matches!(known.require(1, 0)?, SourceKnowledge::Known(_)));
    let mut edges = case.edges.clone();
    edges[1].knowledge = SourceKnowledge::Unknown;
    let unknown = JointSources::new(
        &case.fixture.cas,
        &case.fixture.context,
        case.declared.clone(),
        edges,
        &mut ledger()?,
    )?;
    assert!(matches!(unknown.require(0, 1), Err(SourceError::Unknown)));
    assert!(matches!(
        known.check_compatible(&unknown, &mut ledger()?),
        Err(SourceError::Conflict)
    ));
    let mut edges = case.edges;
    edges[1].knowledge = SourceKnowledge::DeclaredZero(case.fixture.symbol("zero")?);
    let declared = JointSources::new(
        &case.fixture.cas,
        &case.fixture.context,
        case.declared,
        edges,
        &mut ledger()?,
    )?;
    assert!(matches!(
        declared.require(0, 1)?,
        SourceKnowledge::DeclaredZero(_)
    ));
    Ok(())
}

/// An omitted, contradictory or source-substituted pair cannot manufacture independence.
/// ´test:integration:joint-reading-sources-refuse-missing-and-conflicting-relations´
#[test]
fn joint_reading_sources_refuse_missing_and_conflicting_relations() -> TestResult {
    let case = sources()?;
    assert!(matches!(
        JointSources::new(
            &case.fixture.cas,
            &case.fixture.context,
            case.declared.clone(),
            case.edges[..2].to_vec(),
            &mut ledger()?
        ),
        Err(SourceError::Missing)
    ));
    let mut edges = case.edges.clone();
    edges[1].left = 1;
    assert!(matches!(
        JointSources::new(
            &case.fixture.cas,
            &case.fixture.context,
            case.declared.clone(),
            edges,
            &mut ledger()?
        ),
        Err(SourceError::Conflict)
    ));
    let mut edges = case.edges.clone();
    edges[1].knowledge = edges[0].knowledge.clone();
    assert!(
        JointSources::new(
            &case.fixture.cas,
            &case.fixture.context,
            case.declared.clone(),
            edges,
            &mut ledger()?
        )
        .is_err()
    );
    let mut edges = case.edges;
    edges[1].knowledge = SourceKnowledge::DeclaredZero(case.fixture.symbol("v")?);
    assert!(matches!(
        JointSources::new(
            &case.fixture.cas,
            &case.fixture.context,
            case.declared,
            edges,
            &mut ledger()?
        ),
        Err(SourceError::Premise)
    ));
    Ok(())
}

/// A new reading of an original source does not create a second independent coordinate.
/// ´test:integration:joint-reading-sources-refuse-duplicate-coordinate´
#[test]
fn joint_reading_sources_refuse_duplicate_coordinate() -> TestResult {
    let case = sources()?;
    let duplicate = vec![case.declared[0].clone(), case.declared[0].clone()];
    assert!(matches!(
        JointSources::new(
            &case.fixture.cas,
            &case.fixture.context,
            duplicate,
            case.edges,
            &mut ledger()?
        ),
        Err(SourceError::Conflict)
    ));
    Ok(())
}

/// Grid projections retain the same free family and reject changed index order or extent.
/// ´test:integration:indexed-reading-preserves-order-and-source-family´
#[test]
fn indexed_reading_preserves_order_and_source_family() -> TestResult {
    let mut fixture = Fixture::new(&[
        ("z", matrix(3, 1)),
        ("omega", matrix(3, 1)),
        ("k", Sort::Value(AtomicSort::Integer)),
    ])?;
    let entry = meaning("angular_frequency", [0, 0, -1, 0, 0, 0, 0])?;
    let family = CoordinateFamily::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol("z")?,
        vec![entry.clone(); 3],
        &mut ledger()?,
    )?;
    let reading = fixture.define(
        &family,
        "omega",
        SelectionSpec::Column(0),
        AffineMap::identity(3, &mut ledger()?)?,
        vec![entry; 3],
    )?;
    let axis = ReadingAxis::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol("k")?,
        vec![2, 0, 1],
        vec![3],
        &mut ledger()?,
    )?;
    let grid = IndexedReading::new(reading, axis.clone(), &mut ledger()?)?;
    let projected = grid.project(&mut fixture.cas, 0, &mut ledger()?)?;
    assert_eq!(projected.coordinates(), &[2]);
    assert_eq!(projected.reading().family().root(), family.root());
    grid.check_axis(&axis)?;
    for order in [vec![0, 1, 2], vec![0, 1]] {
        let extent = u32::try_from(order.len())?;
        let changed = ReadingAxis::new(
            &fixture.cas,
            &fixture.context,
            &fixture.symbol("k")?,
            order,
            vec![extent],
            &mut ledger()?,
        )?;
        assert!(grid.check_axis(&changed).is_err());
    }
    Ok(())
}

/// Source declaration and complete joint-model checks keep the caller's finite ledger.
/// ´test:integration:reading-source-resource-boundaries´
#[test]
fn reading_source_resource_boundaries() -> TestResult {
    for joint in [false, true] {
        crate::testing::readings::boundaries(|limits, precharge| {
            let case = sources()?;
            let mut budget = nrz_bindings::Budget::new(limits)?;
            budget.work(
                precharge,
                nrz_cas::Location::new("source boundary precharge"),
            )?;
            if joint {
                JointSources::new(
                    &case.fixture.cas,
                    &case.fixture.context,
                    case.declared,
                    case.edges,
                    &mut budget,
                )?;
            } else {
                ReadingSource::new(case.declared[0].reading().clone(), &mut budget)?;
            }
            Ok(budget.usage().work)
        })?;
    }
    Ok(())
}

/// Indexed descriptor construction and selection keep separate cumulative boundaries.
/// ´test:integration:indexed-reading-resource-boundaries´
#[test]
fn indexed_reading_resource_boundaries() -> TestResult {
    for projection in [false, true] {
        crate::testing::readings::boundaries(|limits, precharge| {
            let mut fixture = Fixture::new(&[
                ("z", matrix(2, 1)),
                ("omega", matrix(2, 1)),
                ("k", Sort::Value(AtomicSort::Integer)),
            ])?;
            let entry = meaning("frequency", [0, 0, -1, 0, 0, 0, 0])?;
            let family = CoordinateFamily::new(
                &fixture.cas,
                &fixture.context,
                &fixture.symbol("z")?,
                vec![entry.clone(); 2],
                &mut ledger()?,
            )?;
            let reading = fixture.define(
                &family,
                "omega",
                SelectionSpec::Column(0),
                AffineMap::identity(2, &mut ledger()?)?,
                vec![entry; 2],
            )?;
            let axis = ReadingAxis::new(
                &fixture.cas,
                &fixture.context,
                &fixture.symbol("k")?,
                vec![1, 0],
                vec![2],
                &mut ledger()?,
            )?;
            let mut budget = nrz_bindings::Budget::new(limits)?;
            budget.work(precharge, nrz_cas::Location::new("grid boundary precharge"))?;
            if projection {
                let grid = IndexedReading::new(reading, axis, &mut ledger()?)?;
                grid.project(&mut fixture.cas, 0, &mut budget)?;
            } else {
                IndexedReading::new(reading, axis, &mut budget)?;
            }
            Ok(budget.usage().work)
        })?;
    }
    Ok(())
}

/// Compatibility scans retain exact Work ceilings without allocating new descriptors.
/// ´test:integration:joint-source-compatibility-work-boundary´
#[test]
fn joint_source_compatibility_work_boundary() -> TestResult {
    let case = sources()?;
    let model = JointSources::new(
        &case.fixture.cas,
        &case.fixture.context,
        case.declared,
        case.edges,
        &mut ledger()?,
    )?;
    let mut budget = ledger()?;
    model.check_compatible(&model, &mut budget)?;
    let work = budget.usage().work;
    for exact in [false, true] {
        let limits = nrz_bindings::Limits {
            work: work - u64::from(!exact),
            staged_payload_bytes: 0,
            ..nrz_bindings::Limits::default()
        };
        let mut budget = nrz_bindings::Budget::new(limits)?;
        assert_eq!(model.check_compatible(&model, &mut budget).is_ok(), exact);
    }
    Ok(())
}

/// An empty model still validates its exact context, and a foreign store cannot admit known edges.
/// ´test:integration:joint-reading-sources-refuse-foreign-context´
#[test]
fn joint_reading_sources_refuse_foreign_context() -> TestResult {
    let case = sources()?;
    assert!(
        JointSources::new(
            &case.fixture.cas,
            &case.fixture.symbol("zero")?,
            vec![],
            vec![],
            &mut ledger()?
        )
        .is_err()
    );
    let foreign = nrz_cas::Cas::new(nrz_bindings::Limits::default())?;
    assert!(
        JointSources::new(
            &foreign,
            &case.fixture.context,
            case.declared,
            case.edges,
            &mut ledger()?
        )
        .is_err()
    );
    Ok(())
}

/// Even explicit unknown relations cannot combine inconsistent meanings of shared free coordinates.
/// ´test:integration:joint-reading-sources-refuse-reinterpreted-family´
#[test]
fn joint_reading_sources_refuse_reinterpreted_family() -> TestResult {
    let mut case = sources()?;
    let changed = reinterpreted(&mut case)?;
    let mut unknown = case.edges;
    for edge in &mut unknown {
        edge.knowledge = SourceKnowledge::Unknown;
    }
    assert!(matches!(
        JointSources::new(
            &case.fixture.cas,
            &case.fixture.context,
            vec![changed.clone(), case.declared[1].clone()],
            unknown,
            &mut ledger()?
        ),
        Err(SourceError::Conflict)
    ));
    let diagonal = vec![SourceEdge {
        left: 0,
        right: 0,
        knowledge: SourceKnowledge::Unknown,
    }];
    let original = JointSources::new(
        &case.fixture.cas,
        &case.fixture.context,
        vec![case.declared[0].clone()],
        diagonal.clone(),
        &mut ledger()?,
    )?;
    let other = JointSources::new(
        &case.fixture.cas,
        &case.fixture.context,
        vec![changed],
        diagonal,
        &mut ledger()?,
    )?;
    assert!(matches!(
        original.check_compatible(&other, &mut ledger()?),
        Err(SourceError::Conflict)
    ));
    Ok(())
}

/// Reuse actual mathematical coordinates with an incompatible physical declaration for refusal tests.
fn reinterpreted(case: &mut Sources) -> TestResult<ReadingSource> {
    let entry = meaning("length", [1, 0, 0, 0, 0, 0, 0])?;
    let family = CoordinateFamily::new(
        &case.fixture.cas,
        &case.fixture.context,
        case.declared[0].reading().family().root(),
        vec![
            entry.clone(),
            case.declared[1].reading().entries()[0].clone(),
        ],
        &mut ledger()?,
    )?;
    let reading = case.fixture.define(
        &family,
        "v",
        SelectionSpec::Entry(EntryIndex { row: 0, column: 0 }),
        AffineMap::identity(1, &mut ledger()?)?,
        vec![entry],
    )?;
    Ok(ReadingSource::new(reading, &mut ledger()?)?)
}
