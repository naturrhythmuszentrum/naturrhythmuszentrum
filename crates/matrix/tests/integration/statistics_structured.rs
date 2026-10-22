// SPDX-License-Identifier: AGPL-3.0-only
//! Full logical covariance retains both duplicated entries and unconstrained center-only entries.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{TestResult, axis, execute, output, owner, shape};
use crate::statistics_testing::{attach, check, covariance};
use nrz_bindings::Descriptor;
use nrz_matrix::{
    EntryMap, ExactCoefficient, ExpansionRow, ExpansionTerm, Extent, Heap, HeapLimits,
    OperationDraft, ShapeLimits, StructureScope, StructuredExpansion,
};
use nrz_scalar::{LiteralLimits, Scalar};

/// Structured symmetry duplicates one source coordinate on both covariance axes.
/// ´test:integration:structured-duplication-expands-full-logical-covariance´
#[test]
fn structured_duplication_expands_full_logical_covariance() -> TestResult {
    duplication::<f32>()?;
    duplication::<f64>()
}
/// Reuse the certified affine kernel, then reshape with its exact checked entry map.
fn duplication<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let coordinates = axis(2)?;
    let logical = shape(&declarations, &coordinates, &coordinates)?;
    let free = axis(3)?;
    let column = axis(1)?;
    let input_shape = shape(&declarations, &column, &free)?;
    let rows = [selected(0)?, selected(1)?, selected(1)?, selected(2)?];
    let expansion = StructuredExpansion::check(
        &logical,
        free,
        &rows,
        StructureScope::WholeFamily,
        ShapeLimits::default(),
    )?;
    let map = expansion.linear_map(
        input_shape,
        LiteralLimits::default(),
        ShapeLimits::default(),
    )?;
    let flattened = map.output().clone();
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(
        Extent::new(3, 1)?,
        &Descriptor::ratio(),
        &[P::ONE, P::from_u8(2), P::from_u8(3)],
    )?;
    let fractions = [
        (1, 4),
        (1, 8),
        (0, 1),
        (1, 8),
        (1, 2),
        (1, 16),
        (0, 1),
        (1, 16),
        (3, 4),
    ];
    let values = fractions.map(|(n, d)| P::from_u8(n) / P::from_u8(d));
    attach(&mut heap, source.value(), &values, true)?;
    let flat = output(&execute(
        &mut heap,
        OperationDraft::linear_map(map),
        vec![source.value()],
    )?)?;
    let entry_map =
        EntryMap::reshape(flattened.extent(), logical.extent(), ShapeLimits::default())?;
    let result = output(&execute(
        &mut heap,
        OperationDraft::transform_entries(
            flattened,
            logical.domain().clone(),
            logical.codomain().clone(),
            entry_map,
            ShapeLimits::default(),
        )?,
        vec![flat.value()],
    )?)?;
    let view = heap.moments(heap.moment_companion(result.value())?)?;
    crate::statistics_testing::nonnegative_variances(view.covariance())?;
    let joint = fractions
        .into_iter()
        .map(|(n, d)| Rational::new(i128::from(n), i128::from(d)))
        .collect::<Result<Vec<_>, _>>()?;
    let transform = [1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1];
    check(
        view.covariance(),
        &covariance(&transform, 4, &transform, 4, 3, &joint)?,
    )?;
    assert_eq!(view.source_indices(), [0, 1, 2]);
    assert_eq!(
        heap.value(result.value())?.region().extent(),
        Extent::new(2, 2)?
    );
    assert!(Rational::new(1, 2)?.covered(
        view.covariance().get(1, 2)?.to_f64(),
        view.covariance().error(1, 2)?.to_f64()
    )?);
    Ok(())
}
/// H owns the exact unit coefficient and its free-coordinate index.
fn selected(index: usize) -> TestResult<ExpansionRow> {
    Ok(ExpansionRow::new(
        vec![ExpansionTerm::new(
            index,
            ExactCoefficient::unit(&Descriptor::ratio())?,
        )],
        None,
    ))
}

