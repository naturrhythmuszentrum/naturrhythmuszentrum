// SPDX-License-Identifier: AGPL-3.0-only
//! Independent original DAGs and receiving-store mutation controls.

use super::*;
use crate::budget::{Budget, Limits};
use crate::domain_model::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
use crate::enclosure::lc_root;
use crate::exact_cache::{ExactCacheSession, ExactWarmIndex};
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

/// A second root request authenticates the retained graph judgment without publication.
/// ´test:unit:exact-cache-root-mutable-second-preparation´
#[test]
fn exact_cache_root_mutable_second_preparation() -> TestResult {
    let (mut cas, request) = source_request_fixture(Tag::Box)?;
    let mut session = ExactCacheSession::new();
    let mut cold = ledger()?;
    let first = cas.construct_lc_root_indexed(&mut session, &request.root, &mut cold)?;
    let first_serial = session.last_serial();
    let before = cas.counts();
    let mut warm = ledger()?;
    let second = cas.construct_lc_root_indexed(&mut session, &request.root, &mut warm)?;
    assert_eq!(first.judgment, second.judgment);
    assert_eq!(cas.counts(), before);
    assert_eq!(session.len(), 1);
    assert_eq!(session.last_serial(), first_serial);
    assert!(warm.usage().work < cold.usage().work);
    assert_eq!(warm.usage().work, 163_994);
    assert_eq!(session.retained_bytes(), 6_949);
    Ok(())
}

/// A second source substitution keeps the same particular root dependency and graph node.
/// ´test:unit:exact-cache-substitution-mutable-second-preparation´
#[test]
fn exact_cache_substitution_mutable_second_preparation() -> TestResult {
    let (mut cas, request) = source_request_fixture(Tag::MappedCover)?;
    let mut session = ExactCacheSession::new();
    let root = cas.construct_lc_root_indexed(&mut session, &request.root, &mut ledger()?)?;
    let mut cold = ledger()?;
    let first =
        cas.construct_lc_source_substitution_indexed(&mut session, &request, &root, &mut cold)?;
    let before = cas.counts();
    let mut warm = ledger()?;
    let (second, report) = crate::testing::work_trace::observed(512, || {
        cas.construct_lc_source_substitution_indexed(&mut session, &request, &root, &mut warm)
    });
    let second = second?;
    assert!(report.complete);
    let phase_work = |operation: &str| {
        report
            .receipts
            .iter()
            .filter(|((_, _, _, name), _)| name == operation)
            .map(|(_, receipt)| receipt.inclusive)
            .sum::<u64>()
    };
    assert_eq!(phase_work("scope-key"), 154_896);
    assert_eq!(phase_work("root-proof-key"), 14_072);
    assert_eq!(phase_work("proof-encoding-key"), 13_777);
    assert_eq!(phase_work("probe"), 41_006);
    assert_eq!(phase_work("authentication"), 0);
    assert_eq!(phase_work("copy"), 13_811);
    assert!(
        report
            .receipts
            .keys()
            .any(|(_, _, _, name)| name == "authentication")
    );

    assert_eq!(first.wire, second.wire);
    assert_eq!(cas.counts(), before);
    assert_eq!(session.len(), 2);
    assert!(warm.usage().work < cold.usage().work);
    assert_eq!(warm.usage().work, 237_562);
    assert_eq!(session.retained_bytes(), 13_778);
    Ok(())
}

/// Changed originals, a corrupted proof and a foreign root cannot borrow the indexed witness.
/// ´test:unit:exact-cache-root-substitution-mutations´
#[test]
fn exact_cache_root_substitution_mutations() -> TestResult {
    let (mut cas, request) = source_request_fixture(Tag::Box)?;
    let mut session = ExactCacheSession::new();
    let root = cas.construct_lc_root_indexed(&mut session, &request.root, &mut ledger()?)?;
    cas.construct_lc_source_substitution_indexed(&mut session, &request, &root, &mut ledger()?)?;
    let mut changed = request.clone();
    changed.targets.swap(0, 1);
    assert!(matches!(cas.construct_lc_source_substitution_indexed(
        &mut session, &changed, &root, &mut ledger()?,
    ), Err(error) if matches!(error.kind, crate::FaultKind::Invalid(_))));
    // The current closed root schema has one valid transcript per request. A changed
    // transcript is not another proof and must fail independent replay.
    let mut bytes = root
        .certificate(&cas, &mut ledger()?)?
        .to_bytes(&mut ledger()?)?;
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    let corrupt = lc_root::Certificate::from_bytes(&bytes, &mut ledger()?)?;
    assert!(
        matches!(cas.check_lc_root(&request.root, &corrupt, &mut ledger()?),
        Err(error) if matches!(error.kind, crate::FaultKind::Invalid(_)))
    );
    let statement = wire::scope(&cas, &request, &mut ledger()?)?;
    assert!(
        session
            .lookup(
                &cas,
                crate::exact_cache::Kind::Substitution,
                &statement,
                Some(b"wrong complete proof"),
                Some(&root.judgment),
                &mut ledger()?,
            )?
            .is_none()
    );
    // Failed replay advances the namespace; the original graph ID is authenticated
    // again and readmitted with a fresh session serial.
    let old_serial = session.last_serial();
    let readmitted = cas.construct_lc_root_indexed(&mut session, &request.root, &mut ledger()?)?;
    assert_eq!(readmitted.judgment, root.judgment);
    assert!(session.last_serial() > old_serial);
    let foreign = source_fixture(Tag::Box)?.2;
    assert!(matches!(cas.construct_lc_source_substitution_indexed(
        &mut session, &request, &foreign, &mut ledger()?,
    ), Err(error) if matches!(error.kind, crate::FaultKind::Invalid(crate::InvalidCode::ForeignHandle))));
    Ok(())
}

