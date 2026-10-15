// SPDX-License-Identifier: AGPL-3.0-only
//! Public cross-header restriction fixtures, independent of the checker implementation.

use super::{Fixture, admit};
use crate::test_api::proof::{ApplicationDraft, Statement};
use crate::test_api::solving::{
    CoefficientMapData, MapMode, MapTarget, RealRestrictionTransport, RestrictionPair, SolveDraft,
    SolveView,
};
use crate::test_api::syntax::{AtomicSort, Draft, ExpressionDraft, Operator, Sort};
use crate::test_api::{Handle, Reference};
use crate::testing::common::TestResult;

/// Declare each transport endpoint under its own header and product coordinate identity.
fn declarations(
    base: &mut Fixture,
    real: &Handle,
    complex: &Handle,
) -> TestResult<Vec<RestrictionPair>> {
    let SolveView::CoordinateSpace(left) = base.cas.solve_view(real)? else {
        return Err("missing real coordinates".into());
    };
    let left = left.clone();
    let SolveView::CoordinateSpace(right) = base.cas.solve_view(complex)? else {
        return Err("missing complex coordinates".into());
    };
    let right = right.clone();
    let mut pairs = Vec::new();
    for (a, b) in left.coordinates.iter().zip(&right.coordinates) {
        let mut endpoints = Vec::new();
        for (header, coordinate) in [(&left.header, a), (&right.header, b)] {
            endpoints.push(admit(
                &mut base.cas,
                &Draft::Expression(ExpressionDraft {
                    header: Reference::Existing(header.clone()),
                    scope: Vec::new(),
                    operator: Operator::Free(coordinate.key.clone()),
                    sort: Sort::Value(AtomicSort::Scalar(coordinate.field.clone())),
                    operands: Vec::new(),
                    space: None,
                    binding: None,
                }),
            )?);
        }
        pairs.push(RestrictionPair {
            real: endpoints.first().ok_or("missing real expression")?.clone(),
            complex: endpoints
                .get(1)
                .ok_or("missing complex expression")?
                .clone(),
        });
    }
    Ok(pairs)
}

/// A directed map and explicit covers are mandatory even for ground scalar problems.
pub(crate) fn transport(
    base: &mut Fixture,
    real: &Handle,
    complex: &Handle,
    real_cover: &ApplicationDraft,
    complex_cover: &ApplicationDraft,
) -> TestResult<RealRestrictionTransport> {
    let SolveView::SolveProblem(left) = base.cas.solve_view(real)? else {
        return Err("missing real problem".into());
    };
    let left = left.clone();
    let SolveView::SolveProblem(right) = base.cas.solve_view(complex)? else {
        return Err("missing complex problem".into());
    };
    let right = right.clone();
    let Statement::Cover {
        domain: a,
        members: first,
    } = &real_cover.conclusion
    else {
        return Err("missing real cover".into());
    };
    let Statement::Cover {
        domain: b,
        members: second,
    } = &complex_cover.conclusion
    else {
        return Err("missing complex cover".into());
    };
    let coefficient_map = admit(
        &mut base.cas,
        &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
            source: Reference::Existing(left.coefficient_domain.clone()),
            target: MapTarget::CoefficientDomain {
                domain: Reference::Existing(right.coefficient_domain.clone()),
            },
            mode: MapMode::Embedding,
            region: Reference::Existing(a.clone()),
            images: Vec::new(),
            denominators: Vec::new(),
        })),
    )?;
    let mut regions = vec![RestrictionPair {
        real: a.clone(),
        complex: b.clone(),
    }];
    regions.extend(first.iter().zip(second).map(|(a, b)| RestrictionPair {
        real: a.clone(),
        complex: b.clone(),
    }));
    Ok(RealRestrictionTransport {
        coefficient_map,
        coefficients: declarations(base, &left.coefficients, &right.coefficients)?,
        unknowns: declarations(base, &left.unknowns, &right.unknowns)?,
        regions,
        conditions: Vec::new(),
        real_cover: 2,
    })
}

