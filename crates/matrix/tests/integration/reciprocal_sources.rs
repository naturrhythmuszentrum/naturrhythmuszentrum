// SPDX-License-Identifier: AGPL-3.0-only
//! LC source derivatives retain the common original graph and one cumulative caller policy.

use crate::reading_testing::TestResult;
use crate::reciprocal_testing::plan_with_policy;
use nrz_bindings::readings::algebra::{Location, proof::Statement};
use nrz_bindings::readings::{
    PositiveReciprocalMap, ReciprocalJacobianCache, ReciprocalJacobianKey,
};
use nrz_bindings::{Budget, Limits};

/// One exact operator key serves repeated uses without graph growth or proof reconstruction.
/// ´test:integration:reciprocal-source-jacobian-reuses-original-graph´
#[test]
fn reciprocal_source_jacobian_reuses_original_graph() -> TestResult {
    let mut prepared = plan_with_policy(policy(), false)?;
    let mut budget = Budget::new(policy())?;
    budget.work(17, Location::new("earlier symbolic construction"))?;
    let recipe = PositiveReciprocalMap::prove(
        &mut prepared.case.cas,
        &prepared.frame,
        prepared.definition,
        &mut budget,
    )?;
    let built = budget.usage();
    let key = ReciprocalJacobianKey::new(&mut prepared.case.cas, &recipe, &[], &mut budget)?;
    assert_eq!(key.usage().before, built);
    assert_eq!(key.positions(), [0, 1]);
    assert_eq!(key.native().original(), recipe.original().expression());
    let mut cache = ReciprocalJacobianCache::new();
    let prior = budget.usage();
    let first = cache.get_or_derive(&mut prepared.case.cas, &key, &mut budget)?;
    assert_eq!(first.usage().before, prior);
    assert_eq!(first.usage().after, budget.usage());
    assert!(first.native().witness().is_discharged());
    assert!(
        first
            .native()
            .representation()
            .ok_or("original representation")?
            .is_discharged()
    );
    let Statement::Jacobian { variables, .. } = &first.native().witness().judgment().statement
    else {
        return Err("source derivative statement".into());
    };
    assert_eq!(variables, &["L", "C"]);
    let counts = prepared.case.cas.counts();
    let repeated = cache.get_or_derive(&mut prepared.case.cas, &key, &mut budget)?;
    assert!(std::ptr::eq(first.native(), repeated.native()));
    assert_eq!(cache.len(), 1);
    assert_eq!(prepared.case.cas.counts(), counts);
    assert_eq!(
        (
            built.work,
            key.usage().after.work - built.work,
            first.usage().after.work - prior.work,
            budget.usage().work - first.usage().after.work,
            first.retained_payload_bytes()
        ),
        (925_299, 381_309, 5_991_078, 7_579, 108_158)
    );
    Ok(())
}

/// Fixed declarations remain original positive denominators and introduce no derivative column.
/// ´test:integration:reciprocal-source-jacobian-keeps-fixed-input-domains´
#[test]
fn reciprocal_source_jacobian_keeps_fixed_input_domains() -> TestResult {
    let limits = Limits {
        work: 10_302_957,
        replay_work: 10_302_957,
        ..Limits::default()
    };
    let mut prepared = plan_with_policy(limits.clone(), false)?;
    let mut budget = Budget::new(limits)?;
    let mut costs = Vec::new();
    let recipe = PositiveReciprocalMap::prove(
        &mut prepared.case.cas,
        &prepared.frame,
        prepared.definition.clone(),
        &mut budget,
    )?;
    let mut cache = ReciprocalJacobianCache::new();
    for fixed in [vec!["C".into()], vec!["L".into(), "C".into()]] {
        let before = budget.usage().work;
        let key = ReciprocalJacobianKey::new(&mut prepared.case.cas, &recipe, &fixed, &mut budget)?;
        let derivative = cache.get_or_derive(&mut prepared.case.cas, &key, &mut budget)?;
        costs.push(budget.usage().work - before);
        assert_eq!(key.positions().len(), 2 - fixed.len());
        assert_eq!(derivative.definition().inputs(), recipe.inputs());
        assert_eq!(derivative.key().native().coordinates().fixed, fixed);
        assert!(
            derivative
                .native()
                .representation()
                .ok_or("fixed original")?
                .is_discharged()
        );
    }
    assert_eq!(cache.len(), 2);
    for fixed in [vec!["absent".into()], vec!["L".into(), "L".into()]] {
        assert!(
            ReciprocalJacobianKey::new(&mut prepared.case.cas, &recipe, &fixed, &mut budget)
                .is_err()
        );
    }
    let mut foreign = plan_with_policy(policy(), false)?;
    let key = ReciprocalJacobianKey::new(&mut prepared.case.cas, &recipe, &[], &mut budget)?;
    assert!(
        cache
            .get_or_derive(&mut foreign.case.cas, &key, &mut budget)
            .is_err()
    );
    assert_eq!(cache.len(), 2);
    assert_eq!(
        (costs, budget.usage().work),
        (vec![4_895_920, 3_835_967], 10_302_957)
    );
    Ok(())
}