/// Portable replay reconstructs authority from a receiving store's expected originals.
/// ´test:unit:exact-cache-root-substitution-expected-replay´
#[test]
fn exact_cache_root_substitution_expected_replay() -> TestResult {
    let (mut source, request) = source_request_fixture(Tag::Box)?;
    let mut session = ExactCacheSession::new();
    let root = source.construct_lc_root_indexed(&mut session, &request.root, &mut ledger()?)?;
    let indexed = source.construct_lc_source_substitution_indexed(
        &mut session,
        &request,
        &root,
        &mut ledger()?,
    )?;
    let certificate = indexed.certificate(&source, &mut ledger()?)?;
    let (mut receiver, expected) = source_request_fixture(Tag::Box)?;
    let replayed =
        receiver.check_lc_source_substitution(&expected, &certificate, &mut ledger()?)?;
    assert_eq!(replayed.request(&receiver, &mut ledger()?)?, &expected);
    assert_eq!(receiver.retained_judgment_count(), 1);
    Ok(())
}

/// A foreign store cannot use the session, and rollback requires new admission.
/// ´test:unit:exact-cache-session-foreign-owner-and-epoch-invalidation´
#[test]
fn exact_cache_session_foreign_owner_and_epoch_invalidation() -> TestResult {
    let (mut cas, request) = source_request_fixture(Tag::Box)?;
    let mut session = ExactCacheSession::new();
    cas.construct_lc_root_indexed(&mut session, &request.root, &mut ledger()?)?;
    let serial = session.last_serial();
    let (mut foreign, other) = source_request_fixture(Tag::Box)?;
    assert!(
        matches!(foreign.construct_lc_root_indexed(&mut session, &other.root, &mut ledger()?),
        Err(error) if matches!(error.kind, crate::FaultKind::Invalid(crate::InvalidCode::ForeignHandle)))
    );
    assert!(
        cas.transaction(&mut ledger()?, |_, _| {
            Err::<(), _>(Fault::invalid(crate::InvalidCode::Reference, AT))
        })
        .is_err()
    );
    cas.construct_lc_root_indexed(&mut session, &request.root, &mut ledger()?)?;
    assert_eq!(session.len(), 1);
    assert!(session.last_serial() > serial);
    Ok(())
}

/// A refused graph suffix invalidates older session references without losing the root.
/// ´test:unit:exact-cache-substitution-graph-edge-rollback´
#[test]
fn exact_cache_substitution_graph_edge_rollback() -> TestResult {
    let (mut cas, request) = source_request_fixture(Tag::Box)?;
    let mut session = ExactCacheSession::new();
    let root = cas.construct_lc_root_indexed(&mut session, &request.root, &mut ledger()?)?;
    let serial = session.last_serial();
    let session_bytes = session.retained_bytes();
    let before = cas.counts();
    let mut short = Budget::new(Limits {
        edges: before.edges as u64,
        ..Limits::default()
    })?;
    assert!(
        cas.construct_lc_source_substitution_indexed(&mut session, &request, &root, &mut short,)
            .is_err()
    );
    assert_eq!(cas.counts(), before);
    assert_eq!(session.retained_bytes(), session_bytes);
    cas.construct_lc_root_indexed(&mut session, &request.root, &mut ledger()?)?;
    assert!(session.last_serial() > serial);
    Ok(())
}

