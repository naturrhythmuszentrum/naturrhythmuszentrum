// SPDX-License-Identifier: AGPL-3.0-only
//! Successful explicit zero blocks and exact source-count boundaries.

use crate::testing::TestResult;
use nrz_matrix::{CovarianceBlock, JointLimits, JointModel, Source, SourceFactor};
use nrz_scalar::ArithmeticContext;
use nrz_units::Descriptor;

/// A zero block is available only through an explicit supported zero-covariance declaration.
/// ´test:integration:declared-zero-block-is-available´
#[test]
fn declared_zero_block_is_available() -> TestResult {
    let limits = JointLimits::default();
    let sources = [
        Source::new(&Descriptor::ratio(), 1)?,
        Source::new(&Descriptor::ratio(), 1)?,
    ];
    let factor = SourceFactor::new(2, &[1.0_f64, 0.0, 0.0, 1.0], &[1.0, 1.0], limits)?;
    let zero = CovarianceBlock::DeclaredZero("zero covariance is a model assumption".into());
    let blocks = [
        CovarianceBlock::Known,
        zero.clone(),
        zero,
        CovarianceBlock::Known,
    ];
    let model = JointModel::from_factor(&sources, factor, &blocks, limits)?;
    model.require_blocks(&[0], &[1], limits)?;
    let entry = model.covariance(0, 1, &ArithmeticContext::new()?, limits)?;
    assert_eq!(entry.lower().get().to_bits(), 0.0_f64.to_bits());
    assert_eq!(entry.upper().get().to_bits(), 0.0_f64.to_bits());
    Ok(())
}

/// The ordered source-count limit is inclusive at the required count.
/// ´test:integration:joint-source-count-boundaries´
#[test]
fn joint_source_count_boundaries() {
    for sources in [1, 2, 3] {
        let factor = SourceFactor::new(2, &[1.0_f32, 2.0], &[1.0], JointLimits::new(sources, 2, 3));
        assert_eq!(factor.is_ok(), sources >= 2);
    }
}
