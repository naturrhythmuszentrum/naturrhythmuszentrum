// SPDX-License-Identifier: AGPL-3.0-only
//! Worked interaction oracle and independent faer comparison at the public product boundary.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{TestResult, axis, entry, execute, output, owner, shape};
use nrz_matrix::{Extent, Heap, HeapLimits, OperationDraft, ShapeLimits};
use nrz_scalar::Scalar;
use nrz_units::Descriptor;

/// The design's endpoint attains the linear plus interaction terms in each output entry.
/// ´test:integration:matrix-product-encloses-the-design-interaction-example´
#[test]
fn matrix_product_encloses_the_design_interaction_example() -> TestResult {
    interaction::<f32>()?;
    interaction::<f64>()
}

/// Run the public product and compare both the nominal and all-positive endpoint problems.
fn interaction<P: Scalar + 'static>() -> TestResult {
    let declarations = owner()?;
    let coordinates = axis(2)?;
    let column = axis(1)?;
    let operator = shape(&declarations, &coordinates, &coordinates)?;
    let vector = shape(&declarations, &column, &coordinates)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let tenth = P::ONE / P::from_u8(10);
    let fifth = P::ONE / P::from_u8(5);
    let quarter = P::ONE / P::from_u8(4);
    let left = heap.import_declared_bounds(
        Extent::new(2, 2)?,
        &Descriptor::ratio(),
        &[P::from_u8(2), P::ONE, P::ZERO, P::from_u8(3)],
        &[tenth, P::ZERO, P::ZERO, fifth],
    )?;
    let right = heap.import_declared_bounds(
        Extent::new(2, 1)?,
        &Descriptor::ratio(),
        &[P::ONE, P::from_u8(2)],
        &[tenth, quarter],
    )?;
    let result = execute(
        &mut heap,
        OperationDraft::matmul(operator, vector, ShapeLimits::default())?,
        vec![left.value(), right.value()],
    )?;
    for (index, (nominal, numerator, denominator)) in
        [(2, 231, 100), (7, 83, 10)].into_iter().enumerate()
    {
        let (center, radius, error) = entry(&heap, output(&result)?, index)?;
        assert!(Rational::new(nominal, 1)?.covered(center, error)?);
        assert!(Rational::new(numerator, denominator)?.covered(center, radius)?);
        let corner = represented_corner(index, tenth.to_f64(), fifth.to_f64(), quarter.to_f64())?;
        assert!(corner.covered(center, radius)?);
        assert!(
            Rational::new(if index == 0 { 31 } else { 130 }, 100)?
                .at_most(Rational::binary(radius)?)?
        );
    }
    check_example_terms(&heap, output(&result)?)?;
    check_faer(&heap, output(&result)?)?;
    Ok(())
}

/// Recompute the endpoint from the actual represented input radii using exact integer fractions.
fn represented_corner(index: usize, tenth: f64, fifth: f64, quarter: f64) -> TestResult<Rational> {
    let first = Rational::new(1, 1)?.add(Rational::binary(tenth)?)?;
    if index == 0 {
        Rational::new(2, 1)?
            .add(Rational::binary(tenth)?)?
            .multiply(first)
    } else {
        first.add(
            Rational::new(3, 1)?
                .add(Rational::binary(fifth)?)?
                .multiply(Rational::new(2, 1)?.add(Rational::binary(quarter)?)?)?,
        )
    }
}

/// A separately assembled backend matrix checks each represented center independently.
fn check_faer<P: Scalar + 'static>(
    heap: &Heap,
    result: nrz_matrix::PublishedBundle<P>,
) -> TestResult {
    let mut left = faer::Mat::<f64>::zeros(2, 2);
    left[(0, 0)] = 2.0;
    left[(1, 0)] = 1.0;
    left[(1, 1)] = 3.0;
    let mut right = faer::Mat::<f64>::zeros(2, 1);
    right[(0, 0)] = 1.0;
    right[(1, 0)] = 2.0;
    let expected = &left * &right;
    for row in 0..2 {
        let (center, _, error) = entry(heap, result, row)?;
        assert_eq!(center.to_bits(), expected[(row, 0)].to_bits());
        assert!(Rational::binary(expected[(row, 0)])?.covered(center, error)?);
    }
    Ok(())
}

/// Copies and repeated selections retain each contribution's target and logical map.
/// ´test:integration:product-contributions-follow-selection-copy-and-retirement´
#[test]
fn product_contributions_follow_selection_copy_and_retirement() -> TestResult {
    let declarations = owner()?;
    let coordinate = axis(1)?;
    let contract = shape(&declarations, &coordinate, &coordinate)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let left = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[3.0_f64],
        &[0.125],
    )?;
    let right = heap.import_declared_bounds(
        Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[2.0_f64],
        &[0.25],
    )?;
    let original = heap.value(right.value())?.stamp();
    let product = execute(
        &mut heap,
        OperationDraft::matmul(contract.clone(), contract, ShapeLimits::default())?,
        vec![left.value(), right.value()],
    )?;
    let result_shape = heap
        .operation_record(product.record())?
        .operation()
        .outputs()
        .first()
        .ok_or("shape")?
        .shape()
        .clone();
    let reference = captured_bounds(&heap, output(&product)?)?;
    check_terms(&heap, output(&product)?, 1, &reference)?;
    let copied = execute(
        &mut heap,
        OperationDraft::copy(vec![result_shape.clone()]),
        vec![output(&product)?.value()],
    )?;
    check_terms(&heap, output(&copied)?, 1, &reference)?;
    let selected = repeated_product(&mut heap, &result_shape, output(&product)?)?;
    heap.compact()?;
    check_terms(&heap, selected, 2, &reference)?;
    let interaction = selected
        .contribution(nrz_matrix::ProductTerm::Interaction)
        .ok_or("interaction")?;
    heap.replace_declared_bounds(right.value(), original.version(), &[2.0], &[0.5])?;
    assert!(heap.companion(interaction).is_err());
    heap.retire(selected.value())?;
    heap.retire(output(&copied)?.value())?;
    heap.retire(output(&product)?.value())?;
    heap.compact()?;
    assert!(heap.companion(interaction).is_err());
    assert!(Rational::new(3, 1)?.covered(heap.value(left.value())?.region().get(0, 0)?, 0.0)?);
    Ok(())
}

