// SPDX-License-Identifier: AGPL-3.0-only
//! Portable claims compare complete regions, coordinates, contexts and scopes.

use crate::testing::common::{TestResult, default_budget};
use crate::testing::defined_total::Fixture;
use crate::testing::series_residual::admit;
use nrz_cas::domain::{AmbientDomain, AssumptionDraft, ContextDraft, RegionDraft, RegionExpr};
use nrz_cas::proof::Statement;
use nrz_cas::solving::{Coordinate, CoordinateRole, CoordinateSpaceData, SolveDraft};
use nrz_cas::syntax::{Branch, Draft, Field, Operator};
use nrz_cas::{FaultKind, InvalidCode, Reference};

/// Both coordinate identity and role are part of the exact Product claim.
fn product(f: &mut Fixture, key: &str, role: CoordinateRole, empty: bool) -> TestResult {
    let coordinates = admit(
        &mut f.cas,
        Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
            header: Reference::Existing(f.header.clone()),
            scalar_spaces: Vec::new(),
            coordinates: vec![Coordinate {
                key: key.into(),
                field: Field::Real,
                role,
            }],
        })),
    )?;
    f.region = admit(
        &mut f.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(f.header.clone()),
            scope: f.scope.clone(),
            ambient: AmbientDomain::Product { coordinates },
            expression: if empty {
                RegionExpr::Empty
            } else {
                RegionExpr::Ambient
            },
        }),
    )?;
    f.context = admit(
        &mut f.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(f.header.clone()),
            admissible: Reference::Existing(f.region.clone()),
            assumptions: Vec::new(),
        }),
    )?;
    Ok(())
}

/// Every mutated expectation is independently well-formed and structurally total too.
#[test]
fn defined_total_expected_claim_mutations() -> TestResult {
    let mut source = Fixture::new(Branch::StrictReal, false, 4, false)?;
    let function = source.term(Operator::Free("x".into()), &[])?;
    let witness = source
        .cas
        .defined_total(&function, &source.context, &mut default_budget()?)?;
    let bytes = source
        .cas
        .export_witness(&witness, &mut default_budget()?)?;
    for change in 0..6 {
        let mut f = Fixture::new(Branch::StrictReal, change == 5, 4, true)?;
        let function = f.term(
            Operator::Free(if change == 0 { "y" } else { "x" }.into()),
            &[],
        )?;
        match change {
            1 => product(&mut f, "x", CoordinateRole::Coefficient, true)?,
            2 => product(&mut f, "y", CoordinateRole::Coefficient, false)?,
            3 => product(&mut f, "x", CoordinateRole::Unknown, false)?,
            4 => {
                let Statement::PredicateFact { predicate, .. } = f.expected(&function)?.statement
                else {
                    return Err("predicate".into());
                };
                f.context = admit(
                    &mut f.cas,
                    Draft::Context(ContextDraft::Root {
                        header: Reference::Existing(f.header.clone()),
                        admissible: Reference::Existing(f.region.clone()),
                        assumptions: vec![AssumptionDraft {
                            key: "unused-totality".into(),
                            condition: Reference::Existing(predicate),
                        }],
                    }),
                )?;
            }
            0 | 5 => {}
            _ => return Err("mutation".into()),
        }
        let expected = f.expected(&function)?;
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            f.cas
                .replay(&bytes, &expected, &mut default_budget()?)
                .err()
                .ok_or("changed claim")?
                .kind,
            FaultKind::Invalid(InvalidCode::EndpointMismatch)
        );
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
        let valid = f
            .cas
            .defined_total(&function, &f.context, &mut default_budget()?)?;
        assert_eq!(valid.judgment(), &expected);
        assert!(valid.is_unconditional());
    }
    Ok(())
}

/// The new key retains version one and cannot claim authority under another version.
#[test]
fn defined_total_wire_version_and_certificate() -> TestResult {
    use crate::testing::answer::records;
    let mut source = Fixture::new(Branch::StrictReal, true, 4, false)?;
    let function = source.polynomial()?;
    let witness = source
        .cas
        .defined_total(&function, &source.context, &mut default_budget()?)?;
    let bytes = source
        .cas
        .export_witness(&witness, &mut default_budget()?)?;
    let key = b"defined.total";
    let offset = bytes
        .windows(key.len())
        .position(|value| value == key)
        .ok_or("rule key")?
        + key.len();
    assert_eq!(
        bytes.get(offset..offset + 4),
        Some(1_u32.to_le_bytes().as_slice())
    );
    let mut unsupported = bytes.clone();
    unsupported
        .get_mut(offset..offset + 4)
        .ok_or("version")?
        .copy_from_slice(&2_u32.to_le_bytes());
    let inventory = records(&bytes)?;
    let (_, application) = inventory
        .iter()
        .rev()
        .find(|(tag, _)| *tag == 11)
        .ok_or("application")?;
    assert_eq!(bytes.get(application.end - 1), Some(&0));
    let mut certificate = bytes.clone();
    certificate[application.end - 1] = 255;
    for (changed, code) in [
        (unsupported, InvalidCode::UnsupportedVersion),
        (certificate, InvalidCode::Encoding),
    ] {
        let mut receiver = Fixture::new(Branch::StrictReal, true, 4, true)?;
        let original = receiver.polynomial()?;
        let expected = receiver.expected(&original)?;
        let before = receiver.cas.canonical_snapshot(&mut default_budget()?)?;
        assert_eq!(
            receiver
                .cas
                .replay(&changed, &expected, &mut default_budget()?)
                .err()
                .ok_or("wire mutation")?
                .kind,
            FaultKind::Invalid(code)
        );
        assert_eq!(
            receiver.cas.canonical_snapshot(&mut default_budget()?)?,
            before
        );
        let replayed = receiver
            .cas
            .replay(&bytes, &expected, &mut default_budget()?)?;
        assert_eq!(
            receiver
                .cas
                .export_witness(&replayed, &mut default_budget()?)?,
            bytes
        );
    }
    Ok(())
}
