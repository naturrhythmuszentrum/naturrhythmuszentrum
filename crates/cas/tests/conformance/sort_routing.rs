// SPDX-License-Identifier: AGPL-3.0-only
//! Complete original support and checked ambient selection.

use crate::testing::common::TestResult;
use nrz_cas::limits::{Budget, Limits};
use nrz_cas::solving::{AmbientResolution, ScalarRoute, SolveReason};
use nrz_cas::syntax::{Field, Operator};

/// Real-only, Complex-only and mixed routes retain original declarations without arithmetic.
/// ´test:integration:scalar-routing-partitions-original-support´
#[test]
fn scalar_routing_partitions_original_support() -> TestResult {
    for (field, denominator, route, refusal) in [
        (Field::Real, 0, ScalarRoute::Real, None),
        (Field::Complex, 0, ScalarRoute::Complex, None),
        (Field::Complex, 4, ScalarRoute::Family, None),
        (
            Field::Real,
            1,
            ScalarRoute::Real,
            Some(SolveReason::RealProblemHasComplexSymbol),
        ),
        (
            Field::Real,
            2,
            ScalarRoute::Real,
            Some(SolveReason::RealProblemHasComplexSymbol),
        ),
        (
            Field::Real,
            3,
            ScalarRoute::Real,
            Some(SolveReason::RealProblemHasComplexSymbol),
        ),
    ] {
        let fixture = crate::testing::sort_routing::Fixture::new(field, denominator)?;
        let mut budget = Budget::new(Limits::default())?;
        let actual = fixture
            .cas
            .sort_routing(&fixture.problem, &[], &mut budget)?;
        assert_eq!(actual.route, route);
        assert_eq!(actual.refusal, refusal);
        let usage = budget.usage();
        assert_eq!(
            [
                usage.coefficient_ops,
                usage.gcd_steps,
                usage.orbit_permutations,
                usage.tower_reductions
            ],
            [0; 4]
        );
    }
    Ok(())
}

/// Every additional original root participates even when its value could be discarded.
/// ´test:integration:scalar-routing-retains-additional-source-root´
#[test]
fn scalar_routing_retains_additional_source_root() -> TestResult {
    let fixture = crate::testing::sort_routing::Fixture::new(Field::Real, 0)?;
    let extra = [fixture.real, fixture.complex];
    let route = fixture.cas.sort_routing(
        &fixture.problem,
        &extra,
        &mut Budget::new(Limits::default())?,
    )?;
    assert_eq!(
        route.refusal,
        Some(SolveReason::RealProblemHasComplexSymbol)
    );
    assert_eq!(
        route
            .complex
            .iter()
            .map(|(_, key)| key.as_str())
            .collect::<Vec<_>>(),
        ["c"]
    );
    Ok(())
}

/// The complete preflight has a reproducible cap-minus-one refusal and publishes no record.
/// ´test:integration:scalar-routing-work-cap-minus-one´
#[test]
fn scalar_routing_work_cap_minus_one() -> TestResult {
    let fixture = crate::testing::sort_routing::Fixture::new(Field::Real, 2)?;
    let mut measured = Budget::new(Limits::default())?;
    fixture
        .cas
        .sort_routing(&fixture.problem, &[], &mut measured)?;
    let work = measured.usage().work;
    let before = fixture.cas.counts();
    let limits = Limits {
        work: work.checked_sub(1).ok_or("no routing work")?,
        ..Limits::default()
    };
    let fault = fixture
        .cas
        .sort_routing(&fixture.problem, &[], &mut Budget::new(limits)?)
        .expect_err("preflight exceeded its work cap");
    assert!(matches!(fault.kind, nrz_cas::FaultKind::Exhausted { .. }));
    assert_eq!(fixture.cas.counts(), before);
    let exact = Limits {
        work,
        ..Limits::default()
    };
    fixture
        .cas
        .sort_routing(&fixture.problem, &[], &mut Budget::new(exact)?)?;
    Ok(())
}

/// Own-space, mixed and explicit reverse requests use only direct registered edges.
/// ´test:integration:scalar-ambient-resolution-is-checked´
#[test]
fn scalar_ambient_resolution_is_checked() -> TestResult {
    let fixture = crate::testing::sorts::Fixture::new()?;
    let mut budget = Budget::new(Limits::default())?;
    let own = fixture.cas.resolve_scalar_ambient(
        &fixture.header,
        &Operator::Neg,
        std::slice::from_ref(&fixture.real),
        None,
        &mut budget,
    )?;
    assert_eq!(
        own,
        AmbientResolution::Resolved {
            ambient: fixture.real_space.clone(),
            placements: Vec::new()
        }
    );
    let mixed = fixture.cas.resolve_scalar_ambient(
        &fixture.header,
        &Operator::Add,
        &[fixture.real.clone(), fixture.complex.clone()],
        None,
        &mut budget,
    )?;
    let AmbientResolution::Resolved {
        ambient,
        placements,
    } = mixed
    else {
        return Err("mixed ambient unresolved".into());
    };
    assert_eq!(ambient, fixture.complex_space);
    assert_eq!(placements.len(), 1);
    assert_eq!(placements[0].operand, fixture.real);
    let absent = fixture.cas.resolve_scalar_ambient(
        &fixture.header,
        &Operator::Neg,
        std::slice::from_ref(&fixture.complex),
        Some(&fixture.real_space),
        &mut budget,
    )?;
    assert_eq!(
        absent,
        AmbientResolution::Unresolved(SolveReason::SpacePlacementUnavailable)
    );
    let ambiguous = fixture.cas.resolve_scalar_ambient(
        &fixture.header,
        &Operator::Rational(nrz_cas::Rational::zero()),
        &[],
        None,
        &mut budget,
    )?;
    assert_eq!(
        ambiguous,
        AmbientResolution::Unresolved(SolveReason::AmbientSpaceAmbiguous)
    );
    Ok(())
}