/// Public indexed calls refuse an outer graph journal with the existing Reference code.
/// ´test:unit:exact-cache-indexed-nested-transaction-refusal´
#[test]
fn exact_cache_indexed_nested_transaction_refusal() -> TestResult {
    let (mut cas, request) = source_request_fixture(Tag::Box)?;
    let mut session = ExactCacheSession::new();
    let root = cas.construct_lc_root(&request.root, &mut ledger()?)?;
    cas.transaction(&mut ledger()?, |cas, budget| {
        let root_error = cas
            .construct_lc_root_indexed(&mut session, &request.root, budget)
            .err()
            .ok_or_else(|| Fault::invalid(crate::InvalidCode::Reference, AT))?;
        let sub_error = cas
            .construct_lc_source_substitution_indexed(&mut session, &request, &root, budget)
            .err()
            .ok_or_else(|| Fault::invalid(crate::InvalidCode::Reference, AT))?;
        assert!(matches!(
            root_error.kind,
            crate::FaultKind::Invalid(crate::InvalidCode::Reference)
        ));
        assert!(matches!(
            sub_error.kind,
            crate::FaultKind::Invalid(crate::InvalidCode::Reference)
        ));
        Ok(())
    })?;
    assert!(session.is_empty());
    Ok(())
}

/// Complete-key mismatches are visited in admission order without added Work.
/// ´test:unit:exact-cache-ordered-probes-batch-collision´
#[test]
fn exact_cache_ordered_probes_batch_collision() -> TestResult {
    let (mut cas, request) = source_request_fixture(Tag::Box)?;
    let mut session = ExactCacheSession::new();
    let root = cas.construct_lc_root_indexed(&mut session, &request.root, &mut ledger()?)?;
    let statement = wire::scope(&cas, &request, &mut ledger()?)?;
    let mut one = ledger()?;
    assert!(
        session
            .lookup(
                &cas,
                crate::exact_cache::Kind::Substitution,
                &statement,
                Some(b"wrong complete proof"),
                Some(&root.judgment),
                &mut one,
            )?
            .is_none()
    );
    cas.construct_lc_source_substitution_indexed(&mut session, &request, &root, &mut ledger()?)?;
    let mut two = ledger()?;
    assert!(
        session
            .lookup(
                &cas,
                crate::exact_cache::Kind::Substitution,
                &statement,
                Some(b"wrong complete proof"),
                Some(&root.judgment),
                &mut two,
            )?
            .is_none()
    );
    assert_eq!(two.usage().work, one.usage().work);
    assert_eq!(session.len(), 2);
    Ok(())
}

/// Unrelated ordered entries change neither hit lookup nor miss admission Work.
/// ´test:unit:exact-cache-lookup-and-admission-locality´
#[test]
fn exact_cache_lookup_and_admission_locality() -> TestResult {
    let (mut cas, request) = source_request_fixture(Tag::Box)?;
    let mut single = ExactCacheSession::new();
    let mut crowded = ExactCacheSession::new();
    crowded.seed_unrelated(&cas, &request.targets[0], 64)?;
    let root = cas.construct_lc_root_indexed(&mut single, &request.root, &mut ledger()?)?;
    cas.construct_lc_root_indexed(&mut crowded, &request.root, &mut ledger()?)?;
    let mut root_single = ledger()?;
    cas.construct_lc_root_indexed(&mut single, &request.root, &mut root_single)?;
    let mut root_crowded = ledger()?;
    cas.construct_lc_root_indexed(&mut crowded, &request.root, &mut root_crowded)?;
    assert_eq!(root_single.usage().work, root_crowded.usage().work);

    cas.construct_lc_source_substitution_indexed(&mut single, &request, &root, &mut ledger()?)?;
    cas.construct_lc_source_substitution_indexed(&mut crowded, &request, &root, &mut ledger()?)?;
    let mut sub_single = ledger()?;
    cas.construct_lc_source_substitution_indexed(&mut single, &request, &root, &mut sub_single)?;
    let mut sub_crowded = ledger()?;
    cas.construct_lc_source_substitution_indexed(&mut crowded, &request, &root, &mut sub_crowded)?;
    assert_eq!(sub_single.usage().work, sub_crowded.usage().work);

    let (mut root_cas_a, root_request_a) = source_request_fixture(Tag::Box)?;
    let (mut root_cas_b, root_request_b) = source_request_fixture(Tag::Box)?;
    let mut root_miss_a = ExactCacheSession::new();
    let mut root_miss_b = ExactCacheSession::new();
    root_miss_b.seed_unrelated(&root_cas_b, &root_request_b.targets[0], 64)?;
    let mut cold_a = ledger()?;
    root_cas_a.construct_lc_root_indexed(&mut root_miss_a, &root_request_a.root, &mut cold_a)?;
    let mut cold_b = ledger()?;
    root_cas_b.construct_lc_root_indexed(&mut root_miss_b, &root_request_b.root, &mut cold_b)?;
    assert_eq!(cold_a.usage().work, cold_b.usage().work);

    let (mut sub_cas_a, sub_request_a) = source_request_fixture(Tag::Box)?;
    let (mut sub_cas_b, sub_request_b) = source_request_fixture(Tag::Box)?;
    let sub_root_a = sub_cas_a.construct_lc_root(&sub_request_a.root, &mut ledger()?)?;
    let sub_root_b = sub_cas_b.construct_lc_root(&sub_request_b.root, &mut ledger()?)?;
    let mut sub_miss_a = ExactCacheSession::new();
    let mut sub_miss_b = ExactCacheSession::new();
    sub_miss_b.seed_unrelated(&sub_cas_b, &sub_request_b.targets[0], 64)?;
    let mut cold_a = ledger()?;
    sub_cas_a.construct_lc_source_substitution_indexed(
        &mut sub_miss_a,
        &sub_request_a,
        &sub_root_a,
        &mut cold_a,
    )?;
    let mut cold_b = ledger()?;
    sub_cas_b.construct_lc_source_substitution_indexed(
        &mut sub_miss_b,
        &sub_request_b,
        &sub_root_b,
        &mut cold_b,
    )?;
    assert_eq!(cold_a.usage().work, cold_b.usage().work);
    Ok(())
}

