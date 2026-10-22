// SPDX-License-Identifier: AGPL-3.0-only
//! Public native family requests preserve original roles and replay their expanded dependencies.

use crate::testing::common::TestResult;
use crate::testing::sorts::{Fixture, admit, reference};
use nrz_cas::Cas;
use nrz_cas::domain::ContextDraft;
use nrz_cas::limits::{Budget, Limits, LimitsOverride};
use nrz_cas::proof::{DomainContract, Statement};
use nrz_cas::request::{CoveredInput, Outcome, Request, Task};
use nrz_cas::syntax::Draft;

/// Every native request owns a fresh cumulative generation/checker pair under the unchanged profile.
/// ´test:integration:native-family-selection-and-definedness´
#[test]
fn native_family_selection_and_definedness() -> TestResult {
    let mut fixture = Fixture::new()?;
    let family = fixture.family()?;
    let context = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Root {
            header: reference(&fixture.header),
            admissible: reference(&family.region),
            assumptions: Vec::new(),
        }),
    )?;
    let data = fixture.body_data(&family);
    let cases = fixture
        .cas
        .family_expression(&data, &mut Budget::new(Limits::default())?)?;
    let root =
        fixture
            .cas
            .materialize_derived(&cases, &cases, &mut Budget::new(Limits::default())?)?;
    let receipt = fixture
        .cas
        .walk_derived(&root, &mut Budget::new(Limits::default())?)?
        .receipt;
    for task in [
        Task::Select {
            region: family.region.clone(),
        },
        Task::Coverage {
            domain: family.region,
        },
        Task::Simplify,
    ] {
        let request = Request {
            task,
            context: context.clone(),
            inputs: vec![CoveredInput {
                root: root.clone(),
                receipt: receipt.clone(),
            }],
            premises: Vec::new(),
            limits: LimitsOverride::default(),
        };
        let outcome = fixture.cas.run(&request);
        let Outcome::Proved { result, .. } = outcome else {
            return Err(format!(
                "native family request {:?} not proved: {outcome:?}",
                request.task
            )
            .into());
        };
        let bytes = fixture
            .cas
            .export_witness(&result.witness, &mut Budget::new(Limits::default())?)?;
        assert!(
            Cas::new(Limits::default())?
                .inspect(&bytes, &mut Budget::new(Limits::default())?)?
                .is_discharged()
        );
    }
    Ok(())
}

/// Independent equalities retain the same domain contract through the public request service.
/// ´test:integration:native-family-equality-request´
#[test]
fn native_family_equality_request() -> TestResult {
    let mut fixture = Fixture::new()?;
    let family = fixture.family()?;
    let context = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Root {
            header: reference(&fixture.header),
            admissible: reference(&family.region),
            assumptions: Vec::new(),
        }),
    )?;
    let mut data = fixture.body_data(&family);
    let left = fixture
        .cas
        .family_expression(&data, &mut Budget::new(Limits::default())?)?;
    data.arguments.reverse();
    let right = fixture
        .cas
        .family_expression(&data, &mut Budget::new(Limits::default())?)?;
    let mut inputs = Vec::new();
    for term in [&left, &right] {
        let root =
            fixture
                .cas
                .materialize_derived(term, term, &mut Budget::new(Limits::default())?)?;
        let receipt = fixture
            .cas
            .walk_derived(&root, &mut Budget::new(Limits::default())?)?
            .receipt;
        inputs.push(CoveredInput { root, receipt });
    }
    let request = Request {
        task: Task::Equal {
            domain: family.region.clone(),
            domain_contract: DomainContract::SameDefinedDomain,
        },
        context,
        inputs,
        premises: Vec::new(),
        limits: LimitsOverride::default(),
    };
    let outcome = fixture.cas.run(&request);
    let Outcome::Proved { result, .. } = outcome else {
        return Err(format!("native family equality not proved: {outcome:?}").into());
    };
    assert!(
        matches!(&result.witness.judgment().statement, Statement::Equality { domain, .. } if *domain == family.region)
    );
    Ok(())
}
