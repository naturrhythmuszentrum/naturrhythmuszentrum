// SPDX-License-Identifier: AGPL-3.0-only
//! Independent expected claims and receiving replay.

use crate::proof_model::{ApplicationDraft, Certificate, DomainContract, Statement};
use crate::rule::Primitive;
use crate::testing::common::{TestResult, default_budget};
use crate::testing::half_pi::{Fixture, prepared_with_opaque, term};
use crate::{FaultKind, InvalidCode, Location};

#[derive(Clone)]
struct Parts {
    argument: crate::Handle,
    sine: crate::Handle,
    sine_square: crate::Handle,
    cosine_square: crate::Handle,
    sum: crate::Handle,
}

fn fixture(reordered: bool) -> TestResult<(Fixture, Parts)> {
    fixture_with_opaque(reordered, false)
}

fn fixture_with_opaque(reordered: bool, opaque: bool) -> TestResult<(Fixture, Parts)> {
    let mut f = prepared_with_opaque("0", "0", true, reordered, opaque)?;
    let Statement::HalfPiWitness { u, .. } = &f.expected.statement else {
        return Err("original argument".into());
    };
    let argument = u.clone();
    let sine = term(
        &mut f.cas,
        &f.header,
        crate::syntax::Operator::Sine,
        std::slice::from_ref(&argument),
    )?;
    let cosine = term(
        &mut f.cas,
        &f.header,
        crate::syntax::Operator::Cosine,
        std::slice::from_ref(&argument),
    )?;
    let sine_square = term(
        &mut f.cas,
        &f.header,
        crate::syntax::Operator::IntegerPower(2),
        std::slice::from_ref(&sine),
    )?;
    let cosine_square = term(
        &mut f.cas,
        &f.header,
        crate::syntax::Operator::IntegerPower(2),
        std::slice::from_ref(&cosine),
    )?;
    let sum = term(
        &mut f.cas,
        &f.header,
        crate::syntax::Operator::Add,
        &[sine_square.clone(), cosine_square.clone()],
    )?;
    let one = term(
        &mut f.cas,
        &f.header,
        crate::syntax::Operator::Rational(crate::Rational::one()),
        &[],
    )?;
    f.expected.statement = Statement::Equality {
        left: sum.clone(),
        right: one,
        domain: f.domain.clone(),
        domain_contract: DomainContract::DefinedAndEqual,
    };
    f.steps.truncate(1);
    f.steps.push(ApplicationDraft {
        rule: Primitive::TrigJoint,
        context: f.context.clone(),
        conclusion: f.expected.statement.clone(),
        premises: vec![0],
        substitutions: Vec::new(),
        certificate: Certificate::None,
    });
    Ok((
        f,
        Parts {
            argument,
            sine,
            sine_square,
            cosine_square,
            sum,
        },
    ))
}

fn producer(f: &mut Fixture, parts: &Parts) -> TestResult<crate::proof_model::VerifiedWitness> {
    let guard = f
        .cas
        .check_proof(&f.context, &f.steps[..1], &mut default_budget()?)?;
    Ok(f.cas
        .trig_joint(&parts.argument, &guard.proof, &mut default_budget()?)?)
}

fn refuses(
    f: &mut Fixture,
    steps: &[ApplicationDraft],
    label: &str,
    code: InvalidCode,
) -> TestResult {
    let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
    let fault = f
        .cas
        .check_proof(&f.context, steps, &mut default_budget()?)
        .err()
        .ok_or_else(|| format!("{label} accepted"))?;
    assert_eq!(fault.kind, FaultKind::Invalid(code), "{label}");
    assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
    Ok(())
}

fn endpoints(
    statement: &mut Statement,
) -> TestResult<(
    &mut crate::Handle,
    &mut crate::Handle,
    &mut crate::Handle,
    &mut DomainContract,
)> {
    let Statement::Equality {
        left,
        right,
        domain,
        domain_contract,
    } = statement
    else {
        return Err("joint equality".into());
    };
    Ok((left, right, domain, domain_contract))
}

