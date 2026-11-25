// SPDX-License-Identifier: AGPL-3.0-only
//! Allocation bounds apply to every namespace independently of retained graph occupancy.

use crate::budget::{BudgetKind, Limits};
use crate::error::{Fault, FaultKind, InvalidCode, Location};
use crate::graph::{Cas, Draft, Handle, VertexKind};
use crate::testing::common::{TestResult, default_budget};

/// Every stable namespace, including those published through the shared insertion boundary.
const KINDS: [VertexKind; 42] = [
    VertexKind::Source,
    VertexKind::Context,
    VertexKind::Condition,
    VertexKind::Expression,
    VertexKind::Occurrence,
    VertexKind::Binder,
    VertexKind::Request,
    VertexKind::Application,
    VertexKind::Proof,
    VertexKind::Policy,
    VertexKind::Space,
    VertexKind::Region,
    VertexKind::CaseArm,
    VertexKind::Boundary,
    VertexKind::SourceForm,
    VertexKind::Header,
    VertexKind::CoordinateSpace,
    VertexKind::Tuple,
    VertexKind::SolutionSet,
    VertexKind::FamilyMap,
    VertexKind::CoefficientDomain,
    VertexKind::Polynomial,
    VertexKind::ExtensionLevel,
    VertexKind::CoefficientMap,
    VertexKind::RootRelation,
    VertexKind::RootSelection,
    VertexKind::ComponentSplit,
    VertexKind::CoefficientValue,
    VertexKind::SolutionBranch,
    VertexKind::SolveProblem,
    VertexKind::LocalBranch,
    VertexKind::ParameterBase,
    VertexKind::SpaceFamily,
    VertexKind::FamilyExpression,
    VertexKind::FibrePiece,
    VertexKind::RealCell,
    VertexKind::CoefficientEvaluation,
    VertexKind::ScalarSpace,
    VertexKind::SpacePlacement,
    VertexKind::ScalarPiece,
    VertexKind::AnonymousLemma,
    VertexKind::SeriesObject,
];

/// Deliberate enclosing refusal, independent of allocation failure.
const AT: Location = Location::new("logical identity control");

/// All namespace reservations survive inner success followed by outer refusal.
/// ´test:unit:logical-identity-every-namespace´
#[test]
fn logical_identity_every_namespace() -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    let mut cas = Cas::new(Limits::default())?;
    let before = cas.counts();
    for attempt in 0..3_u64 {
        let mut ledger = default_budget()?;
        let fault = Fault::invalid(InvalidCode::Rule, AT);
        let result: Result<(), Fault> = cas.transaction(&mut ledger, |cas, ledger| {
            cas.transaction(ledger, |cas, _| {
                for (tag, kind) in KINDS.into_iter().enumerate() {
                    assert_eq!(usize::from(kind.tag()), tag);
                    assert_eq!(cas.token.identities.reserve(kind, 1, AT)?, attempt);
                }
                Ok(())
            })?;
            Err(fault.clone())
        });
        assert_eq!(result.as_ref(), Err(&fault));
        assert_eq!(ledger.usage().work, 512);
        assert_eq!(cas.counts(), before);
        assert_eq!(cas.state.proof_count(&mut budget)?, 0);
    }
    for kind in KINDS {
        assert_eq!(cas.token.identities.reserve(kind, 1, AT)?, 3);
    }
    Ok(())
}

/// Overflow rejects the whole reservation, preserving the last available serial in every kind.
/// ´test:unit:logical-identity-counter-boundary´
#[test]
fn logical_identity_counter_boundary() -> TestResult {
    let identities = super::Identities::default();
    for kind in KINDS {
        assert_eq!(identities.reserve(kind, u64::MAX - 1, AT)?, 0);
        assert_eq!(
            identities.reserve(kind, 2, AT),
            Err(Fault::capacity(BudgetKind::Vertices, AT))
        );
        assert_eq!(identities.reserve(kind, 1, AT)?, u64::MAX - 1);
        assert_eq!(
            identities.reserve(kind, 1, AT),
            Err(Fault::capacity(BudgetKind::Vertices, AT))
        );
        assert_eq!(identities.reserve(kind, 0, AT)?, u64::MAX);
    }
    Ok(())
}

/// Ordinary insertion fails closed after its last identity is removed; an intern hit needs none.
/// ´test:unit:logical-identity-exhausted-admission´
#[test]
fn logical_identity_exhausted_admission() -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    let mut cas = Cas::new(Limits::default())?;
    let prefix = source(&mut cas, "prefix")?;
    cas.token
        .identities
        .reserve(VertexKind::Source, u64::MAX - 2, AT)?;
    let before = cas.canonical_snapshot(&mut default_budget()?)?;
    let mut stale = None;
    let fault = Fault::invalid(InvalidCode::Rule, AT);
    let result: Result<(), Fault> = cas.transaction(&mut default_budget()?, |cas, ledger| {
        let handles = cas.admit(
            &[Draft::Source {
                key: "last".into(),
                text: "last".into(),
            }],
            ledger,
        )?;
        stale = handles.into_iter().next();
        Err(fault.clone())
    });
    assert_eq!(result.as_ref(), Err(&fault));
    let stale = stale.ok_or("last identity")?;
    assert_eq!(stale.logical_id().serial, u64::MAX - 1);
    assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, before);
    let error = cas
        .admit(
            &[Draft::Source {
                key: "replacement".into(),
                text: "new".into(),
            }],
            &mut default_budget()?,
        )
        .err()
        .ok_or("wrapped identity")?;
    assert_eq!(
        error,
        Fault::capacity(BudgetKind::Vertices, Location::new("vertex admission"))
    );
    assert_eq!(
        cas.checked(&stale, &mut budget)
            .err()
            .ok_or("stale identity")?
            .kind,
        FaultKind::Invalid(InvalidCode::Reference)
    );
    assert_eq!(source(&mut cas, "prefix")?, prefix);
    assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, before);
    Ok(())
}

