// SPDX-License-Identifier: AGPL-3.0-only
//! Structured coordinates retain typed zeros, affine offsets and exact metric weights.

use crate::shape_testing::{ResultOf, axis, dense, ratio_shape, ratios};
use nrz_matrix::{
    EntryDraft, ExactCoefficient, ExpansionNorm, ExpansionRow, ExpansionTerm, ShapeError,
    ShapeLimits, StructureScope, StructuredExpansion,
};
use nrz_units::{Descriptor, Dimension, Measure};

/// A row duplicates one free coordinate with its exact unit coefficient.
fn selected(index: usize) -> ResultOf<ExpansionRow> {
    Ok(ExpansionRow::new(
        vec![ExpansionTerm::new(
            index,
            ExactCoefficient::unit(&Descriptor::ratio())?,
        )],
        None,
    ))
}

/// Symmetric off-diagonal duplication contributes twice to the logical Frobenius metric.
/// ´test:integration:symmetric-expansion-counts-off-diagonal-twice´
#[test]
fn symmetric_expansion_counts_off_diagonal_twice() -> ResultOf<()> {
    let snapshot = ratios()?;
    let coordinates = axis(&["a", "b"], &Descriptor::ratio())?;
    let logical = ratio_shape(&snapshot, &coordinates, &coordinates)?;
    let free = axis(&["aa", "ab", "bb"], &Descriptor::ratio())?;
    let rows = [selected(0)?, selected(1)?, selected(1)?, selected(2)?];
    let expansion = StructuredExpansion::check(
        &logical,
        free,
        &rows,
        StructureScope::WholeFamily,
        ShapeLimits::default(),
    )?;
    let metric = expansion.metric(ExpansionNorm::LogicalFrobenius, ShapeLimits::default())?;
    let literals = Dimension::integers([1, 2, 0, 0, 0, 0, 0])?;
    let one = literals.exponents().first().ok_or("literal one")?;
    let two = literals.exponents().get(1).ok_or("literal two")?;
    let coherent = Measure::coherent();
    for (index, coefficient) in metric.entries().iter().enumerate() {
        let expected = match index {
            0 | 8 => one,
            4 => two,
            _ => coherent.offset(),
        };
        assert_eq!(&coefficient.scale, expected);
    }
    assert!(matches!(
        expansion.metric(ExpansionNorm::PackedEuclidean, ShapeLimits::default()),
        Err(ShapeError::UnsupportedMetric)
    ));
    assert!(expansion.companion_expansion().is_ok());
    assert_eq!(expansion.extent().rows(), 4);
    assert_eq!(expansion.extent().columns(), 3);
    assert_eq!(
        expansion
            .entries()
            .get(5)
            .ok_or("mirrored coefficient")?
            .exact_map()
            .scale,
        expansion
            .entries()
            .get(6)
            .ok_or("mirrored coefficient")?
            .exact_map()
            .scale
    );
    Ok(())
}

/// Center symmetry alone cannot reduce companion coordinates; exact selections can be isometric.
/// ´test:integration:center-only-expansion-refuses-companion-compression´
#[test]
fn center_only_expansion_refuses_companion_compression() -> ResultOf<()> {
    let snapshot = ratios()?;
    let coordinates = axis(&["a", "b"], &Descriptor::ratio())?;
    let column = axis(&["one"], &Descriptor::ratio())?;
    let logical = ratio_shape(&snapshot, &column, &coordinates)?;
    let rows = [selected(1)?, selected(0)?];
    let expansion = StructuredExpansion::check(
        &logical,
        coordinates,
        &rows,
        StructureScope::CenterOnly,
        ShapeLimits::default(),
    )?;
    assert!(matches!(
        expansion.companion_expansion(),
        Err(ShapeError::StructureScope)
    ));
    assert!(
        expansion
            .metric(ExpansionNorm::PackedEuclidean, ShapeLimits::default())
            .is_ok()
    );
    Ok(())
}