/// Duplicate one checked result coordinate and all of its overlapping bound regions.
fn repeated_product(
    heap: &mut Heap,
    contract: &nrz_matrix::CheckedShape,
    source: nrz_matrix::PublishedBundle<f64>,
) -> TestResult<nrz_matrix::PublishedBundle<f64>> {
    let limits = ShapeLimits::default();
    let coordinate = contract
        .codomain()
        .entries()
        .first()
        .ok_or("coordinate")?
        .clone();
    let codomain = nrz_matrix::Axis::new(
        vec![coordinate.clone(), coordinate],
        nrz_matrix::Partition::whole(2, limits)?,
        limits,
    )?;
    let map =
        nrz_matrix::EntryMap::new(Extent::new(1, 1)?, Extent::new(2, 1)?, vec![0, 0], limits)?;
    let draft = OperationDraft::transform_entries(
        contract.clone(),
        contract.domain().clone(),
        codomain,
        map,
        limits,
    )?;
    output(&execute(heap, draft, vec![source.value()])?)
}

/// Exact dyadic terms expose target identity and cannot be mistaken for extra total coverage.
fn check_terms(
    heap: &Heap,
    result: nrz_matrix::PublishedBundle<f64>,
    rows: usize,
    reference: &[u64; 4],
) -> TestResult {
    use nrz_matrix::ProductTerm;
    let target = heap.value(result.value())?.stamp();
    for ((term, numerator, denominator), original) in [
        (ProductTerm::RightInput, 3, 4),
        (ProductTerm::LeftInput, 1, 4),
        (ProductTerm::Interaction, 1, 32),
    ]
    .into_iter()
    .zip(reference)
    {
        let handle = result.contribution(term).ok_or("term")?;
        let region = heap.companion(handle)?;
        assert_eq!(region.target_stamp(), target);
        assert_eq!(region.region().extent(), Extent::new(rows, 1)?);
        for row in 0..rows {
            let magnitude = region.region().get(row, 0)?;
            assert!(Rational::new(numerator, denominator)?.at_most(Rational::binary(magnitude)?)?);
            assert_eq!(magnitude.to_bits(), *original);
        }
    }
    for row in 0..rows {
        let (center, total, error) = entry(heap, result, row)?;
        assert!(Rational::new(6, 1)?.covered(center, error)?);
        assert!(Rational::new(33, 32)?.at_most(Rational::binary(total)?)?);
        assert_eq!(total.to_bits(), reference[3]);
        assert!(Rational::new(225, 32)?.covered(center, total)?);
    }
    Ok(())
}

/// The exact design vectors are outwardly reproduced, and omitting interaction fails.
fn check_example_terms<P: Scalar + 'static>(
    heap: &Heap,
    result: nrz_matrix::PublishedBundle<P>,
) -> TestResult {
    use nrz_matrix::ProductTerm;
    let terms = [
        ProductTerm::RightInput,
        ProductTerm::LeftInput,
        ProductTerm::Interaction,
    ];
    let exact = [[(1, 5), (17, 20)], [(1, 10), (2, 5)], [(1, 100), (1, 20)]];
    for (term, coordinates) in terms.into_iter().zip(exact) {
        let view = heap.companion(result.contribution(term).ok_or("term")?)?;
        assert_eq!(view.target_stamp(), heap.value(result.value())?.stamp());
        for (row, (numerator, denominator)) in coordinates.into_iter().enumerate() {
            let stored = Rational::binary(view.region().get(row, 0)?.to_f64())?;
            let target = Rational::new(numerator, denominator)?;
            assert!(target.at_most(stored)?);
            let slack = Rational::new(
                1,
                if P::PRECISION == nrz_scalar::Precision::F32 {
                    1_i128 << 20
                } else {
                    1_i128 << 45
                },
            )?;
            assert!(stored.at_most(target.add(slack)?)?);
        }
    }
    let interaction = heap.companion(
        result
            .contribution(ProductTerm::Interaction)
            .ok_or("interaction")?,
    )?;
    for (row, (numerator, denominator)) in [(231, 100), (83, 10)].into_iter().enumerate() {
        let (center, radius, _) = entry(heap, result, row)?;
        let without = Rational::binary(radius)?.subtract(Rational::binary(
            interaction.region().get(row, 0)?.to_f64(),
        )?)?;
        let required =
            Rational::new(numerator, denominator)?.subtract(Rational::binary(center)?)?;
        assert!(!required.at_most(without)?);
    }
    Ok(())
}

/// Snapshot published bound bytes before any copy or entry transformation takes place.
fn captured_bounds(heap: &Heap, result: nrz_matrix::PublishedBundle<f64>) -> TestResult<[u64; 4]> {
    use nrz_matrix::ProductTerm;
    let mut captured = [0; 4];
    for (slot, term) in captured.iter_mut().zip([
        ProductTerm::RightInput,
        ProductTerm::LeftInput,
        ProductTerm::Interaction,
    ]) {
        *slot = heap
            .companion(result.contribution(term).ok_or("term")?)?
            .region()
            .get(0, 0)?
            .to_bits();
    }
    captured[3] = entry(heap, result, 0)?.1.to_bits();
    Ok(captured)
}
