// SPDX-License-Identifier: AGPL-3.0-only
//! Independent conditional claims, adversarial originals and literal local budgets.

use super::*;

mod adversarial;
mod registered;
use crate::budget::Limits;
use crate::domain_model::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
use crate::graph::{Draft, ExpressionDraft};
use crate::model::{AtomicSort, Branch, Declaration, Environment, Field, Operator, Sort};
use crate::{Rational, Reference};

/// Tests preserve structured production faults without panic conversion.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// One fixture owns all original syntax; the checker borrows it only after construction.
struct Fixture {
    /// Original graph owner.
    cas: Cas,
    /// Shared declaration header.
    header: Handle,
    /// Complete original conditional request.
    request: Request,
}

/// Ordinary original ledger, not a replacement inside a production call.
fn ledger() -> Result<Budget, Fault> {
    Budget::new(Limits::default())
}

/// The admitted scalar parameter sort.
fn real() -> Sort {
    Sort::Value(AtomicSort::Scalar(Field::Real))
}

/// Admit an original node without simplification or numerical evaluation.
fn term(
    cas: &mut Cas,
    header: &Handle,
    operator: Operator,
    sort: Sort,
    operands: &[Handle],
) -> TestResult<Handle> {
    let draft = Draft::Expression(ExpressionDraft {
        space: None,
        operator,
        sort,
        header: Reference::Existing(header.clone()),
        scope: Vec::new(),
        binding: None,
        operands: operands.iter().cloned().map(Reference::Existing).collect(),
    });
    cas.admit(&[draft], &mut ledger()?)?
        .into_iter()
        .next()
        .ok_or_else(|| "term".into())
}

/// Original declarations and the complete fibre conditional on q.
fn conditional_context() -> TestResult<(Cas, Handle, Handle, Vec<Handle>)> {
    let mut environment = Environment::default();
    for name in [
        "unused", "l", "c", "pl", "pu", "tl", "tu", "al", "au", "q", "r",
    ] {
        environment.declarations.push(Declaration {
            key: name.into(),
            display_name: name.into(),
            sort: if name == "q" || name == "r" {
                Sort::Value(AtomicSort::Boolean)
            } else {
                real()
            },
        });
    }
    let mut cas = Cas::new(Limits::default())?;
    let drafts = [
        Draft::Header(HeaderDraft::Core(environment)),
        Draft::Region(RegionDraft {
            header: Reference::Local(0),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::Ambient,
        }),
    ];
    let [header, ambient]: [Handle; 2] = cas
        .admit(&drafts, &mut ledger()?)?
        .try_into()
        .map_err(|_| "header")?;
    let q = term(
        &mut cas,
        &header,
        Operator::Free("q".into()),
        Sort::Value(AtomicSort::Boolean),
        &[],
    )?;
    let r = term(
        &mut cas,
        &header,
        Operator::Free("r".into()),
        Sort::Value(AtomicSort::Boolean),
        &[],
    )?;
    let [_, context]: [Handle; 2] = cas
        .admit(
            &[
                Draft::Region(RegionDraft {
                    header: Reference::Existing(header.clone()),
                    scope: Vec::new(),
                    ambient: AmbientDomain::Assignments,
                    expression: RegionExpr::GuardTrue {
                        base: Reference::Existing(ambient),
                        predicate: Reference::Existing(q.clone()),
                    },
                }),
                Draft::Context(ContextDraft::Root {
                    header: Reference::Existing(header.clone()),
                    admissible: Reference::Local(0),
                    assumptions: Vec::new(),
                }),
            ],
            &mut ledger()?,
        )?
        .try_into()
        .map_err(|_| "context")?;
    Ok((cas, header, context, vec![q.clone(), r, q]))
}

/// Original positions include one unused input and one repeat, without erasing the fibre.
fn fixture() -> TestResult<Fixture> {
    let (mut cas, header, context, constraints) = conditional_context()?;
    let mut variables = Vec::new();
    for name in ["unused", "l", "c", "pl", "pu", "tl", "tu", "al", "au"] {
        variables.push(term(
            &mut cas,
            &header,
            Operator::Free(name.into()),
            real(),
            &[],
        )?);
    }
    let inputs = vec![
        variables[0].clone(),
        variables[1].clone(),
        variables[2].clone(),
        variables[1].clone(),
    ];
    let endpoints: [Handle; 6] = variables[3..]
        .to_vec()
        .try_into()
        .map_err(|_| "endpoints")?;
    let product = term(&mut cas, &header, Operator::Mul, real(), &inputs[1..3])?;
    let half = term(
        &mut cas,
        &header,
        Operator::Rational(Rational::from_ratio(1, 2, &mut ledger()?)?),
        real(),
        &[],
    )?;
    let root = term(
        &mut cas,
        &header,
        Operator::GeneralPower(Branch::StrictReal),
        real(),
        &[product.clone(), half],
    )?;
    let one = term(
        &mut cas,
        &header,
        Operator::Rational(Rational::one()),
        real(),
        &[],
    )?;
    let reciprocal = term(
        &mut cas,
        &header,
        Operator::Div,
        real(),
        &[one, root.clone()],
    )?;
    let square = term(
        &mut cas,
        &header,
        Operator::Mul,
        real(),
        &[root.clone(), root.clone()],
    )?;
    let reverse = term(
        &mut cas,
        &header,
        Operator::Sub,
        real(),
        &[square, product.clone()],
    )?;
    Ok(Fixture {
        cas,
        header,
        request: Request {
            context,
            inputs,
            constraints,
            positions: [1, 2],
            endpoints,
            targets: [product, root, reciprocal, reverse],
            arithmetic: 1,
        },
    })
}

