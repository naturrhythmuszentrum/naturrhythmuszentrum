// SPDX-License-Identifier: AGPL-3.0-only
//! Supplied cubic choices with independent exact conjugation and norm witnesses.

use super::{BinomialFixture, Fixture, admit};
use crate::test_api::limits::{Budget, Limits};
use crate::test_api::proof::{ApplicationDraft, Certificate};
use crate::test_api::solving::{
    CertificateConjugationTransport, CertificateRootSelectionRefinement, CoefficientValueData,
    CoefficientValueObject, RefinementMode, RefinementTargetKind, RootSelectionData, SelectionMode,
    SolveCertificate, SolveDraft, SolveSchema, SolveStatement, SolveView,
    StatementCoefficientEquality, StatementRealValues, StatementRootExistence,
};
use crate::test_api::syntax::{
    AtomicSort, Draft, ExpressionDraft, Field, Operator, PredicateKind, Sort,
};
use crate::test_api::{Handle, Rational, Reference};
use crate::testing::common::TestResult;

/// Roots remain actual mapped reduced tower values throughout the public lifecycle.
struct Cubic {
    fixture: BinomialFixture,
    map: Handle,
    roots: Vec<Handle>,
    first: Handle,
    second: Handle,
}

/// Every prerequisite is explicit and in its original order.
fn push(fixture: &mut BinomialFixture, mut step: ApplicationDraft) -> TestResult<u32> {
    let index = u32::try_from(fixture.steps.len())?;
    step.premises = (0..index).collect();
    fixture.steps.push(step);
    Ok(index)
}

/// Rational fixture data is supplied independently of coefficient arithmetic under test.
fn coefficient(base: &mut Fixture, numerator: i64, denominator: u64) -> TestResult<Handle> {
    let value = Rational::from_ratio(
        numerator,
        denominator,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    admit(
        &mut base.cas,
        &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
            domain: Reference::Existing(base.algebra.clone()),
            value: CoefficientValueData::Rational { value },
        })),
    )
}

/// The three unity powers are 1, w and -1-w by the displayed polynomial w²+w+1.
fn unity_value(
    base: &mut Fixture,
    domain: &Handle,
    power: u32,
    numerator: i64,
    denominator: u64,
) -> TestResult<Handle> {
    if numerator == 0 {
        return base.tower_value(domain, &[]);
    }
    let scalar = coefficient(
        base,
        if power == 2 { -numerator } else { numerator },
        denominator,
    )?;
    let terms = match power {
        0 => vec![scalar],
        1 => vec![base.coefficient(0)?, scalar],
        2 => vec![scalar.clone(), scalar],
        _ => return Err("invalid supplied unity power".into()),
    };
    base.tower_value(domain, &terms)
}

/// Parent values encode (a+bi)w^k, with i only present in the Bombelli fixture.
fn parent_value(
    base: &mut Fixture,
    domains: &[Handle; 2],
    power: u32,
    components: [i64; 2],
    denominator: u64,
) -> TestResult<Handle> {
    let real = unity_value(base, &domains[0], power, components[0], denominator)?;
    if domains[0] == domains[1] {
        return Ok(real);
    }
    let terms = if components[1] == 0 {
        if components[0] == 0 {
            Vec::new()
        } else {
            vec![real]
        }
    } else {
        vec![
            real,
            unity_value(base, &domains[0], power, components[1], denominator)?,
        ]
    };
    base.tower_value(&domains[1], &terms)
}

/// Adjoin an arbitrary square root of -1; neither orientation is privileged.
fn imaginary(
    fixture: &mut BinomialFixture,
    unity: &super::UnityFixture,
) -> TestResult<(Handle, Handle, Handle)> {
    let polynomial = fixture.base.polynomial(
        &unity.domain,
        &[(2, unity.one.clone()), (0, unity.one.clone())],
    )?;
    let (relation, selected, root) = fixture.base.root(&polynomial, &unity.map)?;
    let leading = push(fixture, unity.unit.clone())?;
    let mut existence = fixture.base.adjoin(&relation, &selected, leading);
    let Certificate::Solve(certificate) = &mut existence.certificate else {
        return Err("missing certificate".into());
    };
    let SolveCertificate::RootAdjunction(certificate) = certificate.as_mut() else {
        return Err("missing adjunction".into());
    };
    certificate.leading_unit = leading;
    push(fixture, existence)?;
    let domain = fixture.base.tower(&unity.domain, &polynomial)?;
    let map = fixture
        .base
        .scalar_map(&domain, std::slice::from_ref(&root))?;
    Ok((domain, map, root))
}

/// Components have real scalar sort even inside the complex choice binder.
fn real_expression(
    base: &mut Fixture,
    operator: Operator,
    children: &[Handle],
) -> TestResult<Handle> {
    admit(
        &mut base.cas,
        &Draft::Expression(ExpressionDraft {
            header: Reference::Existing(base.header.clone()),
            scope: vec![vec![AtomicSort::Scalar(Field::Complex)]],
            operator,
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            operands: children.iter().cloned().map(Reference::Existing).collect(),
            space: None,
            binding: None,
        }),
    )
}

/// Constraints explicitly name the actual root slot and its real part.
fn predicate(base: &mut Fixture, kind: PredicateKind, arguments: &[Handle]) -> TestResult<Handle> {
    admit(
        &mut base.cas,
        &Draft::Condition {
            header: Reference::Existing(base.header.clone()),
            scope: vec![vec![AtomicSort::Scalar(Field::Complex)]],
            kind,
            arguments: arguments.iter().cloned().map(Reference::Existing).collect(),
        },
    )
}

/// A conjunction consumes Boolean expressions, while the selection itself retains a `Condition`.
fn conjunct(base: &mut Fixture, kind: PredicateKind, arguments: &[Handle]) -> TestResult<Handle> {
    admit(
        &mut base.cas,
        &Draft::Expression(ExpressionDraft {
            header: Reference::Existing(base.header.clone()),
            scope: vec![vec![AtomicSort::Scalar(Field::Complex)]],
            operator: Operator::Predicate(kind),
            sort: Sort::Value(AtomicSort::Boolean),
            operands: arguments.iter().cloned().map(Reference::Existing).collect(),
            space: None,
            binding: None,
        }),
    )
}

