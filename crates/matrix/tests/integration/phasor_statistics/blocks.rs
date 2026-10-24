// SPDX-License-Identifier: AGPL-3.0-only
//! Complete source and result phasor inventories are constructed before importing any means.

use super::reading_testing::{Fixture, TestResult, ledger, map, matrix, real};
use super::{components, tickets};
use nrz_bindings::{
    CompanionKind,
    readings::{
        CompanionParameters, CompanionReading, CompanionReadingKind, CoordinateFamily,
        EntryMeaning, ReadingRelation, Representation, RepresentationDefinition,
        algebra::{
            readings::{AffineMap, SelectionSpec},
            syntax::{AtomicSort, Field, Sort},
        },
    },
};
use nrz_matrix::{CompanionAdapter, ReadingAdapter, ReadingCoverage};

/// Whole phasor and every current component/copy, with complete shared-entry equations.
pub(super) struct Block {
    /// Actual complete Complex reconstruction.
    pub(super) representation: Representation,
    /// Whole reading, four scalar copies, and an optional component vector.
    pub(super) adapters: Vec<ReadingAdapter<usize>>,
    /// Complete required intersection equations.
    pub(super) overlaps: Vec<ReadingRelation>,
    /// Output total/error parameters; input means use represented-input coverage.
    pub(super) companions: Vec<CompanionAdapter<usize>>,
}
impl Block {
    /// A result block owns one complete output value region and two radius regions.
    pub(super) fn coverage(&self) -> TestResult<ReadingCoverage<usize>> {
        Ok(ReadingCoverage::new(
            tickets(0, &[0, 1, 2, 3]),
            self.adapters.clone(),
            self.overlaps.clone(),
            &mut ledger()?,
        )?)
    }
}

/// Original source and output symbols never reuse one another's free declarations.
pub(super) fn declare(
    declarations: &mut Vec<(String, Sort)>,
    prefix: &str,
    width: u32,
    output: bool,
) {
    declarations.extend([
        (format!("{prefix}_root"), matrix(width, 1)),
        (format!("{prefix}_block"), matrix(2, 2)),
        (
            prefix.to_owned(),
            Sort::Value(AtomicSort::Scalar(Field::Complex)),
        ),
    ]);
    for index in 0..4 {
        declarations.push((format!("{prefix}_{index}"), real()));
        if output {
            for channel in ["total", "error"] {
                for suffix in ["source", "output"] {
                    declarations
                        .push((format!("{prefix}_{index}_{channel}_{suffix}"), matrix(1, 1)));
                }
            }
        }
    }
    if output {
        declarations.push((format!("{prefix}_vector"), matrix(2, 1)));
    }
}

/// A fixed imaginary impedance or a real source amplitude expands to its whole exact block.
pub(super) fn input(
    fixture: &mut Fixture,
    prefix: &str,
    entry: EntryMeaning,
    reactance: Option<i64>,
    region: usize,
) -> TestResult<Block> {
    let (coefficients, offsets) = reactance.map_or(([1, 0, 0, 1], [0; 4]), |value| {
        ([0; 4], [0, value, -value, 0])
    });
    let representation = representation(fixture, prefix, entry, 1, &coefficients, &offsets)?;
    views(fixture, prefix, representation, region, false)
}

/// A varying original imaginary coordinate is admitted for solves but has no exact affine current law.
pub(super) fn varying(
    fixture: &mut Fixture,
    prefix: &str,
    entry: EntryMeaning,
    region: usize,
) -> TestResult<Block> {
    let representation = representation(fixture, prefix, entry, 1, &[0, 1, -1, 0], &[0; 4])?;
    views(fixture, prefix, representation, region, false)
}

/// Each current has its own canonical two-coordinate root, while its original equation keeps voltage shared.
pub(super) fn output(
    fixture: &mut Fixture,
    prefix: &str,
    entry: EntryMeaning,
) -> TestResult<Block> {
    let representation = representation(
        fixture,
        prefix,
        entry,
        2,
        &[1, 0, 0, 1, 0, -1, 1, 0],
        &[0; 4],
    )?;
    views(fixture, prefix, representation, 0, true)
}

/// Every original meaning is admitted before applying a zero, repeated or signed coefficient.
fn representation(
    fixture: &mut Fixture,
    prefix: &str,
    entry: EntryMeaning,
    width: u32,
    coefficients: &[i64],
    offsets: &[i64],
) -> TestResult<Representation> {
    let root = CoordinateFamily::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol(&format!("{prefix}_root"))?,
        vec![entry.clone(); usize::try_from(width)?],
        &mut ledger()?,
    )?;
    let symbol = fixture.symbol(&format!("{prefix}_block"))?;
    let family = root.expand(
        &mut fixture.cas,
        &symbol,
        &map(width, 4, coefficients, offsets)?,
        vec![entry.clone(); 4],
        &mut ledger()?,
    )?;
    let whole = fixture.define(
        &family,
        prefix,
        SelectionSpec::Block {
            row: 0,
            column: 0,
            rows: 2,
            columns: 2,
        },
        map(4, 2, &[1, 0, 0, 0, 0, 1, 0, 0], &[0; 2])?,
        vec![entry; 2],
    )?;
    Ok(Representation::prove(
        &mut fixture.cas,
        &whole,
        RepresentationDefinition::ComplexBlock(symbol),
        &mut ledger()?,
    )?)
}