/// Measured warm Work is an exact caller limit; one less refuses after a consumed prefix.
/// ´test:unit:exact-cache-root-substitution-boundaries´
#[test]
fn exact_cache_root_substitution_boundaries() -> TestResult {
    const PREFIX: u64 = 7;
    let (mut cas, request) = source_request_fixture(Tag::MappedCover)?;
    let mut session = ExactCacheSession::new();
    let root = cas.construct_lc_root_indexed(&mut session, &request.root, &mut ledger()?)?;
    let retained = cas.construct_lc_source_substitution_indexed(
        &mut session,
        &request,
        &root,
        &mut ledger()?,
    )?;
    let mut measured = ledger()?;
    cas.construct_lc_source_substitution_indexed(&mut session, &request, &root, &mut measured)?;
    let work = measured.usage().work;
    assert!(work > 1);
    let before = cas.counts();
    let mut exact = Budget::new(Limits {
        work: work + PREFIX,
        ..Limits::default()
    })?;
    exact.work(PREFIX, AT)?;
    cas.construct_lc_source_substitution_indexed(&mut session, &request, &root, &mut exact)?;
    assert_eq!(exact.usage().work, work + PREFIX);
    let mut short = Budget::new(Limits {
        work: work + PREFIX - 1,
        ..Limits::default()
    })?;
    short.work(PREFIX, AT)?;
    assert!(
        cas.construct_lc_source_substitution_indexed(&mut session, &request, &root, &mut short,)
            .is_err()
    );
    assert_eq!(work, 237_562);
    assert_eq!(short.usage().work, 223_758);
    assert_eq!(cas.counts(), before);
    let output = cas
        .retained_judgment_bytes()
        .saturating_add(retained.retained_bytes());
    let mut exact_output = Budget::new(Limits {
        output_bytes: output,
        ..Limits::default()
    })?;
    cas.construct_lc_source_substitution_indexed(&mut session, &request, &root, &mut exact_output)?;
    let mut short_output = Budget::new(Limits {
        output_bytes: output - 1,
        ..Limits::default()
    })?;
    assert!(
        cas.construct_lc_source_substitution_indexed(
            &mut session,
            &request,
            &root,
            &mut short_output,
        )
        .is_err()
    );
    assert!(short_output.usage().work > 0);
    let mut root_work = ledger()?;
    cas.construct_lc_root_indexed(&mut session, &request.root, &mut root_work)?;
    let root_work = root_work.usage().work;
    let mut root_exact = Budget::new(Limits {
        work: root_work + PREFIX,
        ..Limits::default()
    })?;
    root_exact.work(PREFIX, AT)?;
    cas.construct_lc_root_indexed(&mut session, &request.root, &mut root_exact)?;
    assert_eq!(root_exact.usage().work, root_work + PREFIX);
    let mut root_short = Budget::new(Limits {
        work: root_work + PREFIX - 1,
        ..Limits::default()
    })?;
    root_short.work(PREFIX, AT)?;
    assert!(
        cas.construct_lc_root_indexed(&mut session, &request.root, &mut root_short)
            .is_err()
    );
    assert_eq!(root_work, 163_994);
    assert_eq!(root_short.usage().work, 157_156);

    check_crowded_byte_limits(&mut cas, &request, &root, &mut session, output, work)?;
    Ok(())
}

