// SPDX-License-Identifier: AGPL-3.0-only
//! A late nonlinear refusal cannot publish the earlier private entries of its region.

use crate::kernel_oracle::Rational;
use crate::kernel_testing::{TestResult, axis, entry, execute, output, owner, shape};
use crate::{
    Binding, DomainReason, Elementwise, ExecutionLimits, Extent, Heap, HeapLimits, KernelError,
    OperationDraft, OperationError,
};
use nrz_units::Descriptor;

/// A failed second entry discards the entire private output and every companion reservation.
/// ´test:crate:nonlinear-late-domain-refusal-keeps-publication-empty´
#[test]
fn nonlinear_late_domain_refusal_keeps_publication_empty() -> TestResult {
    let declarations = owner()?;
    let rows = axis(2)?;
    let column = axis(1)?;
    let contract = shape(&declarations, &column, &rows)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let source = heap.import_declared_bounds(
        Extent::new(2, 1)?,
        &Descriptor::ratio(),
        &[4.0_f64, 1.0],
        &[0.25, 2.0],
    )?;
    let count = heap.descriptors.iter().count();
    let limits = ExecutionLimits::default();
    let operation = OperationDraft::map_elementwise(
        contract.clone(),
        contract.clone(),
        Elementwise::SquareRoot,
    )?
    .check(limits)?;
    let bound = Binding::new(operation, vec![source.value()], limits).validate(&mut heap)?;
    let failure = bound
        .evaluate(bound.normalized_inputs()?)
        .err()
        .ok_or("invalid second entry admitted")?;
    assert!(matches!(
        failure.cause(),
        OperationError::Kernel(KernelError::Domain {
            entry: 1,
            reason: DomainReason::NegativeRoot,
            ..
        })
    ));
    assert_eq!(failure.record().historical_evidence(), []);
    assert_eq!(bound.heap.descriptors.iter().count(), count);
    drop(bound);
    let squared = execute(
        &mut heap,
        OperationDraft::map_elementwise(contract.clone(), contract, Elementwise::Square)?,
        vec![source.value()],
    )?;
    for (index, (nominal, endpoint, denominator)) in
        [(16, 289, 16), (1, 9, 1)].into_iter().enumerate()
    {
        let (center, radius, error) = entry(&heap, output(&squared)?, index)?;
        assert!(Rational::new(nominal, 1)?.covered(center, error)?);
        assert!(Rational::new(endpoint, denominator)?.covered(center, radius)?);
    }
    Ok(())
}