/// Rebuild declaration spelling independently while keeping explicit coordinate positions.
fn enter(base: &mut Fixture, field: &crate::test_api::syntax::Field, keys: &[&str]) -> TestResult {
    use crate::test_api::domain::{ContextDraft, HeaderDraft};
    use crate::test_api::syntax::{Branch, Declaration, Environment, Field};
    base.field.clone_from(field);
    let mut declarations: Vec<_> = keys
        .iter()
        .map(|key| Declaration {
            key: (*key).into(),
            display_name: "same coefficient".into(),
            sort: Sort::Value(AtomicSort::Scalar(field.clone())),
        })
        .collect();
    declarations.push(Declaration {
        key: "x".into(),
        display_name: "unknown".into(),
        sort: Sort::Value(AtomicSort::Scalar(field.clone())),
    });
    declarations.push(Declaration {
        key: "t".into(),
        display_name: "family".into(),
        sort: Sort::Value(AtomicSort::Scalar(field.clone())),
    });
    base.header = admit(
        &mut base.cas,
        &Draft::Header(HeaderDraft::Core(Environment {
            field: field.clone(),
            branch: if *field == Field::Complex {
                Branch::PrincipalComplex
            } else {
                Branch::StrictReal
            },
            declarations,
        })),
    )?;
    base.coefficients = super::coordinate_product(
        &mut base.cas,
        &base.header,
        keys,
        crate::test_api::solving::CoordinateRole::Coefficient,
        field,
    )?;
    base.region = super::product(&mut base.cas, &base.header, &base.coefficients, Vec::new())?;
    base.context = admit(
        &mut base.cas,
        &Draft::Context(ContextDraft::Root {
            header: Reference::Existing(base.header.clone()),
            admissible: Reference::Existing(base.region.clone()),
            assumptions: Vec::new(),
        }),
    )?;
    Ok(())
}

/// Independently supplied factorization and its original problem.
struct Product {
    problem: Handle,
    set: Handle,
    factors: Vec<crate::test_api::solving::RootFactor<Handle>>,
    scalar: Handle,
    expression: Handle,
}

/// Exact product fixtures cover distinct, repeated, triple, linear and constant strata.
fn factored_problem(base: &mut Fixture, keys: &[&str], shape: u8) -> TestResult<Product> {
    let scalar = base.free(keys.first().ok_or("missing coefficient")?)?;
    let first = base.free(keys.get(1).ok_or("missing first root")?)?;
    let second = base.free(keys.get(2).ok_or("missing second root")?)?;
    let negative = base.expression(Operator::Neg, std::slice::from_ref(&first), false)?;
    let factors = match shape {
        0 => vec![(first, 1), (second, 1)],
        1 => vec![(first, 1), (second, 1), (negative, 1)],
        2 => vec![(first, 2), (second, 1)],
        3 => vec![(first, 3)],
        4 => vec![(first, 1)],
        5 => Vec::new(),
        _ => return Err("unknown product shape".into()),
    };
    let unknown = base.free("x")?;
    let mut polynomial = scalar.clone();
    let mut roots = Vec::new();
    let mut entries = Vec::new();
    for (root, multiplicity) in factors {
        let factor = base.expression(Operator::Sub, &[unknown.clone(), root.clone()], false)?;
        let power = if multiplicity == 1 {
            factor
        } else {
            base.expression(
                Operator::IntegerPower(i64::from(multiplicity)),
                &[factor],
                false,
            )?
        };
        polynomial = base.expression(Operator::Mul, &[polynomial, power], false)?;
        entries.push(root.clone());
        roots.push(crate::test_api::solving::RootFactor { root, multiplicity });
    }
    let zero = base.integer(0, false)?;
    let predicate = base.equation(&polynomial, &zero)?;
    let problem = base.problem("x", &predicate, &[])?;
    let set = base.finite(&problem, &entries)?;
    Ok(Product {
        problem,
        set,
        factors: roots,
        scalar,
        expression: polynomial,
    })
}