/// Warm references precede a new session and evict the oldest eligible reference.
/// ´test:unit:exact-cache-warm-before-store-and-eviction´
#[test]
fn exact_cache_warm_before_store_and_eviction() -> TestResult {
    let (mut cas, request) = source_request_fixture(Tag::Box)?;
    let mut session = ExactCacheSession::new();
    let mut warm = ExactWarmIndex::with_limits(7_000, 1);
    let first = cas.construct_lc_root_indexed_with_warm(
        &mut session,
        &mut warm,
        &request.root,
        &mut ledger()?,
    )?;
    assert_eq!(warm.len(), 1);
    let serial = warm.last_serial();
    let mut both_hit = ledger()?;
    cas.construct_lc_root_indexed_with_warm(&mut session, &mut warm, &request.root, &mut both_hit)?;
    assert_eq!(both_hit.usage().work, 163_994);
    assert_eq!(warm.len(), 1);
    assert_eq!(warm.last_serial(), serial);
    let mut fresh = ExactCacheSession::new();
    let mut hit = ledger()?;
    let again =
        cas.construct_lc_root_indexed_with_warm(&mut fresh, &mut warm, &request.root, &mut hit)?;
    assert_eq!(again.judgment, first.judgment);
    assert!(fresh.is_empty());
    assert_eq!(warm.last_serial(), serial);
    assert!(warm.pin(serial.ok_or("missing warm serial")?));
    let mut substitution = ExactCacheSession::new();
    assert!(
        cas.construct_lc_source_substitution_indexed_with_warm(
            &mut substitution,
            &mut warm,
            &request,
            &first,
            &mut ledger()?,
        )
        .is_err()
    );
    assert_eq!(warm.len(), 1);
    assert!(warm.unpin(serial.ok_or("missing warm serial")?));
    let scope = wire::scope(&cas, &request, &mut ledger()?)?;
    let mut exact = Budget::new(Limits {
        payload_bytes: 6_321,
        staged_payload_bytes: 6_337,
        output_bytes: 6_305,
        ..Limits::default()
    })?;
    assert_eq!(warm.reserve(&scope, &mut exact)?.staged_bytes(), 6_337);
    let mut short = Budget::new(Limits {
        payload_bytes: 6_321,
        staged_payload_bytes: 6_336,
        output_bytes: 6_305,
        ..Limits::default()
    })?;
    assert!(warm.reserve(&scope, &mut short).is_err());
    let mut short_residence = Budget::new(Limits {
        payload_bytes: 6_320,
        staged_payload_bytes: 6_337,
        output_bytes: 6_305,
        ..Limits::default()
    })?;
    assert!(warm.reserve(&scope, &mut short_residence).is_err());
    cas.construct_lc_source_substitution_indexed_with_warm(
        &mut substitution,
        &mut warm,
        &request,
        &first,
        &mut ledger()?,
    )?;
    assert_eq!(warm.len(), 1);
    assert_eq!(warm.history_len(), 1);
    assert_eq!(warm.last_evicted_serial(), serial);
    assert_eq!(warm.recovered_bytes(), 6_957);
    assert_eq!(warm.retained_bytes(), 6_321);
    let before = cas.counts();
    assert!(
        cas.construct_lc_root_indexed_with_warm(
            &mut ExactCacheSession::new(),
            &mut warm,
            &request.root,
            &mut ledger()?,
        )
        .is_err()
    );
    assert_eq!(cas.counts(), before);
    check_session_warm_admission()?;
    Ok(())
}

/// An already checked session hit seeds a newly supplied warm index in both kinds.
fn check_session_warm_admission() -> TestResult {
    let (mut cas, request) = source_request_fixture(Tag::Box)?;
    let mut session = ExactCacheSession::new();
    let root = cas.construct_lc_root_indexed(&mut session, &request.root, &mut ledger()?)?;
    cas.construct_lc_source_substitution_indexed(&mut session, &request, &root, &mut ledger()?)?;
    let before = cas.counts();
    let mut warm = ExactWarmIndex::new();
    cas.construct_lc_root_indexed_with_warm(
        &mut session,
        &mut warm,
        &request.root,
        &mut ledger()?,
    )?;
    assert_eq!(warm.len(), 1);
    cas.construct_lc_source_substitution_indexed_with_warm(
        &mut session,
        &mut warm,
        &request,
        &root,
        &mut ledger()?,
    )?;
    assert_eq!(warm.len(), 2);
    let serial = warm.last_serial();
    cas.construct_lc_source_substitution_indexed_with_warm(
        &mut ExactCacheSession::new(),
        &mut warm,
        &request,
        &root,
        &mut ledger()?,
    )?;
    assert_eq!(warm.last_serial(), serial);
    assert_eq!(cas.counts(), before);
    Ok(())
}