/// The real odd choice has checked sign and zero imaginary part; Bombelli retains its exact norm.
fn constraints(base: &mut Fixture, bombelli: bool, unlinked: bool) -> TestResult<Handle> {
    let root = base.expression(Operator::Bound { depth: 0, slot: 0 }, &[], true)?;
    if unlinked {
        return predicate(base, PredicateKind::Equal, &[root.clone(), root]);
    }
    let real = real_expression(base, Operator::RealPart, std::slice::from_ref(&root))?;
    let two = real_expression(
        base,
        Operator::Rational(Rational::from_ratio(
            2,
            1,
            &mut Budget::new(Limits::blind_cubic())?,
        )?),
        &[],
    )?;
    let fixed = conjunct(base, PredicateKind::Equal, &[real.clone(), two])?;
    let positive = conjunct(base, PredicateKind::Positive, &[real])?;
    let link = if bombelli {
        let conjugate = base.expression(Operator::Conjugate, std::slice::from_ref(&root), true)?;
        let norm = base.expression(Operator::Mul, &[root, conjugate], true)?;
        let five = base.integer(5, true)?;
        conjunct(base, PredicateKind::Equal, &[norm, five])?
    } else {
        let imaginary = real_expression(base, Operator::ImagPart, &[root])?;
        let zero = real_expression(base, Operator::Rational(Rational::zero()), &[])?;
        conjunct(base, PredicateKind::Equal, &[imaginary, zero])?
    };
    predicate(base, PredicateKind::And, &[fixed, positive, link])
}

/// A constructive selection proves both its equation and every sign/conjugation constraint.
fn choice(
    fixture: &mut BinomialFixture,
    relation: &Handle,
    witness: &Handle,
    bombelli: bool,
    unlinked: bool,
) -> TestResult<Handle> {
    let predicate = constraints(&mut fixture.base, bombelli, unlinked)?;
    let selected = admit(
        &mut fixture.base.cas,
        &Draft::Solve(SolveDraft::RootSelection(RootSelectionData {
            cell: None,
            header: Reference::Existing(fixture.base.header.clone()),
            scope: Vec::new(),
            relations: vec![Reference::Existing(relation.clone())],
            predicate: Reference::Existing(predicate),
            mode: SelectionMode::Constrained,
            convention: None,
            interval: None,
        })),
    )?;
    let step = fixture.base.step(
        SolveSchema::RootSelectionRefinement,
        SolveStatement::RootExistence(StatementRootExistence {
            relation: selected.clone(),
            region: fixture.base.region.clone(),
        }),
        SolveCertificate::RootSelectionRefinement(CertificateRootSelectionRefinement {
            mode: RefinementMode::Existence,
            target_kind: RefinementTargetKind::Selection,
            target: selected.clone(),
            witnesses: vec![witness.clone()],
            compatibility: Vec::new(),
            form: None,
        }),
    );
    push(fixture, step)?;
    admit(
        &mut fixture.base.cas,
        &Draft::RootValue {
            header: Reference::Existing(fixture.base.header.clone()),
            selection: Reference::Existing(selected),
            slot: 0,
        },
    )
}

/// The inverse choice is a reduced polynomial: -u²/4 or (2-11i)u²/25.
fn returned(
    fixture: &mut BinomialFixture,
    domains: &[Handle; 2],
    polynomial: &Handle,
    root: &Handle,
    bombelli: bool,
) -> TestResult<(Handle, Vec<Handle>, Handle, Handle)> {
    let domain = fixture.base.tower(&domains[1], polynomial)?;
    let map = fixture
        .base
        .scalar_map(&domain, std::slice::from_ref(root))?;
    let inverse = if bombelli { [2, -11] } else { [-1, 0] };
    let denominator = if bombelli { 25 } else { 4 };
    let zero = parent_value(&mut fixture.base, domains, 0, [0, 0], 1)?;
    let one = parent_value(&mut fixture.base, domains, 0, [1, 0], 1)?;
    let first = fixture.base.tower_value(&domain, &[zero.clone(), one])?;
    let first = fixture.base.literal(&first, &map)?;
    let factor = parent_value(&mut fixture.base, domains, 0, inverse, denominator)?;
    let second = fixture
        .base
        .tower_value(&domain, &[zero.clone(), zero.clone(), factor])?;
    let second = fixture.base.literal(&second, &map)?;
    let mut roots = Vec::new();
    for power in 0..3 {
        let linear = parent_value(&mut fixture.base, domains, power, [1, 0], 1)?;
        let quadratic = parent_value(
            &mut fixture.base,
            domains,
            (2 * power) % 3,
            inverse,
            denominator,
        )?;
        let value = fixture
            .base
            .tower_value(&domain, &[zero.clone(), linear, quadratic])?;
        roots.push(fixture.base.literal(&value, &map)?);
    }
    Ok((map, roots, first, second))
}

/// Ground arithmetic independently checks 2³=8 and (2+i)³=2+11i before classifying any returned root.
fn cubic(bombelli: bool, mutation: u8) -> TestResult<Cubic> {
    let mut base = Fixture::in_field(&Field::Complex)?;
    let unity = base.unity(3)?;
    let mut fixture = BinomialFixture {
        base,
        steps: unity.steps.clone(),
    };
    let (parent, parent_map, imaginary_root) = if bombelli {
        let (domain, map, root) = imaginary(&mut fixture, &unity)?;
        (domain, map, Some(root))
    } else {
        (unity.domain.clone(), unity.map.clone(), None)
    };
    let domains = [unity.domain.clone(), parent];
    let one = parent_value(&mut fixture.base, &domains, 0, [1, 0], 1)?;
    let constant = if bombelli {
        [if mutation == 4 { -3 } else { -2 }, -11]
    } else {
        [if mutation == 4 { -9 } else { -8 }, 0]
    };
    let constant = parent_value(&mut fixture.base, &domains, 0, constant, 1)?;
    let polynomial = fixture
        .base
        .polynomial(&domains[1], &[(3, one), (0, constant)])?;
    let (relation, _, _) = fixture.base.root(&polynomial, &parent_map)?;
    let two = fixture.base.integer(2, false)?;
    let mut witness = if let Some(imaginary) = imaginary_root {
        fixture
            .base
            .expression(Operator::Add, &[two, imaginary], false)?
    } else {
        two
    };
    if mutation == 3 {
        let omega = fixture.base.literal(&unity.generator, &unity.map)?;
        witness = fixture
            .base
            .expression(Operator::Mul, &[witness, omega], false)?;
    }
    let root = choice(&mut fixture, &relation, &witness, bombelli, mutation == 0)?;
    let (map, roots, first, second) =
        returned(&mut fixture, &domains, &polynomial, &root, bombelli)?;
    Ok(Cubic {
        fixture,
        map,
        roots,
        first,
        second,
    })
}

/// A conjugation certificate retains the same exact map and region as its statement.
fn conjugation(
    base: &Fixture,
    map: &Handle,
    values: &[Handle],
    claim: SolveStatement,
) -> ApplicationDraft {
    base.step(
        SolveSchema::ConjugationTransport,
        claim,
        SolveCertificate::ConjugationTransport(CertificateConjugationTransport {
            map: map.clone(),
            conjugate_map: map.clone(),
            images: Vec::new(),
            values: values.to_vec(),
            relations: Vec::new(),
            fixed_field: Vec::new(),
            region: base.region.clone(),
        }),
    )
}