/// Equal nominal off-diagonal entries do not identify their independent fluctuations.
/// ´test:integration:symmetric-center-retains-unconstrained-full-entry-covariance´
#[test]
fn symmetric_center_retains_unconstrained_full_entry_covariance() -> TestResult {
    let declarations = owner()?;
    let coordinates = axis(2)?;
    let contract = shape(&declarations, &coordinates, &coordinates)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(
        Extent::new(2, 2)?,
        &Descriptor::ratio(),
        &[1.0_f64, 2.0, 2.0, 3.0],
    )?;
    let mut values = [0.0; 16];
    for (index, variance) in [0.25, 0.5, 0.75, 1.0].into_iter().enumerate() {
        *values.get_mut(index * 5).ok_or("oracle diagonal")? = variance;
    }
    attach(&mut heap, source.value(), &values, true)?;
    let result = output(&execute(
        &mut heap,
        OperationDraft::copy(vec![contract]),
        vec![source.value()],
    )?)?;
    let view = heap.moments(heap.moment_companion(result.value())?)?;
    crate::statistics_testing::nonnegative_variances(view.covariance())?;
    let expected = values
        .into_iter()
        .map(Rational::binary)
        .collect::<Result<Vec<_>, _>>()?;
    check(view.covariance(), &expected)?;
    assert_eq!(view.source_indices(), [0, 1, 2, 3]);
    Ok(())
}

/// General signed rational coefficients and offsets use the same full covariance contraction.
/// ´test:integration:structured-affine-expansion-encloses-signed-rational-oracle´
#[test]
fn structured_affine_expansion_encloses_signed_rational_oracle() -> TestResult {
    rational_expansion::<f32>()?;
    rational_expansion::<f64>()
}
/// The independent integer Jacobian is twice the declared rational map.
fn rational_expansion<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let free = axis(2)?;
    let column = axis(1)?;
    let logical = shape(&declarations, &column, &free)?;
    let half = crate::conversion_testing::measure(1, 2, 0, 1)?;
    let two = crate::conversion_testing::measure(2, 1, 0, 1)?;
    let quarter = crate::conversion_testing::measure(1, 4, 0, 1)?;
    let negative = nrz_bindings::Dimension::integers([-1, 0, 0, 0, 0, 0, 0])?;
    let zero = nrz_bindings::Measure::coherent().offset().clone();
    let definition = |scale| {
        ExactCoefficient::new(
            &Descriptor::ratio(),
            &nrz_bindings::Conversion {
                scale,
                shift: zero.clone(),
            },
        )
    };
    let rows = [
        ExpansionRow::new(
            vec![
                ExpansionTerm::new(0, definition(half.scale().clone())?),
                ExpansionTerm::new(
                    1,
                    definition(
                        negative
                            .exponents()
                            .first()
                            .ok_or("negative literal")?
                            .clone(),
                    )?,
                ),
            ],
            Some(definition(quarter.scale().clone())?),
        ),
        ExpansionRow::new(
            vec![ExpansionTerm::new(0, definition(two.scale().clone())?)],
            None,
        ),
    ];
    let expansion = StructuredExpansion::check(
        &logical,
        free,
        &rows,
        StructureScope::WholeFamily,
        ShapeLimits::default(),
    )?;
    let map = expansion.linear_map(logical, LiteralLimits::default(), ShapeLimits::default())?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = crate::statistics_testing::vector::<P>(&mut heap, true)?;
    let result = output(&execute(
        &mut heap,
        OperationDraft::linear_map(map),
        vec![source.value()],
    )?)?;
    let view = heap.moments(heap.moment_companion(result.value())?)?;
    crate::statistics_testing::nonnegative_variances(view.covariance())?;
    let doubled = [1, 4, -2, 0];
    let expected = covariance(
        &doubled,
        2,
        &doubled,
        2,
        2,
        &crate::statistics_testing::worked_covariance()?,
    )?
    .into_iter()
    .map(|value| value.divide(Rational::new(4, 1)?))
    .collect::<Result<Vec<_>, _>>()?;
    check(view.covariance(), &expected)?;
    check(
        view.means().ok_or("affine means")?,
        &[Rational::new(-7, 4)?, Rational::new(4, 1)?],
    )
}
