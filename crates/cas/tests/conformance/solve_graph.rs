// SPDX-License-Identifier: AGPL-3.0-only
//! Public solving graph admission and immutable view contracts.

use crate::testing::common::TestResult;
use crate::testing::solve_graph::{Fixture, admit};
use nrz_cas::limits::{Budget, Limits, LimitsOverride};
use nrz_cas::solving::{
    CoefficientDomainData, Coordinate, CoordinateRole, CoordinateSpaceData, SolutionSetData,
    SolveDraft, SolveView, TupleData,
};
use nrz_cas::syntax::{Draft, Field, VertexKind};
use nrz_cas::{FaultKind, InvalidCode, Reference};

/// Proposed empty sets have no completeness flag and intern by their exact problem.
/// ´test:integration:solve-set-is-syntax-only´
#[test]
fn solve_set_is_syntax_only() -> TestResult {
    let mut fixture = Fixture::new()?;
    let draft = Draft::Solve(SolveDraft::SolutionSet(SolutionSetData {
        problem: Reference::Existing(fixture.problem.clone()),
        branches: Vec::new(),
    }));
    let set = admit(&mut fixture.cas, &draft)?;
    assert_eq!(set.logical_id().kind, VertexKind::SolutionSet);
    let before = fixture.cas.counts();
    assert_eq!(admit(&mut fixture.cas, &draft)?, set);
    assert_eq!(fixture.cas.counts(), before);
    let SolveView::SolutionSet(value) = fixture.cas.solve_view(&set)? else {
        return Err("wrong view".into());
    };
    assert_eq!(value.branches, []);
    assert_eq!(value.problem, fixture.problem);
    Ok(())
}

/// Foreign product links and forward references fail without publishing a batch prefix.
/// ´test:integration:solve-admission-rollback´
#[test]
fn solve_admission_rollback() -> TestResult {
    let mut fixture = Fixture::new()?;
    let other = Fixture::new()?;
    let before = fixture.cas.counts();
    let drafts = [
        Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Gaussian,
        )),
        Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
            scalar_spaces: Vec::new(),
            header: Reference::Existing(other.header),
            coordinates: Vec::new(),
        })),
    ];
    let fault = fixture
        .cas
        .admit(&drafts, &mut Budget::new(Limits::default())?)
        .expect_err("foreign header accepted");
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::ForeignHandle));
    assert_eq!(fixture.cas.counts(), before);
    let forward = Draft::Solve(SolveDraft::CoefficientDomain(
        CoefficientDomainData::Quotient {
            level: Reference::Local(0),
        },
    ));
    assert!(
        fixture
            .cas
            .admit(
                std::slice::from_ref(&forward),
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// Coordinate order and field are checked, and an empty product has one empty tuple.
/// ´test:integration:solve-product-and-empty-tuple´
#[test]
fn solve_product_and_empty_tuple() -> TestResult {
    let mut fixture = Fixture::new()?;
    let empty = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
            scalar_spaces: Vec::new(),
            header: Reference::Existing(fixture.header.clone()),
            coordinates: Vec::new(),
        })),
    )?;
    let tuple = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::Tuple(TupleData {
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            coordinates: Reference::Existing(empty),
            entries: Vec::new(),
        })),
    )?;
    let SolveView::Tuple(value) = fixture.cas.solve_view(&tuple)? else {
        return Err("wrong tuple view".into());
    };
    assert_eq!(value.entries, []);
    let coordinate = Coordinate {
        key: "x".into(),
        field: Field::Real,
        role: CoordinateRole::Unknown,
    };
    let duplicate = Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
        scalar_spaces: Vec::new(),
        header: Reference::Existing(fixture.header),
        coordinates: vec![coordinate.clone(), coordinate],
    }));
    assert!(
        fixture
            .cas
            .admit(
                std::slice::from_ref(&duplicate),
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    Ok(())
}

/// Every old or unknown magic refuses before even a zero-work ledger can allocate replay state.
/// ´test:integration:solve-wire-old-magic-before-allocation´
#[test]
fn solve_wire_old_magic_before_allocation() -> TestResult {
    let mut fixture = Fixture::new()?;
    let before = fixture.cas.counts();
    for magic in [
        b"NRZCAS01",
        b"NRZCAS02",
        b"NRZCAS00",
        b"NRZCAS03",
        b"XXXXXXXX",
    ] {
        let mut ledger = Budget::new(Limits {
            work: 0,
            ..Limits::default()
        })?;
        let fault = fixture
            .cas
            .inspect(magic, &mut ledger)
            .err()
            .ok_or("old magic accepted")?;
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::UnsupportedVersion)
        );
        assert_eq!(ledger.usage().work, 0);
        assert_eq!(fixture.cas.counts(), before);
    }
    Ok(())
}

