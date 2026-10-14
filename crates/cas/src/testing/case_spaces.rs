// SPDX-License-Identifier: AGPL-3.0-only
//! Public exact-space construction for coordinate-map contract tests.

use super::budgets::derivation_budget as budget;
use super::builders::{admit, expression_draft};
use super::common::{TestResult, real};
use crate::test_api::domain::{
    AmbientDomain, DeclarationSpaceDraft, HeaderDraft, RegionDraft, RegionExpr, SpaceDraft,
};
use crate::test_api::syntax::{Declaration, Draft, Environment, Field, Operator, Sort};
use crate::test_api::{Cas, Handle, Reference};

/// Create a renamed real axis while preserving the source coordinate's display spelling.
pub(crate) fn axis(cas: &mut Cas, key: &str) -> TestResult<(Handle, Handle)> {
    let header = admit(
        cas,
        Draft::Header(HeaderDraft::Core(Environment {
            declarations: vec![Declaration {
                key: key.into(),
                display_name: "x".into(),
                sort: real(),
            }],
            ..Environment::default()
        })),
        &mut budget()?,
    )?;
    let domain = admit(
        cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Real(key.into()),
            expression: RegionExpr::Ambient,
        }),
        &mut budget()?,
    )?;
    Ok((header, domain))
}

/// Declare one test variable in an earlier exact open real space.
pub(crate) fn space(
    cas: &mut Cas,
    header: &Handle,
    domain: &Handle,
    coordinate: &str,
) -> TestResult<(Handle, Handle, Handle)> {
    let space = admit(
        cas,
        Draft::Space(SpaceDraft {
            header: Reference::Existing(header.clone()),
            domain: Reference::Existing(domain.clone()),
            coordinate: coordinate.into(),
            field: Field::Real,
            test_family: "CompactSmooth".into(),
        }),
        &mut budget()?,
    )?;
    let extended = admit(
        cas,
        Draft::Header(HeaderDraft::Extend {
            parent: Reference::Existing(header.clone()),
            declarations: vec![Declaration {
                key: "phi".into(),
                display_name: "phi".into(),
                sort: Sort::Test,
            }],
            spaces: vec![DeclarationSpaceDraft {
                key: "phi".into(),
                space: Reference::Existing(space.clone()),
            }],
        }),
        &mut budget()?,
    )?;
    let variable = admit(
        cas,
        expression_draft(
            Operator::Free("phi".into()),
            Sort::Test,
            Reference::Existing(extended.clone()),
            Vec::new(),
            None,
            &[],
        ),
        &mut budget()?,
    )?;
    Ok((space, extended, variable))
}
