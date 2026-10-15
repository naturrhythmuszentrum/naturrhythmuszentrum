// SPDX-License-Identifier: AGPL-3.0-only
//! Private witness ownership across shape-operation module boundaries.

use crate::{
    Axis, AxisEntry, BlockDraft, EntryDraft, Partition, ShapeDraft, ShapeLimits, StructureScope,
};
use nrz_units::{Context, Descriptor, ExponentPolicy, Registry};
use std::rc::Rc;

/// Derived contracts share the frozen owner and retain their original operand checks.
/// ´test:crate:shape-derivation-shares-frozen-owner´
#[test]
fn shape_derivation_shares_frozen_owner() -> Result<(), Box<dyn std::error::Error>> {
    #[expect(
        clippy::default_trait_access,
        reason = "infer CAS-owned argument types through units without a forbidden direct CAS dependency"
    )]
    let context = Context::new(
        Registry::new(ExponentPolicy::Rational),
        Default::default(),
        Default::default(),
    )?;
    let checked = context.check(&context.parse("entry", "1 + 1")?)?;
    let owner = Rc::new(context.freeze()?);
    let limits = ShapeLimits::default();
    let axis = Axis::new(
        vec![AxisEntry::new("x", Descriptor::ratio())?],
        Partition::whole(1, limits)?,
        limits,
    )?;
    let shape = ShapeDraft::new(
        axis.clone(),
        axis,
        vec![BlockDraft::new(
            0..1,
            0..1,
            vec![EntryDraft::Checked(Box::new(checked))],
        )],
        StructureScope::WholeFamily,
    )
    .check(Rc::clone(&owner), limits)?;
    let transpose = shape.dual_transpose(limits)?;
    let parent = transpose.parents.first().ok_or("missing operand")?;
    assert!(Rc::ptr_eq(&owner, &transpose.declarations));
    assert!(Rc::ptr_eq(&owner, &parent.declarations));
    assert!(
        parent
            .entries
            .first()
            .ok_or("missing entry")?
            .evidence
            .is_some()
    );
    assert_eq!(
        transpose.entries.first().ok_or("missing entry")?.pairing,
        parent.entries.first().ok_or("missing entry")?.pairing
    );
    Ok(())
}