/// One guard remains visible in both problem headers and both coefficient covers.
fn guard(base: &mut Fixture, key: &str) -> TestResult {
    use crate::test_api::domain::{AmbientDomain, RegionDraft, RegionExpr};
    let value = base.free(key)?;
    let zero = base.integer(0, false)?;
    let predicate = base.equation(&value, &zero)?;
    let region = admit(
        &mut base.cas,
        &Draft::Region(RegionDraft {
            header: Reference::Existing(base.header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Product {
                coordinates: base.coefficients.clone(),
            },
            expression: RegionExpr::GuardFalse {
                base: Reference::Existing(base.region.clone()),
                predicate: Reference::Existing(predicate),
            },
        }),
    )?;
    base.enter_region(region)
}

/// Source proof and explicit complex cover, before the real header exists.
struct Source {
    fixture: super::BinomialFixture,
    problem: Handle,
    set: Handle,
    cover: ApplicationDraft,
}

/// Checkable supplied factorization uses a Gaussian domain and symbolic guarded coefficients.
fn source(shape: u8) -> TestResult<Source> {
    use crate::test_api::solving::{
        CertificateProductCompleteness, CoefficientDomainData, SolveCertificate, SolveSchema,
        SolveStatement, StatementCoverage,
    };
    use crate::test_api::syntax::Field;
    let mut base = Fixture::in_field(&Field::Complex)?;
    enter(&mut base, &Field::Complex, &["a", "b", "c"])?;
    base.algebra = admit(
        &mut base.cas,
        &Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Gaussian,
        )),
    )?;
    guard(&mut base, "a")?;
    let product = factored_problem(&mut base, &["a", "b", "c"], shape)?;
    let algebra = base.algebra.clone();
    let map = base.scalar_map(&algebra, &[])?;
    let identity = base.identity(&product.expression, &product.expression, &map);
    let mut complete = base.step(
        SolveSchema::ProductCompleteness,
        SolveStatement::Coverage(StatementCoverage {
            problem: product.problem.clone(),
            set: product.set.clone(),
            region: base.region.clone(),
        }),
        SolveCertificate::ProductCompleteness(CertificateProductCompleteness {
            problem: product.problem.clone(),
            set: product.set.clone(),
            scalar: product.scalar,
            factors: product.factors,
            product: 0,
            existence: Vec::new(),
            region: base.region.clone(),
        }),
    );
    complete.premises = vec![0];
    let zero = base.integer(0, false)?;
    let cover = base.split(&zero)?;
    Ok(Source {
        fixture: super::BinomialFixture {
            base,
            steps: vec![identity, complete, cover.clone()],
        },
        problem: product.problem,
        set: product.set,
        cover,
    })
}

/// Symbolic quadratic and cubic terminal shapes have no ground-only shortcut.
pub(crate) fn symbolic_restriction(shape: u8) -> TestResult<super::BinomialFixture> {
    use crate::test_api::solving::{
        CertificateRealRestriction, CoefficientDomainData, SolveCertificate, SolveSchema,
        SolveStatement, StatementCoverage,
    };
    use crate::test_api::syntax::Field;
    let Source {
        mut fixture,
        problem: complex_problem,
        set: complex_set,
        cover: complex_cover,
    } = source(shape)?;
    let base = &mut fixture.base;
    enter(base, &Field::Real, &["p", "q", "r"])?;
    base.algebra = admit(
        &mut base.cas,
        &Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Rational,
        )),
    )?;
    guard(base, "p")?;
    let product = factored_problem(base, &["p", "q", "r"], shape)?;
    let zero = base.integer(0, false)?;
    let real_cover = base.split(&zero)?;
    let correspondence = transport(
        base,
        &product.problem,
        &complex_problem,
        &real_cover,
        &complex_cover,
    )?;
    fixture.steps.push(real_cover);
    let SolveView::SolutionSet(set) = base.cas.solve_view(&complex_set)? else {
        return Err("missing source set".into());
    };
    let retained = set.branches.clone();
    let mut restricted = base.step(
        SolveSchema::RealRestriction,
        SolveStatement::Coverage(StatementCoverage {
            problem: product.problem.clone(),
            set: product.set.clone(),
            region: base.region.clone(),
        }),
        SolveCertificate::RealRestriction(CertificateRealRestriction {
            coverage: 0,
            problem: product.problem,
            set: product.set,
            transport: correspondence,
            retained,
            rejected: Vec::new(),
            classification: Vec::new(),
            cover: 1,
        }),
    );
    restricted.premises = vec![1, 2, 3];
    fixture.steps.push(restricted);
    Ok(fixture)
}

