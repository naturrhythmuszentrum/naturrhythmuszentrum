// SPDX-License-Identifier: AGPL-3.0-only
//! Retained capacity, conditional context and real ground-receipt controls.

use super::*;

/// Spare vector capacity cannot hide from the local staged-payload boundary.
/// ´test:unit:lc-root-family-capacity-refusals´
/// ´claim:cas:lc-root-family-capacity-refusals´
#[test]
fn lc_root_family_capacity_refusals() -> TestResult {
    let f = fixture()?;
    for slot in 0..3 {
        let mut p = proposed(&f)?;
        match slot {
            0 => p.universal.try_reserve_exact(64)?,
            1 => p.original.inputs.try_reserve_exact(64)?,
            _ => p.original.constraints.try_reserve_exact(64)?,
        }
        let mut budget = crate::budget::Budget::new(Limits {
            staged_payload_bytes: 1408,
            ..Limits::default()
        })?;
        let result = check::verify(&f.cas, &f.request, &p, &mut budget, &mut Events::default());
        let error = result.err().ok_or("hidden retained capacity")?;
        assert!(matches!(
            error.kind,
            crate::FaultKind::Exhausted {
                budget: BudgetKind::StagedPayloadBytes,
                ..
            }
        ));
        assert_eq!(budget.usage().work, 0);
    }
    let mut request = f.request.clone();
    request.inputs.try_reserve_exact(64)?;
    let mut budget = crate::budget::Budget::new(Limits {
        staged_payload_bytes: 1392,
        ..Limits::default()
    })?;
    assert!(claim::construct(&f.cas, &request, &mut budget, &mut Events::default()).is_err());
    assert_eq!(budget.usage().work, 0);
    Ok(())
}

/// Equivalent selected values do not erase positions or the exact conditional fibre.
/// ´test:unit:lc-root-family-original-closure´
/// ´claim:cas:lc-root-family-original-closure´
#[test]
fn lc_root_family_original_closure() -> TestResult {
    let mut f = fixture()?;
    let p = proposed(&f)?;
    let mut repeated = f.request.clone();
    repeated.positions[0] = 3;
    let same_value = claim::construct(&f.cas, &repeated, &mut ledger()?, &mut Events::default())?;
    check::verify(
        &f.cas,
        &repeated,
        &same_value,
        &mut ledger()?,
        &mut Events::default(),
    )?;
    refused(&f, &f.request, &same_value)?;
    let [_, context]: [Handle; 2] = f
        .cas
        .admit(
            &[
                Draft::Region(RegionDraft {
                    header: Reference::Existing(f.header.clone()),
                    scope: Vec::new(),
                    ambient: AmbientDomain::Assignments,
                    expression: RegionExpr::Ambient,
                }),
                Draft::Context(ContextDraft::Root {
                    header: Reference::Existing(f.header.clone()),
                    admissible: Reference::Local(0),
                    assumptions: Vec::new(),
                }),
            ],
            &mut ledger()?,
        )?
        .try_into()
        .map_err(|_| "unconditional context")?;
    let mut unconditional = f.request.clone();
    unconditional.context = context;
    refused(&f, &unconditional, &p)?;
    let unguarded = claim::construct(
        &f.cas,
        &unconditional,
        &mut ledger()?,
        &mut Events::default(),
    )?;
    refused(&f, &f.request, &unguarded)?;
    let mut changed = p;
    changed.universal.swap(3, 4);
    refused(&f, &f.request, &changed)?;
    Ok(())
}