/// An inflated record count is rejected boundedly without a partially imported graph.
/// ´test:integration:solve-wire-count-preflight´
#[test]
fn solve_wire_count_preflight() -> TestResult {
    let mut fixture = Fixture::new()?;
    let mut bytes = b"NRZCAS04".to_vec();
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    let before = fixture.cas.counts();
    assert!(
        fixture
            .cas
            .inspect(&bytes, &mut Budget::new(Limits::default())?)
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// Every new public default and override intersects with the permanent store ceiling.
/// ´test:integration:solve-limit-defaults-and-overrides´
#[test]
fn solve_limit_defaults_and_overrides() -> TestResult {
    for case in &SOLVING_LIMITS {
        let mut store = Limits::default();
        assert_eq!(store.get(case.kind), case.default);
        (case.limit)(&mut store, 9);
        let mut overrides = nrz_cas::limits::LimitsOverride::default();
        (case.override_limit)(&mut overrides, 2);
        assert_eq!(store.resolve(&overrides)?.get(case.kind), 2);
        (case.override_limit)(&mut overrides, 10);
        assert_eq!(store.resolve(&overrides)?.get(case.kind), 9);
    }
    Ok(())
}

/// All semantic counters accept the exact boundary, reject N minus one, and retain failed usage.
/// ´test:integration:solve-limit-boundaries´
#[test]
fn solve_limit_boundaries() -> TestResult {
    let at = nrz_cas::Location::new("solving limit boundary");
    for case in &SOLVING_LIMITS {
        let mut limits = Limits::default();
        (case.limit)(&mut limits, 2);
        let mut ledger = Budget::new(limits.clone())?;
        ledger.solving(case.kind, 1, at)?;
        ledger.solving(case.kind, 1, at)?;
        assert_eq!(
            (case.usage)(ledger.usage()),
            if case.cumulative { 2 } else { 1 }
        );
        let amount = if case.cumulative { 1 } else { 3 };
        let fault = ledger
            .solving(case.kind, amount, at)
            .err()
            .ok_or("limit not enforced")?;
        assert_eq!(fault, nrz_cas::Fault::exhausted(case.kind, 2, 3, at));
        assert_eq!((case.usage)(ledger.usage()), 3);
        (case.limit)(&mut limits, 1);
        let mut below = Budget::new(limits.clone())?;
        assert_eq!(
            below.solving(case.kind, 2, at),
            Err(nrz_cas::Fault::exhausted(case.kind, 1, 2, at))
        );
        (case.limit)(&mut limits, 0);
        let mut zero = Budget::new(limits)?;
        assert_eq!(
            zero.solving(case.kind, 1, at),
            Err(nrz_cas::Fault::exhausted(case.kind, 0, 1, at))
        );
        assert_eq!((case.usage)(zero.usage()), 1);
    }
    Ok(())
}

/// Nested cumulative counters refuse checked overflow without wrapping or refunding prior work.
/// ´test:integration:solve-limit-overflow´
#[test]
fn solve_limit_overflow() -> TestResult {
    let at = nrz_cas::Location::new("solving overflow");
    for case in &SOLVING_LIMITS {
        if !case.cumulative {
            continue;
        }
        let mut limits = Limits::default();
        (case.limit)(&mut limits, u64::MAX);
        let mut ledger = Budget::new(limits)?;
        ledger.solving(case.kind, u64::MAX, at)?;
        assert_eq!(
            ledger.solving(case.kind, 1, at),
            Err(nrz_cas::Fault::capacity(case.kind, at))
        );
        assert_eq!((case.usage)(ledger.usage()), u64::MAX);
    }
    Ok(())
}

/// The explicit cubic profile changes exactly the published finite fields.
/// ´test:integration:solve-blind-cubic-profile´
#[test]
fn solve_blind_cubic_profile() -> TestResult {
    let expected = Limits {
        arithmetic_steps: 1_534_599_168,
        work: 2_843_222_016,
        replay_work: 2_843_222_016,
        component_bits: 64,
        intermediate_bits: 256,
        coefficient_ops: 4_472_832,
        gcd_steps: 82_982_016,
        tower_reductions: 124_992,
        orbit_permutations: 3840,
        orbit_terms: 196_608,
        symmetric_steps: 864,
        tower_levels: 4,
        tower_degree: 16,
        orbit_members: 16,
        resolvent_depth: 4,
        resolvent_degree: 3,
        selection_slots: 16,
        solution_branches: 256,
        witness_bytes: 4_194_304,
        output_bytes: 4_194_304,
        condition_atoms: 1024,
        condition_steps: 1_000_000,
        ..Limits::default()
    };
    assert_eq!(Limits::blind_cubic(), expected);
    expected.validate()?;
    Ok(())
}

/// Registered schema metadata stays stable; unavailable schemas refuse admission and replay.
/// ´test:integration:solve-unavailable-schemas´
#[test]
fn solve_unavailable_schemas() -> TestResult {
    use nrz_cas::proof::{ApplicationDraft, Certificate, Primitive, Statement};
    use nrz_cas::solving::{Availability, SOLVE_SCHEMAS};
    let mut fixture = Fixture::new()?;
    let before = fixture.cas.counts();
    let mut expected_bitmap = 0_u32;
    for (index, schema) in SOLVE_SCHEMAS.iter().copied().enumerate() {
        let spec = schema.specification();
        assert_eq!(usize::from(schema.tag()), index + usize::from(index >= 13));
        assert_eq!(
            spec.available,
            matches!(schema.availability(), Availability::Available)
        );
        if spec.available {
            expected_bitmap |= 1_u32 << schema.tag();
            continue;
        }
        let application = ApplicationDraft {
            rule: Primitive::Solve(schema),
            context: fixture.context.clone(),
            conclusion: Statement::Empty {
                region: fixture.problem.clone(),
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::None,
        };
        let fault = fixture
            .cas
            .check_proof(
                &fixture.context,
                std::slice::from_ref(&application),
                &mut Budget::new(Limits::default())?,
            )
            .err()
            .ok_or("unavailable checker accepted")?;
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::UnsupportedFragment)
        );
        let mut bytes = b"NRZCAS04".to_vec();
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&u32::try_from(spec.key.len())?.to_le_bytes());
        bytes.extend_from_slice(spec.key.as_bytes());
        bytes.extend_from_slice(&spec.version.to_le_bytes());
        let fault = fixture
            .cas
            .inspect(&bytes, &mut Budget::new(Limits::default())?)
            .err()
            .ok_or("unavailable manifest accepted")?;
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::UnsupportedVersion)
        );
        assert_eq!(fixture.cas.counts(), before);
    }
    assert_eq!(nrz_cas::solving::active_schema_bitmap(), expected_bitmap);
    assert_eq!(
        nrz_cas::request::SearchPolicy::default().active_schema_bitmap(),
        expected_bitmap
    );
    Ok(())
}