/// Construct one exact regional assumption and its optional checked hypothesis application.
fn assumption(
    base: &mut Fixture,
    header: &Handle,
    parameter: &Handle,
    region: &Handle,
    assume: bool,
) -> TestResult<(Handle, Handle, Option<ApplicationDraft>)> {
    use crate::test_api::domain::{AssumptionDraft, ContextDraft, HypothesisRef};
    use crate::test_api::proof::{Certificate, Primitive};
    let predicate = admit(
        &mut base.cas,
        &Draft::Condition {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            kind: crate::test_api::syntax::PredicateKind::NonZero,
            arguments: vec![Reference::Existing(parameter.clone())],
        },
    )?;
    let assumptions = if assume {
        vec![AssumptionDraft {
            key: "parameter-nonzero".into(),
            condition: Reference::Existing(predicate.clone()),
        }]
    } else {
        Vec::new()
    };
    let context = admit(
        &mut base.cas,
        &Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header.clone()),
            admissible: Reference::Existing(region.clone()),
            assumptions,
        }),
    )?;
    let fact = assume.then(|| ApplicationDraft {
        rule: Primitive::ContextHypothesis,
        context: context.clone(),
        conclusion: Statement::PredicateFact {
            region: region.clone(),
            predicate: predicate.clone(),
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Hypothesis(HypothesisRef {
            context: context.clone(),
            slot: 0,
        }),
    });
    Ok((context, predicate, fact))
}

/// A source assumption is transported to a real assumption or retained as a real residual.
pub(crate) fn assumed_restriction(discharge: bool) -> TestResult<super::BinomialFixture> {
    use crate::test_api::proof::Certificate;
    use crate::test_api::solving::SolveCertificate;
    let mut fixture = symbolic_restriction(0)?;
    let Certificate::Solve(last) = &fixture
        .steps
        .last()
        .ok_or("missing restriction")?
        .certificate
    else {
        return Err("wrong certificate".into());
    };
    let SolveCertificate::RealRestriction(last) = last.as_ref() else {
        return Err("wrong restriction".into());
    };
    let parameters = last
        .transport
        .coefficients
        .get(1)
        .ok_or("missing parameter")?
        .clone();
    let regions = last
        .transport
        .regions
        .first()
        .ok_or("missing regions")?
        .clone();
    let original = coverage(fixture.steps.get(1).ok_or("missing coverage")?)?;
    let SolveView::SolveProblem(problem) = fixture.base.cas.solve_view(&original.problem)? else {
        return Err("missing problem".into());
    };
    let complex_header = problem.header.clone();
    let real_header = fixture.base.header.clone();
    let mut facts = Vec::new();
    let mut contexts = Vec::new();
    let mut predicates = Vec::new();
    for (header, parameter, region, assume) in [
        (&complex_header, &parameters.complex, &regions.complex, true),
        (&real_header, &parameters.real, &regions.real, discharge),
    ] {
        let (context, predicate, fact) =
            assumption(&mut fixture.base, header, parameter, region, assume)?;
        facts.extend(fact);
        contexts.push(context);
        predicates.push(predicate);
    }
    install_assumptions(
        &mut fixture,
        facts,
        &contexts,
        &predicates,
        &regions,
        discharge,
    )?;
    Ok(fixture)
}

