// SPDX-License-Identifier: AGPL-3.0-only
//! Portable identities distinguish every retained declaration, domain, guard and representation.

use crate::testing::builders::{admit, core_context};
use crate::testing::common::{TestResult, real};
use crate::testing::map_inverses::{ledger, limits};
use crate::testing::readings::Fixture;
use nrz_cas::domain::{AmbientDomain, AssumptionDraft, ContextDraft, RegionDraft, RegionExpr};
use nrz_cas::readings::ReuseKey;
use nrz_cas::syntax::{Draft, Operator, PredicateKind};
use nrz_cas::{Cas, Fault, FaultKind, Handle, InvalidCode, Rational, Reference};

fn fixture(display: &str, reverse: bool) -> TestResult<Fixture> {
    let mut cas = Cas::new(limits())?;
    let mut environment =
        crate::testing::builders::environment(nrz_cas::syntax::Field::Real, &["C", "D"]);
    environment
        .declarations
        .first_mut()
        .ok_or("C declaration")?
        .display_name = display.into();
    let context = core_context(&mut cas, environment, Vec::new(), &mut ledger()?)?;
    let mut result = Fixture {
        cas,
        context,
        symbols: Vec::new(),
    };
    for key in if reverse { ["D", "C"] } else { ["C", "D"] } {
        let value = result.term(Operator::Free(key.into()), real(), &[])?;
        result.symbols.push((key.into(), value));
    }
    Ok(result)
}

fn reciprocal(case: &mut Fixture, denominator: &str) -> TestResult<Handle> {
    let one = case.term(Operator::Rational(Rational::one()), real(), &[])?;
    let divisor = case.symbol(denominator)?;
    case.term(Operator::Div, real(), &[one, divisor])
}

fn premise(case: &mut Fixture, kind: PredicateKind, symbol: &str, domain: bool) -> TestResult {
    let header = case.cas.context_header(&case.context)?;
    let source = case.cas.admissible_region(&case.context)?;
    let term = case.symbol(symbol)?;
    let predicate = admit(
        &mut case.cas,
        Draft::Condition {
            kind,
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            arguments: vec![Reference::Existing(term)],
        },
        &mut ledger()?,
    )?;
    let mut assumptions = Vec::new();
    let region = if domain {
        admit(
            &mut case.cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Assignments,
                expression: RegionExpr::GuardTrue {
                    base: Reference::Existing(source),
                    predicate: Reference::Existing(predicate),
                },
            }),
            &mut ledger()?,
        )?
    } else {
        assumptions.push(AssumptionDraft {
            key: "scale-premise".into(),
            condition: Reference::Existing(predicate),
        });
        source
    };
    case.context = admit(
        &mut case.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header),
            admissible: Reference::Existing(region),
            assumptions,
        }),
        &mut ledger()?,
    )?;
    Ok(())
}

fn key(case: &mut Fixture, term: &Handle) -> TestResult<ReuseKey> {
    Ok(case
        .cas
        .expression_reuse_key(&case.context, term, &mut ledger()?)?)
}

/// Canonical keys cross stores and allocation orders while foreign authority still refuses.
/// ´test:integration:reuse-key-portable-store-identity´
#[test]
fn reuse_key_portable_store_identity() -> TestResult {
    let mut first = fixture("capacitance", false)?;
    let original = reciprocal(&mut first, "C")?;
    let first_key = key(&mut first, &original)?;
    let mut second = fixture("capacitance", true)?;
    let copy = reciprocal(&mut second, "C")?;
    let second_key = key(&mut second, &copy)?;
    assert_eq!(first_key, second_key);
    assert_eq!(first_key.as_bytes(), second_key.as_bytes());
    first.cas.validate_reuse_key(&first_key, &mut ledger()?)?;
    second.cas.validate_reuse_key(&second_key, &mut ledger()?)?;
    for result in [
        second.cas.validate_reuse_key(&first_key, &mut ledger()?),
        second
            .cas
            .expression_reuse_key(&second.context, &original, &mut ledger()?)
            .map(|_| ()),
    ] {
        assert!(matches!(
            result,
            Err(Fault {
                kind: FaultKind::Invalid(InvalidCode::ForeignHandle),
                ..
            })
        ));
    }
    Ok(())
}

