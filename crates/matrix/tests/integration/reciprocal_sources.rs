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
/// Local-entry increments on this caller ledger are prove +72 = 24 retained-kind
/// counts +24 identity-index insertions +24 vertex admissions; key +12 = 4+4+4;
/// derivative +1,886 = 602 retained-kind counts +16 first-kind insertions
/// +634 identity-index insertions +634 vertex admissions. The old graph lookup
/// floors and new retained-map lookup floors are equal. The cache hit adds zero.
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
        (812_613, 329_052, 4_816_828, 8_553, 110_593)
    );
    Ok(())
}

/// Fixed declarations remain original positive denominators and introduce no derivative column.
/// First fixed derivative: 3,845,341 operation Work + 478 retained counts + 16
/// first-kind insertions + 510 identity insertions + 510 vertex admissions =
/// 3,846,855; the measured policy-key and manifest walk adds 248 to 3,847,103.
/// Second: 2,917,358 operation Work + 86 + 16 + 118 + 118 = 2,917,696;
/// the measured walk adds 296 to 2,917,992. The caller's 8,123,364 operation
/// Work + 72 prove + 1,514 + 338 = 8,125,288; the measured walk adds 1,104
/// to 8,126,392. The earlier and current lookup floors match.
/// ´test:integration:reciprocal-source-jacobian-keeps-fixed-input-domains´
#[test]
fn reciprocal_source_jacobian_keeps_fixed_input_domains() -> TestResult {
    let limits = Limits {
        work: 8_126_392,
        replay_work: 8_126_392,
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
        (vec![3_847_103, 2_917_992], 8_126_392)
    );
    Ok(())
}

/// Complete construction, derivation and reuse consume one allowance across requests and retries.
/// 5,964,692 operation Work + 72 prove + 12 key + 1,886 derivative = 5,966,662;
/// the longer policy key and manifest walk adds a measured 384 to 5,967,046.
/// ´test:integration:reciprocal-source-jacobian-cumulative-boundary´
#[test]
fn reciprocal_source_jacobian_cumulative_boundary() -> TestResult {
    let mut measured = Budget::new(policy())?;
    pipeline(&mut measured)?;
    let required = measured.usage().work;
    assert_eq!(required, 5_967_046);
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
        work: 5_967_046,
        replay_work: 5_967_046,
        ..Limits::default()
    }
}

/// Exported representation and derivative witnesses replay independently under one explicit audit ledger.
/// Representation replay: unchanged operation Work 695,696 +54 retained counts +8 first-kind
/// insertions +70 identity insertions +70 admissions = 695,898. Derivative
/// replay: unchanged operation Work 947,622 +166 +8 +182 +182 = 948,160. Old/new lookup
/// floors match; wire lengths remain 1,981 and 3,796 bytes.
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
    assert_eq!(costs, vec![(1_981, 695_898), (3_796, 948_160)]);
    Ok(())
}
