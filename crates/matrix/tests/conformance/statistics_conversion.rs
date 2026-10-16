// SPDX-License-Identifier: AGPL-3.0-only
//! Statistical conversion failure cannot publish an earlier deterministic candidate.

use crate::conversion_testing::{TestResult, owner, shape};
use nrz_matrix::{
    Binding, ExecutionLimits, Extent, Heap, HeapLimits, OperationDraft, OperationError,
    PrecisionContract, StatisticsError,
};
use nrz_scalar::Precision;
use nrz_units::Descriptor;

/// A finite value conversion cannot publish when its covariance cannot fit the target format.
/// ´test:integration:statistical-precision-overflow-is-atomic´
#[test]
fn statistical_precision_overflow_is_atomic() -> TestResult {
    let declarations = owner()?;
    let coordinates = shape(&declarations, &Descriptor::ratio(), 1)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
    let companion =
        crate::statistics_testing::attach(&mut heap, source.value(), &[1.0e100], false)?;
    let limits = ExecutionLimits::default();
    let contract = PrecisionContract::new(coordinates, &Descriptor::ratio())?;
    let operation =
        OperationDraft::precision(vec![contract], Precision::F64, Precision::F32).check(limits)?;
    let bound = Binding::<f64, f32>::convert_precision(operation, vec![source.value()], limits)
        .validate(&mut heap)?;
    let failure = bound
        .evaluate(bound.normalized_inputs()?)
        .err()
        .ok_or("overflow covariance admitted")?;
    assert!(matches!(
        failure.cause(),
        OperationError::Statistics(StatisticsError::Arithmetic(_))
    ));
    drop(bound);
    assert_eq!(heap.provenance_usage().records(), 0);
    assert!(heap.statistical(companion).is_ok());
    assert_eq!(heap.value(source.value())?.region().get(0, 0)?, 2.0);
    Ok(())
}

/// Center-only expansion cannot impose a companion duplication law.
/// ´test:integration:statistical-expansion-requires-whole-family-scope´
#[test]
fn statistical_expansion_requires_whole_family_scope() -> TestResult {
    let declarations = crate::kernel_testing::owner()?;
    let free = crate::kernel_testing::axis(1)?;
    let logical = crate::kernel_testing::shape(&declarations, &free, &free)?;
    let expansion = nrz_matrix::StructuredExpansion::check(
        &logical,
        free,
        &[nrz_matrix::ExpansionRow::new(
            vec![nrz_matrix::ExpansionTerm::new(
                0,
                nrz_matrix::ExactCoefficient::unit(&Descriptor::ratio())?,
            )],
            None,
        )],
        nrz_matrix::StructureScope::CenterOnly,
        nrz_matrix::ShapeLimits::default(),
    )?;
    assert!(matches!(
        expansion.linear_map(
            logical,
            nrz_scalar::LiteralLimits::default(),
            nrz_matrix::ShapeLimits::default()
        ),
        Err(OperationError::Shape(
            nrz_matrix::ShapeError::StructureScope
        ))
    ));
    Ok(())
}