/// Every proof reference is relocated explicitly when the qualified assumptions are inserted.
fn install_assumptions(
    fixture: &mut super::BinomialFixture,
    mut facts: Vec<ApplicationDraft>,
    contexts: &[Handle],
    predicates: &[Handle],
    regions: &RestrictionPair,
    discharge: bool,
) -> TestResult {
    use crate::test_api::proof::Certificate;
    use crate::test_api::solving::{
        RestrictionCondition, RestrictionConditionKind, SolveCertificate,
    };
    let source = contexts.first().ok_or("missing source context")?;
    let target = contexts.get(1).ok_or("missing target context")?;
    let offset = u32::try_from(facts.len())?;
    for (index, step) in fixture.steps.iter_mut().enumerate() {
        step.context
            .clone_from(if index < 3 { source } else { target });
        for slot in &mut step.premises {
            *slot = slot.checked_add(offset).ok_or("proof index overflow")?;
        }
    }
    fixture
        .steps
        .first_mut()
        .ok_or("missing identity")?
        .premises
        .push(0);
    let last = fixture.steps.last_mut().ok_or("missing restriction")?;
    if discharge {
        last.premises.push(1);
    }
    let Certificate::Solve(certificate) = &mut last.certificate else {
        return Err("wrong certificate".into());
    };
    let SolveCertificate::RealRestriction(certificate) = certificate.as_mut() else {
        return Err("wrong restriction".into());
    };
    certificate.transport.conditions.push(RestrictionCondition {
        premise: 0,
        kind: RestrictionConditionKind::Hypothesis,
        index: 0,
        contexts: RestrictionPair {
            real: target.clone(),
            complex: source.clone(),
        },
        regions: regions.clone(),
        predicates: RestrictionPair {
            real: predicates.get(1).ok_or("missing target predicate")?.clone(),
            complex: predicates
                .first()
                .ok_or("missing source predicate")?
                .clone(),
        },
        discharge: if discharge { Some(3) } else { None },
    });
    facts.append(&mut fixture.steps);
    fixture.steps = facts;
    fixture.base.context.clone_from(target);
    Ok(())
}

/// Borrow a supplied coverage claim without consulting the checker under test.
fn coverage(step: &ApplicationDraft) -> TestResult<&crate::test_api::solving::StatementCoverage> {
    let Statement::Solve(statement) = &step.conclusion else {
        return Err("missing solve statement".into());
    };
    let crate::test_api::solving::SolveStatement::Coverage(value) = statement.as_ref() else {
        return Err("missing coverage".into());
    };
    Ok(value)
}

/// Add the actual closed branch regions as explicit transport pairs.
fn branch_regions(
    base: &Fixture,
    transport: &mut RealRestrictionTransport,
    real: &Handle,
    complex: &Handle,
) -> TestResult {
    let SolveView::SolutionSet(left) = base.cas.solve_view(real)? else {
        return Err("missing real set".into());
    };
    let SolveView::SolutionSet(right) = base.cas.solve_view(complex)? else {
        return Err("missing complex set".into());
    };
    for (a, b) in left.branches.iter().zip(&right.branches) {
        let SolveView::SolutionBranch(a) = base.cas.solve_view(a)? else {
            return Err("missing real branch".into());
        };
        let SolveView::SolutionBranch(b) = base.cas.solve_view(b)? else {
            return Err("missing complex branch".into());
        };
        let pair = RestrictionPair {
            real: a.region.clone(),
            complex: b.region.clone(),
        };
        if !transport.regions.contains(&pair) {
            transport.regions.push(pair);
        }
    }
    Ok(())
}

