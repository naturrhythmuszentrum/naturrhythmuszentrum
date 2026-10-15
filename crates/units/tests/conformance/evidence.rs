// SPDX-License-Identifier: AGPL-3.0-only
//! Snapshot admission checks the whole original declaration environment.

use nrz_cas::limits::LimitsOverride;
use nrz_units::{Declaration, Descriptor, Failure, FunctionSignature, SnapshotError, UnitError};

use crate::testing::declarations::{TestResult, context, derived, input, length};

/// Successful semantic changes invalidate prior checks even with unchanged value descriptors.
/// ´test:integration:snapshot-declaration-revision-refuses-stale-checks´
#[test]
fn snapshot_declaration_revision_refuses_stale_checks() -> TestResult {
    let mut builder = context(&["x", "y"])?;
    input(&mut builder, "x", &Descriptor::ratio())?;
    let original = builder.parse("original", "x")?;
    let checked = builder.check(&original)?;
    builder.validate_checked(&checked)?;
    let before = builder.snapshot_id();
    assert!(
        builder
            .declare("x".into(), Declaration::Metadata("duplicate".into()))
            .is_err()
    );
    assert_eq!(builder.snapshot_id(), before);
    input(&mut builder, "y", &Descriptor::ratio())?;
    assert_eq!(builder.snapshot_id().revision(), before.revision() + 1);
    assert_eq!(
        builder.validate_checked(&checked),
        Err(SnapshotError::StaleContext)
    );
    assert!(
        builder
            .prove_equal(&checked, &original, LimitsOverride::default())
            .is_err()
    );
    let refreshed = builder.check(&original)?;
    let frozen = builder.freeze()?;
    frozen.validate_checked(&refreshed)?;
    assert_eq!(
        frozen.validate_checked(&checked),
        Err(SnapshotError::StaleContext)
    );
    Ok(())
}

/// Function rules share the same atomic semantic revision boundary as declarations.
/// ´test:integration:snapshot-function-revision-refuses-stale-checks´
#[test]
fn snapshot_function_revision_refuses_stale_checks() -> TestResult {
    let mut builder = context(&["project"])?;
    let original = builder.parse("original", "1")?;
    let checked = builder.check(&original)?;
    let signature = FunctionSignature {
        parameters: vec![Descriptor::ratio()],
        result: Descriptor::ratio(),
    };
    let before = builder.snapshot_id();
    assert!(
        builder
            .declare_function("sqrt".into(), signature.clone())
            .is_err()
    );
    assert_eq!(builder.snapshot_id(), before);
    builder.declare_function("project".into(), signature.clone())?;
    assert_eq!(
        builder.validate_checked(&checked),
        Err(SnapshotError::StaleContext)
    );
    let frozen = builder.freeze()?;
    assert_eq!(frozen.functions().get("project"), Some(&signature));
    Ok(())
}

/// Identical logical headers and revisions still belong to different owning stores.
/// ´test:integration:snapshot-foreign-context-refusal´
#[test]
fn snapshot_foreign_context_refusal() -> TestResult {
    let first = context(&[])?;
    let second = context(&[])?;
    let original = first.parse("literal", "1")?;
    let checked = first.check(&original)?;
    assert_eq!(
        first.snapshot_id().revision(),
        second.snapshot_id().revision()
    );
    assert_ne!(first.snapshot_id(), second.snapshot_id());
    assert_eq!(
        second.validate_checked(&checked),
        Err(SnapshotError::ForeignContext)
    );
    assert_eq!(
        second.freeze()?.validate_checked(&checked),
        Err(SnapshotError::ForeignContext)
    );
    let frozen = first.freeze()?;
    frozen.validate_checked(&checked)?;
    Ok(())
}

/// Freeze cannot skip unused invalid bodies or erase their operands by normalization.
/// ´test:integration:snapshot-hidden-invalid-operands-refuse´
#[test]
fn snapshot_hidden_invalid_operands_refuse() -> TestResult {
    for text in ["0*(x+1)", "(x+1)-(x+1)", "(x+1)^0"] {
        let mut builder = context(&["x", "unused"])?;
        input(&mut builder, "x", &length()?)?;
        derived(&mut builder, "unused", text, &length()?)?;
        let error = builder
            .freeze()
            .err()
            .ok_or("invalid original was frozen")?;
        assert_eq!(error.failure, Failure::Units(UnitError::DimensionMismatch));
        assert_eq!(error.location.definitions, ["unused"]);
        assert!(error.location.span.is_some());
        assert_ne!(error.location.path, [] as [usize; 0]);
    }
    Ok(())
}

