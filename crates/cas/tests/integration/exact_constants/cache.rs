// SPDX-License-Identifier: AGPL-3.0-only
//! Checked preparation reuse, original identity and cumulative resource boundaries.

use super::{TestResult, ledger, original};
use nrz_cas::domain::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::syntax::{Constant, Draft, Operator};
use nrz_cas::{
    Cas, ConstantIdentityKind, ExactConstant, Fault, FaultKind, Handle, InvalidCode, Rational,
    Reference,
};

/// Exact original target and its actual context in one immutable graph namespace.
struct Fixture {
    /// Source graph.
    cas: Cas,
    /// Complete context identity.
    context: Handle,
    /// Original expression before normalization.
    root: Handle,
}

/// Add a distinct context without changing the target's declaration header.
fn frame(cas: &mut Cas, root: &Handle, empty: bool) -> TestResult<Handle> {
    let header = cas.expression_header(root)?;
    let drafts = [
        Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression: if empty {
                RegionExpr::Empty
            } else {
                RegionExpr::Ambient
            },
        }),
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header),
            admissible: Reference::Local(0),
            assumptions: Vec::new(),
        }),
    ];
    cas.admit(&drafts, &mut ledger()?)?
        .pop()
        .ok_or_else(|| "context".into())
}

/// The measured non-rational original period.
fn fixture() -> TestResult<Fixture> {
    let (mut cas, root) = original(
        &[
            (
                Operator::Rational(Rational::from_ratio(2, 1, &mut ledger()?)?),
                vec![],
            ),
            (Operator::Constant(Constant::Pi), vec![]),
            (Operator::Mul, vec![0, 1]),
        ],
        false,
    )?;
    let context = frame(&mut cas, &root, false)?;
    Ok(Fixture { cas, context, root })
}

/// A second request borrows the identical preparation for exactly one Work.
/// ´test:integration:constant-cache-reuses-checked-preparation´
/// ´claim:cas:constant-cache-reuses-checked-preparation´
#[test]
fn constant_cache_reuses_checked_preparation() -> TestResult {
    let mut case = fixture()?;
    let mut budget = ledger()?;
    let prepared = case
        .cas
        .prepare_constant(&case.context, &case.root, &mut budget)?;
    let first = &raw const *prepared;
    assert_eq!(prepared.original(), &case.root);
    assert_eq!(prepared.context(), &case.context);
    assert_eq!(
        prepared.positive().ok_or("positive period")?.value(),
        prepared.value()
    );
    let uncached = budget.usage().work;
    assert_eq!(uncached, 27_938);
    let again = case
        .cas
        .prepare_constant(&case.context, &case.root, &mut budget)?;
    assert!(std::ptr::eq(first, again));
    assert_eq!(budget.usage().work, uncached + 1);
    for kind in [
        ConstantIdentityKind::MultiplyOne,
        ConstantIdentityKind::AddZero,
    ] {
        let witness = again.identity(kind, &mut budget)?;
        assert_eq!(witness.kind(), kind);
        assert!(std::ptr::eq(witness.preparation(), again));
        assert!(std::ptr::eq(witness.value(), again.value()));
    }
    assert_eq!(budget.usage().work, uncached + 3);
    assert_eq!(case.cas.retained_judgment_count(), 1);
    Ok(())
}

/// A changed original that normalizes to the same printed value pays for its own check.
/// ´test:integration:constant-cache-original-identity´
/// ´claim:cas:constant-cache-original-identity´
#[test]
fn constant_cache_original_identity() -> TestResult {
    let mut case = fixture()?;
    let expected = case
        .cas
        .prepare_constant(&case.context, &case.root, &mut ledger()?)?
        .value()
        .clone();
    let header = case.cas.expression_header(&case.root)?;
    let zero = ExactConstant::zero()
        .expression_drafts(Reference::Existing(header), &mut ledger()?)?
        .pop()
        .ok_or("zero draft")?;
    let changed = nrz_cas::syntax::ExpressionDraft {
        operator: Operator::Add,
        operands: vec![Reference::Existing(case.root.clone()), Reference::Local(0)],
        ..zero.clone()
    };
    let root = case
        .cas
        .admit(
            &[Draft::Expression(zero), Draft::Expression(changed)],
            &mut ledger()?,
        )?
        .pop()
        .ok_or("changed original")?;
    let mut direct = ledger()?;
    ExactConstant::from_expression(&case.cas, &root, &mut direct)?;
    let mut measured = ledger()?;
    let prepared = case
        .cas
        .prepare_constant(&case.context, &root, &mut measured)?;
    assert_eq!(prepared.value(), &expected);
    assert_eq!(prepared.original(), &root);
    assert_eq!(measured.usage().work, direct.usage().work);
    assert_eq!(case.cas.retained_judgment_count(), 2);
    Ok(())
}