/// Scalar equality is checked by the same finite component interpreter as realness.
fn equality(cubic: &mut Cubic, left: &Handle, right: &Handle) -> TestResult<u32> {
    let algebra = cubic.fixture.base.algebra.clone();
    let map = cubic.fixture.base.scalar_map(&algebra, &[])?;
    let step = conjugation(
        &cubic.fixture.base,
        &map,
        &[],
        SolveStatement::CoefficientEquality(StatementCoefficientEquality {
            left: left.clone(),
            right: right.clone(),
            map: map.clone(),
            region: cubic.fixture.base.region.clone(),
        }),
    );
    push(&mut cubic.fixture, step)
}

/// Both linkage obligations precede realness: the product is exact and the choices are conjugate when required.
fn linkage(cubic: &mut Cubic, bombelli: bool, rotated: bool) -> TestResult<[u32; 2]> {
    let product = cubic.fixture.base.expression(
        Operator::Mul,
        &[cubic.first.clone(), cubic.second.clone()],
        false,
    )?;
    let expected = cubic
        .fixture
        .base
        .integer(if bombelli { 5 } else { -2 }, false)?;
    let product = equality(cubic, &product, &expected)?;
    let left = cubic.fixture.base.expression(
        Operator::Conjugate,
        std::slice::from_ref(if bombelli {
            &cubic.first
        } else {
            cubic.roots.get(1).ok_or("missing root")?
        }),
        false,
    )?;
    let mut right = if bombelli {
        cubic.second.clone()
    } else {
        cubic.roots.get(2).ok_or("missing root")?.clone()
    };
    if rotated {
        let unity = cubic.fixture.base.unity(3)?;
        let omega = cubic.fixture.base.literal(&unity.generator, &unity.map)?;
        right = cubic
            .fixture
            .base
            .expression(Operator::Mul, &[right, omega], false)?;
    }
    let conjugate = equality(cubic, &left, &right)?;
    references_for(
        cubic
            .fixture
            .steps
            .last_mut()
            .ok_or("missing conjugation")?,
        &[product],
        &[],
    )?;
    Ok([product, conjugate])
}

/// The original cubic is independent of its supplied factorization.
fn problem(base: &mut Fixture, bombelli: bool) -> TestResult<(Handle, Handle)> {
    let unknown = base.free("x")?;
    let cube = base.expression(
        Operator::IntegerPower(3),
        std::slice::from_ref(&unknown),
        false,
    )?;
    let coefficient = base.integer(if bombelli { -15 } else { 6 }, false)?;
    let linear = base.expression(Operator::Mul, &[coefficient, unknown], false)?;
    let sum = base.expression(Operator::Add, &[cube, linear], false)?;
    let constant = base.integer(if bombelli { 4 } else { 7 }, false)?;
    let polynomial = base.expression(Operator::Sub, &[sum, constant], false)?;
    let zero = base.integer(0, false)?;
    let predicate = base.equation(&polynomial, &zero)?;
    Ok((base.problem("x", &predicate, &[])?, polynomial))
}

/// Each returned root occupies its own branch, so restriction must classify all three independently.
fn branches(
    base: &mut Fixture,
    problem: &Handle,
    roots: &[Handle],
) -> TestResult<(Handle, Vec<Handle>)> {
    let mut branches = Vec::new();
    for root in roots {
        let set = base.finite(problem, std::slice::from_ref(root))?;
        let SolveView::SolutionSet(set) = base.cas.solve_view(&set)? else {
            return Err("missing set".into());
        };
        branches.push(set.branches.first().ok_or("missing branch")?.clone());
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

/// Independent exact multiplication checks the complete three-factor cubic before classification.
fn coverage(cubic: &mut Cubic, bombelli: bool) -> TestResult<(u32, Handle, Vec<Handle>)> {
    use crate::test_api::solving::{CertificateProductCompleteness, RootFactor, StatementCoverage};
    let (problem, polynomial) = problem(&mut cubic.fixture.base, bombelli)?;
    let unknown = cubic.fixture.base.free("x")?;
    let one = cubic.fixture.base.integer(1, false)?;
    let mut product = one.clone();
    for root in &cubic.roots {
        let factor = cubic.fixture.base.expression(
            Operator::Sub,
            &[unknown.clone(), root.clone()],
            false,
        )?;
        product = cubic
            .fixture
            .base
            .expression(Operator::Mul, &[product, factor], false)?;
    }
    let identity = cubic
        .fixture
        .base
        .identity(&polynomial, &product, &cubic.map);
    let product = push(&mut cubic.fixture, identity)?;
    let (set, branches) = branches(&mut cubic.fixture.base, &problem, &cubic.roots)?;
    let step = cubic.fixture.base.step(
        SolveSchema::ProductCompleteness,
        SolveStatement::Coverage(StatementCoverage {
            problem: problem.clone(),
            set: set.clone(),
            region: cubic.fixture.base.region.clone(),
        }),
        SolveCertificate::ProductCompleteness(CertificateProductCompleteness {
            problem: problem.clone(),
            set,
            scalar: one,
            factors: cubic
                .roots
                .iter()
                .map(|root| RootFactor {
                    root: root.clone(),
                    multiplicity: 1,
                })
                .collect(),
            product,
            existence: Vec::new(),
            region: cubic.fixture.base.region.clone(),
        }),
    );
    let index = push(&mut cubic.fixture, step)?;
    Ok((index, problem, branches))
}

/// Supplied cubic realness and conjugate-pair claims remain independent of the blind producer.
pub(crate) fn linked_cubic(bombelli: bool, mutation: u8) -> TestResult<BinomialFixture> {
    let mut cubic = cubic(bombelli, mutation)?;
    coverage(&mut cubic, bombelli)?;
    let links = linkage(&mut cubic, bombelli, mutation == 5)?;
    let values = if bombelli {
        cubic.roots.clone()
    } else {
        vec![cubic.roots.first().ok_or("missing real root")?.clone()]
    };
    let algebra = cubic.fixture.base.algebra.clone();
    let map = cubic.fixture.base.scalar_map(&algebra, &[])?;
    let mut step = conjugation(
        &cubic.fixture.base,
        &map,
        &values,
        SolveStatement::RealValues(StatementRealValues {
            values: values.clone(),
            region: cubic.fixture.base.region.clone(),
        }),
    );
    references_for(&mut step, &[], &links)?;
    if mutation == 1 {
        let Certificate::Solve(certificate) = &mut step.certificate else {
            return Err("missing certificate".into());
        };
        let SolveCertificate::ConjugationTransport(certificate) = certificate.as_mut() else {
            return Err("missing conjugation".into());
        };
        certificate.conjugate_map.clone_from(&cubic.map);
    }
    if mutation == 2 {
        let region = cubic.fixture.base.coefficient_guard(true)?;
        let Certificate::Solve(certificate) = &mut step.certificate else {
            return Err("missing certificate".into());
        };
        let SolveCertificate::ConjugationTransport(certificate) = certificate.as_mut() else {
            return Err("missing conjugation".into());
        };
        certificate.region = region;
    }
    push(&mut cubic.fixture, step)?;
    Ok(cubic.fixture)
}

/// A rational-function literal retains its actual parameter image and nonzero denominator guard.
pub(crate) fn fraction_realness(guarded: bool) -> TestResult<BinomialFixture> {
    let mut base = Fixture::new()?;
    if guarded {
        let region = base.coefficient_guard(true)?;
        base.enter_region(region)?;
    }
    let parameter = base.free("a")?;
    let (polynomial, map) = base.parameter_binomial(&parameter)?;
    let SolveView::Polynomial(polynomial) = base.cas.solve_view(&polynomial)? else {
        return Err("missing polynomial".into());
    };
    let source = polynomial
        .terms
        .first()
        .ok_or("missing coefficient")?
        .coefficient
        .clone();
    let SolveView::CoefficientValue(value) = base.cas.solve_view(&source)? else {
        return Err("missing fraction".into());
    };
    let CoefficientValueData::Fraction {
        numerator,
        denominator,
    } = &value.value
    else {
        return Err("wrong coefficient".into());
    };
    let draft = Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
        domain: Reference::Existing(value.domain.clone()),
        value: CoefficientValueData::Fraction {
            numerator: Reference::Existing(denominator.clone()),
            denominator: Reference::Existing(numerator.clone()),
        },
    }));
    let inverse = admit(&mut base.cas, &draft)?;
    let literal = base.literal(&inverse, &map)?;
    let algebra = base.algebra.clone();
    let ground = base.scalar_map(&algebra, &[])?;
    let step = conjugation(
        &base,
        &ground,
        std::slice::from_ref(&literal),
        SolveStatement::RealValues(StatementRealValues {
            values: vec![literal.clone()],
            region: base.region.clone(),
        }),
    );
    Ok(BinomialFixture {
        base,
        steps: vec![step],
    })
}