/// A refused branch remains charged even when admission publishes no graph state.
/// ´test:integration:solve-staged-branch-rollback-usage´
#[test]
fn solve_staged_branch_rollback_usage() -> TestResult {
    use nrz_cas::solving::{SolutionBranchData, SolutionBranchKind};
    let mut fixture = Fixture::new()?;
    let SolveView::SolveProblem(problem) = fixture.cas.solve_view(&fixture.problem)? else {
        return Err("problem missing".into());
    };
    let draft = Draft::Solve(SolveDraft::SolutionBranch(SolutionBranchData {
        problem: Reference::Existing(fixture.problem.clone()),
        region: Reference::Existing(problem.region.clone()),
        domain: Reference::Existing(problem.domain.clone()),
        kind: SolutionBranchKind::Empty,
    }));
    let before = fixture.cas.counts();
    let mut ledger = Budget::new(Limits {
        solution_branches: 0,
        ..Limits::default()
    })?;
    assert!(
        fixture
            .cas
            .admit(std::slice::from_ref(&draft), &mut ledger)
            .is_err()
    );
    assert_eq!(ledger.usage().solution_branches, 1);
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// A bound family slot is admitted while a free use of its declaration is rejected atomically.
/// ´test:integration:solve-family-capture-refusal´
#[test]
fn solve_family_capture_refusal() -> TestResult {
    let mut fixture = Fixture::new()?;
    let valid = crate::testing::solve_graph::family_draft(&mut fixture, false)?;
    let family = admit(&mut fixture.cas, &valid)?;
    assert!(matches!(
        fixture.cas.solve_view(&family)?,
        SolveView::FamilyMap(_)
    ));
    let escaped = crate::testing::solve_graph::family_draft(&mut fixture, true)?;
    let before = fixture.cas.counts();
    assert!(
        fixture
            .cas
            .admit(
                std::slice::from_ref(&escaped),
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}

/// One independently expected semantic ceiling and its public accessors.
struct LimitCase {
    /// Exact resource identity.
    kind: nrz_cas::limits::BudgetKind,
    /// Pinned ordinary default.
    default: u64,
    /// Cumulative accounting rather than a peak.
    cumulative: bool,
    /// Set only this ceiling.
    limit: fn(&mut Limits, u64),
    /// Set only this request override.
    override_limit: fn(&mut LimitsOverride, u64),
    /// Read this observed usage counter.
    usage: fn(nrz_cas::limits::BudgetUsage) -> u64,
}

/// Independent table for every solving ceiling in the design's public order.
const SOLVING_LIMITS: [LimitCase; 21] = [
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::CoefficientOps,
        default: 2_000_000,
        cumulative: true,
        limit: |value, ceiling| value.coefficient_ops = ceiling,
        override_limit: |value, ceiling| value.coefficient_ops = Some(ceiling),
        usage: |value| value.coefficient_ops,
    },
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::DenominatorFactors,
        default: 256,
        cumulative: false,
        limit: |value, ceiling| value.denominator_factors = ceiling,
        override_limit: |value, ceiling| value.denominator_factors = Some(ceiling),
        usage: |value| value.denominator_factors,
    },
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::CoefficientMaps,
        default: 256,
        cumulative: true,
        limit: |value, ceiling| value.coefficient_maps = ceiling,
        override_limit: |value, ceiling| value.coefficient_maps = Some(ceiling),
        usage: |value| value.coefficient_maps,
    },
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::TowerLevels,
        default: 8,
        cumulative: false,
        limit: |value, ceiling| value.tower_levels = ceiling,
        override_limit: |value, ceiling| value.tower_levels = Some(ceiling),
        usage: |value| value.tower_levels,
    },
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::TowerDegree,
        default: 256,
        cumulative: false,
        limit: |value, ceiling| value.tower_degree = ceiling,
        override_limit: |value, ceiling| value.tower_degree = Some(ceiling),
        usage: |value| value.tower_degree,
    },
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::TowerReductions,
        default: 100_000,
        cumulative: true,
        limit: |value, ceiling| value.tower_reductions = ceiling,
        override_limit: |value, ceiling| value.tower_reductions = Some(ceiling),
        usage: |value| value.tower_reductions,
    },
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::GcdSteps,
        default: 100_000,
        cumulative: true,
        limit: |value, ceiling| value.gcd_steps = ceiling,
        override_limit: |value, ceiling| value.gcd_steps = Some(ceiling),
        usage: |value| value.gcd_steps,
    },
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::ComponentSplits,
        default: 128,
        cumulative: true,
        limit: |value, ceiling| value.component_splits = ceiling,
        override_limit: |value, ceiling| value.component_splits = Some(ceiling),
        usage: |value| value.component_splits,
    },
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::OrbitPermutations,
        default: 40_320,
        cumulative: true,
        limit: |value, ceiling| value.orbit_permutations = ceiling,
        override_limit: |value, ceiling| value.orbit_permutations = Some(ceiling),
        usage: |value| value.orbit_permutations,
    },
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::OrbitMembers,
        default: 256,
        cumulative: false,
        limit: |value, ceiling| value.orbit_members = ceiling,
        override_limit: |value, ceiling| value.orbit_members = Some(ceiling),
        usage: |value| value.orbit_members,
    },
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::OrbitTerms,
        default: 262_144,
        cumulative: true,
        limit: |value, ceiling| value.orbit_terms = ceiling,
        override_limit: |value, ceiling| value.orbit_terms = Some(ceiling),
        usage: |value| value.orbit_terms,
    },
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::SymmetricSteps,
        default: 4_096,
        cumulative: true,
        limit: |value, ceiling| value.symmetric_steps = ceiling,
        override_limit: |value, ceiling| value.symmetric_steps = Some(ceiling),
        usage: |value| value.symmetric_steps,
    },
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::ResolventDepth,
        default: 8,
        cumulative: false,
        limit: |value, ceiling| value.resolvent_depth = ceiling,
        override_limit: |value, ceiling| value.resolvent_depth = Some(ceiling),
        usage: |value| value.resolvent_depth,
    },
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::ResolventDegree,
        default: 32,
        cumulative: false,
        limit: |value, ceiling| value.resolvent_degree = ceiling,
        override_limit: |value, ceiling| value.resolvent_degree = Some(ceiling),
        usage: |value| value.resolvent_degree,
    },
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::SelectionSlots,
        default: 64,
        cumulative: false,
        limit: |value, ceiling| value.selection_slots = ceiling,
        override_limit: |value, ceiling| value.selection_slots = Some(ceiling),
        usage: |value| value.selection_slots,
    },
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::SolutionBranches,
        default: 256,
        cumulative: true,
        limit: |value, ceiling| value.solution_branches = ceiling,
        override_limit: |value, ceiling| value.solution_branches = Some(ceiling),
        usage: |value| value.solution_branches,
    },
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::FamilyParameters,
        default: 64,
        cumulative: false,
        limit: |value, ceiling| value.family_parameters = ceiling,
        override_limit: |value, ceiling| value.family_parameters = Some(ceiling),
        usage: |value| value.family_parameters,
    },
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::SolveIdentities,
        default: 8_192,
        cumulative: true,
        limit: |value, ceiling| value.solve_identities = ceiling,
        override_limit: |value, ceiling| value.solve_identities = Some(ceiling),
        usage: |value| value.solve_identities,
    },
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::LinearPivots,
        default: 256,
        cumulative: true,
        limit: |value, ceiling| value.linear_pivots = ceiling,
        override_limit: |value, ceiling| value.linear_pivots = Some(ceiling),
        usage: |value| value.linear_pivots,
    },
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::IsolationSteps,
        default: 100_000,
        cumulative: true,
        limit: |value, ceiling| value.isolation_steps = ceiling,
        override_limit: |value, ceiling| value.isolation_steps = Some(ceiling),
        usage: |value| value.isolation_steps,
    },
    LimitCase {
        kind: nrz_cas::limits::BudgetKind::LocalBranches,
        default: 64,
        cumulative: true,
        limit: |value, ceiling| value.local_branches = ceiling,
        override_limit: |value, ceiling| value.local_branches = Some(ceiling),
        usage: |value| value.local_branches,
    },
];