/// Distinct contexts and source stores cannot borrow an earlier judgment.
/// ´test:integration:constant-cache-context-and-owner´
/// ´claim:cas:constant-cache-context-and-owner´
#[test]
fn constant_cache_context_and_owner() -> TestResult {
    let mut case = fixture()?;
    case.cas
        .prepare_constant(&case.context, &case.root, &mut ledger()?)?;
    let changed = frame(&mut case.cas, &case.root, true)?;
    let mut budget = ledger()?;
    case.cas
        .prepare_constant(&changed, &case.root, &mut budget)?;
    assert_eq!(budget.usage().work, 27_938);
    assert_eq!(case.cas.retained_judgment_count(), 2);
    let mut other = fixture()?;
    let fault = refused(
        &other
            .cas
            .prepare_constant(&other.context, &case.root, &mut ledger()?),
        "foreign original",
    );
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::ForeignHandle));
    assert_eq!(fault.location.operation, "handle lookup");
    assert_eq!(case.cas.retained_judgment_count(), 2);
    let mut budget = ledger()?;
    other
        .cas
        .prepare_constant(&other.context, &other.root, &mut budget)?;
    assert_eq!(budget.usage().work, 27_938);
    assert_eq!(other.cas.retained_judgment_count(), 1);
    assert_eq!(case.cas.retained_judgment_count(), 2);
    Ok(())
}

/// Literal Work limits include the hit and never refund a prior caller charge.
/// ´test:integration:constant-cache-work-boundaries´
/// ´claim:cas:constant-cache-work-boundaries´
#[test]
fn constant_cache_work_boundaries() -> TestResult {
    let mut case = fixture()?;
    let mut miss = Budget::new(Limits {
        work: 27_938,
        ..Limits::default()
    })?;
    case.cas
        .prepare_constant(&case.context, &case.root, &mut miss)?;
    let bytes = case.cas.retained_judgment_bytes();
    for work in [1, 0] {
        let mut budget = Budget::new(Limits {
            work,
            ..Limits::default()
        })?;
        let result = case
            .cas
            .prepare_constant(&case.context, &case.root, &mut budget);
        if work == 1 {
            result?;
            assert_eq!(budget.usage().work, 1);
        } else {
            assert_work(refused(&result, "one Work required"));
        }
        assert_eq!(case.cas.retained_judgment_count(), 1);
        assert_eq!(case.cas.retained_judgment_bytes(), bytes);
    }
    let mut prior = Budget::new(Limits {
        work: 8,
        ..Limits::default()
    })?;
    prior.work(7, nrz_cas::Location::new("prior caller work"))?;
    case.cas
        .prepare_constant(&case.context, &case.root, &mut prior)?;
    assert_eq!(prior.usage().work, 8);
    assert_work(refused(
        &case
            .cas
            .prepare_constant(&case.context, &case.root, &mut prior),
        "spent ledger",
    ));
    assert_eq!(prior.usage().work, 8);
    let mut fresh = fixture()?;
    let mut short = Budget::new(Limits {
        work: 27_937,
        ..Limits::default()
    })?;
    assert_work(refused(
        &fresh
            .cas
            .prepare_constant(&fresh.context, &fresh.root, &mut short),
        "miss needs full admission",
    ));
    assert_eq!(fresh.cas.retained_judgment_count(), 0);
    assert!(short.usage().work > 0);
    Ok(())
}

/// Pin the budget that refused, without comparing an opaque diagnostic structure.
fn assert_work(fault: Fault) {
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::Work,
            ..
        }
    ));
}

