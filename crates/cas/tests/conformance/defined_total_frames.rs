// SPDX-License-Identifier: AGPL-3.0-only
//! Exact frames, undecided restrictions and unused assumptions retain their identities.

use crate::testing::common::{TestResult, default_budget, real};
use crate::testing::defined_total::Fixture;
use crate::testing::series_residual::admit;
use nrz_cas::domain::{
    AmbientDomain, AssumptionDraft, ContextDraft, HeaderDraft, RegionDraft, RegionExpr,
};
use nrz_cas::proof::Statement;
use nrz_cas::syntax::{Branch, Declaration, Draft, Operator, PredicateKind};
use nrz_cas::{FaultKind, InvalidCode, Reference};

/// A caller may retain an unused assumption without turning it into proof evidence.
#[test]
fn defined_total_unused_assumption() -> TestResult {
    let mut f = Fixture::new(Branch::StrictReal, false, 4, false)?;
    let function = f.polynomial()?;
    let condition = admit(
        &mut f.cas,
        Draft::Condition {
            header: Reference::Existing(f.header.clone()),
            scope: Vec::new(),
            kind: PredicateKind::NonZero,
            arguments: vec![Reference::Existing(function.clone())],
        },
    )?;
    f.context = admit(
        &mut f.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(f.header.clone()),
            admissible: Reference::Existing(f.region.clone()),
            assumptions: vec![AssumptionDraft {
                key: "unused".into(),
                condition: Reference::Existing(condition),
            }],
        }),
    )?;
    let witness = f
        .cas
        .defined_total(&function, &f.context, &mut default_budget()?)?;
    assert_eq!(witness.judgment(), &f.expected(&function)?);
    assert_eq!(
        witness.hypotheses(),
        [] as [nrz_cas::domain::HypothesisRef; 0]
    );
    assert!(witness.is_unconditional());
    assert_eq!(
        witness.context_consistency(),
        nrz_cas::request::ContextConsistency::Unknown
    );
    Ok(())
}

/// Generic conservative embedding is insufficient for this primitive's exact frame.
#[test]
fn defined_total_exact_header_and_scope() -> TestResult {
    for scoped in [false, true] {
        let mut f = Fixture::new(Branch::StrictReal, scoped, 0, false)?;
        let function = f.polynomial()?;
        let mut draft = f.draft(&function)?;
        let extension = admit(
            &mut f.cas,
            Draft::Header(HeaderDraft::Extend {
                parent: Reference::Existing(f.header.clone()),
                declarations: vec![Declaration {
                    key: "extension".into(),
                    display_name: "extension".into(),
                    sort: real(),
                }],
                spaces: Vec::new(),
            }),
        )?;
        f.context = admit(
            &mut f.cas,
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(extension),
                admissible: Reference::Existing(f.region.clone()),
                assumptions: Vec::new(),
            }),
        )?;
        draft.context = f.context.clone();
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        for supplied in [false, true] {
            let result = if supplied {
                f.cas.check_proof(
                    &f.context,
                    std::slice::from_ref(&draft),
                    &mut default_budget()?,
                )
            } else {
                f.cas
                    .defined_total(&function, &f.context, &mut default_budget()?)
            };
            assert_eq!(
                result.err().ok_or("conservative header accepted")?.kind,
                FaultKind::Invalid(InvalidCode::Context)
            );
            assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
        }
    }
    let mut f = Fixture::new(Branch::StrictReal, false, 0, false)?;
    f.scope = vec![vec![nrz_cas::syntax::AtomicSort::Scalar(
        nrz_cas::syntax::Field::Real,
    )]];
    let function = f.polynomial()?;
    let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
    assert_eq!(
        f.cas
            .defined_total(&function, &f.context, &mut default_budget()?)
            .err()
            .ok_or("scope mismatch")?
            .kind,
        FaultKind::Invalid(InvalidCode::Scope)
    );
    assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
    Ok(())
}

/// Unknown membership and emptiness neither supply nor remove structural obligations.
#[test]
fn defined_total_undecided_and_empty_regions() -> TestResult {
    for empty in [false, true] {
        let mut f = Fixture::new(Branch::StrictReal, false, 4, false)?;
        let function = f.polynomial()?;
        let x = f.term(Operator::Free("x".into()), &[])?;
        let call = f.term(Operator::Call("opaque".into()), &[x])?;
        let condition = admit(
            &mut f.cas,
            Draft::Condition {
                header: Reference::Existing(f.header.clone()),
                scope: Vec::new(),
                kind: PredicateKind::Positive,
                arguments: vec![Reference::Existing(call.clone())],
            },
        )?;
        let coordinates = admit(
            &mut f.cas,
            Draft::Solve(nrz_cas::solving::SolveDraft::CoordinateSpace(
                nrz_cas::solving::CoordinateSpaceData {
                    header: Reference::Existing(f.header.clone()),
                    scalar_spaces: Vec::new(),
                    coordinates: vec![nrz_cas::solving::Coordinate {
                        key: "x".into(),
                        field: nrz_cas::syntax::Field::Real,
                        role: nrz_cas::solving::CoordinateRole::Coefficient,
                    }],
                },
            )),
        )?;
        let restriction = admit(
            &mut f.cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(f.header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Product { coordinates },
                expression: if empty {
                    RegionExpr::Empty
                } else {
                    RegionExpr::GuardTrue {
                        base: Reference::Existing(f.region.clone()),
                        predicate: Reference::Existing(condition),
                    }
                },
            }),
        )?;
        let mut wrong = f.draft(&function)?;
        let Statement::PredicateFact { region, .. } = &mut wrong.conclusion else {
            return Err("predicate".into());
        };
        *region = restriction.clone();
        assert_eq!(
            f.cas
                .check_proof(&f.context, &[wrong], &mut default_budget()?)
                .err()
                .ok_or("wrong region")?
                .kind,
            FaultKind::Invalid(InvalidCode::Rule)
        );
        f.region = restriction;
        f.context = admit(
            &mut f.cas,
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(f.header.clone()),
                admissible: Reference::Existing(f.region.clone()),
                assumptions: Vec::new(),
            }),
        )?;
        let witness = f
            .cas
            .defined_total(&function, &f.context, &mut default_budget()?)?;
        assert_eq!(witness.judgment(), &f.expected(&function)?);
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.cas
                .defined_total(&call, &f.context, &mut default_budget()?)
                .err()
                .ok_or("opaque on empty")?
                .kind,
            FaultKind::Invalid(InvalidCode::UnsupportedFragment)
        );
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
    }
    Ok(())
}
