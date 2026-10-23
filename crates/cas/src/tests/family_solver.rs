// SPDX-License-Identifier: AGPL-3.0-only
//! Original mixed source requests with independent replay and retained declarations.

mod closure;
mod cover;
mod empty;
mod lc;
mod placement;
mod properties;

use crate::Cas;
use crate::domain::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
use crate::limits::{Budget, Limits, LimitsOverride};
use crate::request::{CoveredInput, Outcome, Request, Task};
use crate::solving::{
    CoefficientDomainData, CoordinateRole, ExclusionPolicy, FamilyArgumentRole, SolveDraft,
    SolveForm, SolveGoal, SolveProblemData, SolveSpec,
};
use crate::syntax::{AtomicSort, Draft, ExpressionDraft, Field, Operator, PredicateKind, Sort};
use crate::testing::common::TestResult;
use crate::testing::sorts::{Fixture, admit, expression, reference};

/// Construct the original mixed equation without selecting parameter values.
fn request(degree: i64) -> TestResult<(Fixture, Request)> {
    let mut fixture = Fixture::with_limits(Limits::blind_cubic())?;
    let (family, left, zero) = equation(&mut fixture, degree)?;
    let (problem, context) = problem(&mut fixture, &family, &left, &zero)?;
    let input = source(&mut fixture, &left, &zero, degree)?;
    Ok((
        fixture,
        Request {
            task: Task::Solve(Box::new(SolveSpec {
                problem,
                goal: SolveGoal::Complete,
                form: if degree == 4 {
                    SolveForm::AlgebraicRoots
                } else {
                    SolveForm::Radicals
                },
                exclusions: ExclusionPolicy::PreserveOriginal,
            })),
            context,
            inputs: vec![input],
            premises: Vec::new(),
            limits: profile(),
        },
    ))
}

/// The unknown keeps its declared Complex sort; the base operand retains its Real sort.
fn equation(
    fixture: &mut Fixture,
    degree: i64,
) -> TestResult<(crate::testing::sorts::Family, crate::Handle, crate::Handle)> {
    let family = fixture.family()?;
    let power = build_power(fixture, degree)?;
    let mut data = fixture.body_data(&family);
    data.operator = Operator::Sub;
    data.arguments.swap(0, 1);
    data.arguments[0].value = reference(&power);
    data.arguments[0].role = FamilyArgumentRole::Fibre;
    if degree == 5 {
        let zero = expression(
            &mut fixture.cas,
            &fixture.header,
            Field::Real,
            Operator::Rational(crate::Rational::zero()),
            Vec::new(),
            Vec::new(),
        )?;
        data.arguments[1].value = reference(&zero);
    }
    let left = fixture
        .cas
        .family_expression(&data, &mut Budget::new(Limits::blind_cubic())?)?;
    let zero = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Complex,
        Operator::Rational(crate::Rational::zero()),
        Vec::new(),
        Vec::new(),
    )?;
    Ok((family, left, zero))
}

/// Build only the ordinary Complex side before the explicit local placement is added.
fn build_power(fixture: &mut Fixture, degree: i64) -> TestResult<crate::Handle> {
    let unknown = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Complex,
        Operator::Free("z".into()),
        Vec::new(),
        Vec::new(),
    )?;
    let power = if degree == 5 {
        expression(
            &mut fixture.cas,
            &fixture.header,
            Field::Complex,
            Operator::Rational(crate::Rational::one()),
            Vec::new(),
            Vec::new(),
        )?
    } else {
        expression(
            &mut fixture.cas,
            &fixture.header,
            Field::Complex,
            Operator::IntegerPower(degree.min(3)),
            Vec::new(),
            vec![reference(&unknown)],
        )?
    };
    let power = if degree == 4 {
        let linear = expression(
            &mut fixture.cas,
            &fixture.header,
            Field::Complex,
            Operator::Mul,
            Vec::new(),
            vec![reference(&fixture.complex), reference(&unknown)],
        )?;
        expression(
            &mut fixture.cas,
            &fixture.header,
            Field::Complex,
            Operator::Add,
            Vec::new(),
            vec![reference(&power), reference(&linear)],
        )?
    } else {
        power
    };
    Ok(power)
}