/// Complete construction, derivation and reuse consume one allowance across requests and retries.
/// ´test:integration:reciprocal-source-jacobian-cumulative-boundary´
#[test]
fn reciprocal_source_jacobian_cumulative_boundary() -> TestResult {
    let mut measured = Budget::new(policy())?;
    pipeline(&mut measured)?;
    let required = measured.usage().work;
    assert_eq!(required, 7_305_265);
    for ceiling in [required, required - 1] {
        let mut budget = Budget::new(Limits {
            work: ceiling,
            ..policy()
        })?;
        let result = pipeline(&mut budget);
        assert_eq!(result.is_ok(), ceiling == required, "{result:?}");
        if result.is_err() {
            let spent = budget.usage();
            assert!(pipeline(&mut budget).is_err());
            assert!(budget.usage().work >= spent.work);
        }
    }
    Ok(())
}

/// Original fixture setup is independent; every admitted recipe and derivative shares the supplied ledger.
fn pipeline(budget: &mut Budget) -> TestResult {
    let mut prepared = plan_with_policy(policy(), false)?;
    budget.work(17, Location::new("previous operation"))?;
    let recipe = PositiveReciprocalMap::prove(
        &mut prepared.case.cas,
        &prepared.frame,
        prepared.definition,
        budget,
    )?;
    let key = ReciprocalJacobianKey::new(&mut prepared.case.cas, &recipe, &[], budget)?;
    let mut cache = ReciprocalJacobianCache::new();
    cache.get_or_derive(&mut prepared.case.cas, &key, budget)?;
    cache.get_or_derive(&mut prepared.case.cas, &key, budget)?;
    Ok(())
}

/// Exact measured recipe, key, native derivative and one cache hit, including 17 prior Work.
fn policy() -> Limits {
    Limits {
        work: 7_305_265,
        replay_work: 7_305_265,
        ..Limits::default()
    }
}

/// Exported representation and derivative witnesses replay independently under one explicit audit ledger.
/// ´test:integration:reciprocal-source-jacobian-fresh-witness-costs´
#[test]
fn reciprocal_source_jacobian_fresh_witness_costs() -> TestResult {
    let mut prepared = plan_with_policy(policy(), false)?;
    let mut budget = Budget::new(policy())?;
    let recipe = PositiveReciprocalMap::prove(
        &mut prepared.case.cas,
        &prepared.frame,
        prepared.definition,
        &mut budget,
    )?;
    let key = ReciprocalJacobianKey::new(&mut prepared.case.cas, &recipe, &[], &mut budget)?;
    let mut cache = ReciprocalJacobianCache::new();
    let value = cache.get_or_derive(&mut prepared.case.cas, &key, &mut budget)?;
    let mut audit = Budget::new(policy())?;
    let mut costs = Vec::new();
    for witness in value
        .native()
        .representation()
        .into_iter()
        .chain(std::iter::once(value.native().witness()))
    {
        let bytes = prepared.case.cas.export_witness(witness, &mut audit)?;
        let mut receiver = nrz_bindings::readings::algebra::Cas::new(policy())?;
        let before = audit.usage();
        let checked = receiver.inspect(&bytes, &mut audit)?;
        let cost = audit.usage().work - before.work;
        assert!(checked.is_discharged());
        let expected = prepared.case.cas.witness_reuse_key(witness, &mut audit)?;
        let actual = receiver.witness_reuse_key(&checked, &mut audit)?;
        assert_eq!(expected.as_bytes(), actual.as_bytes());
        costs.push((bytes.len(), cost));
    }
    assert_eq!(costs, vec![(1_981, 813_625), (3_796, 1_081_882)]);
    Ok(())
}
