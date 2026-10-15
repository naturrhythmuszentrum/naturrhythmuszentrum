// SPDX-License-Identifier: AGPL-3.0-only
//! The numerical client's preparation retains map identity beside the checked sensitivity.

use nrz_cas::{
    Rational,
    limits::{Budget, Limits, LimitsOverride},
};
use nrz_units::{Descriptor, Kind, Measure, Role, Scale, SensitivityRecipe};

use crate::testing::declarations::{TestResult, context, input};

/// A client obtains exact conversion constants and native derivative evidence from one owner.
/// ´test:integration:descriptor-constants-and-sensitivity-lifecycle´
#[test]
fn descriptor_constants_and_sensitivity_lifecycle() -> TestResult {
    let mut builder = context(&["x"])?;
    let descriptor = Descriptor::new(
        Kind::Ratio,
        Measure::new(
            "percent",
            Scale::Exact(Rational::from_ratio(
                1,
                100,
                &mut Budget::new(Limits::default())?,
            )?),
            Rational::zero(),
        )?,
        Role::Difference,
    )?;
    input(&mut builder, "x", &descriptor)?;
    let original = builder.parse("square", "x^2")?;
    let checked = builder.check_sensitivity(
        SensitivityRecipe::Square,
        &[original],
        &["x".into()],
        &LimitsOverride::default(),
    )?;
    let map = checked
        .declarations()
        .conversion_map(&descriptor, &descriptor.coherent())?;
    map.validate_snapshot(checked.declarations())?;
    let scale = map.scale().clone();
    let identity = map.id().clone();
    drop(map);
    assert_eq!(scale.export(&LimitsOverride::default())?, "1/100");
    assert_eq!(scale.map_id(), &identity);
    assert_eq!(scale.map_id().snapshot_id(), checked.snapshot_id());
    assert_eq!(checked.inputs(), [descriptor]);
    Ok(())
}