/// Component views reuse actual selected copies; output parameters are declared before numerical use.
fn views(
    fixture: &mut Fixture,
    prefix: &str,
    representation: Representation,
    region: usize,
    output: bool,
) -> TestResult<Block> {
    let mut adapters = vec![ReadingAdapter::new(
        representation.clone(),
        tickets(region, &[0, 1, 2, 3]),
        &mut ledger()?,
    )?];
    let mut overlaps = Vec::new();
    components(
        fixture,
        representation.reading(),
        prefix,
        region,
        &mut adapters,
        &mut overlaps,
    )?;
    let companions = if output {
        vector(fixture, prefix, &mut adapters, &mut overlaps)?;
        parameters(fixture, prefix, &adapters[1..5])?
    } else {
        Vec::new()
    };
    Ok(Block {
        representation,
        adapters,
        overlaps,
        companions,
    })
}

/// Complete first-column projections retain both scalar-copy overlap equations.
fn vector(
    fixture: &mut Fixture,
    prefix: &str,
    adapters: &mut Vec<ReadingAdapter<usize>>,
    relations: &mut Vec<ReadingRelation>,
) -> TestResult {
    let whole = adapters[0].reading().clone();
    let key = format!("{prefix}_vector");
    let reading = fixture.define(
        whole.family(),
        &key,
        SelectionSpec::Column(0),
        AffineMap::identity(2, &mut ledger()?)?,
        whole.entries().to_vec(),
    )?;
    let symbol = fixture.symbol(&key)?;
    let representation = Representation::prove(
        &mut fixture.cas,
        &reading,
        RepresentationDefinition::Defined {
            symbol,
            expansion: AffineMap::identity(2, &mut ledger()?)?,
        },
        &mut ledger()?,
    )?;
    for (original, selected, result) in [
        (0, vec![0, 1], vec![0, 1]),
        (1, vec![0], vec![0]),
        (2, vec![0], vec![1]),
    ] {
        let left = adapters[original].reading().project_selected(
            &mut fixture.cas,
            &selected,
            &mut ledger()?,
        )?;
        let right = reading.project_selected(&mut fixture.cas, &result, &mut ledger()?)?;
        relations.push(ReadingRelation::prove(
            &mut fixture.cas,
            left,
            right,
            &mut ledger()?,
        )?);
    }
    adapters.push(ReadingAdapter::new(
        representation,
        tickets(0, &[0, 1]),
        &mut ledger()?,
    )?);
    Ok(())
}

/// Every current copy has independently addressed total and numerical-error parameters.
fn parameters(
    fixture: &mut Fixture,
    prefix: &str,
    copies: &[ReadingAdapter<usize>],
) -> TestResult<Vec<CompanionAdapter<usize>>> {
    let mut result = Vec::new();
    for (index, target) in copies.iter().enumerate() {
        for (channel, kind, region) in [
            ("total", CompanionKind::DeterministicBound, 1),
            ("error", CompanionKind::NumericalError, 2),
        ] {
            let key = format!("{prefix}_{index}_{channel}");
            let parameters = CompanionParameters::new(
                &fixture.cas,
                &fixture.context,
                &fixture.symbol(&format!("{key}_source"))?,
                CompanionReadingKind::Radius(kind),
                std::slice::from_ref(target.reading()),
                &mut ledger()?,
            )?;
            let symbol = fixture.symbol(&format!("{key}_output"))?;
            let companion = CompanionReading::define(
                &mut fixture.cas,
                &symbol,
                &parameters,
                std::slice::from_ref(target.reading()),
                &mut ledger()?,
            )?;
            result.push(CompanionAdapter::new(
                companion,
                tickets(region, &[u64::try_from(index)?]),
                vec![target.clone()],
                &mut ledger()?,
            )?);
        }
    }
    Ok(result)
}

/// Only the nonzero fixed-reactance copies need explicit conservative literal-import coverage.
pub(super) fn error_declarations(declarations: &mut Vec<(String, Sort)>, prefix: &str) {
    for index in [1, 2] {
        for channel in ["total", "error"] {
            for suffix in ["source", "output"] {
                declarations.push((
                    format!("{prefix}_{index}_import_{channel}_{suffix}"),
                    matrix(1, 1),
                ));
            }
        }
    }
}

/// Stored exact integers retain declared numerical radii covering the conservative literal decoder.
pub(super) fn cover_reactance(
    fixture: &mut Fixture,
    prefix: &str,
    block: &mut Block,
    offset: u64,
) -> TestResult {
    for (position, index) in [1, 2].into_iter().enumerate() {
        let target = &block.adapters[index + 1];
        for (channel, kind, region) in [
            ("total", CompanionKind::DeterministicBound, 6),
            ("error", CompanionKind::NumericalError, 5),
        ] {
            let key = format!("{prefix}_{index}_import_{channel}");
            let parameters = CompanionParameters::new(
                &fixture.cas,
                &fixture.context,
                &fixture.symbol(&format!("{key}_source"))?,
                CompanionReadingKind::Radius(kind),
                std::slice::from_ref(target.reading()),
                &mut ledger()?,
            )?;
            let symbol = fixture.symbol(&format!("{key}_output"))?;
            let companion = CompanionReading::define(
                &mut fixture.cas,
                &symbol,
                &parameters,
                std::slice::from_ref(target.reading()),
                &mut ledger()?,
            )?;
            block.companions.push(CompanionAdapter::new(
                companion,
                tickets(region, &[offset + u64::try_from(position)?]),
                vec![target.clone()],
                &mut ledger()?,
            )?);
        }
    }
    Ok(())
}
