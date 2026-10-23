// SPDX-License-Identifier: AGPL-3.0-only
//! Original positive LC operator and physical scalar readings over one shared entry family.

use crate::reading_testing::TestResult;
use nrz_bindings::{
    Budget, Limits,
    readings::{
        CoordinateFamily, PhysicalReading, PositiveReciprocalMap, ReadingDefinition,
        ReciprocalJacobianCache, ReciprocalJacobianKey, Representation, RepresentationDefinition,
        algebra::{
            Cas,
            readings::{AffineMap, EntryIndex, SelectionSpec},
        },
    },
};
use nrz_matrix::{
    DependencyPolicy, EntryTicket, Extent, Heap, ReadingAdapter, ReadingCoverage, ReadingImport,
    ReadingRegion, ReciprocalDerivativeContract,
};
use nrz_scalar::{LiteralLimits, Scalar};
use std::rc::Rc;

/// Frozen exact graph, complete physical coverage and its shared derivative program.
pub(crate) struct Prepared {
    /// Original owner of every exact handle and witness.
    pub(crate) graph: Rc<Cas>,
    /// Every original stored input and its exact reading.
    pub(crate) coverage: ReadingCoverage<usize>,
    /// Shared exact derivative and numerical program.
    pub(crate) contract: ReciprocalDerivativeContract,
}

/// Measured exact preparation plus five complete live import/evaluation/access uses.
pub(crate) fn policy() -> Limits {
    Limits {
        work: 6_443_904 + 5 * (476_745 + 213 + 2),
        replay_work: 6_443_904 + 5 * (476_745 + 213 + 2),
        ..Limits::default()
    }
}

/// Every recipe, derivative, lowering and physical reading uses this one supplied ledger.
pub(crate) fn prepare(fixed: &[String], budget: &mut Budget) -> TestResult<Prepared> {
    let mut prepared = crate::reciprocal_testing::plan_with_policy(policy(), true)?;
    let recipe = PositiveReciprocalMap::prove(
        &mut prepared.case.cas,
        &prepared.frame,
        prepared.definition,
        budget,
    )?;
    let key = ReciprocalJacobianKey::new(&mut prepared.case.cas, &recipe, fixed, budget)?;
    let mut cache = ReciprocalJacobianCache::new();
    let exact = cache.get_or_derive(&mut prepared.case.cas, &key, budget)?;
    let contract = ReciprocalDerivativeContract::new(
        &prepared.case.cas,
        exact,
        LiteralLimits::default(),
        budget,
    )?;
    let family = CoordinateFamily::new(
        &prepared.case.cas,
        &prepared.case.context,
        &prepared.case.symbol("p")?,
        recipe.frame().entries().to_vec(),
        budget,
    )?;
    let mut adapters = Vec::new();
    let mut tickets = Vec::new();
    for (index, (name, stored)) in [("L", "storedL"), ("C", "storedC")].into_iter().enumerate() {
        let symbol = prepared.case.symbol(name)?;
        let definition = ReadingDefinition {
            symbol,
            selection: SelectionSpec::Entry(EntryIndex {
                row: u32::try_from(index)?,
                column: 0,
            }),
            map: AffineMap::identity(1, budget)?,
            entries: vec![recipe.frame().entries()[index].clone()],
        };
        let reading = PhysicalReading::define(&mut prepared.case.cas, &family, definition, budget)?;
        let symbol = prepared.case.symbol(stored)?;
        let representation = Representation::prove(
            &mut prepared.case.cas,
            &reading,
            RepresentationDefinition::Defined {
                symbol,
                expansion: AffineMap::identity(1, budget)?,
            },
            budget,
        )?;
        let ticket = EntryTicket {
            region: 0,
            index: u64::try_from(index)?,
        };
        adapters.push(ReadingAdapter::new(
            representation,
            vec![ticket.clone()],
            budget,
        )?);
        tickets.push(ticket);
    }
    let coverage = ReadingCoverage::new(tickets, adapters, Vec::new(), budget)?;
    Ok(Prepared {
        graph: Rc::new(prepared.case.cas),
        coverage,
        contract,
    })
}

/// Separate numerical occupations reuse the immutable exact graph and derivative contract.
pub(crate) fn import<P: Scalar + 'static>(
    heap: &mut Heap,
    prepared: &Prepared,
    values: &[P; 2],
    budget: &mut Budget,
) -> TestResult<ReadingImport<P>> {
    let region = ReadingRegion {
        extent: Extent::new(2, 1)?,
        values: values.to_vec(),
    };
    Ok(heap.import_readings(
        Rc::clone(&prepared.graph),
        prepared.coverage.clone(),
        Vec::new(),
        &[region],
        DependencyPolicy::Entries,
        budget,
    )?)
}