/// Every malformed exact role and premise is refused at its specified kind.
/// ´test:crate:trig-joint-producing-store-mutations´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "all independent role and premise mutations share one original fixture"
)]
fn trig_joint_producing_store_mutations() -> TestResult {
    let (mut f, parts) = fixture_with_opaque(false, true)?;
    let witness = producer(&mut f, &parts)?;
    assert_eq!(witness.judgment(), &f.expected);
    let original = f.steps.clone();
    let two = term(
        &mut f.cas,
        &f.header,
        crate::syntax::Operator::Rational(crate::Rational::from_ratio(
            2,
            1,
            &mut default_budget()?,
        )?),
        &[],
    )?;
    let three = term(
        &mut f.cas,
        &f.header,
        crate::syntax::Operator::IntegerPower(3),
        std::slice::from_ref(&parts.sine),
    )?;
    let scaled = term(
        &mut f.cas,
        &f.header,
        crate::syntax::Operator::Mul,
        &[two.clone(), parts.cosine_square.clone()],
    )?;
    let wrong_exponent = term(
        &mut f.cas,
        &f.header,
        crate::syntax::Operator::Add,
        &[three, parts.cosine_square.clone()],
    )?;
    let wrong_coefficient = term(
        &mut f.cas,
        &f.header,
        crate::syntax::Operator::Add,
        &[parts.sine_square.clone(), scaled],
    )?;
    let opaque_sine = term(
        &mut f.cas,
        &f.header,
        crate::syntax::Operator::Call("sine".into()),
        std::slice::from_ref(&parts.argument),
    )?;
    let opaque_sine_square = term(
        &mut f.cas,
        &f.header,
        crate::syntax::Operator::IntegerPower(2),
        std::slice::from_ref(&opaque_sine),
    )?;
    let opaque_sum = term(
        &mut f.cas,
        &f.header,
        crate::syntax::Operator::Add,
        &[opaque_sine_square, parts.cosine_square.clone()],
    )?;
    let changed_argument = term(
        &mut f.cas,
        &f.header,
        crate::syntax::Operator::Sub,
        &[parts.argument.clone(), parts.argument.clone()],
    )?;
    let changed_sine = term(
        &mut f.cas,
        &f.header,
        crate::syntax::Operator::Sine,
        std::slice::from_ref(&changed_argument),
    )?;
    let changed_sine_square = term(
        &mut f.cas,
        &f.header,
        crate::syntax::Operator::IntegerPower(2),
        std::slice::from_ref(&changed_sine),
    )?;
    let mixed_argument = term(
        &mut f.cas,
        &f.header,
        crate::syntax::Operator::Add,
        &[changed_sine_square, parts.cosine_square.clone()],
    )?;
    let empty = f
        .cas
        .admit(
            &[crate::test_api::syntax::Draft::Region(
                crate::test_api::domain::RegionDraft {
                    header: crate::test_api::Reference::Existing(f.header.clone()),
                    scope: Vec::new(),
                    ambient: crate::test_api::domain::AmbientDomain::Real("x".into()),
                    expression: crate::test_api::domain::RegionExpr::Empty,
                },
            )],
            &mut default_budget()?,
        )?
        .pop()
        .ok_or("alternate domain")?;
    for (label, left_change, right_change, domain_change, contract_change, code) in [
        (
            "wrong exponent",
            Some(wrong_exponent),
            None,
            None,
            None,
            InvalidCode::Rule,
        ),
        (
            "cosine coefficient two",
            Some(wrong_coefficient),
            None,
            None,
            None,
            InvalidCode::Rule,
        ),
        (
            "opaque sine call",
            Some(opaque_sum),
            None,
            None,
            None,
            InvalidCode::Rule,
        ),
        (
            "changed argument",
            Some(mixed_argument),
            None,
            None,
            None,
            InvalidCode::EndpointMismatch,
        ),
        (
            "result other than one",
            None,
            Some(two),
            None,
            None,
            InvalidCode::EndpointMismatch,
        ),
        (
            "changed domain",
            None,
            None,
            Some(empty),
            None,
            InvalidCode::EndpointMismatch,
        ),
        (
            "wrong contract",
            None,
            None,
            None,
            Some(DomainContract::SameDefinedDomain),
            InvalidCode::EndpointMismatch,
        ),
    ] {
        let mut changed = original.clone();
        let (left, right, domain, contract) = endpoints(&mut changed[1].conclusion)?;
        if let Some(value) = left_change {
            *left = value;
        }
        if let Some(value) = right_change {
            *right = value;
        }
        if let Some(value) = domain_change {
            *domain = value;
        }
        if let Some(value) = contract_change {
            *contract = value;
        }
        refuses(&mut f, &changed, label, code)?;
    }
    for (label, premises) in [
        ("missing premise", Vec::new()),
        ("duplicate premise", vec![0, 0]),
    ] {
        let mut changed = original.clone();
        changed[1].premises = premises;
        refuses(&mut f, &changed, label, InvalidCode::Rule)?;
    }
    let mut dropped = original.clone();
    dropped.remove(0);
    refuses(&mut f, &dropped, "dropped premise", InvalidCode::Rule)?;
    let (foreign, foreign_parts) = fixture(false)?;
    let mut changed = original.clone();
    *endpoints(&mut changed[1].conclusion)?.0 = foreign_parts.sum;
    refuses(
        &mut f,
        &changed,
        "foreign component",
        InvalidCode::ForeignHandle,
    )?;
    let mut changed = original.clone();
    changed[0].conclusion = foreign.steps[0].conclusion.clone();
    refuses(
        &mut f,
        &changed,
        "foreign source",
        InvalidCode::ForeignHandle,
    )?;
    // A checked guard for sine(u) is same-store and valid, but its subject is not u.
    let guard = f
        .cas
        .check_proof(&f.context, &f.steps[..1], &mut default_budget()?)?;
    let other_guard = f
        .cas
        .trig_defined(&parts.sine, &guard.proof, &mut default_budget()?)?;
    let fault = f
        .cas
        .trig_joint(&parts.argument, other_guard.proof(), &mut default_budget()?)
        .err()
        .ok_or("other-subject guard accepted")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    // A rolled-back expression handle belongs to an earlier store revision.
    let mut stale = None;
    let header = f.header.clone();
    let argument = parts.argument.clone();
    let fault = f
        .cas
        .transaction(&mut default_budget()?, |cas, _| {
            stale = Some(
                term(
                    cas,
                    &header,
                    crate::syntax::Operator::IntegerPower(7),
                    std::slice::from_ref(&argument),
                )
                .map_err(|_| {
                    crate::Fault::invalid(InvalidCode::Rule, Location::new("rollback fixture"))
                })?,
            );
            Err::<(), _>(crate::Fault::invalid(
                InvalidCode::Rule,
                Location::new("rollback fixture"),
            ))
        })
        .expect_err("deliberate rollback");
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Rule));
    let mut changed = original;
    *endpoints(&mut changed[1].conclusion)?.0 = stale.ok_or("stale handle")?;
    refuses(&mut f, &changed, "stale component", InvalidCode::Reference)?;
    Ok(())
}