/// Construct a proposal outside the independently consumed checker ledger.
fn proposed(f: &Fixture) -> Result<Candidate, Fault> {
    claim::construct(&f.cas, &f.request, &mut ledger()?, &mut Events::default())
}

/// The independent expected inventory keeps all original constraints and conditional fibres.
/// ´test:unit:lc-root-family-expected-claim´
/// ´claim:cas:lc-root-family-expected-claim´
#[test]
fn lc_root_family_expected_claim() -> TestResult {
    let f = fixture()?;
    let p = proposed(&f)?;
    let checked = check::verify(
        &f.cas,
        &f.request,
        &p,
        &mut ledger()?,
        &mut Events::default(),
    )?;
    assert!(std::ptr::eq(checked.owner, std::ptr::from_ref(&f.cas)));
    assert!(std::ptr::eq(checked.candidate, std::ptr::from_ref(&p)));
    assert_eq!(p.original, f.request);
    assert_eq!(
        p.universal,
        f.request.inputs[..3]
            .iter()
            .chain(&f.request.endpoints)
            .cloned()
            .collect::<Vec<_>>()
    );
    assert_eq!(
        p.premises[2],
        Predicate(Term::Zero, Relation::Less, Term::Endpoint(0))
    );
    assert_eq!(
        p.premises[5],
        Predicate(Term::Zero, Relation::Less, Term::Endpoint(2))
    );
    assert_eq!(
        p.premises[7],
        Predicate(Term::Square(2), Relation::AtMost, Term::Endpoint(0))
    );
    assert_eq!(
        p.premises[8],
        Predicate(Term::Endpoint(1), Relation::AtMost, Term::Square(3))
    );
    assert_eq!(
        p.conclusions[2],
        Predicate(Term::Reverse, Relation::Equal, Term::Zero)
    );
    assert_eq!(
        p.native,
        [
            Native::ProductTotal,
            Native::RootTotal,
            Native::ReciprocalTotal,
            Native::ReciprocalLocalOperation
        ]
    );
    assert_eq!(
        p.expansion,
        [
            Step::ProductPositive,
            Step::RootUnique,
            Step::RootBracket,
            Step::ReciprocalOrder,
            Step::ReciprocalBracket
        ]
    );
    Ok(())
}

/// Reject a changed claim without letting a resource error disguise a semantic refusal.
fn refused(f: &Fixture, expected: &Request, p: &Candidate) -> TestResult {
    let error = check::verify(&f.cas, expected, p, &mut ledger()?, &mut Events::default())
        .err()
        .ok_or("accepted mutation")?;
    assert!(
        matches!(error.kind, crate::FaultKind::Invalid(_)),
        "{error:?}"
    );
    Ok(())
}

