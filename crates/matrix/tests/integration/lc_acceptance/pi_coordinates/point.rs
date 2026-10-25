// SPDX-License-Identifier: AGPL-3.0-only
//! Pi-dependent point offsets affect centers while both error families retain only scale.

use super::{TestResult, exact_policy, fixture, ledger, oracle, pi, state_fixture};
use crate::kernel_oracle::Rational;
use nrz_bindings::{
    Descriptor, Kind, LimitsOverride, Measure, OriginDefinition, ReferenceDefinition, Role, Scale,
    readings::EntryMeaning,
};
use nrz_matrix::PiCoordinateContract;
use nrz_scalar::{ArithmeticContext, Scalar};

/// Point normalization encloses its irrational offset and restores the same physical endpoint boxes.
/// ´test:integration:pi-live-coordinate-affine-point-endpoints´
#[test]
fn pi_live_coordinate_affine_point_endpoints() -> TestResult {
    point::<f32>()?;
    point::<f64>()
}

/// The exact origin is one half of a coherent angle and the reference is one pi angle unit.
pub(super) fn point_plan() -> TestResult<state_fixture::Plan> {
    let angle = Descriptor::new(Kind::Angle, Measure::coherent(), Role::Point)?;
    let half = nrz_bindings::readings::algebra::Rational::parse("1/2", &mut ledger()?)?;
    let origin = Descriptor::new(
        Kind::Angle,
        Measure::new(
            "phase origin",
            Scale::Exact(nrz_bindings::readings::algebra::Rational::one()),
            half.clone(),
        )?,
        Role::Point,
    )?;
    let reference = ReferenceDefinition::new("half-turn", &angle.difference(), Scale::pi()?)?;
    let current = crate::reading_testing::meaning("current", [0, 0, 0, 1, 0, 0, 0])?;
    let current_reference =
        ReferenceDefinition::new("half-current", current.descriptor(), Scale::Exact(half))?;
    let entries = [
        EntryMeaning::new(
            angle,
            reference,
            Some(OriginDefinition::new("phase origin", &origin)?),
            &mut ledger()?,
        )?,
        EntryMeaning::new(
            current.descriptor().clone(),
            current_reference,
            None,
            &mut ledger()?,
        )?,
    ];
    state_fixture::covariance_plan(&entries)
}

/// Independent rational endpoints include the offset exactly once in each center family.
fn point<P: Scalar + 'static>() -> TestResult {
    let plan = point_plan()?;
    let context = ArithmeticContext::new()?;
    let circle = pi::<P>(&context, &mut ledger()?)?;
    let contract = PiCoordinateContract::new(
        &plan.graph,
        &plan.contract,
        &circle,
        &exact_policy(),
        &context,
        &mut ledger()?,
    )?;
    let (heap, source) = fixture::imported::<P>(&plan)?;
    let policy = LimitsOverride::default();
    let normalized =
        heap.normalize_certified_reading(&source, &contract, &policy, &context, &mut ledger()?)?;
    let factors = oracle::factor(1, 1, true, 56)?;
    for (bounds, radii) in [
        ([(2, 1), (3, 1)], normalized.totals()),
        ([(19, 8), (21, 8)], normalized.errors()),
    ] {
        for ((n, d), factor) in bounds.into_iter().zip(factors) {
            assert!(
                factor
                    .multiply(Rational::new(n, d)?)?
                    .covered(normalized.centers()[0].to_f64(), radii[0].to_f64())?
            );
        }
    }
    let restored =
        heap.restore_certified_reading(&normalized, &contract, &policy, &context, &mut ledger()?)?;
    super::check(&restored, true)
}