/// The problem retains the total original coefficient frame and the independent unknown space.
fn problem(
    fixture: &mut Fixture,
    family: &crate::testing::sorts::Family,
    left: &crate::Handle,
    zero: &crate::Handle,
) -> TestResult<(crate::Handle, crate::Handle)> {
    let unknowns = fixture.coordinates(&[("z", Field::Complex, CoordinateRole::Unknown)])?;
    let condition = admit(
        &mut fixture.cas,
        Draft::Condition {
            header: reference(&fixture.header),
            scope: Vec::new(),
            kind: PredicateKind::Equal,
            arguments: vec![reference(left), reference(zero)],
        },
    )?;
    let ground = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Rational,
        )),
    )?;
    let algebra = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Fraction {
                base: reference(&ground),
                coordinates: reference(&family.total),
            },
        )),
    )?;
    let domain = admit(
        &mut fixture.cas,
        Draft::Region(RegionDraft {
            header: reference(&fixture.header),
            scope: Vec::new(),
            ambient: AmbientDomain::Product {
                coordinates: unknowns.clone(),
            },
            expression: RegionExpr::Ambient,
        }),
    )?;
    let problem = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::SolveProblem(SolveProblemData {
            header: reference(&fixture.header),
            scope: Vec::new(),
            ambient: Field::Complex,
            coefficient_domain: reference(&algebra),
            coefficients: reference(&family.total),
            unknowns: reference(&unknowns),
            predicate: reference(&condition),
            region: reference(&family.region),
            domain: reference(&domain),
            denominators: Vec::new(),
        })),
    )?;
    let context = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Root {
            header: reference(&fixture.header),
            admissible: reference(&family.region),
            assumptions: Vec::new(),
        }),
    )?;
    Ok((problem, context))
}