/// Every premise, conclusion, native obligation and proof step is checked, not just the recipe key.
/// ´test:unit:lc-root-family-mutations´
/// ´claim:cas:lc-root-family-mutations´
#[test]
fn lc_root_family_mutations() -> TestResult {
    let f = fixture()?;
    let original = proposed(&f)?;
    for index in 0..11 {
        let mut p = original.clone();
        p.premises[index] = Predicate(Term::Zero, Relation::Equal, Term::Zero);
        refused(&f, &f.request, &p)?;
    }
    for index in 0..7 {
        let mut p = original.clone();
        p.conclusions[index].0 = Term::Inverse(0);
        refused(&f, &f.request, &p)?;
    }
    for index in 0..4 {
        let mut p = original.clone();
        p.native.swap(index, (index + 1) % 4);
        refused(&f, &f.request, &p)?;
    }
    for index in 0..5 {
        let mut p = original.clone();
        p.expansion.swap(index, (index + 1) % 5);
        refused(&f, &f.request, &p)?;
    }
    for index in [0, 1, 2, 5] {
        let mut p = original.clone();
        p.premises[index].1 = Relation::AtMost;
        refused(&f, &f.request, &p)?;
    }
    let mut p = original.clone();
    p.universal.swap(1, 2);
    refused(&f, &f.request, &p)?;
    let mut p = original.clone();
    p.universal.pop();
    refused(&f, &f.request, &p)?;
    let mut p = original.clone();
    p.original.constraints.swap(0, 1);
    refused(&f, &f.request, &p)?;
    let mut p = original.clone();
    p.original.constraints.pop();
    refused(&f, &f.request, &p)?;
    let mut p = original.clone();
    p.original.inputs.swap(0, 3);
    refused(&f, &f.request, &p)?;
    let mut p = original.clone();
    p.original.positions.swap(0, 1);
    refused(&f, &f.request, &p)?;
    let mut p = original.clone();
    p.original.arithmetic = 2;
    let error = check::verify(
        &f.cas,
        &f.request,
        &p,
        &mut ledger()?,
        &mut Events::default(),
    )
    .err()
    .ok_or("accepted arithmetic revision")?;
    assert_eq!(
        error.kind,
        crate::FaultKind::Invalid(InvalidCode::UnsupportedVersion)
    );
    let mut p = original;
    p.version = 2;
    refused(&f, &f.request, &p)?;
    Ok(())
}

/// Wrong roots, sign-flipped targets and swapped Product operands are refused before any family claim.
/// ´test:unit:lc-root-family-original-refusals´
/// ´claim:cas:lc-root-family-original-refusals´
#[test]
fn lc_root_family_original_refusals() -> TestResult {
    let mut budget = crate::testing::common::default_budget()?;
    let mut f = fixture()?;
    let p = proposed(&f)?;
    let wrong = term(
        &mut f.cas,
        &f.header,
        Operator::Neg,
        real(),
        &[f.request.targets[1].clone()],
    )?;
    let swapped = term(
        &mut f.cas,
        &f.header,
        Operator::Mul,
        real(),
        &[f.request.inputs[2].clone(), f.request.inputs[1].clone()],
    )?;
    let flipped = term(
        &mut f.cas,
        &f.header,
        Operator::Neg,
        real(),
        &[f.request.targets[2].clone()],
    )?;
    for (slot, target) in [
        (1, wrong),
        (0, swapped),
        (2, flipped),
        (1, f.request.targets[3].clone()),
    ] {
        let mut request = f.request.clone();
        request.targets[slot] = target;
        assert!(
            claim::construct(&f.cas, &request, &mut ledger()?, &mut Events::default()).is_err()
        );
        refused(&f, &request, &p)?;
    }
    let mut request = f.request.clone();
    request.positions.swap(0, 1);
    refused(&f, &request, &p)?;
    let mut request = f.request.clone();
    request.endpoints[0] = request.inputs[0].clone();
    refused(&f, &request, &p)?;
    let foreign = fixture()?;
    refused(&f, &foreign.request, &proposed(&foreign)?)?;
    // Corrupt original exponent metadata in place: replay must not trust a cached shape flag.
    let root = f.cas.checked(&f.request.targets[1], &mut budget)?;
    let exponent = f.cas.state.expression(root, &mut budget)?.operands[1];
    let index = f
        .cas
        .state
        .indices
        .peek(&exponent)
        .copied()
        .ok_or("index missing")?;
    if let crate::graph::Vertex::Expression(value) = &mut f.cas.state.graph[index] {
        value.operator = Operator::Rational(Rational::from_ratio(1, 3, &mut ledger()?)?);
    } else {
        return Err("exponent".into());
    }
    refused(&f, &f.request, &p)?;
    Ok(())
}

/// Receipt identities from other judgments never select the family checker law.
/// ´test:unit:lc-root-family-receipt-separation´
/// ´claim:cas:lc-root-family-receipt-separation´
#[test]
fn lc_root_family_receipt_separation() -> TestResult {
    let f = fixture()?;
    for recipe in [
        "enclosure.root-grid",
        "positive-root-sensitivity",
        "lc-root-ground-witness",
    ] {
        let mut p = proposed(&f)?;
        p.recipe = recipe;
        refused(&f, &f.request, &p)?;
    }
    Ok(())
}