/// A complex witness cannot silently establish existence in a real root relation.
pub(crate) fn choice_wrong_ambient() -> TestResult<BinomialFixture> {
    let (mut base, mut step) = super::root_refinement(2, false)?;
    let witness = admit(
        &mut base.cas,
        &Draft::Expression(ExpressionDraft {
            header: Reference::Existing(base.header.clone()),
            scope: Vec::new(),
            operator: Operator::Rational(Rational::from_ratio(
                2,
                1,
                &mut Budget::new(Limits::blind_cubic())?,
            )?),
            sort: Sort::Value(AtomicSort::Scalar(Field::Complex)),
            operands: Vec::new(),
            space: None,
            binding: None,
        }),
    )?;
    let Certificate::Solve(certificate) = &mut step.certificate else {
        return Err("missing certificate".into());
    };
    let SolveCertificate::RootSelectionRefinement(certificate) = certificate.as_mut() else {
        return Err("missing refinement".into());
    };
    certificate.witnesses = vec![witness];
    Ok(BinomialFixture {
        base,
        steps: vec![step],
    })
}

/// Expression powers keep their strict definedness, unlike zero exponents in polynomial monomials.
pub(crate) fn partial_power_realness() -> TestResult<BinomialFixture> {
    let mut base = Fixture::in_field(&Field::Complex)?;
    let zero = base.integer(0, false)?;
    let value = base.expression(Operator::IntegerPower(0), &[zero], false)?;
    let algebra = base.algebra.clone();
    let map = base.scalar_map(&algebra, &[])?;
    let step = conjugation(
        &base,
        &map,
        std::slice::from_ref(&value),
        SolveStatement::RealValues(StatementRealValues {
            values: vec![value.clone()],
            region: base.region.clone(),
        }),
    );
    Ok(BinomialFixture {
        base,
        steps: vec![step],
    })
}

/// A checked scalar equality is necessary evidence for an otherwise unconstrained complex parameter.
pub(crate) fn referenced_realness(references: u8) -> TestResult<BinomialFixture> {
    let mut base = Fixture::in_field(&Field::Complex)?;
    let value = base.free("a")?;
    let conjugate = base.expression(Operator::Conjugate, std::slice::from_ref(&value), false)?;
    let hypothesis = assumed_self_conjugate(&mut base, &value, &conjugate)?;
    let mut fixture = BinomialFixture {
        base,
        steps: vec![hypothesis],
    };
    let algebra = fixture.base.algebra.clone();
    let map = fixture.base.scalar_map(&algebra, &[])?;
    let mut equality = conjugation(
        &fixture.base,
        &map,
        &[],
        SolveStatement::CoefficientEquality(StatementCoefficientEquality {
            left: value.clone(),
            right: conjugate,
            map: map.clone(),
            region: fixture.base.region.clone(),
        }),
    );
    references_for(&mut equality, &[], &[0])?;
    push(&mut fixture, equality)?;
    let mut classification = conjugation(
        &fixture.base,
        &map,
        std::slice::from_ref(&value),
        SolveStatement::RealValues(StatementRealValues {
            values: vec![value.clone()],
            region: fixture.base.region.clone(),
        }),
    );
    match references {
        0 => {}
        1 => references_for(&mut classification, &[], &[1])?,
        2 => references_for(&mut classification, &[1], &[1, 1])?,
        _ => return Err("invalid reference test mode".into()),
    }
    push(&mut fixture, classification)?;
    Ok(fixture)
}

/// Existing premise-slot vectors name exactly the facts a transport imports.
fn references_for(
    step: &mut ApplicationDraft,
    relations: &[u32],
    fixed_field: &[u32],
) -> TestResult {
    let Certificate::Solve(certificate) = &mut step.certificate else {
        return Err("missing certificate".into());
    };
    let SolveCertificate::ConjugationTransport(certificate) = certificate.as_mut() else {
        return Err("missing conjugation".into());
    };
    certificate.relations = relations.to_vec();
    certificate.fixed_field = fixed_field.to_vec();
    Ok(())
}

