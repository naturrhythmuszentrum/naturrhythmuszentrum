// SPDX-License-Identifier: AGPL-3.0-only
//! Every component of a complete reading statement participates in its request key.

use crate::domain_model::{ContextDraft, Layer};
use crate::graph::{Draft, Reference};
use crate::proof_model::{DomainContract, Judgment, Statement};
use crate::reading_memo::key;
use crate::testing::common::{TestResult, default_budget as ledger};
use crate::testing::native::{Fixture, admit};
use std::collections::BTreeSet;

/// A graph-local canonical statement retains complete declaration and domain references.
fn judgment(fixture: &Fixture) -> Judgment {
    Judgment {
        context: fixture.context.clone(),
        layer: Layer::Pointwise,
        space: None,
        statement: Statement::Equality {
            left: fixture.x.clone(),
            right: fixture.x.clone(),
            domain: fixture.domain.clone(),
            domain_contract: DomainContract::DefinedAndEqual,
        },
        residuals: Vec::new(),
    }
}

/// Altering context, stratum, premise, representation or revision cannot hit the original key.
/// ´test:crate:reading-memo-separates-every-statement-component´
#[test]
fn reading_memo_separates_every_statement_component() -> TestResult {
    let mut fixture = Fixture::new()?;
    let claim = judgment(&fixture);
    let original = key(&claim, &[], b"representation", 1, &mut ledger()?)?;
    let retained = BTreeSet::from([original]);
    let child = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Local {
            parent: Reference::Existing(fixture.context.clone()),
            region: Reference::Existing(fixture.domain.clone()),
            hypotheses: Vec::new(),
        }),
    )?;
    let mut context = claim.clone();
    context.context = child;
    let mut layer = claim.clone();
    layer.layer = Layer::Logic;
    let changed = [
        key(&context, &[], b"representation", 1, &mut ledger()?)?,
        key(&layer, &[], b"representation", 1, &mut ledger()?)?,
        key(
            &claim,
            &[fixture.x.logical_id()],
            b"representation",
            1,
            &mut ledger()?,
        )?,
        key(&claim, &[], b"other representation", 1, &mut ledger()?)?,
        key(&claim, &[], b"representation", 2, &mut ledger()?)?,
    ];
    for candidate in changed {
        assert!(!retained.contains(&candidate));
    }
    assert!(retained.contains(&key(&claim, &[], b"representation", 1, &mut ledger()?)?));
    Ok(())
}

/// Rolled-back checked proofs cannot be cited after a different theorem reuses their serial.
/// ´test:crate:reading-memo-invalidates-rolled-back-proof´
#[test]
fn reading_memo_invalidates_rolled_back_proof() -> TestResult {
    let mut fixture = Fixture::new()?;
    let selected = fixture.cas.define_reading(
        &fixture.x,
        &fixture.x,
        &crate::readings::AffineMap::identity(1, &mut ledger()?)?,
        &mut ledger()?,
    )?;
    let mut budget = ledger()?;
    let mut former = None;
    let refusal: Result<(), crate::Fault> = fixture.cas.transaction(&mut budget, |cas, budget| {
        let witness = cas.prove_reading_equality(&fixture.context, &selected, &selected, budget)?;
        former = Some(witness.proof().logical_id());
        Err(crate::Fault::invalid(
            crate::InvalidCode::Rule,
            crate::Location::new("memo rollback"),
        ))
    });
    assert!(refusal.is_err());
    let replacement = fixture.cas.check_proof(
        &fixture.context,
        &[crate::proof_model::ApplicationDraft {
            rule: crate::rule::Primitive::Reflexivity,
            context: fixture.context.clone(),
            conclusion: Statement::Equality {
                left: fixture.one.clone(),
                right: fixture.one.clone(),
                domain: fixture.domain.clone(),
                domain_contract: DomainContract::DefinedAndEqual,
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: crate::proof_model::Certificate::None,
        }],
        &mut ledger()?,
    )?;
    assert_eq!(
        replacement.proof().logical_id(),
        former.ok_or("original proof")?
    );
    let actual =
        fixture
            .cas
            .prove_reading_equality(&fixture.context, &selected, &selected, &mut budget)?;
    assert_ne!(actual.judgment(), replacement.judgment());
    assert_ne!(actual.proof(), replacement.proof());
    Ok(())
}

/// A stricter source-depth ceiling misses the citation and retains no duplicate memo allowance.
/// ´test:crate:reading-memo-respects-tighter-policy´
#[test]
fn reading_memo_respects_tighter_policy() -> TestResult {
    let mut fixture = Fixture::new()?;
    let selected = fixture.cas.define_reading(
        &fixture.x,
        &fixture.x,
        &crate::readings::AffineMap::identity(1, &mut ledger()?)?,
        &mut ledger()?,
    )?;
    let mut budget = ledger()?;
    let original =
        fixture
            .cas
            .prove_reading_equality(&fixture.context, &selected, &selected, &mut budget)?;
    let retained = budget.reading_proofs.bytes;
    let before = fixture.cas.counts();
    let strict = crate::limits::Limits {
        depth: 1,
        ..budget.limits().clone()
    };
    let previous = budget.restrict_storage(&strict);
    assert!(
        fixture
            .cas
            .prove_reading_equality(&fixture.context, &selected, &selected, &mut budget)
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), before);
    assert_eq!(budget.reading_proofs.bytes, retained);
    budget.restore_limits(previous);
    let repeated =
        fixture
            .cas
            .prove_reading_equality(&fixture.context, &selected, &selected, &mut budget)?;
    assert_eq!(repeated.judgment(), original.judgment());
    Ok(())
}