/// Affine offsets retain physical descriptors and omitted coefficients are typed quotient zeros.
/// ´test:integration:affine-expansion-keeps-typed-offsets-and-zeros´
#[test]
fn affine_expansion_keeps_typed_offsets_and_zeros() -> ResultOf<()> {
    let snapshot = ratios()?;
    let coordinates = axis(&["a", "b"], &Descriptor::ratio())?;
    let column = axis(&["one"], &Descriptor::ratio())?;
    let logical = ratio_shape(&snapshot, &column, &coordinates)?;
    let rows = [
        ExpansionRow::new(
            Vec::new(),
            Some(ExactCoefficient::unit(&Descriptor::ratio())?),
        ),
        selected(0)?,
    ];
    let expansion = StructuredExpansion::check(
        &logical,
        column,
        &rows,
        StructureScope::WholeFamily,
        ShapeLimits::default(),
    )?;
    assert!(
        expansion
            .entries()
            .first()
            .ok_or("typed zero")?
            .exact_map()
            .scale
            .is_zero()
    );
    assert_eq!(
        expansion
            .entries()
            .first()
            .ok_or("typed zero")?
            .pairing()
            .dimension(),
        &Dimension::zero()
    );
    assert_eq!(
        expansion
            .offsets()
            .first()
            .ok_or("offset")?
            .exact_map()
            .scale,
        Measure::coherent().scale().clone()
    );
    assert!(
        expansion
            .offsets()
            .get(1)
            .ok_or("zero offset")?
            .exact_map()
            .scale
            .is_zero()
    );
    assert!(matches!(
        expansion.metric(
            ExpansionNorm::LogicalFrobenius,
            ShapeLimits::new(2, 4, 2, 1)
        ),
        Err(ShapeError::Limit)
    ));
    Ok(())
}

/// A structure cannot overwrite a typed zero or hide an invalid coefficient in a repeated term.
/// ´test:integration:structured-expansion-refuses-contradictory-rows´
#[test]
fn structured_expansion_refuses_contradictory_rows() -> ResultOf<()> {
    let snapshot = ratios()?;
    let coordinate = axis(&["a"], &Descriptor::ratio())?;
    let logical = dense(
        &snapshot,
        &coordinate,
        &coordinate,
        vec![EntryDraft::Zero(Descriptor::ratio())],
    )?;
    assert!(
        StructuredExpansion::check(
            &logical,
            coordinate.clone(),
            &[selected(0)?],
            StructureScope::WholeFamily,
            ShapeLimits::default()
        )
        .is_err()
    );
    let zero = ExpansionRow::new(Vec::new(), None);
    assert!(
        StructuredExpansion::check(
            &logical,
            coordinate.clone(),
            &[zero],
            StructureScope::WholeFamily,
            ShapeLimits::default()
        )
        .is_ok()
    );
    let ordinary = ratio_shape(&snapshot, &coordinate, &coordinate)?;
    let duplicate = ExpansionRow::new(
        vec![
            ExpansionTerm::new(0, ExactCoefficient::unit(&Descriptor::ratio())?),
            ExpansionTerm::new(0, ExactCoefficient::zero(&Descriptor::ratio())?),
        ],
        None,
    );
    assert!(matches!(
        StructuredExpansion::check(
            &ordinary,
            coordinate.clone(),
            &[duplicate],
            StructureScope::WholeFamily,
            ShapeLimits::default()
        ),
        Err(ShapeError::Partition)
    ));
    let wrong = crate::shape_testing::quantity("length", [1, 0, 0, 0, 0, 0, 0])?;
    let row = ExpansionRow::new(
        vec![ExpansionTerm::new(0, ExactCoefficient::unit(&wrong)?)],
        None,
    );
    assert!(
        StructuredExpansion::check(
            &ordinary,
            coordinate,
            &[row],
            StructureScope::WholeFamily,
            ShapeLimits::default()
        )
        .is_err()
    );
    Ok(())
}

/// A general dense expansion produces off-diagonal Gram terms, without an assumed packed norm.
/// ´test:integration:dense-expansion-derives-full-gram-matrix´
#[test]
fn dense_expansion_derives_full_gram_matrix() -> ResultOf<()> {
    let snapshot = ratios()?;
    let coordinates = axis(&["a", "b"], &Descriptor::ratio())?;
    let column = axis(&["one"], &Descriptor::ratio())?;
    let logical = ratio_shape(&snapshot, &column, &coordinates)?;
    let mixed = ExpansionRow::new(
        vec![
            ExpansionTerm::new(0, ExactCoefficient::unit(&Descriptor::ratio())?),
            ExpansionTerm::new(1, ExactCoefficient::unit(&Descriptor::ratio())?),
        ],
        None,
    );
    let expansion = StructuredExpansion::check(
        &logical,
        coordinates,
        &[mixed, selected(1)?],
        StructureScope::WholeFamily,
        ShapeLimits::default(),
    )?;
    let metric = expansion.metric(ExpansionNorm::LogicalFrobenius, ShapeLimits::default())?;
    let literals = Dimension::integers([1, 1, 1, 2, 0, 0, 0])?;
    for (actual, expected) in metric.entries().iter().zip(literals.exponents()) {
        assert_eq!(&actual.scale, expected);
    }
    assert!(matches!(
        expansion.metric(ExpansionNorm::PackedEuclidean, ShapeLimits::default()),
        Err(ShapeError::UnsupportedMetric)
    ));
    Ok(())
}