/// The zero stratum keeps every real value, while the nonzero stratum keeps only zero.
pub(crate) fn degree_restriction() -> TestResult<super::BinomialFixture> {
    use crate::test_api::solving::{
        CertificateRealRestriction, SolveCertificate, SolveSchema, SolveStatement,
    };
    use crate::test_api::syntax::Field;
    let source = super::degree_drop_into(Fixture::in_field(&Field::Complex)?)?;
    let source_claim = coverage(source.steps.last().ok_or("missing source")?)?.clone();
    let source_cover = source.steps.first().ok_or("missing source cover")?.clone();
    let source_index = u32::try_from(source.steps.len().checked_sub(1).ok_or("missing source")?)?;
    let super::BinomialFixture {
        mut base,
        mut steps,
    } = source;
    base.enter_real()?;
    let target = super::degree_drop_into(base)?;
    let target_claim = coverage(target.steps.last().ok_or("missing target")?)?.clone();
    let target_cover = target.steps.first().ok_or("missing target cover")?.clone();
    let mut base = target.base;
    let mut correspondence = transport(
        &mut base,
        &target_claim.problem,
        &source_claim.problem,
        &target_cover,
        &source_cover,
    )?;
    branch_regions(
        &base,
        &mut correspondence,
        &target_claim.set,
        &source_claim.set,
    )?;
    let target_index = u32::try_from(steps.len())?;
    steps.push(target_cover);
    let SolveView::SolutionSet(set) = base.cas.solve_view(&source_claim.set)? else {
        return Err("missing complex set".into());
    };
    let retained = set.branches.clone();
    let mut restricted = base.step(
        SolveSchema::RealRestriction,
        SolveStatement::Coverage(target_claim.clone()),
        SolveCertificate::RealRestriction(CertificateRealRestriction {
            coverage: 0,
            problem: target_claim.problem,
            set: target_claim.set,
            transport: correspondence,
            retained,
            rejected: Vec::new(),
            classification: Vec::new(),
            cover: 1,
        }),
    );
    restricted.premises = vec![source_index, 0, target_index];
    steps.push(restricted);
    Ok(super::BinomialFixture { base, steps })
}

/// A symbolic real linear factor times a strictly positive quadratic has one real root.
fn mixed_problem(base: &mut Fixture, quadratic: &Handle) -> TestResult<(Handle, Handle, Handle)> {
    let unknown = base.free("x")?;
    let parameter = base.free("b")?;
    let factor = base.expression(Operator::Sub, &[unknown, parameter.clone()], false)?;
    let polynomial = base.expression(Operator::Mul, &[factor, quadratic.clone()], false)?;
    let zero = base.integer(0, false)?;
    let predicate = base.equation(&polynomial, &zero)?;
    Ok((base.problem("x", &predicate, &[])?, polynomial, parameter))
}

/// Keep each classified source root in its own branch, independently of set ordering.
fn separate(
    base: &mut Fixture,
    problem: &Handle,
    entries: &[Handle],
) -> TestResult<(Handle, Vec<Handle>)> {
    let mut branches = Vec::new();
    for entry in entries {
        let set = base.finite(problem, std::slice::from_ref(entry))?;
        let SolveView::SolutionSet(set) = base.cas.solve_view(&set)? else {
            return Err("missing finite set".into());
        };
        branches.push(set.branches.first().ok_or("missing finite branch")?.clone());
    }
    let set = admit(
        &mut base.cas,
        &Draft::Solve(SolveDraft::SolutionSet(
            crate::test_api::solving::SolutionSetData {
                problem: Reference::Existing(problem.clone()),
                branches: branches.iter().cloned().map(Reference::Existing).collect(),
            },
        )),
    )?;
    Ok((set, branches))
}