/// A genuine checked `RootGrid` result remains a ground calculation, not a quantified family law.
/// ´test:unit:lc-root-family-ground-receipt´
/// ´claim:cas:lc-root-family-ground-receipt´
#[test]
fn lc_root_family_ground_receipt() -> TestResult {
    use crate::enclosure::{EnclosureGoal, EnclosureRequest, RefinementPolicy};
    let mut f = fixture()?;
    let two = term(
        &mut f.cas,
        &f.header,
        Operator::Rational(Rational::from_ratio(2, 1, &mut ledger()?)?),
        real(),
        &[],
    )?;
    let product = term(
        &mut f.cas,
        &f.header,
        Operator::Mul,
        real(),
        &[two.clone(), two.clone()],
    )?;
    let half = term(
        &mut f.cas,
        &f.header,
        Operator::Rational(Rational::from_ratio(1, 2, &mut ledger()?)?),
        real(),
        &[],
    )?;
    let root = term(
        &mut f.cas,
        &f.header,
        Operator::GeneralPower(Branch::StrictReal),
        real(),
        &[product.clone(), half],
    )?;
    let report = f.cas.enclose(
        &EnclosureRequest {
            target: root.clone(),
            context: f.request.context.clone(),
            coordinates: Vec::new(),
            goal: EnclosureGoal::Width,
        },
        RefinementPolicy {
            version: 1,
            precision: 4,
            grid_ceiling: 64,
            rounds: 2,
            reduction_ceiling: 16,
            term_ceiling: 64,
            checking_work: 50_000,
        },
        &mut ledger()?,
        &mut |_| false,
    )?;
    let receipt = report.checked.ok_or("ground root receipt")?;
    assert_eq!(receipt.formal_schema().name, "enclosure.root-grid");
    let wire = receipt.certificate().to_bytes(&mut ledger()?)?;
    let wrong_category = Certificate::from_bytes(&wire, &mut ledger()?)?;
    let retained = f.cas.retained_judgment_count();
    assert!(
        f.cas
            .check_lc_root(&f.request, &wrong_category, &mut ledger()?)
            .is_err()
    );
    assert_eq!(f.cas.retained_judgment_count(), retained);
    let mut substitution = proposed(&f)?;
    substitution.recipe = receipt.formal_schema().name;
    refused(&f, &f.request, &substitution)?;
    let mut ground = f.request.clone();
    ground.inputs[1] = two.clone();
    ground.inputs[2] = two;
    ground.targets[0] = product;
    ground.targets[1] = root;
    assert!(claim::construct(&f.cas, &ground, &mut ledger()?, &mut Events::default()).is_err());
    Ok(())
}

/// Semantic refusals retain literal prefixes, and their one-below neighbours exhaust honestly.
/// ´test:unit:lc-root-family-refusal-boundaries´
/// ´claim:cas:lc-root-family-refusal-boundaries´
#[test]
fn lc_root_family_refusal_boundaries() -> TestResult {
    let f = fixture()?;
    let p = proposed(&f)?;
    let mut weak = p;
    weak.premises[2].1 = Relation::AtMost;
    for (limit, invalid, spent) in [(111, true, 111), (110, false, 110)] {
        let mut budget = crate::budget::Budget::new(Limits {
            work: limit + 7,
            ..Limits::default()
        })?;
        budget.work(7, Location::new("caller prefix"))?;
        let error = check::verify(
            &f.cas,
            &f.request,
            &weak,
            &mut budget,
            &mut Events::default(),
        )
        .err()
        .ok_or("weak premise")?;
        assert_eq!(matches!(error.kind, crate::FaultKind::Invalid(_)), invalid);
        assert_eq!(budget.usage().work, spent + 7);
    }
    let foreign = fixture()?;
    let foreign_proposal = proposed(&foreign)?;
    let mut budget = ledger()?;
    assert!(
        check::verify(
            &f.cas,
            &foreign.request,
            &foreign_proposal,
            &mut budget,
            &mut Events::default()
        )
        .is_err()
    );
    assert_eq!(budget.usage().work, 1);
    let mut altered = fixture()?;
    let original = proposed(&altered)?;
    let root = altered
        .cas
        .checked(&altered.request.targets[1], &mut budget)?;
    let exponent = altered.cas.state.expression(root, &mut budget)?.operands[1];
    let index = altered
        .cas
        .state
        .indices
        .peek(&exponent)
        .copied()
        .ok_or("index missing")?;
    if let crate::graph::Vertex::Expression(value) = &mut altered.cas.state.graph[index] {
        value.operator = Operator::Rational(Rational::from_ratio(1, 3, &mut ledger()?)?);
    } else {
        return Err("exponent".into());
    }
    let mut budget = ledger()?;
    assert!(
        check::verify(
            &altered.cas,
            &altered.request,
            &original,
            &mut budget,
            &mut Events::default()
        )
        .is_err()
    );
    assert_eq!(budget.usage().work, 57);
    Ok(())
}