/// Symbol, binder and coordinate-slot contradictions use the named declared-sort refusal.
/// ´test:integration:scalar-routing-sort-contradictions-are-named´
#[test]
fn scalar_routing_sort_contradictions_are_named() -> TestResult {
    use crate::testing::sorts::reference;
    use nrz_cas::solving::{Coordinate, CoordinateRole, CoordinateSpaceData, SolveDraft};
    use nrz_cas::syntax::{AtomicSort, Draft, ExpressionDraft, Sort};
    let mut fixture = crate::testing::sorts::Fixture::new()?;
    for (operator, scope) in [
        (Operator::Free("r".into()), Vec::new()),
        (
            Operator::Bound { depth: 0, slot: 0 },
            vec![vec![AtomicSort::Scalar(Field::Real)]],
        ),
    ] {
        let draft = Draft::Expression(ExpressionDraft {
            space: None,
            operator,
            sort: Sort::Value(AtomicSort::Scalar(Field::Complex)),
            header: reference(&fixture.header),
            scope,
            binding: None,
            operands: Vec::new(),
        });
        let fault = fixture
            .cas
            .admit(&[draft], &mut Budget::new(Limits::default())?)
            .expect_err("false declared sort accepted");
        assert_eq!(
            fault.kind,
            nrz_cas::FaultKind::Invalid(nrz_cas::InvalidCode::SymbolSortMismatch)
        );
    }
    let frame = Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
        scalar_spaces: Vec::new(),
        header: reference(&fixture.header),
        coordinates: vec![Coordinate {
            key: "r".into(),
            field: Field::Complex,
            role: CoordinateRole::Coefficient,
        }],
    }));
    let fault = fixture
        .cas
        .admit(&[frame], &mut Budget::new(Limits::default())?)
        .expect_err("false coordinate slot accepted");
    assert_eq!(
        fault.kind,
        nrz_cas::FaultKind::Invalid(nrz_cas::InvalidCode::SymbolSortMismatch)
    );
    Ok(())
}

/// An earlier true source clause does not erase a later Complex-dependent guard.
/// ´test:integration:scalar-routing-inspects-inactive-source-clauses´
#[test]
fn scalar_routing_inspects_inactive_source_clauses() -> TestResult {
    use crate::testing::sorts::{admit, reference};
    use nrz_cas::solving::SolveView;
    use nrz_cas::source::{SourceClauseDraft, SourceFormDraft, SourceFormKind};
    use nrz_cas::syntax::{AtomicSort, Draft, ExpressionDraft, PredicateKind, Sort};
    let mut fixture = crate::testing::sort_routing::Fixture::new(Field::Real, 0)?;
    let SolveView::SolveProblem(problem) = fixture.cas.solve_view(&fixture.problem)? else {
        return Err("missing original problem".into());
    };
    let header = problem.header.clone();
    let region = problem.region.clone();
    let mut clauses = Vec::new();
    for operand in [&fixture.real, &fixture.complex] {
        let guard = admit(
            &mut fixture.cas,
            Draft::Expression(ExpressionDraft {
                space: None,
                header: reference(&header),
                scope: Vec::new(),
                binding: None,
                operator: Operator::Predicate(PredicateKind::Equal),
                sort: Sort::Value(AtomicSort::Boolean),
                operands: vec![reference(operand), reference(operand)],
            }),
        )?;
        clauses.push(SourceClauseDraft {
            selector: reference(&guard),
            value: reference(&fixture.real),
        });
    }
    let source = admit(
        &mut fixture.cas,
        Draft::SourceForm(SourceFormDraft {
            header: reference(&header),
            scope: Vec::new(),
            kind: SourceFormKind::OrderedCases,
            domain: reference(&region),
            clauses,
            otherwise: None,
        }),
    )?;
    let actual = fixture.cas.sort_routing(
        &fixture.problem,
        &[source],
        &mut Budget::new(Limits::default())?,
    )?;
    assert_eq!(
        actual.refusal,
        Some(SolveReason::RealProblemHasComplexSymbol)
    );
    Ok(())
}
