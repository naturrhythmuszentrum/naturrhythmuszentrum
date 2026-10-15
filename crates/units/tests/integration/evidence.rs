// SPDX-License-Identifier: AGPL-3.0-only
//! Existing checking calls lead to an independently owned immutable snapshot.

use nrz_cas::limits::{Budget, Limits, LimitsOverride};
use nrz_cas::request::Outcome;
use nrz_cas::source::WalkEvent;
use nrz_cas::syntax::{Operator, Span};
use nrz_units::{BinaryOp, Declaration, Descriptor, FunctionSignature};

use crate::testing::declarations::{TestResult, context, derived, input, length};

/// The existing parse/check/equation/block/equality calls compose with the additive freeze API.
/// ´test:integration:freeze-preserves-existing-checking-workflow´
#[test]
fn freeze_preserves_existing_checking_workflow() -> TestResult {
    let mut builder = context(&["x", "y", "note", "project"])?;
    let distance = length()?;
    input(&mut builder, "x", &distance)?;
    derived(&mut builder, "y", "x-x", &distance)?;
    builder.declare("note".into(), Declaration::Metadata("source notes".into()))?;
    builder.declare_function(
        "project".into(),
        FunctionSignature {
            parameters: vec![distance.clone()],
            result: distance.clone(),
        },
    )?;
    let original = builder.parse("entry", "y")?;
    let checked = builder.check_against(&original, &distance)?;
    builder.check_equation(&original, &original)?;
    builder.check_block(std::slice::from_ref(&original), &distance)?;
    let zero = builder.parse("coordinate zero", "0")?;
    assert!(matches!(
        builder.prove_equal(&checked, &zero, LimitsOverride::default())?,
        Outcome::Proved { .. }
    ));
    let previous = builder.check_all()?;
    let frozen = builder.freeze()?;
    frozen.validate_checked(&checked)?;
    assert_eq!(frozen.checks().len(), previous.len());
    assert_eq!(frozen.checked("y")?.descriptor(), &distance);
    assert_eq!(
        frozen
            .registry()
            .binary(BinaryOp::Subtract, &distance, &distance)?,
        distance
    );
    let Declaration::Derived(observable) = frozen.declarations().get("y").ok_or("missing y")?
    else {
        return Err("missing original observable".into());
    };
    assert_eq!(observable.evidence.source, "recipe:y");
    assert_eq!(observable.evidence.assurance, "declared");
    let cas = frozen.cas()?;
    assert_eq!(
        cas.expression(&observable.expression.syntax)?.0,
        &Operator::Sub
    );
    let walk = cas.walk_original(
        &observable.expression.root,
        &mut Budget::new(Limits::default())?,
    )?;
    assert_eq!(walk.events.len(), 3);
    assert!(
        walk.events
            .iter()
            .all(|event| matches!(event, WalkEvent::Visit(_)))
    );
    assert!(frozen.checked("y")?.nodes().iter().any(|node| {
        node.location.definitions == ["y"] && node.location.span == Some(Span { start: 0, end: 3 })
    }));
    assert_eq!(frozen.checked("y")?.snapshot_id(), frozen.id());
    Ok(())
}

/// Previously cloned checks remain inspectable through the moved, frozen CAS owner.
/// ´test:integration:frozen-owner-retains-original-occurrences´
#[test]
fn frozen_owner_retains_original_occurrences() -> TestResult {
    let builder = context(&[])?;
    let first = builder.parse("first", "1-1")?;
    let second = builder.parse("second", "1-1")?;
    let checked = builder.check(&first)?;
    assert_ne!(first.root, second.root);
    assert_eq!(first.syntax, second.syntax);
    let frozen = builder.freeze()?;
    frozen.validate_checked(&checked)?;
    let cas = frozen.cas()?;
    for original in [&first, &second, checked.original()] {
        let walk = cas.walk_original(&original.root, &mut Budget::new(Limits::default())?)?;
        assert_eq!(walk.events.len(), 3);
    }
    assert_eq!(checked.descriptor(), &Descriptor::ratio());
    Ok(())
}