/// Copies sharing a token retain prefix handles but cannot mint colliding suffix identities.
/// ´test:unit:logical-identity-divergent-test-copies´
#[test]
fn logical_identity_divergent_test_copies() -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    let mut cas = Cas::new(Limits::default())?;
    let prefix = source(&mut cas, "prefix")?;
    let mut copy = cas.full_copy_for_test();
    let left = source(&mut cas, "same content")?;
    let right = source(&mut copy, "same content")?;
    assert_ne!(left, right);
    assert!(right.logical_id().serial > left.logical_id().serial);
    assert_eq!(
        cas.source(&left, &mut budget)?,
        copy.source(&right, &mut budget)?
    );
    assert_eq!(
        cas.checked(&right, &mut budget)
            .err()
            .ok_or("copy collision")?
            .kind,
        FaultKind::Invalid(InvalidCode::Reference)
    );
    assert_eq!(
        copy.checked(&left, &mut budget)
            .err()
            .ok_or("original collision")?
            .kind,
        FaultKind::Invalid(InvalidCode::Reference)
    );
    assert_eq!(source(&mut cas, "prefix")?, prefix);
    assert_eq!(source(&mut copy, "prefix")?, prefix);
    assert_eq!(cas.counts(), copy.counts());
    Ok(())
}

/// A single source isolates identity allocation from dependency construction.
fn source(cas: &mut Cas, key: &str) -> TestResult<Handle> {
    cas.admit(
        &[Draft::Source {
            key: key.into(),
            text: key.into(),
        }],
        &mut default_budget()?,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "source".into())
}

/// Local references retain all 64 bits and charge nine bytes regardless of allocation history.
/// ´test:unit:logical-identity-fixed-width-keys´
#[test]
fn logical_identity_fixed_width_keys() -> TestResult {
    let mut keys = std::collections::BTreeSet::new();
    for kind in KINDS {
        for serial in [0, 255, u64::from(u32::MAX), u64::MAX - 1] {
            let mut ledger = default_budget()?;
            let mut out = crate::encoding::Encoder::new(&mut ledger, AT);
            crate::graph::encode_id(crate::graph::LogicalId { kind, serial }, &mut out)?;
            let key = out.finish();
            assert_eq!(key.len(), 9);
            assert_eq!(ledger.usage().work, 9);
            assert_eq!(&key[1..], &serial.to_le_bytes());
            assert!(keys.insert(key));
        }
    }
    Ok(())
}

/// Nested outcomes preserve prefix authority and permanently discard every failed suffix.
/// ´test:unit:logical-identity-nested-lifetimes´
#[test]
fn logical_identity_nested_lifetimes() -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    for inner_refusal in [false, true] {
        let mut cas = Cas::new(Limits::default())?;
        let prefix = source(&mut cas, "prefix")?;
        let clone = prefix.clone();
        let before = cas.canonical_snapshot(&mut default_budget()?)?;
        let fault = Fault::invalid(InvalidCode::Rule, AT);
        let mut discarded = Vec::new();
        let mut ledger = default_budget()?;
        for _ in 0..3 {
            let spent = ledger.usage().work;
            let result = cas.transaction(&mut ledger, |cas, ledger| {
                cas.admit(
                    &[Draft::Source {
                        key: "outer".into(),
                        text: "kept".into(),
                    }],
                    ledger,
                )?;
                let inner = cas.transaction(ledger, |cas, ledger| {
                    discarded.extend(cas.admit(
                        &[Draft::Source {
                            key: "discarded".into(),
                            text: "discarded".into(),
                        }],
                        ledger,
                    )?);
                    if inner_refusal {
                        Err(fault.clone())
                    } else {
                        Ok(())
                    }
                });
                if inner_refusal {
                    assert_eq!(inner.as_ref(), Err(&fault));
                    Ok(())
                } else {
                    inner?;
                    Err(fault.clone())
                }
            });
            assert!(ledger.usage().work > spent + 512);
            if inner_refusal {
                result?;
                assert_eq!(cas.counts().vertices, 2);
            } else {
                assert_eq!(result.as_ref(), Err(&fault));
                assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, before);
            }
            assert_eq!(source(&mut cas, "prefix")?, clone);
            assert_eq!(cas.source(&prefix, &mut budget)?, ("prefix", "prefix"));
            for stale in &discarded {
                assert_eq!(
                    cas.checked(stale, &mut budget)
                        .err()
                        .ok_or("discarded suffix survived")?
                        .kind,
                    FaultKind::Invalid(InvalidCode::Reference)
                );
            }
        }
        let same = source(&mut cas, "discarded")?;
        let different = source(&mut cas, "different")?;
        for pair in discarded.windows(2) {
            assert!(pair[1].logical_id().serial > pair[0].logical_id().serial);
        }
        for stale in &discarded {
            assert_ne!(stale, &same);
            assert_ne!(stale, &different);
            assert!(same.logical_id().serial > stale.logical_id().serial);
            assert_eq!(
                cas.checked(stale, &mut budget)
                    .err()
                    .ok_or("discarded suffix reincarnated")?
                    .kind,
                FaultKind::Invalid(InvalidCode::Reference)
            );
        }
        let counts = cas.counts();
        assert_eq!(source(&mut cas, "discarded")?, same);
        assert_eq!(source(&mut cas, "prefix")?, clone);
        assert_eq!(cas.counts(), counts);
    }
    Ok(())
}
