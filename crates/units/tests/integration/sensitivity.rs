// SPDX-License-Identifier: AGPL-3.0-only
//! A client retains its complete original and symbolic evidence through the frozen owner.

use nrz_cas::{
    limits::{Budget, Limits, LimitsOverride},
    source::WalkEvent,
};
use nrz_units::{Descriptor, SensitivityRecipe};

use crate::testing::declarations::{TestResult, context, input};
use crate::testing::sensitivity::{coefficient, equal_fraction};

/// Existing checks, native lowering and read-only proof inspection share one immutable owner.
/// ´test:integration:native-sensitivity-frozen-owner-lifecycle´
#[test]
fn native_sensitivity_frozen_owner_lifecycle() -> TestResult {
    let mut builder = context(&["x"])?;
    input(&mut builder, "x", &Descriptor::ratio())?;
    let original = builder.parse("client square", "x^2")?;
    let earlier = builder.check(&original)?;
    let sensitivity = builder.check_sensitivity(
        SensitivityRecipe::Square,
        std::slice::from_ref(&original),
        &["x".into()],
        &LimitsOverride::default(),
    )?;
    sensitivity.declarations().validate_checked(&earlier)?;
    let cas = sensitivity.declarations().cas()?;
    let walk = cas.walk_original(&original.root, &mut Budget::new(Limits::default())?)?;
    assert!(
        walk.events
            .iter()
            .all(|event| matches!(event, WalkEvent::Visit(_)))
    );
    assert_eq!(
        sensitivity
            .jacobian()
            .originals()
            .first()
            .ok_or("missing original")?
            .original()
            .root,
        original.root
    );
    let entry = sensitivity
        .jacobian()
        .entries()
        .first()
        .ok_or("missing derivative")?;
    equal_fraction(
        coefficient(&cas, entry.expression(), &[("x", -4)])?,
        (-8, 1),
    )?;
    assert!(entry.identity().is_discharged());
    assert_eq!(entry.pairing().target(), &Descriptor::ratio());
    assert_eq!(sensitivity.snapshot_id(), earlier.snapshot_id());
    Ok(())
}