/// Local record construction and rechecking have independent exact Work and storage neighbours.
/// ´test:unit:lc-root-family-local-boundaries´
/// ´claim:cas:lc-root-family-local-boundaries´
#[test]
fn lc_root_family_local_boundaries() -> TestResult {
    let f = fixture()?;
    let mut build = ledger()?;
    let mut build_events = Events::default();
    let p = claim::construct(&f.cas, &f.request, &mut build, &mut build_events)?;
    let mut replay = ledger()?;
    let mut replay_events = Events::default();
    let _checked = check::verify(&f.cas, &f.request, &p, &mut replay, &mut replay_events)?;
    // Original visits: 2 context/region + 10 input/endpoint + 3 constraint + 7 target = 22.
    // Endpoint distinctness: 6*4 input comparisons + 15 prior endpoints = 39.
    // Construction quantifiers: 10 candidate visits + 38 prior comparisons = 48;
    // copies: 9 binders + 7 vector entries + 11 fixed handles = 27.
    // Recheck adds 22 original-field and 10 binder comparisons to 39; its quantifier
    // traversal visits 10 candidates and 5 prior original positions, without copying.
    assert_eq!(
        (build.usage().work, build_events.0),
        (163, [22, 39, 48, 27, 22, 5])
    );
    assert_eq!(
        (replay.usage().work, replay_events.0),
        (135, [22, 71, 15, 0, 22, 5])
    );
    // Request: eleven 24-byte handles + two 24-byte Vec headers + 16 positions + 4 revision
    // = 332, padded to 336. Candidate: 336 request + 16 name + 24 Vec + 4 version
    // + 18*5 predicates + 4 native obligations + 5 steps = 479, padded to 480.
    assert_eq!(
        (
            size_of::<Term>(),
            size_of::<Relation>(),
            size_of::<Predicate>()
        ),
        (2, 1, 5)
    );
    assert_eq!((size_of::<Native>(), size_of::<Step>()), (1, 1));
    assert_eq!(
        (
            size_of::<Handle>(),
            size_of::<Request>(),
            size_of::<Candidate>(),
            size_of::<Checked<'_>>()
        ),
        (24, 336, 480, 16)
    );
    // Original request: 336 + seven handles = 504. Candidate: 480 + seventeen handles
    // (four inputs, three constraints, ten reserved binders) = 888. Recheck adds two borrows.
    assert_eq!((request_bytes(&f.request), bytes(&f.request)), (504, 888));
    assert_eq!(candidate_bytes(&p), 888);
    for (construct, work, storage) in [(true, 163, 1392), (false, 135, 1408)] {
        for (work_limit, byte_limit, success, spent) in [
            (work, storage, true, work),
            (work - 1, storage, false, work - 1),
            (work, storage - 1, false, 0),
        ] {
            let mut budget = Budget::new(Limits {
                work: work_limit + 7,
                staged_payload_bytes: byte_limit,
                ..Limits::default()
            })?;
            budget.work(7, Location::new("caller prefix"))?;
            let mut events = Events::default();
            let result = if construct {
                claim::construct(&f.cas, &f.request, &mut budget, &mut events).map(|_| ())
            } else {
                check::verify(&f.cas, &f.request, &p, &mut budget, &mut events).map(|_| ())
            };
            assert_eq!(result.is_ok(), success, "{result:?}");
            assert_eq!(budget.usage().work, spent + 7);
        }
    }
    Ok(())
}

/// Squared endpoint and reciprocal order checks use exact rational arithmetic, not native floats.
/// ´test:unit:lc-root-family-endpoint-oracles´
/// ´claim:cas:lc-root-family-endpoint-oracles´
#[test]
fn lc_root_family_endpoint_oracles() -> TestResult {
    use std::cmp::Ordering;
    for lower in 1..=8_i64 {
        for upper in lower..=9_i64 {
            let mut budget = ledger()?;
            let pl = Rational::from_ratio(lower * lower, 4, &mut budget)?;
            let pu = Rational::from_ratio(upper * upper, 4, &mut budget)?;
            let tl = Rational::from_ratio(lower, 2, &mut budget)?;
            let tu = Rational::from_ratio(upper, 2, &mut budget)?;
            let input = super::super::Interval::new(pl.clone(), pu.clone(), &mut budget)?;
            let root = super::super::Interval::new(tl.clone(), tu.clone(), &mut budget)?;
            super::super::roots::check(&input, &root, 1, &mut budget)?;
            assert_eq!(tl.mul(&tl, &mut budget)?, pl);
            assert_eq!(tu.mul(&tu, &mut budget)?, pu);
            let al = Rational::from_ratio(2, u64::try_from(upper)?, &mut budget)?;
            let au = Rational::from_ratio(2, u64::try_from(lower)?, &mut budget)?;
            assert_eq!(al.mul(&tu, &mut budget)?, Rational::one());
            assert_eq!(au.mul(&tl, &mut budget)?, Rational::one());
            assert_ne!(al.compare(&au, &mut budget)?, Ordering::Greater);
            // Integer inequalities independently give the same antitone ordering.
            assert!(2 * lower <= 2 * upper);
            let negative =
                super::super::Interval::new(tl.negated(&mut budget)?, tu.clone(), &mut budget)?;
            assert!(super::super::roots::check(&input, &negative, 1, &mut budget).is_err());
        }
    }
    Ok(())
}