/// Metadata and cycles remain refusals even when no client requested their bodies.
/// ´test:integration:snapshot-unused-metadata-and-cycles-refuse´
#[test]
fn snapshot_unused_metadata_and_cycles_refuse() -> TestResult {
    let mut metadata = context(&["note", "unused"])?;
    metadata.declare("note".into(), Declaration::Metadata("text".into()))?;
    derived(&mut metadata, "unused", "0*note", &Descriptor::ratio())?;
    assert_eq!(
        metadata
            .freeze()
            .err()
            .ok_or("metadata operand accepted")?
            .failure,
        Failure::Metadata("note".into())
    );
    let mut cyclic = context(&["a", "b"])?;
    derived(&mut cyclic, "a", "b", &Descriptor::ratio())?;
    derived(&mut cyclic, "b", "a", &Descriptor::ratio())?;
    assert!(matches!(
        cyclic.freeze().err().ok_or("cycle accepted")?.failure,
        Failure::Cycle(_)
    ));
    Ok(())
}

/// Snapshot name resolution preserves the distinction between metadata and numerical evidence.
/// ´test:integration:snapshot-evidence-name-refusals´
#[test]
fn snapshot_evidence_name_refusals() -> TestResult {
    let mut builder = context(&["note"])?;
    builder.declare("note".into(), Declaration::Metadata("text".into()))?;
    let frozen = builder.freeze()?;
    assert!(frozen.checks().is_empty());
    assert_eq!(
        frozen
            .checked("note")
            .err()
            .ok_or("metadata checked")?
            .failure,
        Failure::Metadata("note".into())
    );
    assert_eq!(
        frozen
            .checked("missing")
            .err()
            .ok_or("unknown checked")?
            .failure,
        Failure::UnknownSymbol("missing".into())
    );
    Ok(())
}

/// A valid physical zero freezes with its declared kind; an untyped zero cannot replace it.
/// ´test:integration:snapshot-typed-zero-and-kind-boundaries´
#[test]
fn snapshot_typed_zero_and_kind_boundaries() -> TestResult {
    let distance = length()?;
    let mut builder = context(&["zero", "copy"])?;
    builder.declare(
        "zero".into(),
        Declaration::Constant {
            descriptor: distance.clone(),
            value: nrz_cas::Rational::zero(),
            evidence: nrz_units::Evidence::default(),
        },
    )?;
    derived(&mut builder, "copy", "zero", &distance)?;
    assert_eq!(builder.freeze()?.checked("copy")?.descriptor(), &distance);
    let mut untyped = context(&["zero"])?;
    derived(&mut untyped, "zero", "0", &distance)?;
    assert_eq!(
        untyped
            .freeze()
            .err()
            .ok_or("untyped physical zero accepted")?
            .failure,
        Failure::Units(UnitError::DimensionMismatch)
    );
    let mut wrong_kind = context(&["angle"])?;
    let angle = Descriptor::new(
        nrz_units::Kind::Angle,
        nrz_units::Measure::coherent(),
        nrz_units::Role::Difference,
    )?;
    derived(&mut wrong_kind, "angle", "1", &angle)?;
    assert_eq!(
        wrong_kind
            .freeze()
            .err()
            .ok_or("ratio accepted as angle")?
            .failure,
        Failure::Units(UnitError::KindMismatch)
    );
    Ok(())
}

/// Freeze keeps the checker's ordinary resource failure instead of publishing partial evidence.
/// ´test:integration:snapshot-resource-refusal´
#[test]
fn snapshot_resource_refusal() -> TestResult {
    use nrz_cas::limits::{BudgetKind, Limits};
    use nrz_cas::syntax::{AtomicSort, Declaration as Mathematical, Environment, Field, Sort};
    use nrz_units::{Context, ExponentPolicy, Registry};

    let mut builder = Context::new(
        Registry::new(ExponentPolicy::Rational),
        Limits {
            depth: 1,
            ..Limits::default()
        },
        Environment {
            declarations: vec![Mathematical {
                key: "deep".into(),
                display_name: "deep".into(),
                sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            }],
            ..Environment::default()
        },
    )?;
    derived(&mut builder, "deep", "1+1", &Descriptor::ratio())?;
    let error = builder.freeze().err().ok_or("resource refusal missing")?;
    assert!(matches!(
        error.failure,
        Failure::Algebra(nrz_cas::Fault {
            kind: nrz_cas::FaultKind::Exhausted {
                budget: BudgetKind::Depth,
                ..
            },
            ..
        })
    ));
    Ok(())
}