/// The independent premise is scoped to an explicit assumption, never to a display name.
fn assumed_self_conjugate(
    base: &mut Fixture,
    value: &Handle,
    conjugate: &Handle,
) -> TestResult<ApplicationDraft> {
    use crate::test_api::domain::{AssumptionDraft, ContextDraft, HypothesisRef};
    use crate::test_api::proof::{Primitive, Statement};
    let predicate = base.equation(value, conjugate)?;
    base.context = admit(
        &mut base.cas,
        &Draft::Context(ContextDraft::Root {
            header: Reference::Existing(base.header.clone()),
            admissible: Reference::Existing(base.region.clone()),
            assumptions: vec![AssumptionDraft {
                key: "self-conjugate".into(),
                condition: Reference::Existing(predicate.clone()),
            }],
        }),
    )?;
    Ok(ApplicationDraft {
        rule: Primitive::ContextHypothesis,
        context: base.context.clone(),
        premises: Vec::new(),
        substitutions: Vec::new(),
        conclusion: Statement::PredicateFact {
            predicate,
            region: base.region.clone(),
        },
        certificate: Certificate::Hypothesis(HypothesisRef {
            context: base.context.clone(),
            slot: 0,
        }),
    })
}

/// An explicitly requested full product equality exercises the same once-per-application import path.
pub(crate) fn cubic_product_reference(duplicate: bool) -> TestResult<BinomialFixture> {
    let mut fixture = linked_cubic(true, u8::MAX)?;
    let product = fixture.steps.iter().position(|step| matches!(&step.certificate,
        Certificate::Solve(certificate) if matches!(certificate.as_ref(), SolveCertificate::CoefficientIdentity(_))))
        .ok_or("missing product identity")?;
    let product = u32::try_from(product)?;
    let step = fixture.steps.last_mut().ok_or("missing classification")?;
    let Certificate::Solve(certificate) = &mut step.certificate else {
        return Err("missing certificate".into());
    };
    let SolveCertificate::ConjugationTransport(certificate) = certificate.as_mut() else {
        return Err("missing conjugation".into());
    };
    certificate.relations = vec![product];
    if duplicate {
        certificate.relations.push(product);
        certificate.fixed_field.push(product);
    }
    Ok(fixture)
}

/// Each refusal changes one checked cancellation input while keeping ordinary graph syntax.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PositiveMutation {
    /// Complete ordered-real certificate.
    Valid,
    /// The strictly positive leaf is only nonnegative.
    WeakSign,
    /// The mixed product has no nonnegative premise.
    MissingSign,
    /// A proposed factorization is false.
    Factorization,
    /// The claimed factor differs from the checked product's factor.
    UnrelatedFactor,
    /// The application names another coefficient region.
    Region,
    /// Cited evidence belongs to an ancestor rather than this exact context.
    Context,
    /// Complex expressions cannot be ordered-real cofactors.
    Complex,
    /// Earlier evidence is not imported implicitly.
    MissingReference,
    /// Explicit premise slots are checked.
    Reference,
    /// A displayed positive term has changed sign.
    NegativeTerm,
}

/// Real projections remain explicitly typed even when their containing header is complex.
fn real_projection(base: &mut Fixture, value: &Handle) -> TestResult<Handle> {
    admit(
        &mut base.cas,
        &Draft::Expression(ExpressionDraft {
            header: Reference::Existing(base.header.clone()),
            scope: Vec::new(),
            operator: Operator::RealPart,
            sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            operands: vec![Reference::Existing(value.clone())],
            space: None,
            binding: None,
        }),
    )
}

/// Arbitrary real expressions display the factorization of a difference of third powers.
fn positive_expressions(
    base: &mut Fixture,
    first: &Handle,
    second: &Handle,
    mutation: PositiveMutation,
) -> TestResult<[Handle; 3]> {
    let first_cube = base.expression(
        Operator::IntegerPower(3),
        std::slice::from_ref(first),
        false,
    )?;
    let second_cube = base.expression(
        Operator::IntegerPower(3),
        std::slice::from_ref(second),
        false,
    )?;
    let source = base.expression(Operator::Sub, &[first_cube, second_cube], false)?;
    let factor = base.expression(Operator::Sub, &[first.clone(), second.clone()], false)?;
    let first_square = base.expression(
        Operator::IntegerPower(2),
        std::slice::from_ref(first),
        false,
    )?;
    let mixed = base.expression(Operator::Mul, &[first.clone(), second.clone()], false)?;
    let second_square = base.expression(
        Operator::IntegerPower(2),
        std::slice::from_ref(second),
        false,
    )?;
    let tail = base.expression(
        if mutation == PositiveMutation::NegativeTerm {
            Operator::Sub
        } else {
            Operator::Add
        },
        &[mixed, second_square],
        false,
    )?;
    let cofactor = base.expression(Operator::Add, &[first_square, tail], false)?;
    Ok([source, factor, cofactor])
}

/// Root hypotheses remain qualified assumptions; cancellation does not discharge any of them.
fn positive_hypotheses(
    base: &mut Fixture,
    source: &Handle,
    first: &Handle,
    second: &Handle,
    mutation: PositiveMutation,
) -> TestResult<Vec<ApplicationDraft>> {
    use crate::test_api::domain::{AssumptionDraft, ContextDraft, HypothesisRef};
    use crate::test_api::proof::{Primitive, Statement};
    let zero = base.integer(0, false)?;
    let mut predicates = vec![base.equation(source, &zero)?];
    for (index, value) in [first, second].into_iter().enumerate() {
        let argument = if mutation == PositiveMutation::Complex {
            real_projection(base, value)?
        } else {
            value.clone()
        };
        let kind = if index == 0 || mutation == PositiveMutation::WeakSign {
            PredicateKind::NonNegative
        } else {
            PredicateKind::Positive
        };
        predicates.push(admit(
            &mut base.cas,
            &Draft::Condition {
                header: Reference::Existing(base.header.clone()),
                scope: Vec::new(),
                kind,
                arguments: vec![Reference::Existing(argument)],
            },
        )?);
    }
    let assumptions = predicates
        .iter()
        .enumerate()
        .map(|(index, predicate)| AssumptionDraft {
            key: format!("positive-{index}"),
            condition: Reference::Existing(predicate.clone()),
        })
        .collect();
    base.context = admit(
        &mut base.cas,
        &Draft::Context(ContextDraft::Root {
            header: Reference::Existing(base.header.clone()),
            admissible: Reference::Existing(base.region.clone()),
            assumptions,
        }),
    )?;
    predicates
        .into_iter()
        .enumerate()
        .map(|(index, predicate)| {
            Ok(ApplicationDraft {
                rule: Primitive::ContextHypothesis,
                context: base.context.clone(),
                premises: Vec::new(),
                substitutions: Vec::new(),
                conclusion: Statement::PredicateFact {
                    predicate,
                    region: base.region.clone(),
                },
                certificate: Certificate::Hypothesis(HypothesisRef {
                    context: base.context.clone(),
                    slot: u32::try_from(index)?,
                }),
            })
        })
        .collect()
}

