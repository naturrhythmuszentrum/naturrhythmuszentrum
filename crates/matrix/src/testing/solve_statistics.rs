// SPDX-License-Identifier: AGPL-3.0-only
//! Shared-source solve fixtures contain declarations only; rational expectations stay independent.

use crate::kernel_testing::TestResult;
use crate::test_api::{
    CompanionSetDraft, CovarianceBlock, Extent, Heap, JointLimits, JointModel, PublishedBundle,
    Source, SourceFactor, TargetMap,
};
use nrz_bindings::Descriptor;
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// One scalar operator and two RHS entries share a declared rank-one source family.
pub(crate) fn common<P: Scalar + 'static>(
    heap: &mut Heap,
    coefficients: &[P],
    means: bool,
) -> TestResult<[PublishedBundle<P>; 2]> {
    let quantity = Descriptor::ratio();
    let operator = heap.import_represented(Extent::new(1, 1)?, &quantity, &[P::from_u8(2)])?;
    let rhs = heap.import_represented(
        Extent::new(1, 2)?,
        &quantity,
        &[P::from_u8(2), P::from_u8(4)],
    )?;
    let limits = JointLimits::default();
    let sources = (0..3)
        .map(|_| Source::new(&quantity, 1))
        .collect::<Result<Vec<_>, _>>()?;
    let model = JointModel::from_factor(
        &sources,
        SourceFactor::new(3, coefficients, &[P::ONE / P::from_u8(16)], limits)?,
        &std::array::from_fn::<_, 9, _>(|_| CovarianceBlock::Known),
        limits,
    )?;
    let targets = [
        TargetMap::capture(heap, operator.value())?,
        TargetMap::capture(heap, rhs.value())?,
    ];
    let mut companion = CompanionSetDraft::new(model, targets, [vec![0], vec![1, 2]]);
    if means {
        let point = |value| Finite::new(value).map(Enclosure::singleton);
        companion = companion.with_means(
            heap,
            [
                vec![point(P::from_u8(2))?],
                vec![point(P::from_u8(2))?, point(P::from_u8(4))?],
            ],
            &ArithmeticContext::new()?,
            limits,
        )?;
    }
    heap.publish_joint(companion, limits)?;
    Ok([operator, rhs])
}