/// Dropping caller references cannot remove the graph theorem or import foreign authority.
/// ´test:unit:exact-cache-warm-release-authority´
#[test]
fn exact_cache_warm_release_authority() -> TestResult {
    let (mut cas, request) = source_request_fixture(Tag::Box)?;
    let mut session = ExactCacheSession::new();
    let mut warm = ExactWarmIndex::new();
    let receipt = cas.construct_lc_root_indexed_with_warm(
        &mut session,
        &mut warm,
        &request.root,
        &mut ledger()?,
    )?;
    let serial = warm.last_serial();
    let before = cas.counts();
    let bytes = warm.retained_bytes();
    let mut short = Budget::new(Limits {
        edges: before.edges as u64,
        ..Limits::default()
    })?;
    assert!(
        cas.construct_lc_source_substitution_indexed_with_warm(
            &mut session,
            &mut warm,
            &request,
            &receipt,
            &mut short,
        )
        .is_err()
    );
    assert_eq!(warm.retained_bytes(), bytes);
    assert_eq!(warm.last_serial(), serial);
    assert_eq!(cas.counts(), before);
    let mut readmitted = ExactCacheSession::new();
    cas.construct_lc_root_indexed_with_warm(
        &mut readmitted,
        &mut warm,
        &request.root,
        &mut ledger()?,
    )?;
    assert!(warm.last_serial() > serial);
    assert_eq!(warm.len(), 1);
    drop(session);
    drop(readmitted);
    warm.release();
    assert_eq!(warm.retained_bytes(), 0);
    assert_eq!(receipt.request(&cas, &mut ledger()?)?, &request.root);
    assert_eq!(cas.counts(), before);
    let (mut foreign, other) = source_request_fixture(Tag::Box)?;
    assert!(matches!(foreign.construct_lc_root_indexed_with_warm(
        &mut ExactCacheSession::new(), &mut warm, &other.root, &mut ledger()?,
    ), Err(error) if matches!(error.kind, crate::FaultKind::Invalid(crate::InvalidCode::ForeignHandle))));
    assert!(
        cas.transaction(&mut ledger()?, |_, _| {
            Err::<(), _>(Fault::invalid(crate::InvalidCode::Reference, AT))
        })
        .is_err()
    );
    let mut fresh = ExactCacheSession::new();
    cas.construct_lc_root_indexed_with_warm(&mut fresh, &mut warm, &request.root, &mut ledger()?)?;
    assert!(warm.last_serial() > serial);
    warm.saturate_serial_for_test();
    let before = cas.counts();
    assert!(
        cas.construct_lc_source_substitution_indexed_with_warm(
            &mut fresh,
            &mut warm,
            &request,
            &receipt,
            &mut ledger()?,
        )
        .is_err()
    );
    assert_eq!(cas.counts(), before);
    Ok(())
}