/// An exact coefficient claim uses the ordinary conjugation vocabulary and explicit references.
fn positive_equality(
    base: &Fixture,
    map: &Handle,
    left: &Handle,
    right: &Handle,
    references: &[u32],
    signs: &[u32],
) -> TestResult<ApplicationDraft> {
    let mut step = conjugation(
        base,
        map,
        &[],
        SolveStatement::CoefficientEquality(StatementCoefficientEquality {
            left: left.clone(),
            right: right.clone(),
            map: map.clone(),
            region: base.region.clone(),
        }),
    );
    references_for(&mut step, references, signs)?;
    Ok(step)
}

/// Public supplied-certificate fixture exercises cancellation without a polynomial-root oracle.
pub(crate) fn positive_cancellation(mutation: PositiveMutation) -> TestResult<BinomialFixture> {
    let mut base = Fixture::in_field(if mutation == PositiveMutation::Complex {
        &Field::Complex
    } else {
        &Field::Real
    })?;
    let first = base.free("a")?;
    let second = base.free("b")?;
    let [source, factor, cofactor] = positive_expressions(&mut base, &first, &second, mutation)?;
    let steps = positive_hypotheses(&mut base, &source, &first, &second, mutation)?;
    let mut fixture = BinomialFixture { base, steps };
    let domain = fixture.base.algebra.clone();
    let map = fixture.base.scalar_map(&domain, &[])?;
    let zero = fixture.base.integer(0, false)?;
    let source_step = positive_equality(&fixture.base, &map, &source, &zero, &[0], &[])?;
    push(&mut fixture, source_step)?;
    let product = fixture
        .base
        .expression(Operator::Mul, &[factor.clone(), cofactor], false)?;
    let factorization = if mutation == PositiveMutation::Factorization {
        fixture.base.identity(&source, &factor, &map)
    } else {
        fixture.base.identity(&source, &product, &map)
    };
    push(&mut fixture, factorization)?;
    let target = if mutation == PositiveMutation::UnrelatedFactor {
        fixture
            .base
            .expression(Operator::Add, &[first, second], false)?
    } else {
        factor
    };
    let references: &[u32] = if mutation == PositiveMutation::MissingReference {
        &[3]
    } else if mutation == PositiveMutation::Reference {
        &[3, 99]
    } else {
        &[3, 4]
    };
    let signs: &[u32] = if mutation == PositiveMutation::MissingSign {
        &[2]
    } else {
        &[1, 2]
    };
    let mut cancellation =
        positive_equality(&fixture.base, &map, &target, &zero, references, signs)?;
    positive_location(&mut fixture, &mut cancellation, mutation)?;
    push(&mut fixture, cancellation)?;
    Ok(fixture)
}