/// Every closed fixture shares checked sign and identity results without a second traversal.
/// ´test:integration:constant-cache-closed-fixtures´
/// ´claim:cas:constant-cache-closed-fixtures´
#[test]
fn constant_cache_closed_fixtures() -> TestResult {
    let inputs = [
        (Operator::Constant(Constant::Pi), 10_888, true),
        (
            Operator::Rational(Rational::from_ratio(3, 7, &mut ledger()?)?),
            6_741,
            true,
        ),
        (Operator::Rational(Rational::one()), 6_453, true),
        (Operator::Rational(Rational::zero()), 3_956, false),
    ];
    for (operator, work, positive) in inputs {
        let (mut cas, root) = original(&[(operator, Vec::new())], false)?;
        let context = frame(&mut cas, &root, false)?;
        let mut budget = ledger()?;
        cas.prepare_constant(&context, &root, &mut budget)?;
        assert_eq!(budget.usage().work, work);
        let prepared = cas.prepare_constant(&context, &root, &mut budget)?;
        assert_eq!(budget.usage().work, work + 1);
        assert_eq!(prepared.positive().is_some(), positive);
        for kind in [
            ConstantIdentityKind::MultiplyOne,
            ConstantIdentityKind::AddZero,
        ] {
            for _ in 0..2 {
                let mut exact = Budget::new(Limits {
                    work: 1,
                    ..Limits::default()
                })?;
                let witness = prepared.identity(kind, &mut exact)?;
                assert!(std::ptr::eq(witness.preparation(), prepared));
                assert_eq!(exact.usage().work, 1);
            }
            let mut exhausted = Budget::new(Limits {
                work: 0,
                ..Limits::default()
            })?;
            assert_work(refused(
                &prepared.identity(kind, &mut exhausted),
                "identity lookup",
            ));
        }
    }
    Ok(())
}

/// Undefined originals never enter the cache, even after a successful same-store preparation.
/// ´test:integration:constant-cache-domain-refusal´
/// ´claim:cas:constant-cache-domain-refusal´
#[test]
fn constant_cache_domain_refusal() -> TestResult {
    let mut case = fixture()?;
    case.cas
        .prepare_constant(&case.context, &case.root, &mut ledger()?)?;
    let bytes = case.cas.retained_judgment_bytes();
    let zero = ExactConstant::zero()
        .expression_drafts(
            Reference::Existing(case.cas.expression_header(&case.root)?),
            &mut ledger()?,
        )?
        .pop()
        .ok_or("zero draft")?;
    let division = nrz_cas::syntax::ExpressionDraft {
        operator: Operator::Div,
        operands: vec![Reference::Existing(case.root.clone()), Reference::Local(0)],
        ..zero.clone()
    };
    let root = case
        .cas
        .admit(
            &[Draft::Expression(zero), Draft::Expression(division)],
            &mut ledger()?,
        )?
        .pop()
        .ok_or("undefined target")?;
    let snapshot = case.cas.canonical_snapshot(&mut ledger()?)?;
    let mut budget = ledger()?;
    let fault = refused(
        &case.cas.prepare_constant(&case.context, &root, &mut budget),
        "zero divisor",
    );
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::RefutedDomain));
    assert_eq!(fault.location.operation, "exact constant expression");
    assert!(budget.usage().work > 0);
    assert_eq!(case.cas.retained_judgment_count(), 1);
    assert_eq!(case.cas.retained_judgment_bytes(), bytes);
    assert_eq!(case.cas.canonical_snapshot(&mut ledger()?)?, snapshot);
    Ok(())
}

/// Inspect an expected refusal without ever formatting a successful graph or receipt.
fn refused<T>(result: &Result<T, Fault>, expectation: &str) -> Fault {
    let Err(fault) = result else {
        panic!("{expectation}");
    };
    *fault
}

/// Cached retention is bounded at literal N/N−1 on every hit and before miss publication.
/// ´test:integration:constant-cache-storage-boundaries´
/// ´claim:cas:constant-cache-storage-boundaries´
#[test]
fn constant_cache_storage_boundaries() -> TestResult {
    let mut case = fixture()?;
    case.cas
        .prepare_constant(&case.context, &case.root, &mut ledger()?)?;
    assert_eq!(case.cas.retained_judgment_bytes(), 3_654);
    for kind in [BudgetKind::OutputBytes, BudgetKind::StagedPayloadBytes] {
        for bytes in [3_654, 3_653] {
            let mut limits = Limits::default();
            if kind == BudgetKind::OutputBytes {
                limits.output_bytes = bytes;
            } else {
                limits.staged_payload_bytes = bytes;
            }
            let mut budget = Budget::new(limits)?;
            let result = case
                .cas
                .prepare_constant(&case.context, &case.root, &mut budget);
            if bytes == 3_654 {
                result?;
            } else {
                let fault = refused(&result, "retained storage ceiling");
                assert!(
                    matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind)
                );
                assert_eq!(fault.location.operation, "exact constant cache");
            }
            assert_eq!(budget.usage().work, 1);
            assert_eq!(case.cas.retained_judgment_count(), 1);
            assert_eq!(case.cas.retained_judgment_bytes(), 3_654);
        }
    }
    Ok(())
}