/// Warm lookup Work is local to the candidate, and exact limits refuse at N minus one.
/// ´test:unit:exact-cache-warm-byte-work-neighbors´
#[test]
#[allow(clippy::too_many_lines)]
fn exact_cache_warm_byte_work_neighbors() -> TestResult {
    const PREFIX: u64 = 7;
    let (mut cas, request) = source_request_fixture(Tag::MappedCover)?;
    let mut session = ExactCacheSession::new();
    let mut warm = ExactWarmIndex::new();
    let mut root_cold = ledger()?;
    let root = cas.construct_lc_root_indexed_with_warm(
        &mut session,
        &mut warm,
        &request.root,
        &mut root_cold,
    )?;
    let mut root_hit = ledger()?;
    cas.construct_lc_root_indexed_with_warm(
        &mut ExactCacheSession::new(),
        &mut warm,
        &request.root,
        &mut root_hit,
    )?;
    let mut sub_cold = ledger()?;
    cas.construct_lc_source_substitution_indexed_with_warm(
        &mut session,
        &mut warm,
        &request,
        &root,
        &mut sub_cold,
    )?;
    let mut sub_hit = ledger()?;
    cas.construct_lc_source_substitution_indexed_with_warm(
        &mut ExactCacheSession::new(),
        &mut warm,
        &request,
        &root,
        &mut sub_hit,
    )?;
    let (mut ordinary_cas, ordinary_request) = source_request_fixture(Tag::MappedCover)?;
    let mut root_ordinary = ledger()?;
    let ordinary_root =
        ordinary_cas.construct_lc_root(&ordinary_request.root, &mut root_ordinary)?;
    let mut sub_ordinary = ledger()?;
    ordinary_cas.construct_lc_source_substitution(
        &ordinary_request,
        &ordinary_root,
        &mut sub_ordinary,
    )?;
    let root_work = root_hit.usage().work;
    let sub_work = sub_hit.usage().work;
    assert_eq!(root_cold.usage().work, 351_107);
    assert_eq!(root_work, 163_994);
    assert_eq!(sub_cold.usage().work, 294_136);
    assert_eq!(sub_work, 237_562);
    assert_eq!(root_ordinary.usage().work, 173_205);
    assert_eq!(sub_ordinary.usage().work, 196_585);
    assert_eq!(warm.retained_bytes(), 13_794);
    let scope = wire::scope(&cas, &request, &mut ledger()?)?;
    let mut exact_bytes = Budget::new(Limits {
        payload_bytes: 6_837,
        staged_payload_bytes: 6_837,
        output_bytes: 6_837,
        ..Limits::default()
    })?;
    let mut local_warm = ExactWarmIndex::new();
    assert_eq!(
        local_warm
            .reserve(&scope, &mut exact_bytes)?
            .retained_bytes(),
        6_837
    );
    let mut short_bytes = Budget::new(Limits {
        payload_bytes: 6_837,
        staged_payload_bytes: 6_837,
        output_bytes: 6_836,
        ..Limits::default()
    })?;
    assert!(local_warm.reserve(&scope, &mut short_bytes).is_err());
    let mut short_payload = Budget::new(Limits {
        payload_bytes: 6_836,
        staged_payload_bytes: 6_837,
        output_bytes: 6_837,
        ..Limits::default()
    })?;
    assert!(local_warm.reserve(&scope, &mut short_payload).is_err());
    let mut crowded_reserve = ExactWarmIndex::new();
    crowded_reserve.seed_unrelated(&cas, &request.targets[0], 64)?;
    let mut crowded_budget = ledger()?;
    crowded_reserve.reserve(&scope, &mut crowded_budget)?;
    assert_eq!(crowded_budget.usage().work, exact_bytes.usage().work);
    for (work, root_call) in [(root_work, true), (sub_work, false)] {
        let mut exact = Budget::new(Limits {
            work: work + PREFIX,
            ..Limits::default()
        })?;
        exact.work(PREFIX, AT)?;
        if root_call {
            cas.construct_lc_root_indexed_with_warm(
                &mut ExactCacheSession::new(),
                &mut warm,
                &request.root,
                &mut exact,
            )?;
        } else {
            cas.construct_lc_source_substitution_indexed_with_warm(
                &mut ExactCacheSession::new(),
                &mut warm,
                &request,
                &root,
                &mut exact,
            )?;
        }
        assert_eq!(exact.usage().work, work + PREFIX);
        let mut short = Budget::new(Limits {
            work: work + PREFIX - 1,
            ..Limits::default()
        })?;
        short.work(PREFIX, AT)?;
        let refused = if root_call {
            cas.construct_lc_root_indexed_with_warm(
                &mut ExactCacheSession::new(),
                &mut warm,
                &request.root,
                &mut short,
            )
            .is_err()
        } else {
            cas.construct_lc_source_substitution_indexed_with_warm(
                &mut ExactCacheSession::new(),
                &mut warm,
                &request,
                &root,
                &mut short,
            )
            .is_err()
        };
        assert!(refused);
        assert_eq!(
            short.usage().work,
            if root_call { 157_156 } else { 223_758 }
        );
    }
    let root_peak = warm_staged_limit(|budget| {
        cas.construct_lc_root_indexed_with_warm(
            &mut ExactCacheSession::new(),
            &mut warm,
            &request.root,
            budget,
        )
        .map(|_| ())
    })?;
    let sub_peak = warm_staged_limit(|budget| {
        cas.construct_lc_source_substitution_indexed_with_warm(
            &mut ExactCacheSession::new(),
            &mut warm,
            &request,
            &root,
            budget,
        )
        .map(|_| ())
    })?;
    assert_eq!(root_peak, 24_168);
    assert_eq!(sub_peak, 59_340);
    let session_root_peak = warm_staged_limit(|budget| {
        cas.construct_lc_root_indexed(&mut session, &request.root, budget)
            .map(|_| ())
    })?;
    let session_sub_peak = warm_staged_limit(|budget| {
        cas.construct_lc_source_substitution_indexed(&mut session, &request, &root, budget)
            .map(|_| ())
    })?;
    assert_eq!(session_root_peak, 24_168);
    assert_eq!(session_sub_peak, 59_340);
    check_crowded_warm_history()?;
    let (mut other, other_request) = source_request_fixture(Tag::MappedCover)?;
    let mut crowded = ExactWarmIndex::new();
    crowded.seed_unrelated(&other, &other_request.targets[0], 64)?;
    let mut other_session = ExactCacheSession::new();
    let mut other_root_cold = ledger()?;
    let other_root = other.construct_lc_root_indexed_with_warm(
        &mut other_session,
        &mut crowded,
        &other_request.root,
        &mut other_root_cold,
    )?;
    assert_eq!(other_root_cold.usage().work, root_cold.usage().work);
    let mut root_crowded = ledger()?;
    other.construct_lc_root_indexed_with_warm(
        &mut ExactCacheSession::new(),
        &mut crowded,
        &other_request.root,
        &mut root_crowded,
    )?;
    assert_eq!(root_crowded.usage().work, root_work);
    let mut other_sub_cold = ledger()?;
    other.construct_lc_source_substitution_indexed_with_warm(
        &mut other_session,
        &mut crowded,
        &other_request,
        &other_root,
        &mut other_sub_cold,
    )?;
    assert_eq!(other_sub_cold.usage().work, sub_cold.usage().work);
    let mut sub_crowded = ledger()?;
    other.construct_lc_source_substitution_indexed_with_warm(
        &mut ExactCacheSession::new(),
        &mut crowded,
        &other_request,
        &other_root,
        &mut sub_crowded,
    )?;
    assert_eq!(sub_crowded.usage().work, sub_work);
    Ok(())
}