/// The submitted original occurrence is walked before any solve proposal exists.
fn source(
    fixture: &mut Fixture,
    left: &crate::Handle,
    zero: &crate::Handle,
    degree: i64,
) -> TestResult<CoveredInput> {
    let boolean = admit(
        &mut fixture.cas,
        Draft::Expression(ExpressionDraft {
            header: reference(&fixture.header),
            scope: Vec::new(),
            operator: Operator::Predicate(PredicateKind::Equal),
            sort: Sort::Value(AtomicSort::Boolean),
            operands: vec![reference(left), reference(zero)],
            space: None,
            binding: None,
        }),
    )?;
    let text = match degree {
        4 => "z^3+c*z-r=0".into(),
        5 => "1-placed(0)=0".into(),
        _ => format!("z^{degree}-r=0"),
    };
    let source = admit(
        &mut fixture.cas,
        Draft::Source {
            key: "mixed-original".into(),
            text,
        },
    )?;
    let root = fixture.cas.materialize_original(
        &boolean,
        &source,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let receipt = fixture
        .cas
        .walk_original(&root, &mut Budget::new(Limits::blind_cubic())?)?
        .receipt;
    Ok(CoveredInput { root, receipt })
}

/// The mixed request uses exactly the unchanged cubic acceptance ceilings.
fn profile() -> LimitsOverride {
    let limits = Limits::blind_cubic();
    LimitsOverride {
        arithmetic_steps: Some(limits.arithmetic_steps),
        work: Some(limits.work),
        replay_work: Some(limits.replay_work),
        component_bits: Some(limits.component_bits),
        intermediate_bits: Some(limits.intermediate_bits),
        coefficient_ops: Some(limits.coefficient_ops),
        gcd_steps: Some(limits.gcd_steps),
        tower_reductions: Some(limits.tower_reductions),
        orbit_permutations: Some(limits.orbit_permutations),
        orbit_terms: Some(limits.orbit_terms),
        symmetric_steps: Some(limits.symmetric_steps),
        tower_levels: Some(limits.tower_levels),
        tower_degree: Some(limits.tower_degree),
        orbit_members: Some(limits.orbit_members),
        resolvent_depth: Some(limits.resolvent_depth),
        resolvent_degree: Some(limits.resolvent_degree),
        selection_slots: Some(limits.selection_slots),
        solution_branches: Some(limits.solution_branches),
        witness_bytes: Some(limits.witness_bytes),
        output_bytes: Some(limits.output_bytes),
        condition_atoms: Some(limits.condition_atoms),
        condition_steps: Some(limits.condition_steps),
        ..LimitsOverride::default()
    }
}

/// A complete local linear family is independently replayable without a cover of its base.
/// ´test:crate:mixed-linear-piece-replays´
#[test]
fn mixed_linear_piece_replays() -> TestResult {
    replay(1)
}

/// Every returned complete piece must survive independent import.
fn replay(degree: i64) -> TestResult {
    let (mut fixture, request) = request(degree)?;
    let outcome = fixture.cas.run(&request);
    let Outcome::Solved { answer, .. } = outcome else {
        return Err(format!("mixed solve: {outcome:?}").into());
    };
    let coverage = answer.coverage.ok_or("missing mixed coverage")?;
    if degree == 4 {
        crate::testing::solve_blind::family_cubic(&fixture.cas, &coverage)?;
    }
    let bytes = fixture
        .cas
        .export_witness(&coverage, &mut Budget::new(Limits::blind_cubic())?)?;
    assert!(
        Cas::new(Limits::blind_cubic())?
            .inspect(&bytes, &mut Budget::new(Limits::blind_cubic())?)?
            .is_discharged()
    );
    Ok(())
}

/// A mixed binomial retains its Real radicand through Complex root selection.
/// ´test:crate:mixed-quadratic-piece-replays´
#[test]
fn mixed_quadratic_piece_replays() -> TestResult {
    replay(2)
}

/// A mixed cubic binomial retains all three factor occurrences.
/// ´test:crate:mixed-cubic-piece-replays´
#[test]
fn mixed_cubic_piece_replays() -> TestResult {
    replay(3)
}

/// Exact algebraic selections completely solve a generic mixed cubic without claiming radicals.
/// ´test:crate:mixed-general-cubic-piece-replays´
#[test]
fn mixed_general_cubic_piece_replays() -> TestResult {
    replay(4)
}

/// Restrict the same original problem to a Real sign stratum without specializing its parameter.
fn stratum(fixture: &mut Fixture, request: &mut Request, sign: i8) -> TestResult {
    let Task::Solve(specification) = &request.task else {
        return Err("mixed request".into());
    };
    let original =
        crate::solving::graph::problem(&fixture.cas.state, specification.problem.logical_id())?
            .clone();
    let region = fixture
        .cas
        .state
        .region(original.region.logical_id())?
        .clone();
    let over = region.lies_over.as_ref().ok_or("family projection")?;
    let family = fixture.cas.handle(over.family);
    let base = fixture.cas.handle(over.stratum);
    let base_region = fixture.cas.state.region(base.logical_id())?.clone();
    let subject = if sign < 0 {
        expression(
            &mut fixture.cas,
            &fixture.header,
            Field::Real,
            Operator::Neg,
            Vec::new(),
            vec![reference(&fixture.real)],
        )?
    } else {
        fixture.real.clone()
    };
    let zero = expression(
        &mut fixture.cas,
        &fixture.header,
        Field::Real,
        Operator::Rational(crate::Rational::zero()),
        Vec::new(),
        Vec::new(),
    )?;
    let predicate = admit(
        &mut fixture.cas,
        Draft::Condition {
            header: reference(&fixture.header),
            scope: Vec::new(),
            kind: if sign == 0 {
                PredicateKind::Equal
            } else {
                PredicateKind::Positive
            },
            arguments: if sign == 0 {
                vec![reference(&subject), reference(&zero)]
            } else {
                vec![reference(&subject)]
            },
        },
    )?;
    let stratum = admit(
        &mut fixture.cas,
        Draft::Region(RegionDraft {
            header: reference(&fixture.header),
            scope: Vec::new(),
            ambient: base_region.ambient,
            expression: RegionExpr::GuardTrue {
                base: reference(&base),
                predicate: reference(&predicate),
            },
        }),
    )?;
    let total = admit(
        &mut fixture.cas,
        Draft::FamilyRegion {
            region: RegionDraft {
                header: reference(&fixture.header),
                scope: Vec::new(),
                ambient: region.ambient,
                expression: RegionExpr::GuardTrue {
                    base: reference(&original.region),
                    predicate: reference(&predicate),
                },
            },
            lies_over: crate::solving::LiesOver {
                family: reference(&family),
                stratum: reference(&stratum),
            },
        },
    )?;
    request.context = admit(
        &mut fixture.cas,
        Draft::Context(ContextDraft::Local {
            parent: reference(&request.context),
            region: reference(&total),
            hypotheses: Vec::new(),
        }),
    )?;
    Ok(())
}

/// All three Real strata give independently closed complete quadratic pieces.
/// ´test:crate:mixed-square-root-sign-pieces´
#[test]
fn mixed_square_root_sign_pieces() -> TestResult {
    for sign in [-1, 0, 1] {
        let (mut fixture, mut request) = request(2)?;
        stratum(&mut fixture, &mut request, sign)?;
        let outcome = fixture.cas.run(&request);
        let Outcome::Solved { answer, .. } = outcome else {
            return Err(format!("sign {sign}: {outcome:?}").into());
        };
        let coverage = answer.coverage.ok_or("missing stratum coverage")?;
        crate::testing::solve_blind::family_quadratic(&fixture.cas, &coverage, sign)?;
        let bytes = fixture
            .cas
            .export_witness(&coverage, &mut Budget::new(Limits::blind_cubic())?)?;
        assert!(
            Cas::new(Limits::blind_cubic())?
                .inspect(&bytes, &mut Budget::new(Limits::blind_cubic())?)?
                .is_discharged()
        );
    }
    Ok(())
}