/// A receiver checks the proof first and then compares a distinct valid judgment.
/// ´test:crate:trig-joint-receiving-store-replay´
#[test]
fn trig_joint_receiving_store_replay() -> TestResult {
    let (mut source, parts) = fixture(false)?;
    let witness = producer(&mut source, &parts)?;
    let mut syntax_budget = default_budget()?;
    let mut syntax =
        crate::encoding::Encoder::new(&mut syntax_budget, Location::new("joint wire fixture"));
    crate::statement::encode(&source.expected.statement, &mut syntax, |_, out| {
        crate::wire::index(0, out)
    })?;
    assert_eq!(syntax.finish().len(), 14); // Equality tag, two endpoints, domain, contract
    let mut certificate_budget = default_budget()?;
    let mut certificate = crate::encoding::Encoder::new(
        &mut certificate_budget,
        Location::new("joint certificate fixture"),
    );
    crate::statement::certificate(&Certificate::None, &mut certificate, |_, out| {
        crate::wire::index(0, out)
    })?;
    assert_eq!(certificate.finish().len(), 1);
    let bytes = source
        .cas
        .export_witness(&witness, &mut default_budget()?)?;
    let (mut receiver, _) = fixture(true)?;
    assert_eq!(receiver.cas.state.proof_count(&mut default_budget()?)?, 0);
    let mut replay_budget = default_budget()?;
    let checked = receiver
        .cas
        .replay(&bytes, &receiver.expected, &mut replay_budget)?;
    assert_eq!(checked.judgment(), &receiver.expected);
    let mut other = receiver.expected.clone();
    let empty = receiver
        .cas
        .admit(
            &[crate::test_api::syntax::Draft::Region(
                crate::test_api::domain::RegionDraft {
                    header: crate::test_api::Reference::Existing(receiver.header.clone()),
                    scope: Vec::new(),
                    ambient: crate::test_api::domain::AmbientDomain::Real("x".into()),
                    expression: crate::test_api::domain::RegionExpr::Empty,
                },
            )],
            &mut default_budget()?,
        )?
        .pop()
        .ok_or("narrow domain")?;
    *endpoints(&mut other.statement)?.2 = empty;
    let fault = receiver
        .cas
        .replay(&bytes, &other, &mut default_budget()?)
        .err()
        .ok_or("different expected claim accepted")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    let key = b"trig.pythagorean";
    let name_start = bytes
        .windows(key.len())
        .position(|part| part == key)
        .ok_or("joint key")?;
    let entry_start = name_start.checked_sub(4).ok_or("joint entry")?;
    assert_eq!(
        bytes[entry_start..name_start],
        u32::try_from(key.len())?.to_le_bytes()
    );
    let version_start = name_start + key.len();
    assert_eq!(bytes[version_start..version_start + 4], 1_u32.to_le_bytes());
    let mut wrong_version = bytes.clone();
    wrong_version[version_start] = 2;
    let fault = receiver
        .cas
        .replay(&wrong_version, &receiver.expected, &mut default_budget()?)
        .err()
        .ok_or("wrong primitive version accepted")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedVersion)
    );
    let mut duplicate = bytes.clone();
    duplicate.splice(
        version_start + 4..version_start + 4,
        bytes[entry_start..version_start + 4].iter().copied(),
    );
    let count = u32::from_le_bytes(bytes[8..12].try_into()?);
    duplicate[8..12].copy_from_slice(&(count + 1).to_le_bytes());
    let fault = receiver
        .cas
        .replay(&duplicate, &receiver.expected, &mut default_budget()?)
        .err()
        .ok_or("duplicate primitive accepted")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Encoding));
    Ok(())
}
