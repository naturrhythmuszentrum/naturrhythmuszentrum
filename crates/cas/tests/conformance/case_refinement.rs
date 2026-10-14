// SPDX-License-Identifier: AGPL-3.0-only
//! Independent common-refinement regions and exact shared pair-budget boundaries.

use crate::testing::budgets::derivation_budget as budget;
use crate::testing::builders::admit;
use crate::testing::cases::Fixture;
use crate::testing::common::TestResult;
use nrz_cas::domain::{AmbientDomain, RegionDraft, RegionExpr};
use nrz_cas::limits::BudgetKind;
use nrz_cas::proof::{DomainContract, Statement};
use nrz_cas::request::{Outcome, Task};
use nrz_cas::source::SyntaxRole;
use nrz_cas::syntax::{Draft, Operator, PredicateKind};
use nrz_cas::{FaultKind, Handle, Reference};

/// Four pairs contain exactly one empty intersection, regardless of their source spelling.
/// ´test:integration:case-refinement-four-pair-boundary´
#[test]
fn case_refinement_four_pair_boundary() -> TestResult {
    let mut f = Fixture::new()?;
    let (sum, regions) = partition_sum(&mut f)?;
    let mut request = f.request(Task::Simplify, &[sum])?;
    request.limits.refinement_pairs = Some(4);
    let (result, usage) = match f.cas.run(&request) {
        Outcome::Proved { result, usage, .. } => (result, usage),
        other => return Err(format!("expected complete refinement, got {other:?}").into()),
    };
    assert_eq!(usage.refinement_pairs, 4);
    let Statement::Equality { right, .. } = &result.witness.judgment().statement else {
        return Err("expected refinement equality".into());
    };
    let output = right.clone();
    let descriptor = f.cas.describe_syntax(&output, &mut budget()?)?;
    assert_eq!(
        descriptor
            .dependencies
            .iter()
            .filter(|child| matches!(child.role, SyntaxRole::Arm(_)))
            .count(),
        3
    );
    for (region, expected) in regions.into_iter().zip(["0", "1", "3"]) {
        let value = f.number(expected)?;
        let request = f.request(
            Task::Equal {
                domain: region,
                domain_contract: DomainContract::SameDefinedDomain,
            },
            &[output.clone(), value],
        )?;
        f.proved(&request)?;
    }
    request.limits.refinement_pairs = Some(3);
    assert!(
        matches!(f.cas.run(&request), Outcome::Exhausted { fault, usage, best: None, .. }
        if matches!(fault.kind, FaultKind::Exhausted { budget: BudgetKind::RefinementPairs, needed: 4, .. })
            && usage.refinement_pairs == 3)
    );
    Ok(())
}

/// Original operands define the independently expected cells: below zero, zero through one, above one.
fn partition_sum(f: &mut Fixture) -> TestResult<(Handle, Vec<Handle>)> {
    let coordinate = f.x.clone();
    let ge = f.predicate(
        PredicateKind::NonNegative,
        std::slice::from_ref(&coordinate),
    )?;
    let negative = f.region(&ge, false)?;
    let nonnegative = f.region(&ge, true)?;
    let difference = f.term(Operator::Sub, &[f.one.clone(), f.x.clone()])?;
    let le_one = f.predicate(PredicateKind::NonNegative, &[difference])?;
    let below = f.region(&le_one, true)?;
    let above = f.region(&le_one, false)?;
    let middle = admit(
        &mut f.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(f.header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Real("x".into()),
            expression: RegionExpr::Meet(vec![
                Reference::Existing(nonnegative.clone()),
                Reference::Existing(below.clone()),
            ]),
        }),
        &mut budget()?,
    )?;
    let left = f.cases(&[
        (negative.clone(), f.zero.clone()),
        (nonnegative, f.one.clone()),
    ])?;
    let two = f.number("2")?;
    let right = f.cases(&[(below, f.zero.clone()), (above.clone(), two)])?;
    Ok((
        f.term(Operator::Add, &[left, right])?,
        vec![negative, middle, above],
    ))
}

/// Native association permutations preserve canonical theorem bytes in independent stores.
/// ´test:integration:case-native-permutation-replay-bytes´
#[test]
fn case_native_permutation_replay_bytes() -> TestResult {
    assert_eq!(permutation_bytes(false)?, permutation_bytes(true)?);
    Ok(())
}

/// Construct both spellings independently; no producer supplies the expected byte equality.
fn permutation_bytes(reverse: bool) -> TestResult<Vec<u8>> {
    let mut f = Fixture::new()?;
    let predicate = f.predicate(PredicateKind::NonNegative, &[f.x.clone()])?;
    let negative = f.region(&predicate, false)?;
    let nonnegative = f.region(&predicate, true)?;
    let mut pairs = vec![(negative, f.zero.clone()), (nonnegative, f.one.clone())];
    if reverse {
        pairs.reverse();
    }
    let cases = f.cases(&pairs)?;
    let request = f.request(
        Task::Equal {
            domain: f.domain.clone(),
            domain_contract: DomainContract::SameDefinedDomain,
        },
        &[cases.clone(), cases],
    )?;
    let result = f.proved(&request)?;
    Ok(f.cas.export_witness(&result.witness, &mut budget()?)?)
}

/// Nested refinements retain the outer request's ledger instead of resetting pair credits.
/// ´test:integration:case-nested-refinement-shared-ledger´
#[test]
fn case_nested_refinement_shared_ledger() -> TestResult {
    let mut f = Fixture::new()?;
    let (sum, _) = partition_sum(&mut f)?;
    let predicate = f.predicate(PredicateKind::NonNegative, &[f.x.clone()])?;
    let negative = f.region(&predicate, false)?;
    let nonnegative = f.region(&predicate, true)?;
    let extra = f.cases(&[(negative, f.zero.clone()), (nonnegative, f.one.clone())])?;
    let nested = f.term(Operator::Add, &[sum, extra])?;
    let mut request = f.request(Task::Simplify, &[nested])?;
    request.limits.refinement_pairs = Some(4);
    assert!(
        matches!(f.cas.run(&request), Outcome::Exhausted { fault, usage, .. }
        if matches!(fault.kind, FaultKind::Exhausted { budget: BudgetKind::RefinementPairs, needed: 5, .. })
            && usage.refinement_pairs == 4)
    );
    Ok(())
}