/// Independently reconstruct the cubic factor identity from the supplied quadratic lifecycle.
fn mixed_source() -> TestResult<(Source, Vec<Handle>)> {
    use crate::test_api::proof::Certificate;
    use crate::test_api::solving::{
        CertificateProductCompleteness, RootFactor, SolveCertificate, SolveSchema, SolveStatement,
        StatementCoverage,
    };
    let mut fixture = super::BinomialFixture::new(2, -1)?;
    let Certificate::Solve(identity) = &fixture
        .steps
        .get(5)
        .ok_or("missing quadratic identity")?
        .certificate
    else {
        return Err("wrong identity".into());
    };
    let SolveCertificate::CoefficientIdentity(identity) = identity.as_ref() else {
        return Err("wrong identity".into());
    };
    let (input, product, map) = (
        identity.left.clone(),
        identity.right.clone(),
        identity.map.clone(),
    );
    let Certificate::Solve(complete) = &fixture
        .steps
        .get(6)
        .ok_or("missing quadratic coverage")?
        .certificate
    else {
        return Err("wrong coverage".into());
    };
    let SolveCertificate::ProductCompleteness(complete) = complete.as_ref() else {
        return Err("wrong coverage".into());
    };
    let mut factors = complete.factors.clone();
    let (problem, polynomial, parameter) = mixed_problem(&mut fixture.base, &input)?;
    let unknown = fixture.base.free("x")?;
    let linear = fixture
        .base
        .expression(Operator::Sub, &[unknown, parameter.clone()], false)?;
    let reconstruction = fixture
        .base
        .expression(Operator::Mul, &[linear, product], false)?;
    factors.insert(
        0,
        RootFactor {
            root: parameter,
            multiplicity: 1,
        },
    );
    let entries: Vec<_> = factors.iter().map(|factor| factor.root.clone()).collect();
    let (set, branches) = separate(&mut fixture.base, &problem, &entries)?;
    fixture.steps.truncate(5);
    let mut identity = fixture.base.identity(&polynomial, &reconstruction, &map);
    identity.premises = vec![1, 2, 4];
    fixture.steps.push(identity);
    let scalar = fixture.base.integer(1, false)?;
    let mut complete = fixture.base.step(
        SolveSchema::ProductCompleteness,
        SolveStatement::Coverage(StatementCoverage {
            problem: problem.clone(),
            set: set.clone(),
            region: fixture.base.region.clone(),
        }),
        SolveCertificate::ProductCompleteness(CertificateProductCompleteness {
            problem: problem.clone(),
            set: set.clone(),
            scalar,
            factors,
            product: 0,
            existence: vec![1, 2],
            region: fixture.base.region.clone(),
        }),
    );
    complete.premises = vec![5, 1, 4];
    fixture.steps.push(complete);
    let zero = fixture.base.integer(0, false)?;
    let cover = fixture.base.split(&zero)?;
    fixture.steps.push(cover.clone());
    Ok((
        Source {
            fixture,
            problem,
            set,
            cover,
        },
        branches,
    ))
}

/// Retention and rejection use independent branch evidence on the corresponding coefficient region.
pub(crate) fn mixed_restriction() -> TestResult<super::BinomialFixture> {
    use crate::test_api::solving::{
        CertificateRealRestriction, SolveCertificate, SolveSchema, SolveStatement,
        StatementCoverage,
    };
    let (
        Source {
            mut fixture,
            problem: complex_problem,
            set: _,
            cover: complex_cover,
        },
        branches,
    ) = mixed_source()?;
    fixture.base.enter_real()?;
    let (_, quadratic) = fixture.base.quadratic(-1)?;
    let (problem, _, parameter) = mixed_problem(&mut fixture.base, &quadratic)?;
    let set = fixture.base.finite(&problem, &[parameter])?;
    let zero = fixture.base.integer(0, false)?;
    let real_cover = fixture.base.split(&zero)?;
    let correspondence = transport(
        &mut fixture.base,
        &problem,
        &complex_problem,
        &real_cover,
        &complex_cover,
    )?;
    fixture.steps.push(real_cover);
    let mut restricted = fixture.base.step(
        SolveSchema::RealRestriction,
        SolveStatement::Coverage(StatementCoverage {
            problem: problem.clone(),
            set: set.clone(),
            region: fixture.base.region.clone(),
        }),
        SolveCertificate::RealRestriction(CertificateRealRestriction {
            coverage: 0,
            problem,
            set,
            transport: correspondence,
            retained: vec![branches.first().ok_or("missing real branch")?.clone()],
            rejected: branches
                .get(1..)
                .ok_or("missing rejected branches")?
                .to_vec(),
            classification: Vec::new(),
            cover: 1,
        }),
    );
    restricted.premises = vec![6, 7, 8, 1, 4];
    fixture.steps.push(restricted);
    Ok(fixture)
}