/// Every original declaration, context guard, stratum, divisor and expression representation participates.
/// ´test:integration:reuse-key-retains-complete-original-context´
#[test]
fn reuse_key_retains_complete_original_context() -> TestResult {
    let mut original = fixture("capacitance", false)?;
    let term = reciprocal(&mut original, "C")?;
    let baseline = key(&mut original, &term)?;
    let mut keys = vec![baseline];
    for change in 0..7 {
        let mut case = fixture(
            if change == 0 {
                "changed capacitance"
            } else {
                "capacitance"
            },
            false,
        )?;
        match change {
            1 => premise(&mut case, PredicateKind::NonZero, "C", false)?,
            2 => premise(&mut case, PredicateKind::Positive, "C", false)?,
            3 => premise(&mut case, PredicateKind::Positive, "C", true)?,
            4 => premise(&mut case, PredicateKind::Positive, "D", true)?,
            _ => {}
        }
        let mut term = reciprocal(&mut case, if change == 5 { "D" } else { "C" })?;
        if change == 6 {
            term = case.term(
                Operator::Matrix {
                    rows: 1,
                    columns: 1,
                },
                crate::testing::readings::matrix(1, 1),
                &[term],
            )?;
        }
        let changed = key(&mut case, &term)?;
        assert!(keys.iter().all(|previous| *previous != changed));
        keys.push(changed);
    }
    Ok(())
}

/// Equal algebraic values still have different keys when their original denominator relations differ.
/// ´test:integration:reuse-key-keeps-original-cancelled-denominator´
#[test]
fn reuse_key_keeps_original_cancelled_denominator() -> TestResult {
    let mut case = fixture("capacitance", false)?;
    let parameter = case.symbol("C")?;
    let cancelled = case.term(Operator::Div, real(), &[parameter.clone(), parameter])?;
    let one = case.term(Operator::Rational(Rational::one()), real(), &[])?;
    assert_ne!(key(&mut case, &cancelled)?, key(&mut case, &one)?);
    let bytes = key(&mut case, &cancelled)?;
    assert!(bytes.as_bytes().starts_with(b"NRZREU04"));
    for primitive in nrz_cas::proof::PRIMITIVES {
        let specification = primitive.specification();
        assert!(
            bytes
                .as_bytes()
                .windows(specification.key.len())
                .any(|window| window == specification.key.as_bytes())
        );
    }
    Ok(())
}

/// Proof derivation choice is excluded when the original relation and retained requirements agree.
/// ´test:integration:reuse-key-excludes-proof-allocation-order´
#[test]
fn reuse_key_excludes_proof_allocation_order() -> TestResult {
    use nrz_cas::proof::{Certificate, Primitive};
    let mut case = fixture("capacitance", false)?;
    let original = reciprocal(&mut case, "C")?;
    let domain = case.cas.admissible_region(&case.context)?;
    let first = crate::testing::witness::equality(
        &case.context,
        &domain,
        Primitive::Reflexivity,
        &original,
        &original,
    );
    let direct =
        case.cas
            .check_proof(&case.context, std::slice::from_ref(&first), &mut ledger()?)?;
    let mut second = first.clone();
    second.rule = Primitive::Symmetry;
    second.premises = vec![0];
    second.certificate = Certificate::None;
    let reversed = case
        .cas
        .check_proof(&case.context, &[first, second], &mut ledger()?)?;
    assert_eq!(
        case.cas.witness_reuse_key(&direct, &mut ledger()?)?,
        case.cas.witness_reuse_key(&reversed, &mut ledger()?)?
    );
    Ok(())
}

/// Defined reading keys retain their original symbol, source selection and coefficient table.
/// ´test:integration:defined-reading-reuse-key-recomputes´
#[test]
fn defined_reading_reuse_key_recomputes() -> TestResult {
    let mut source = crate::testing::readings::state_case(0)?;
    let mut receiver = crate::testing::readings::state_case(0)?;
    let first = source.fixture.cas.expression_reuse_key(
        &source.fixture.context,
        &source.left,
        &mut ledger()?,
    )?;
    let second = receiver.fixture.cas.expression_reuse_key(
        &receiver.fixture.context,
        &receiver.left,
        &mut ledger()?,
    )?;
    assert_eq!(first, second);
    Ok(())
}