/// Canonical parent constants allow monic extensions at every earlier quotient level.
/// ´test:integration:solve-monic-nested-towers´
#[test]
fn solve_monic_nested_towers() -> TestResult {
    use nrz_cas::solving::{
        CoefficientValueData, CoefficientValueObject, ExtensionLevelData, PolynomialData,
        PolynomialTerm, SquareFreeRequest,
    };
    use nrz_cas::syntax::AtomicSort;
    let mut cas = nrz_cas::Cas::new(Limits::default())?;
    let mut domain = admit(
        &mut cas,
        &Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Rational,
        )),
    )?;
    let mut one = admit(
        &mut cas,
        &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
            domain: Reference::Existing(domain.clone()),
            value: CoefficientValueData::Rational {
                value: nrz_cas::Rational::one(),
            },
        })),
    )?;
    for _ in 0..2 {
        let polynomial = admit(
            &mut cas,
            &Draft::Solve(SolveDraft::Polynomial(PolynomialData {
                domain: Reference::Existing(domain.clone()),
                variables: vec![Field::Rational],
                terms: vec![PolynomialTerm {
                    exponents: vec![1],
                    coefficient: Reference::Existing(one.clone()),
                }],
            })),
        )?;
        let level = admit(
            &mut cas,
            &Draft::Solve(SolveDraft::ExtensionLevel(ExtensionLevelData {
                parent: Reference::Existing(domain),
                polynomial: Reference::Existing(polynomial),
                generator: vec![AtomicSort::Scalar(Field::Rational)],
                square_free: SquareFreeRequest::NotRequested,
            })),
        )?;
        domain = admit(
            &mut cas,
            &Draft::Solve(SolveDraft::CoefficientDomain(
                CoefficientDomainData::Quotient {
                    level: Reference::Existing(level),
                },
            )),
        )?;
        one = admit(
            &mut cas,
            &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
                domain: Reference::Existing(domain.clone()),
                value: CoefficientValueData::Tower {
                    coefficients: vec![Reference::Existing(one)],
                },
            })),
        )?;
    }
    assert!(matches!(
        cas.solve_view(&one)?,
        SolveView::CoefficientValue(_)
    ));
    Ok(())
}

