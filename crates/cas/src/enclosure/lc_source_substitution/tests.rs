// SPDX-License-Identifier: AGPL-3.0-only
//! Independent original DAGs and receiving-store mutation controls.

use super::*;
use crate::budget::{Budget, Limits};
use crate::domain_model::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
use crate::enclosure::lc_root;
use crate::graph::{Draft, ExpressionDraft};
use crate::model::{AtomicSort, Branch, Declaration, Environment, Field, Operator, Sort};
use crate::{Cas, Fault, Handle, Rational, Reference};

/// Tests preserve structured production faults without panic conversion.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// One fixture owns all original syntax; the checker borrows it only after construction.
struct Fixture {
    /// Original graph owner.
    cas: Cas,
    /// Shared declaration header.
    header: Handle,
    /// Complete original conditional request.
    request: lc_root::Request,
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
        request: lc_root::Request {
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

fn mapped_targets(
    cas: &mut Cas,
    header: &Handle,
    l: &Handle,
    c: &Handle,
) -> Result<[Handle; 4], Box<dyn std::error::Error>> {
    let product = term(cas, header, Operator::Mul, real(), &[l.clone(), c.clone()])?;
    let half = term(
        cas,
        header,
        Operator::Rational(Rational::from_ratio(1, 2, &mut ledger()?)?),
        real(),
        &[],
    )?;
    let root = term(
        cas,
        header,
        Operator::GeneralPower(Branch::StrictReal),
        real(),
        &[product.clone(), half],
    )?;
    let one = term(
        cas,
        header,
        Operator::Rational(Rational::one()),
        real(),
        &[],
    )?;
    let reciprocal = term(cas, header, Operator::Div, real(), &[one, root.clone()])?;
    let square = term(
        cas,
        header,
        Operator::Mul,
        real(),
        &[root.clone(), root.clone()],
    )?;
    let reverse = term(
        cas,
        header,
        Operator::Sub,
        real(),
        &[square, product.clone()],
    )?;
    Ok([product, root, reciprocal, reverse])
}

fn source_fixture(
    tag: Tag,
) -> Result<(Cas, Request, lc_root::Receipt), Box<dyn std::error::Error>> {
    let (mut cas, request) = source_request_fixture(tag)?;
    let root = cas.construct_lc_root(&request.root, &mut ledger()?)?;
    Ok((cas, request, root))
}

/// Admit the same original syntax without pre-publishing a root judgment.
fn source_request_fixture(tag: Tag) -> TestResult<(Cas, Request)> {
    let mut f = fixture()?;
    let mut substitution = f.request.inputs.clone();
    let targets = if tag == Tag::MappedCover {
        let one = term(
            &mut f.cas,
            &f.header,
            Operator::Rational(Rational::one()),
            real(),
            &[],
        )?;
        let mapped = term(
            &mut f.cas,
            &f.header,
            Operator::Add,
            real(),
            &[f.request.inputs[1].clone(), one],
        )?;
        substitution[1] = mapped.clone();
        substitution[3] = mapped.clone();
        mapped_targets(&mut f.cas, &f.header, &mapped, &f.request.inputs[2])?
    } else {
        f.request.targets.clone()
    };
    let request = Request {
        root: f.request.clone(),
        context: f.request.context.clone(),
        sources: f.request.inputs.clone(),
        constraints: f.request.constraints.clone(),
        substitution,
        targets,
        tag,
        revision: 1,
    };
    Ok((f.cas, request))
}

/// Source and target correspond to original DAGs, not equal-valued replacement inputs.
#[test]
fn lc_source_substitution_expected_claim() -> TestResult {
    for tag in [Tag::Box, Tag::MappedCover] {
        let (cas, request, root) = source_fixture(tag)?;
        let receipt = cas.construct_lc_source_substitution(&request, &root, &mut ledger()?)?;
        assert_eq!(receipt.request(&cas, &mut ledger()?)?, &request);
        assert_eq!(receipt.formal_schema().name, "lc-source-substitution");
        assert_eq!(receipt.formal_schema().version, 1);
    }
    Ok(())
}

/// Build an independent receiver from the original fixture; proposal bytes grant no authority.
#[test]
fn lc_source_substitution_registered_replay() -> TestResult {
    for tag in [Tag::Box, Tag::MappedCover] {
        let (cas, request, root) = source_fixture(tag)?;
        let receipt = cas.construct_lc_source_substitution(&request, &root, &mut ledger()?)?;
        let certificate = receipt.certificate(&cas, &mut ledger()?)?;
        let (mut receiver, expected) = source_request_fixture(tag)?;
        assert_eq!(receiver.retained_judgment_count(), 0);
        let checked =
            receiver.check_lc_source_substitution(&expected, &certificate, &mut ledger()?)?;
        assert_eq!(checked.request(&receiver, &mut ledger()?)?, &expected);
        assert_eq!(receiver.retained_judgment_count(), 1);
    }
    Ok(())
}

/// A failed composite replay rolls back any root judgment published during the attempt.
#[test]
fn lc_source_substitution_replay_atomicity() -> TestResult {
    let (cas, request, root) = source_fixture(Tag::Box)?;
    let certificate = cas
        .construct_lc_source_substitution(&request, &root, &mut ledger()?)?
        .certificate(&cas, &mut ledger()?)?;
    let (mut measured_receiver, expected) = source_request_fixture(Tag::Box)?;
    assert_eq!(measured_receiver.retained_judgment_count(), 0);
    let mut measured = ledger()?;
    measured_receiver.check_lc_source_substitution(&expected, &certificate, &mut measured)?;
    let work = measured.usage().work;
    assert_eq!(measured_receiver.retained_judgment_count(), 1);
    let (mut receiver, expected) = source_request_fixture(Tag::Box)?;
    assert_eq!(receiver.retained_judgment_count(), 0);
    let mut short = Budget::new(Limits {
        work: work - 1,
        ..Limits::default()
    })?;
    assert!(
        receiver
            .check_lc_source_substitution(&expected, &certificate, &mut short)
            .is_err()
    );
    assert_eq!(receiver.retained_judgment_count(), 0);
    receiver.check_lc_source_substitution(&expected, &certificate, &mut ledger()?)?;
    assert_eq!(receiver.retained_judgment_count(), 1);
    Ok(())
}

/// Malformed expected root positions refuse before indexing during composite replay.
#[test]
fn lc_source_substitution_replay_bad_positions() -> TestResult {
    let (cas, request, root) = source_fixture(Tag::Box)?;
    let certificate = cas
        .construct_lc_source_substitution(&request, &root, &mut ledger()?)?
        .certificate(&cas, &mut ledger()?)?;
    let (mut receiver, mut expected) = source_request_fixture(Tag::Box)?;
    expected.root.positions[1] = expected.root.inputs.len();
    assert!(
        receiver
            .check_lc_source_substitution(&expected, &certificate, &mut ledger()?)
            .is_err()
    );
    assert_eq!(receiver.retained_judgment_count(), 0);
    Ok(())
}

/// Each independent expected original component is part of the checked statement.
#[test]
fn lc_source_substitution_mutations() -> TestResult {
    let (cas, request, root) = source_fixture(Tag::MappedCover)?;
    let certificate = cas
        .construct_lc_source_substitution(&request, &root, &mut ledger()?)?
        .certificate(&cas, &mut ledger()?)?;
    let (mut receiver, expected, _) = source_fixture(Tag::MappedCover)?;
    let mut changed = expected.clone();
    changed.sources.swap(0, 1);
    assert!(
        receiver
            .check_lc_source_substitution(&changed, &certificate, &mut ledger()?)
            .is_err()
    );
    let mut changed = expected.clone();
    changed.substitution.swap(1, 2);
    assert!(
        receiver
            .check_lc_source_substitution(&changed, &certificate, &mut ledger()?)
            .is_err()
    );
    let mut changed = expected.clone();
    changed.targets.swap(0, 1);
    assert!(
        receiver
            .check_lc_source_substitution(&changed, &certificate, &mut ledger()?)
            .is_err()
    );
    let mut changed = expected.clone();
    changed.constraints.swap(0, 1);
    assert!(
        receiver
            .check_lc_source_substitution(&changed, &certificate, &mut ledger()?)
            .is_err()
    );
    let mut changed = expected;
    changed.tag = Tag::Box;
    assert!(
        receiver
            .check_lc_source_substitution(&changed, &certificate, &mut ledger()?)
            .is_err()
    );
    Ok(())
}

/// A source operation outside the admitted grammar has no accelerated receipt.
#[test]
fn lc_source_substitution_fallback_boundary() -> TestResult {
    let mut f = fixture()?;
    let root = f.cas.construct_lc_root(&f.request, &mut ledger()?)?;
    let negated = term(
        &mut f.cas,
        &f.header,
        Operator::Neg,
        real(),
        &[f.request.inputs[1].clone()],
    )?;
    let mut substitution = f.request.inputs.clone();
    substitution[1] = negated.clone();
    substitution[3] = negated.clone();
    let targets = mapped_targets(&mut f.cas, &f.header, &negated, &f.request.inputs[2])?;
    let request = Request {
        root: f.request.clone(),
        context: f.request.context.clone(),
        sources: f.request.inputs.clone(),
        constraints: f.request.constraints.clone(),
        substitution,
        targets,
        tag: Tag::MappedCover,
        revision: 1,
    };
    assert!(
        f.cas
            .construct_lc_source_substitution(&request, &root, &mut ledger()?)
            .is_err()
    );
    let divided = term(
        &mut f.cas,
        &f.header,
        Operator::Div,
        real(),
        &[f.request.inputs[1].clone(), f.request.inputs[2].clone()],
    )?;
    let mut unsigned = request;
    unsigned.substitution[1] = divided.clone();
    unsigned.substitution[3] = divided.clone();
    unsigned.targets = mapped_targets(&mut f.cas, &f.header, &divided, &f.request.inputs[2])?;
    assert!(
        f.cas
            .construct_lc_source_substitution(&unsigned, &root, &mut ledger()?)
            .is_err()
    );
    Ok(())
}

/// The same original request has exact and one-below caller-ledger boundaries.
#[test]
fn lc_source_substitution_resource_boundaries() -> TestResult {
    for tag in [Tag::Box, Tag::MappedCover] {
        let (cas, request, root) = source_fixture(tag)?;
        let mut measured = ledger()?;
        let receipt = cas.construct_lc_source_substitution(&request, &root, &mut measured)?;
        let work = measured.usage().work;
        let bytes = receipt.retained_bytes();
        assert!(work > 0 && bytes > 0);
        let mut exact = Budget::new(Limits {
            work,
            output_bytes: bytes,
            ..Limits::default()
        })?;
        cas.construct_lc_source_substitution(&request, &root, &mut exact)?;
        assert_eq!(exact.usage().work, work);
        let mut one_below_work = Budget::new(Limits {
            work: work - 1,
            ..Limits::default()
        })?;
        assert!(
            cas.construct_lc_source_substitution(&request, &root, &mut one_below_work)
                .is_err()
        );
        assert!(one_below_work.usage().work > 0);
        assert!(one_below_work.usage().work < work);
        let mut one_below_bytes = Budget::new(Limits {
            output_bytes: bytes - 1,
            ..Limits::default()
        })?;
        assert!(
            cas.construct_lc_source_substitution(&request, &root, &mut one_below_bytes)
                .is_err()
        );
    }
    Ok(())
}

/// Semantic mutations run through the checker, before any canonical-scope comparison.
#[test]
fn lc_source_substitution_checker_controls() -> TestResult {
    let (mut cas, request, root) = source_fixture(Tag::Box)?;
    let mut changed = request.clone();
    changed.constraints.clear();
    assert!(
        cas.construct_lc_source_substitution(&changed, &root, &mut ledger()?)
            .is_err()
    );
    let header = cas.context_header(&request.context, &mut ledger()?)?;
    let ambient = cas
        .admit(
            &[Draft::Region(RegionDraft {
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Assignments,
                expression: RegionExpr::Ambient,
            })],
            &mut ledger()?,
        )?
        .remove(0);
    let alternate = cas
        .admit(
            &[Draft::Context(ContextDraft::Root {
                header: Reference::Existing(header.clone()),
                admissible: Reference::Existing(ambient),
                assumptions: Vec::new(),
            })],
            &mut ledger()?,
        )?
        .remove(0);
    changed = request.clone();
    changed.context = alternate;
    assert!(
        cas.construct_lc_source_substitution(&changed, &root, &mut ledger()?)
            .is_err()
    );
    changed = request.clone();
    let pl = request.root.endpoints[0].clone();
    changed.sources.push(pl.clone());
    changed.substitution[1] = pl.clone();
    changed.substitution[3] = pl.clone();
    changed.targets = mapped_targets(&mut cas, &header, &pl, &request.root.inputs[2])?;
    assert!(
        cas.construct_lc_source_substitution(&changed, &root, &mut ledger()?)
            .is_err()
    );
    changed = request.clone();
    changed.sources[0] = request.root.endpoints[0].clone();
    assert!(
        cas.construct_lc_source_substitution(&changed, &root, &mut ledger()?)
            .is_err()
    );
    changed = request.clone();
    changed.substitution[0] = request.root.endpoints[0].clone();
    assert!(
        cas.construct_lc_source_substitution(&changed, &root, &mut ledger()?)
            .is_err()
    );
    changed = request.clone();
    changed.substitution[request.root.positions[1]] =
        changed.substitution[request.root.positions[0]].clone();
    changed.targets = mapped_targets(
        &mut cas,
        &header,
        &changed.substitution[1],
        &changed.substitution[2],
    )?;
    assert!(
        cas.construct_lc_source_substitution(&changed, &root, &mut ledger()?)
            .is_err()
    );
    let constant = term(
        &mut cas,
        &header,
        Operator::Rational(Rational::one()),
        real(),
        &[],
    )?;
    changed = request.clone();
    changed.substitution[1] = constant.clone();
    changed.substitution[3] = constant.clone();
    changed.targets = mapped_targets(&mut cas, &header, &constant, &request.root.inputs[2])?;
    assert!(
        cas.construct_lc_source_substitution(&changed, &root, &mut ledger()?)
            .is_err()
    );
    changed = request;
    changed.substitution[3] = changed.substitution[2].clone();
    assert!(
        cas.construct_lc_source_substitution(&changed, &root, &mut ledger()?)
            .is_err()
    );
    Ok(())
}

/// Portable format controls distinguish the outer framing from the root transcript.
#[test]
fn lc_source_substitution_wire_controls() -> TestResult {
    let (cas, request, root) = source_fixture(Tag::Box)?;
    let receipt = cas.construct_lc_source_substitution(&request, &root, &mut ledger()?)?;
    let good = receipt
        .certificate(&cas, &mut ledger()?)?
        .to_bytes(&mut ledger()?)?;
    assert!(good.starts_with(b"NRZLSS01"));
    let scope = wire::scope(&cas, &request, &mut ledger()?)?;
    assert_eq!(
        &scope[..29],
        &[
            b"NRZLCS01".as_slice(),
            &1_u64.to_le_bytes(),
            &[0],
            &4_u32.to_le_bytes(),
            &3_u32.to_le_bytes(),
            &4_u32.to_le_bytes()
        ]
        .concat()
    );
    assert_eq!(
        &good[..12],
        &[
            b"NRZLSS01".as_slice(),
            &(u32::try_from(scope.len())?).to_le_bytes()
        ]
        .concat()
    );
    assert!(good.windows(scope.len()).any(|part| part == scope));
    let (mapped_cas, mapped, _) = source_fixture(Tag::MappedCover)?;
    let mapped_scope = wire::scope(&mapped_cas, &mapped, &mut ledger()?)?;
    assert_eq!(mapped_scope[16], 1);

    let mut changed = good.clone();
    changed[0] ^= 1;
    assert!(wire::decode(&changed, &scope, &mut ledger()?).is_err());
    changed = good.clone();
    changed.push(0);
    assert!(wire::decode(&changed, &scope, &mut ledger()?).is_err());
    let mut revision = request;
    revision.revision = 2;
    assert!(
        cas.construct_lc_source_substitution(&revision, &root, &mut ledger()?)
            .is_err()
    );
    let mut changed = good;
    *changed.last_mut().ok_or("empty wire")? ^= 1;
    let (mut receiver, expected, _) = source_fixture(Tag::Box)?;
    assert!(
        receiver
            .check_lc_source_substitution(
                &expected,
                &Certificate::from_bytes(&changed, &mut ledger()?)?,
                &mut ledger()?
            )
            .is_err()
    );
    Ok(())
}

/// Domain predicates, leaf declarations and exact root exponent are checked as syntax.
#[test]
fn lc_source_substitution_map_premises() -> TestResult {
    let (mut cas, request, root) = source_fixture(Tag::MappedCover)?;
    let header = cas.context_header(&request.context, &mut ledger()?)?;
    let l = request.root.inputs[1].clone();
    let c = request.root.inputs[2].clone();
    let positive_l = term(
        &mut cas,
        &header,
        Operator::Predicate(crate::model::PredicateKind::Positive),
        Sort::Value(AtomicSort::Boolean),
        std::slice::from_ref(&l),
    )?;
    let positive_c = term(
        &mut cas,
        &header,
        Operator::Predicate(crate::model::PredicateKind::Positive),
        Sort::Value(AtomicSort::Boolean),
        std::slice::from_ref(&c),
    )?;
    let half = term(
        &mut cas,
        &header,
        Operator::Rational(Rational::from_ratio(1, 2, &mut ledger()?)?),
        real(),
        &[],
    )?;
    let two = term(
        &mut cas,
        &header,
        Operator::Rational(Rational::from_ratio(2, 1, &mut ledger()?)?),
        real(),
        &[],
    )?;
    for (operator, operands) in [
        (Operator::Div, vec![l.clone(), c.clone()]),
        (Operator::Mul, vec![l.clone(), c.clone()]),
        (
            Operator::GeneralPower(Branch::StrictReal),
            vec![l.clone(), half],
        ),
    ] {
        let mapped = term(&mut cas, &header, operator, real(), &operands)?;
        let mut changed = request.clone();
        changed.substitution[1] = mapped.clone();
        changed.substitution[3] = mapped.clone();
        changed.targets = mapped_targets(&mut cas, &header, &mapped, &c)?;
        assert!(
            cas.construct_lc_source_substitution(&changed, &root, &mut ledger()?)
                .is_err()
        );
        changed
            .constraints
            .extend([positive_l.clone(), positive_c.clone()]);
        assert!(
            cas.construct_lc_source_substitution(&changed, &root, &mut ledger()?)
                .is_ok()
        );
    }
    let bad_root = term(
        &mut cas,
        &header,
        Operator::GeneralPower(Branch::StrictReal),
        real(),
        &[l, two],
    )?;
    let mut changed = request.clone();
    changed.constraints.extend([positive_l, positive_c]);
    changed.substitution[1] = bad_root.clone();
    changed.substitution[3] = bad_root.clone();
    changed.targets = mapped_targets(&mut cas, &header, &bad_root, &c)?;
    assert!(
        cas.construct_lc_source_substitution(&changed, &root, &mut ledger()?)
            .is_err()
    );
    changed = request;
    changed.sources.remove(0);
    assert!(
        cas.construct_lc_source_substitution(&changed, &root, &mut ledger()?)
            .is_err()
    );
    Ok(())
}
