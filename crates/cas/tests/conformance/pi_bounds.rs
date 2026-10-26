// SPDX-License-Identifier: AGPL-3.0-only
//! Exact Machin reductions, rejected certificates and cumulative resource boundaries.

use crate::testing::common::TestResult;
use crate::testing::readings::ledger;
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::pi_bounds::PiBoundCertificate;
use nrz_cas::{FaultKind, InvalidCode, Location, Rational};

fn rational(text: &str) -> TestResult<Rational> {
    Ok(Rational::parse(text, &mut ledger()?)?)
}

pub(super) fn cycle() -> TestResult<PiBoundCertificate> {
    Ok(PiBoundCertificate {
        version: 1,
        terms: [2, 1],
        sums: [rational("74/375")?, rational("1/239")?],
        lower: rational("640/205")?,
        upper: rational("640/203")?,
    })
}

fn tight() -> TestResult<PiBoundCertificate> {
    Ok(PiBoundCertificate {
        version: 1,
        terms: [21, 7],
        sums: [
            rational(
                "2457995196577241446223727248411469732874429/12452130121095660797436721622943878173828125",
            )?,
            rational("1564655994390665032665206134020499/373954964875115017694768228605199355")?,
        ],
        lower: rational("248902613312231085230521944622/79228162514264337593543950336")?,
        upper: rational("248902613312231085230521944623/79228162514264337593543950336")?,
    })
}

/// LC and adjacent dyadic endpoints follow from independently reconstructed exact sums.
/// ´test:integration:pi-machin-bounds´
#[test]
fn pi_machin_bounds() -> TestResult {
    for certificate in [cycle()?, tight()?] {
        let checked = certificate.check(&mut ledger()?)?;
        assert_eq!(checked.contract(), &certificate);
        let schema = checked.formal_schema();
        assert_eq!(
            (schema.name, schema.version),
            ("constant.pi-machin-bounds", 1)
        );
        assert_eq!(schema.label, "thm:cas:constant-pi-machin-bounds");
    }
    Ok(())
}

fn refusal(certificate: &PiBoundCertificate, expected: InvalidCode) -> TestResult {
    let before = certificate.clone();
    let mut budget = ledger()?;
    let fault = certificate
        .check(&mut budget)
        .err()
        .ok_or("false pi bound accepted")?;
    assert_eq!(fault.kind, FaultKind::Invalid(expected));
    assert_eq!(certificate, &before);
    assert!(budget.usage().work > 0);
    Ok(())
}

/// Inward endpoints, false arithmetic, insufficient terms and unknown revisions confer no authority.
/// ´test:integration:pi-machin-refusals´
#[test]
fn pi_machin_refusals() -> TestResult {
    let original = cycle()?;
    let mut certificate = original.clone();
    certificate.version = 2;
    refusal(&certificate, InvalidCode::UnsupportedVersion)?;
    certificate.clone_from(&original);
    certificate.sums[0] = rational("1/5")?;
    refusal(&certificate, InvalidCode::EndpointMismatch)?;
    certificate.terms[0] = 1;
    refusal(&certificate, InvalidCode::DecisionBox)?;
    certificate.clone_from(&original);
    certificate.lower = rational("22/7")?;
    refusal(&certificate, InvalidCode::DecisionBox)?;
    certificate.clone_from(&original);
    certificate.upper = rational("3")?;
    refusal(&certificate, InvalidCode::DecisionBox)?;
    certificate.clone_from(&original);
    std::mem::swap(&mut certificate.lower, &mut certificate.upper);
    refusal(&certificate, InvalidCode::DecisionBox)?;
    certificate = tight()?;
    certificate.lower.clone_from(&certificate.upper);
    refusal(&certificate, InvalidCode::DecisionBox)?;
    certificate = tight()?;
    certificate.upper.clone_from(&certificate.lower);
    refusal(&certificate, InvalidCode::DecisionBox)
}

/// Empty expansions and both remainder parities retain their stated outward meaning.
/// ´test:integration:pi-machin-remainder-parities´
#[test]
fn pi_machin_remainder_parities() -> TestResult {
    let mut certificate = cycle()?;
    certificate.terms = [0, 0];
    certificate.sums = [Rational::zero(), Rational::zero()];
    certificate.lower = rational("-1")?;
    certificate.upper = rational("4")?;
    certificate.check(&mut ledger()?)?;
    certificate.terms = [1, 2];
    certificate.sums = [rational("1/5")?, rational("171362/40955757")?];
    certificate.check(&mut ledger()?)?;
    Ok(())
}

/// Measured complete checks include arithmetic validation and retained ownership.
/// ´test:integration:pi-machin-costs´
#[test]
fn pi_machin_costs() -> TestResult {
    let mut measurements = Vec::new();
    for certificate in [cycle()?, tight()?] {
        let mut budget = ledger()?;
        let checked = certificate.check(&mut budget)?;
        measurements.push((
            budget.usage().work,
            budget.usage().arithmetic_steps,
            checked.retained_bytes(),
        ));
    }
    assert_eq!(measurements, [(18_179, 539, 680), (355_924, 4_669, 1_104)]);
    Ok(())
}

fn limited(
    certificate: &PiBoundCertificate,
    kind: BudgetKind,
    amount: u64,
    prior: u64,
    accepted: bool,
) -> TestResult {
    let mut limits = Limits::default();
    match kind {
        BudgetKind::Work => limits.work = amount,
        BudgetKind::ArithmeticSteps => limits.arithmetic_steps = amount,
        BudgetKind::OutputBytes => limits.output_bytes = amount,
        _ => return Err("unsupported resource fixture".into()),
    }
    let original = certificate.clone();
    let mut budget = Budget::new(limits.clone())?;
    budget.work(prior, Location::new("prior pi work"))?;
    let result = certificate.check(&mut budget);
    if accepted {
        assert_eq!(result?.contract(), certificate);
    } else {
        let fault = result.err().ok_or("pi certificate exceeded budget")?;
        assert!(matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind));
        assert!(budget.usage().work > prior);
    }
    assert_eq!(certificate, &original);
    assert_eq!(budget.limits(), &limits);
    Ok(())
}

/// Exact N/N−1 checks keep the certificate immutable and prior Work on the monotone ledger.
/// ´test:integration:pi-machin-resources´
#[test]
fn pi_machin_resources() -> TestResult {
    for (certificate, work, steps, bytes) in [
        (cycle()?, 18_179, 539, 680),
        (tight()?, 355_924, 4_669, 1_104),
    ] {
        for (kind, amount) in [
            (BudgetKind::Work, work),
            (BudgetKind::ArithmeticSteps, steps),
            (BudgetKind::OutputBytes, bytes),
        ] {
            limited(&certificate, kind, amount, 0, true)?;
            limited(&certificate, kind, amount - 1, 0, false)?;
        }
        limited(&certificate, BudgetKind::Work, work, 1, false)?;
        limited(&certificate, BudgetKind::Work, work + 1, 1, true)?;
    }
    Ok(())
}