/// A finite solution proposal cannot leave an original unknown as a free family parameter.
/// ´test:integration:solve-finite-branch-capture-refusal´
#[test]
fn solve_finite_branch_capture_refusal() -> TestResult {
    use nrz_cas::solving::{SolutionBranchData, SolutionBranchKind};
    use nrz_cas::syntax::{AtomicSort, Operator, Sort};
    let mut fixture = Fixture::new()?;
    let SolveView::SolveProblem(problem) = fixture.cas.solve_view(&fixture.problem)? else {
        return Err("problem missing".into());
    };
    let coordinates = Reference::Existing(problem.unknowns.clone());
    let region = Reference::Existing(problem.region.clone());
    let domain = Reference::Existing(problem.domain.clone());
    let entry = admit(
        &mut fixture.cas,
        &crate::testing::solve_graph::expression(
            Reference::Existing(fixture.header.clone()),
            Operator::Free("x".into()),
            Sort::Value(AtomicSort::Scalar(Field::Real)),
            Vec::new(),
        ),
    )?;
    let tuple = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::Tuple(TupleData {
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            coordinates,
            entries: vec![Reference::Existing(entry)],
        })),
    )?;
    let branch = Draft::Solve(SolveDraft::SolutionBranch(SolutionBranchData {
        problem: Reference::Existing(fixture.problem.clone()),
        region,
        domain,
        kind: SolutionBranchKind::Finite {
            tuples: vec![Reference::Existing(tuple)],
        },
    }));
    let before = fixture.cas.counts();
    assert!(
        fixture
            .cas
            .admit(
                std::slice::from_ref(&branch),
                &mut Budget::new(Limits::default())?
            )
            .is_err()
    );
    assert_eq!(fixture.cas.counts(), before);
    Ok(())
}