/// A failed output reservation never publishes a newly checked constant.
/// ´test:integration:constant-cache-miss-storage´
/// ´claim:cas:constant-cache-miss-storage´
#[test]
fn constant_cache_miss_storage() -> TestResult {
    for bytes in [3_654, 3_653] {
        let mut case = fixture()?;
        let mut budget = Budget::new(Limits {
            output_bytes: bytes,
            ..Limits::default()
        })?;
        let result = case
            .cas
            .prepare_constant(&case.context, &case.root, &mut budget);
        if bytes == 3_654 {
            result?;
        } else {
            let fault = refused(&result, "miss publication storage");
            assert!(matches!(
                fault.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::OutputBytes,
                    ..
                }
            ));
            assert_eq!(case.cas.retained_judgment_count(), 0);
        }
        assert_eq!(budget.usage().work, 27_938);
    }
    Ok(())
}

/// Caller-retained payload and failed same-store insertion preserve earlier checked results.
/// ´test:integration:constant-cache-retained-refusals´
/// ´claim:cas:constant-cache-retained-refusals´
#[test]
fn constant_cache_retained_refusals() -> TestResult {
    let mut case = fixture()?;
    case.cas
        .prepare_constant(&case.context, &case.root, &mut ledger()?)?;
    let mut budget = Budget::new(Limits {
        staged_payload_bytes: 3_654,
        ..Limits::default()
    })?;
    let result =
        budget.with_retained_payload(1, nrz_cas::Location::new("caller retention"), |budget| {
            case.cas.prepare_constant(&case.context, &case.root, budget)
        });
    let fault = refused(&result, "one retained input byte");
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::StagedPayloadBytes,
            ..
        }
    ));
    assert_eq!(budget.usage().work, 1);
    assert_eq!(budget.limits().staged_payload_bytes, 3_654);
    let changed = frame(&mut case.cas, &case.root, true)?;
    let mut budget = Budget::new(Limits {
        output_bytes: 3_654,
        ..Limits::default()
    })?;
    let fault = refused(
        &case.cas.prepare_constant(&changed, &case.root, &mut budget),
        "second retained result",
    );
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::OutputBytes,
            ..
        }
    ));
    assert_eq!(budget.usage().work, 27_938);
    assert_eq!(case.cas.retained_judgment_count(), 1);
    assert_eq!(case.cas.retained_judgment_bytes(), 3_654);
    Ok(())
}

/// A new declaration snapshot preserves the displayed value but changes original identity.
fn declared_copy(case: &mut Fixture, value: &ExactConstant) -> TestResult<Handle> {
    let header = Draft::Header(nrz_cas::domain::HeaderDraft::Core(
        nrz_cas::syntax::Environment {
            declarations: vec![nrz_cas::syntax::Declaration {
                key: "x".into(),
                display_name: "Pi".into(),
                sort: nrz_cas::syntax::Sort::Value(nrz_cas::syntax::AtomicSort::Scalar(
                    nrz_cas::syntax::Field::Real,
                )),
            }],
            ..nrz_cas::syntax::Environment::default()
        },
    ));
    let header = case
        .cas
        .admit(&[header], &mut ledger()?)?
        .pop()
        .ok_or("new header")?;
    let drafts = value
        .expression_drafts(Reference::Existing(header), &mut ledger()?)?
        .into_iter()
        .map(Draft::Expression)
        .collect::<Vec<_>>();
    case.cas
        .admit(&drafts, &mut ledger()?)?
        .pop()
        .ok_or_else(|| "new original".into())
}

/// Header mismatch refuses before lookup, and a changed declaration frame requires a fresh check.
/// ´test:integration:constant-cache-declaration-identity´
/// ´claim:cas:constant-cache-declaration-identity´
#[test]
fn constant_cache_declaration_identity() -> TestResult {
    let mut case = fixture()?;
    let expected = case
        .cas
        .prepare_constant(&case.context, &case.root, &mut ledger()?)?
        .value()
        .clone();
    let changed = declared_copy(&mut case, &expected)?;
    let fault = refused(
        &case
            .cas
            .prepare_constant(&case.context, &changed, &mut ledger()?),
        "header mismatch",
    );
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Scope));
    assert_eq!(fault.location.operation, "exact constant cache");
    assert_eq!(case.cas.retained_judgment_count(), 1);
    let context = frame(&mut case.cas, &changed, false)?;
    let mut direct = ledger()?;
    ExactConstant::from_expression(&case.cas, &changed, &mut direct)?;
    let mut measured = ledger()?;
    let prepared = case
        .cas
        .prepare_constant(&context, &changed, &mut measured)?;
    assert_eq!(prepared.value(), &expected);
    assert_eq!(measured.usage().work, direct.usage().work);
    assert_eq!(case.cas.retained_judgment_count(), 2);
    Ok(())
}