/// A planned eviction has one local charge even after a prior eviction record.
fn check_crowded_warm_history() -> TestResult {
    let (mut cas, request) = source_request_fixture(Tag::Box)?;
    let mut session = ExactCacheSession::new();
    let mut warm = ExactWarmIndex::with_limits(7_000, 2);
    let root = cas.construct_lc_root_indexed_with_warm(
        &mut session,
        &mut warm,
        &request.root,
        &mut ledger()?,
    )?;
    cas.construct_lc_source_substitution_indexed_with_warm(
        &mut session,
        &mut warm,
        &request,
        &root,
        &mut ledger()?,
    )?;
    assert_eq!(warm.history_len(), 1);
    let scope = wire::scope(&cas, &request, &mut ledger()?)?;
    let mut budget = ledger()?;
    let proposal = warm.reserve(&scope, &mut budget)?;
    assert_eq!(proposal.retained_bytes(), 6_305);
    assert_eq!(budget.usage().work, 6_307);
    Ok(())
}

/// Find the exact staged-byte peak by monotone bounded calls on a fixed hit.
fn warm_staged_limit(mut call: impl FnMut(&mut Budget) -> Result<(), Fault>) -> TestResult<u64> {
    let mut lower = 0_u64;
    let mut upper = Limits::default().staged_payload_bytes;
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        let mut budget = Budget::new(Limits {
            staged_payload_bytes: middle,
            ..Limits::default()
        })?;
        if call(&mut budget).is_ok() {
            upper = middle;
        } else {
            lower = middle + 1;
        }
    }
    let mut exact = Budget::new(Limits {
        staged_payload_bytes: upper,
        ..Limits::default()
    })?;
    call(&mut exact)?;
    if upper > 0 {
        let mut short = Budget::new(Limits {
            staged_payload_bytes: upper - 1,
            ..Limits::default()
        })?;
        assert!(call(&mut short).is_err());
    }
    Ok(upper)
}

/// A crowded session meets the uncrowded exact byte ceilings on the same fixture.
fn check_crowded_byte_limits(
    cas: &mut Cas,
    request: &Request,
    root: &lc_root::Receipt,
    session: &mut ExactCacheSession,
    output: u64,
    work: u64,
) -> TestResult {
    // Find the exact staged limit of this uncrowded warm call, then reuse both
    // exact byte ceilings after many unrelated entries are already resident.
    let mut lower = 0_u64;
    let mut upper = Limits::default().staged_payload_bytes;
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        let mut probe = Budget::new(Limits {
            staged_payload_bytes: middle,
            ..Limits::default()
        })?;
        if cas
            .construct_lc_source_substitution_indexed(session, request, root, &mut probe)
            .is_ok()
        {
            upper = middle;
        } else {
            lower = middle + 1;
        }
    }
    let staged = upper;
    let mut uncrowded = Budget::new(Limits {
        output_bytes: output,
        staged_payload_bytes: staged,
        ..Limits::default()
    })?;
    cas.construct_lc_source_substitution_indexed(session, request, root, &mut uncrowded)?;
    let (mut crowded_cas, crowded_request) = source_request_fixture(Tag::MappedCover)?;
    let mut crowded = ExactCacheSession::new();
    crowded.seed_unrelated(&crowded_cas, &crowded_request.targets[0], 64)?;
    let crowded_root = crowded_cas.construct_lc_root_indexed(
        &mut crowded,
        &crowded_request.root,
        &mut ledger()?,
    )?;
    let crowded_receipt = crowded_cas.construct_lc_source_substitution_indexed(
        &mut crowded,
        &crowded_request,
        &crowded_root,
        &mut ledger()?,
    )?;
    assert_eq!(
        crowded_cas
            .retained_judgment_bytes()
            .saturating_add(crowded_receipt.retained_bytes()),
        output
    );
    let mut crowded_bytes = Budget::new(Limits {
        output_bytes: output,
        staged_payload_bytes: staged,
        ..Limits::default()
    })?;
    crowded_cas.construct_lc_source_substitution_indexed(
        &mut crowded,
        &crowded_request,
        &crowded_root,
        &mut crowded_bytes,
    )?;
    assert_eq!(crowded_bytes.usage().work, work);
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