/// Exact provenance changes remain well-formed so the checker must refuse them.
fn positive_location(
    fixture: &mut BinomialFixture,
    step: &mut ApplicationDraft,
    mutation: PositiveMutation,
) -> TestResult {
    use crate::test_api::domain::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
    if mutation == PositiveMutation::Context {
        fixture.base.context = admit(
            &mut fixture.base.cas,
            &Draft::Context(ContextDraft::Local {
                parent: Reference::Existing(fixture.base.context.clone()),
                region: Reference::Existing(fixture.base.region.clone()),
                hypotheses: Vec::new(),
            }),
        )?;
        step.context.clone_from(&fixture.base.context);
    }
    if mutation == PositiveMutation::Region {
        let region = admit(
            &mut fixture.base.cas,
            &Draft::Region(RegionDraft {
                header: Reference::Existing(fixture.base.header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Product {
                    coordinates: fixture.base.coefficients.clone(),
                },
                expression: RegionExpr::Empty,
            }),
        )?;
        let crate::test_api::proof::Statement::Solve(statement) = &mut step.conclusion else {
            return Err("positive statement".into());
        };
        let SolveStatement::CoefficientEquality(claim) = statement.as_mut() else {
            return Err("positive claim".into());
        };
        claim.region.clone_from(&region);
        let Certificate::Solve(certificate) = &mut step.certificate else {
            return Err("positive certificate".into());
        };
        let SolveCertificate::ConjugationTransport(certificate) = certificate.as_mut() else {
            return Err("positive transport".into());
        };
        certificate.region = region;
    }
    Ok(())
}

/// An actual unconstrained selected root supplies its equation before any norm cancellation.
fn positive_norm_root() -> TestResult<(BinomialFixture, Handle, Handle)> {
    let mut base = Fixture::in_field(&Field::Complex)?;
    let domain = base.algebra.clone();
    let map = base.scalar_map(&domain, &[])?;
    let one = base.coefficient(1)?;
    let constant = base.coefficient(-8)?;
    let polynomial = base.polynomial(&domain, &[(3, one), (0, constant)])?;
    let (relation, selection, root) = base.root(&polynomial, &map)?;
    let unit = base.ground_unit()?;
    let mut fixture = BinomialFixture {
        base,
        steps: vec![unit],
    };
    let existence = fixture.base.adjoin(&relation, &selection, 0);
    push(&mut fixture, existence)?;
    Ok((fixture, map, root))
}

/// The two-slot predicate links the second slot to the actual conjugate of the first.
fn norm_joint_predicate(fixture: &mut BinomialFixture) -> TestResult<Handle> {
    let scope = vec![vec![AtomicSort::Scalar(Field::Complex); 2]];
    let mut bound = Vec::new();
    for slot in 0..2 {
        bound.push(admit(
            &mut fixture.base.cas,
            &Draft::Expression(ExpressionDraft {
                header: Reference::Existing(fixture.base.header.clone()),
                scope: scope.clone(),
                operator: Operator::Bound { depth: 0, slot },
                sort: Sort::Value(AtomicSort::Scalar(Field::Complex)),
                operands: Vec::new(),
                space: None,
                binding: None,
            }),
        )?);
    }
    let image = admit(
        &mut fixture.base.cas,
        &Draft::Expression(ExpressionDraft {
            header: Reference::Existing(fixture.base.header.clone()),
            scope: scope.clone(),
            operator: Operator::Conjugate,
            sort: Sort::Value(AtomicSort::Scalar(Field::Complex)),
            operands: vec![Reference::Existing(
                bound.first().ok_or("norm first bound")?.clone(),
            )],
            space: None,
            binding: None,
        }),
    )?;
    let predicate = admit(
        &mut fixture.base.cas,
        &Draft::Condition {
            header: Reference::Existing(fixture.base.header.clone()),
            scope,
            kind: PredicateKind::Equal,
            arguments: vec![
                Reference::Existing(bound.get(1).ok_or("norm second bound")?.clone()),
                Reference::Existing(image),
            ],
        },
    )?;
    Ok(predicate)
}

/// The actual conjugate is a supplied witness for a linked two-slot choice, never an independent root.
fn norm_joint_roots(
    fixture: &mut BinomialFixture,
    map: &Handle,
    root: &Handle,
    mutation: u8,
) -> TestResult<[Handle; 2]> {
    let conjugate =
        fixture
            .base
            .expression(Operator::Conjugate, std::slice::from_ref(root), false)?;
    let domain = fixture.base.algebra.clone();
    let one = fixture.base.coefficient(1)?;
    let negative = fixture.base.coefficient(-8)?;
    let polynomial = fixture
        .base
        .polynomial(&domain, &[(3, one), (0, negative)])?;
    let (relation, _, _) = fixture.base.root(&polynomial, map)?;
    let predicate = norm_joint_predicate(fixture)?;
    let selection = admit(
        &mut fixture.base.cas,
        &Draft::Solve(SolveDraft::RootSelection(RootSelectionData {
            cell: None,
            header: Reference::Existing(fixture.base.header.clone()),
            scope: Vec::new(),
            relations: vec![
                Reference::Existing(relation.clone()),
                Reference::Existing(relation),
            ],
            predicate: Reference::Existing(predicate),
            mode: SelectionMode::Constrained,
            convention: None,
            interval: None,
        })),
    )?;
    let step = fixture.base.step(
        SolveSchema::RootSelectionRefinement,
        SolveStatement::RootExistence(StatementRootExistence {
            relation: selection.clone(),
            region: fixture.base.region.clone(),
        }),
        SolveCertificate::RootSelectionRefinement(CertificateRootSelectionRefinement {
            mode: RefinementMode::Existence,
            target_kind: RefinementTargetKind::Selection,
            target: selection.clone(),
            witnesses: vec![root.clone(), conjugate],
            compatibility: Vec::new(),
            form: None,
        }),
    );
    let index = push(fixture, step)?;
    if mutation == 1 {
        fixture
            .steps
            .get_mut(usize::try_from(index)?)
            .ok_or("joint norm step")?
            .premises = vec![0];
    }
    let mut roots = Vec::new();
    for slot in 0..2 {
        roots.push(admit(
            &mut fixture.base.cas,
            &Draft::RootValue {
                header: Reference::Existing(fixture.base.header.clone()),
                selection: Reference::Existing(selection.clone()),
                slot,
            },
        )?);
    }
    roots.try_into().map_err(|_| "joint root count".into())
}

/// Both root equations are explicitly cited, and their finite ideal combination proves the product power.
fn norm_product_power(
    fixture: &mut BinomialFixture,
    map: &Handle,
    roots: &[Handle; 2],
) -> TestResult<u32> {
    use crate::test_api::solving::{IdentityMode, RelationMultiple};
    let eight = fixture.base.integer(8, false)?;
    let zero = fixture.base.integer(0, false)?;
    let mut cubes = Vec::new();
    let mut relations = Vec::new();
    for root in roots {
        let cube = fixture.base.expression(
            Operator::IntegerPower(3),
            std::slice::from_ref(root),
            false,
        )?;
        let difference =
            fixture
                .base
                .expression(Operator::Sub, &[cube.clone(), eight.clone()], false)?;
        let equation = positive_equality(&fixture.base, map, &difference, &zero, &[], &[])?;
        push(fixture, equation)?;
        cubes.push(cube);
        relations.push(difference);
    }
    let product = fixture.base.expression(Operator::Mul, roots, false)?;
    let left = fixture
        .base
        .expression(Operator::IntegerPower(3), &[product], false)?;
    let right = fixture.base.integer(64, false)?;
    let mut step = fixture.base.identity(&left, &right, map);
    let Certificate::Solve(certificate) = &mut step.certificate else {
        return Err("norm identity".into());
    };
    let SolveCertificate::CoefficientIdentity(certificate) = certificate.as_mut() else {
        return Err("norm combination".into());
    };
    certificate.mode = IdentityMode::RelationCombination;
    certificate.combinations = vec![
        RelationMultiple {
            multiplier: cubes.get(1).ok_or("second cube")?.clone(),
            relation: relations.first().ok_or("first relation")?.clone(),
            hypothesis: None,
        },
        RelationMultiple {
            multiplier: eight,
            relation: relations.get(1).ok_or("second relation")?.clone(),
            hypothesis: None,
        },
    ];
    push(fixture, step)
}

/// A real norm retains the exact complex selection and has no independently chosen conjugate slot.
pub(crate) fn positive_norm_cancellation(mutation: u8) -> TestResult<BinomialFixture> {
    let (mut fixture, map, mut root) = positive_norm_root()?;
    if mutation == 2 {
        let one = fixture.base.integer(1, false)?;
        let zero = fixture.base.integer(0, false)?;
        root = fixture
            .base
            .expression(Operator::Div, &[one, zero], false)?;
    }
    let roots = norm_joint_roots(&mut fixture, &map, &root, mutation)?;
    let bridge = norm_product_power(&mut fixture, &map, &roots)?;
    let root = roots.first().ok_or("first linked root")?.clone();
    let conjugate =
        fixture
            .base
            .expression(Operator::Conjugate, std::slice::from_ref(&root), false)?;
    let norm = fixture
        .base
        .expression(Operator::Mul, &[root, conjugate], false)?;
    let norm = real_projection(&mut fixture.base, &norm)?;
    fixture.base.field = Field::Real;
    let four = fixture.base.integer(4, false)?;
    let [source, factor, _] =
        positive_expressions(&mut fixture.base, &norm, &four, PositiveMutation::Valid)?;
    let square = fixture.base.expression(
        Operator::IntegerPower(2),
        std::slice::from_ref(&norm),
        false,
    )?;
    let mixed = fixture
        .base
        .expression(Operator::Mul, &[four.clone(), norm.clone()], false)?;
    let sixteen = fixture.base.integer(16, false)?;
    let tail = fixture
        .base
        .expression(Operator::Add, &[mixed, sixteen], false)?;
    let cofactor = fixture
        .base
        .expression(Operator::Add, &[square, tail], false)?;
    let zero = fixture.base.integer(0, false)?;
    let source_step = positive_equality(&fixture.base, &map, &source, &zero, &[bridge], &[])?;
    let source_index = push(&mut fixture, source_step)?;
    let product = fixture
        .base
        .expression(Operator::Mul, &[factor.clone(), cofactor], false)?;
    let expansion = positive_equality(&fixture.base, &map, &source, &product, &[], &[])?;
    let expansion_index = push(&mut fixture, expansion)?;
    let target = if mutation == 3 {
        fixture.base.field = Field::Complex;
        let second = roots.get(1).ok_or("changed norm slot")?.clone();
        let conjugate =
            fixture
                .base
                .expression(Operator::Conjugate, std::slice::from_ref(&second), false)?;
        let product = fixture
            .base
            .expression(Operator::Mul, &[second, conjugate], false)?;
        let norm = real_projection(&mut fixture.base, &product)?;
        fixture.base.field = Field::Real;
        fixture
            .base
            .expression(Operator::Sub, &[norm, four.clone()], false)?
    } else {
        factor
    };
    let cancellation = positive_equality(
        &fixture.base,
        &map,
        &target,
        &zero,
        &[source_index, expansion_index],
        &[],
    )?;
    let cancellation_index = push(&mut fixture, cancellation)?;
    let consumer = positive_equality(
        &fixture.base,
        &map,
        &norm,
        &four,
        &[cancellation_index],
        &[],
    )?;
    push(&mut fixture, consumer)?;
    Ok(fixture)
}

/// Test-only prefix replay returns the precise first refusal without printing or weakening it.
pub(crate) fn positive_norm_prefix() -> TestResult {
    let mut fixture = positive_norm_cancellation(0)?;
    for index in 0..fixture.steps.len() {
        let prefix = fixture.steps.get(..=index).ok_or("norm prefix")?;
        let current = fixture.steps.get(index).ok_or("norm step")?;
        let mut budget = Budget::new(Limits::blind_cubic())?;
        fixture.base.cas.check_proof(&current.context, prefix, &mut budget).map_err(|error| {
            format!("norm prefix index={index} rule={:?} conclusion={:?} fault={error:?} usage={:?}", current.rule, current.conclusion, budget.usage())
        })?;
    }
    Ok(())
}

/// The source is a named real odd root; the reverse mutation keeps a complex source instead.
fn included_root(mutation: u8) -> TestResult<(BinomialFixture, Handle)> {
    let mut base = Fixture::in_field(&Field::Complex)?;
    base.field = if mutation == 5 {
        Field::Complex
    } else {
        Field::Real
    };
    let domain = base.algebra.clone();
    let map = base.scalar_map(&domain, &[])?;
    let leading = base.coefficient(1)?;
    let constant = base.coefficient(-8)?;
    let polynomial = base.polynomial(&domain, &[(3, leading), (0, constant)])?;
    let (relation, selection, root) = base.root(&polynomial, &map)?;
    let unit = base.ground_unit()?;
    let mut existence = base.adjoin(&relation, &selection, 0);
    if mutation != 5 {
        let Certificate::Solve(certificate) = &mut existence.certificate else {
            return Err("real witness certificate".into());
        };
        let SolveCertificate::RootAdjunction(certificate) = certificate.as_mut() else {
            return Err("real witness adjunction".into());
        };
        certificate.mode = crate::test_api::solving::AdjunctionMode::RealOddRoot;
        certificate.order = Some(3);
    }
    Ok((
        BinomialFixture {
            base,
            steps: vec![unit, existence],
        },
        root,
    ))
}

/// A zero imaginary part is checked after inclusion; the false mutation asks for the root itself to vanish.
fn included_constraint(base: &mut Fixture, mutation: u8) -> TestResult<Handle> {
    let root = base.expression(Operator::Bound { depth: 0, slot: 0 }, &[], true)?;
    if mutation == 5 {
        return admit(
            &mut base.cas,
            &Draft::Condition {
                header: Reference::Existing(base.header.clone()),
                scope: vec![vec![AtomicSort::Scalar(Field::Real)]],
                kind: PredicateKind::Equal,
                arguments: vec![Reference::Existing(root.clone()), Reference::Existing(root)],
            },
        );
    }
    if mutation == 6 {
        let zero = base.integer(0, true)?;
        return predicate(base, PredicateKind::Equal, &[root, zero]);
    }
    let imaginary = real_expression(base, Operator::ImagPart, &[root])?;
    let zero = real_expression(base, Operator::Rational(Rational::zero()), &[])?;
    predicate(base, PredicateKind::Equal, &[imaginary, zero])
}

/// The target relation retains its own polynomial, ambient, region, and complete joint predicate.
fn included_selection(base: &mut Fixture, mutation: u8) -> TestResult<Handle> {
    base.field = if mutation == 5 {
        Field::Real
    } else {
        Field::Complex
    };
    let domain = base.algebra.clone();
    let map = base.scalar_map(&domain, &[])?;
    let leading = base.coefficient(1)?;
    let constant = base.coefficient(if mutation == 1 { -9 } else { -8 })?;
    let polynomial = base.polynomial(&domain, &[(3, leading), (0, constant)])?;
    let (relation, _, _) = base.root(&polynomial, &map)?;
    let constraint = included_constraint(base, mutation)?;
    admit(
        &mut base.cas,
        &Draft::Solve(SolveDraft::RootSelection(RootSelectionData {
            cell: None,
            header: Reference::Existing(base.header.clone()),
            scope: Vec::new(),
            relations: vec![Reference::Existing(relation)],
            predicate: Reference::Existing(constraint),
            mode: SelectionMode::Constrained,
            convention: None,
            interval: None,
        })),
    )
}

/// Public proof uses a checked real odd root as the actual complex binomial witness.
pub(crate) fn real_witness_selection(mutation: u8) -> TestResult<BinomialFixture> {
    let (mut fixture, root) = included_root(mutation)?;
    if mutation == 3 || mutation == 7 {
        if mutation == 7 {
            fixture.base.field = Field::Complex;
        }
        let region = fixture.base.coefficient_guard(true)?;
        if mutation == 7 {
            fixture.base.region = region;
        } else {
            fixture.base.enter_region(region)?;
        }
    }
    if mutation == 4 {
        fixture.base.enter_real()?;
    }
    let selected = included_selection(&mut fixture.base, mutation)?;
    let step = fixture.base.step(
        SolveSchema::RootSelectionRefinement,
        SolveStatement::RootExistence(StatementRootExistence {
            relation: selected.clone(),
            region: fixture.base.region.clone(),
        }),
        SolveCertificate::RootSelectionRefinement(CertificateRootSelectionRefinement {
            mode: RefinementMode::Existence,
            target_kind: RefinementTargetKind::Selection,
            target: selected,
            witnesses: vec![root],
            compatibility: Vec::new(),
            form: None,
        }),
    );
    push(&mut fixture, step)?;
    if mutation == 2 {
        fixture.steps.remove(1);
        fixture
            .steps
            .last_mut()
            .ok_or("real witness target")?
            .premises = vec![0];
    }
    Ok(fixture)
}
