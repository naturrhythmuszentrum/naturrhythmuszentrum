// SPDX-License-Identifier: AGPL-3.0-only
//! Positive rational roots, exact LC phases and independently squared interval bounds.

mod pi;

use super::{TestResult, ledger, original, rational};
use nrz_cas::decision::{EnclosureOrigin, RationalEnclosure};
use nrz_cas::domain::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::pi_bounds::PiBoundCertificate;
use nrz_cas::syntax::{Branch, Constant, Draft, Operator};
use nrz_cas::{Cas, ExactConstant, Fault, FaultKind, Handle, InvalidCode, Rational, Reference};

/// Wrap the last original operand in a strict-real half power without first normalizing it.
fn rooted(mut operators: Vec<(Operator, Vec<u32>)>) -> TestResult<(Cas, Handle)> {
    let half = u32::try_from(operators.len())?;
    let base = half.checked_sub(1).ok_or("radicand")?;
    operators.push((
        Operator::Rational(Rational::from_ratio(1, 2, &mut ledger()?)?),
        vec![],
    ));
    operators.push((Operator::GeneralPower(Branch::StrictReal), vec![base, half]));
    original(&operators, false)
}

/// Independently specified positive rational radicand.
fn root(numerator: i64, denominator: u64) -> TestResult<ExactConstant> {
    let (cas, expression) = rooted(vec![(
        Operator::Rational(Rational::from_ratio(
            numerator,
            denominator,
            &mut ledger()?,
        )?),
        vec![],
    )])?;
    Ok(ExactConstant::from_expression(
        &cas,
        &expression,
        &mut ledger()?,
    )?)
}

/// Perfect squares and positive products reduce exactly without floating values or sign assumptions.
/// ´test:integration:constant-square-root-exact-rules´
/// ´claim:cas:constant-square-root-exact-rules´
#[test]
fn constant_square_root_exact_rules() -> TestResult {
    assert_eq!(root(9, 49)?, rational(3, 7)?);
    let two = root(2, 1)?;
    assert_eq!(two.as_rational(), None);
    assert!(two.positive().is_some());
    assert_eq!(two.pow(2, &mut ledger()?)?, rational(2, 1)?);
    assert_eq!(two.pow(-2, &mut ledger()?)?, rational(1, 2)?);
    assert_eq!(two.div(&two, &mut ledger()?)?, ExactConstant::one());
    assert_eq!(two.mul(&root(3, 1)?, &mut ledger()?)?, root(6, 1)?);
    assert_eq!(two.mul(&root(8, 1)?, &mut ledger()?)?, rational(4, 1)?);
    assert_eq!(
        root(3, 7)?.mul(&root(7, 3)?, &mut ledger()?)?,
        ExactConstant::one()
    );
    let positive = rational(9, 49)?.positive().ok_or("positive rational")?;
    assert_eq!(positive.sqrt(&mut ledger()?)?, rational(3, 7)?);
    let (cas, expression) = rooted(vec![
        (
            Operator::Rational(Rational::from_ratio(3, 7, &mut ledger()?)?),
            vec![],
        ),
        (Operator::IntegerPower(2), vec![0]),
    ])?;
    assert_eq!(
        ExactConstant::from_expression(&cas, &expression, &mut ledger()?)?,
        rational(3, 7)?
    );
    Ok(())
}

/// Original inductance and capacitance, preserving their product before the root.
fn lc() -> TestResult<(Cas, Handle)> {
    rooted(vec![
        (
            Operator::Rational(Rational::from_ratio(1, 40, &mut ledger()?)?),
            vec![],
        ),
        (
            Operator::Rational(Rational::from_ratio(1, 100_000, &mut ledger()?)?),
            vec![],
        ),
        (Operator::Mul, vec![0, 1]),
    ])
}

/// The original LC product derives its period, and the duration phase keeps exact radian scaling.
/// ´test:integration:constant-square-root-lc-phase´
/// ´claim:cas:constant-square-root-lc-phase´
#[test]
fn constant_square_root_lc_phase() -> TestResult {
    let (cas, expression) = lc()?;
    let tau = ExactConstant::from_expression(&cas, &expression, &mut ledger()?)?;
    assert_eq!(tau, rational(1, 2_000)?);
    let duration = rational(1, 100)?;
    let radian = ExactConstant::one();
    let phase = radian.mul(&duration.div(&tau, &mut ledger()?)?, &mut ledger()?)?;
    assert_eq!(phase, rational(20, 1)?);
    let pi = ExactConstant::named(Constant::Pi, &mut ledger()?)?;
    let turn = rational(2, 1)?.mul(&pi, &mut ledger()?)?;
    assert_eq!(
        phase.div(&turn, &mut ledger()?)?,
        rational(10, 1)?.div(&pi, &mut ledger()?)?
    );
    Ok(())
}

/// Mutation-specific original-domain refusals cannot be erased by arithmetic normalization.
/// ´test:integration:constant-square-root-refusals´
/// ´claim:cas:constant-square-root-refusals´
#[test]
fn constant_square_root_refusals() -> TestResult {
    for number in [0, -1] {
        let (cas, expression) = rooted(vec![(
            Operator::Rational(Rational::from_ratio(number, 1, &mut ledger()?)?),
            vec![],
        )])?;
        refused(
            &cas,
            &expression,
            InvalidCode::RefutedDomain,
            "exact constant expression",
        )?;
    }
    let (cas, expression) = rooted(vec![
        (Operator::Constant(Constant::Pi), vec![]),
        (Operator::Rational(Rational::one()), vec![]),
        (Operator::Sub, vec![0, 1]),
    ])?;
    refused(
        &cas,
        &expression,
        InvalidCode::PendingPremise,
        "exact constant expression",
    )?;
    let (cas, expression) = rooted(vec![
        (Operator::Rational(Rational::one()), vec![]),
        (Operator::Rational(Rational::zero()), vec![]),
        (Operator::Div, vec![0, 1]),
    ])?;
    refused(
        &cas,
        &expression,
        InvalidCode::RefutedDomain,
        "exact constant expression",
    )?;
    Ok(())
}

/// Other powers and unsupported named radicands remain explicit fragment refusals.
/// ´test:integration:constant-square-root-fragment´
/// ´claim:cas:constant-square-root-fragment´
#[test]
fn constant_square_root_fragment() -> TestResult {
    let (cas, expression) = rooted(vec![(Operator::Constant(Constant::E), vec![])])?;
    refused(
        &cas,
        &expression,
        InvalidCode::UnsupportedFragment,
        "finite coefficient jets",
    )?;
    for (branch, denominator) in [(Branch::PrincipalComplex, 2), (Branch::StrictReal, 3)] {
        let (cas, expression) = original(
            &[
                (Operator::Rational(Rational::one()), vec![]),
                (
                    Operator::Rational(Rational::from_ratio(1, denominator, &mut ledger()?)?),
                    vec![],
                ),
                (Operator::GeneralPower(branch), vec![0, 1]),
            ],
            false,
        )?;
        let spent = refused(
            &cas,
            &expression,
            InvalidCode::UnsupportedFragment,
            "exact constant expression",
        )?;
        assert_eq!(spent, 1_047);
    }
    Ok(())
}

/// Pin the actual rejecting checker and unchanged source graph, retaining spent work.
fn refused(cas: &Cas, root: &Handle, expected: InvalidCode, operation: &str) -> TestResult<u64> {
    let before = cas.canonical_snapshot(&mut ledger()?)?;
    let mut budget = ledger()?;
    let Err(fault) = ExactConstant::from_expression(cas, root, &mut budget) else {
        panic!("expected square-root refusal");
    };
    assert_eq!(fault.kind, FaultKind::Invalid(expected));
    assert_eq!(fault.location.operation, operation);
    assert!(budget.usage().work > 0);
    assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, before);
    Ok(budget.usage().work)
}

/// Existing finite Machin origin, independently checked before closed interval propagation.
fn certificate() -> TestResult<PiBoundCertificate> {
    Ok(PiBoundCertificate {
        version: 1,
        terms: [2, 1],
        sums: [
            Rational::parse("74/375", &mut ledger()?)?,
            Rational::parse("1/239", &mut ledger()?)?,
        ],
        lower: Rational::parse("640/205", &mut ledger()?)?,
        upper: Rational::parse("640/203", &mut ledger()?)?,
    })
}

/// An original square root with an exact context and an outward rational interval request.
fn enclosure() -> TestResult<(Cas, RationalEnclosure)> {
    let (cas, target) = rooted(vec![(
        Operator::Rational(Rational::from_ratio(2, 1, &mut ledger()?)?),
        vec![],
    )])?;
    enclosure_for(cas, target)
}

/// Bind the exact original expression to its actual family before checking a closed box.
fn enclosure_for(mut cas: Cas, target: Handle) -> TestResult<(Cas, RationalEnclosure)> {
    let header = cas.expression_header(&target)?;
    let drafts = [
        Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::Ambient,
        }),
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header),
            admissible: Reference::Local(0),
            assumptions: Vec::new(),
        }),
    ];
    let [input_family, context]: [Handle; 2] = cas
        .admit(&drafts, &mut ledger()?)?
        .try_into()
        .map_err(|_| "enclosure context")?;
    let contract = RationalEnclosure {
        context,
        input_family,
        origin: EnclosureOrigin::PiBound {
            expression: target.clone(),
            certificate: Box::new(certificate()?),
        },
        target,
        lower: Rational::one(),
        upper: Rational::from_ratio(2, 1, &mut ledger()?)?,
    };
    Ok((cas, contract))
}

/// Rational endpoint squares certify the exact root; inward bounds fail at interval containment.
/// ´test:integration:constant-square-root-enclosures´
/// ´claim:cas:constant-square-root-enclosures´
#[test]
fn constant_square_root_enclosures() -> TestResult {
    let (cas, contract) = enclosure()?;
    let before = cas.canonical_snapshot(&mut ledger()?)?;
    cas.check_rational_enclosure(&contract, &mut ledger()?)?;
    let mut inward = contract;
    inward.lower = Rational::from_ratio(3, 2, &mut ledger()?)?;
    let Err(fault) = cas.check_rational_enclosure(&inward, &mut ledger()?) else {
        panic!("inward square-root bound");
    };
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::DecisionBox));
    assert_eq!(fault.location.operation, "closed pi enclosure");
    assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, before);
    Ok(())
}

/// Pin each complete operation at literal N/N−1, including prior Work and a spent ledger.
fn pin<T>(operation: impl Fn(&mut Budget) -> Result<T, Fault>, work: u64) -> TestResult {
    for ceiling in [work, work - 1] {
        let mut budget = Budget::new(Limits {
            work: ceiling,
            ..Limits::default()
        })?;
        let result = operation(&mut budget);
        if ceiling == work {
            result?;
            assert_eq!(budget.usage().work, work);
        } else {
            let Err(fault) = result else {
                panic!("square-root Work boundary");
            };
            assert!(matches!(
                fault.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::Work,
                    ..
                }
            ));
            assert!(budget.usage().work > 0);
        }
    }
    let mut prior = Budget::new(Limits {
        work: work + 7,
        ..Limits::default()
    })?;
    prior.work(7, nrz_cas::Location::new("prior caller work"))?;
    operation(&mut prior)?;
    assert_eq!(prior.usage().work, work + 7);
    let Err(fault) = operation(&mut prior) else {
        panic!("spent square-root ledger");
    };
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::Work,
            ..
        }
    ));
    assert_eq!(prior.usage().work, work + 7);
    Ok(())
}

/// Literal costs for admission and the complete enclosure lifecycle of one original.
struct Costs {
    /// Original finite-real admission and normalization equality.
    admission: u64,
    /// Complete certified outward enclosure including original admission.
    enclosure: u64,
}

/// Exact root admission and certified enclosures preserve literal costs and atomic Work refusals.
/// ´test:integration:constant-square-root-costs´
/// ´claim:cas:constant-square-root-costs´
#[test]
fn constant_square_root_costs() -> TestResult {
    let cases = [
        (
            enclosure()?,
            Costs {
                admission: 57_657,
                enclosure: 82_954,
            },
        ),
        (
            lc_enclosure()?,
            Costs {
                admission: 31_324,
                enclosure: 33_668,
            },
        ),
    ];
    for ((cas, contract), costs) in cases {
        let before = cas.canonical_snapshot(&mut ledger()?)?;
        pin(
            |budget| ExactConstant::from_expression(&cas, &contract.target, budget),
            costs.admission,
        )?;
        pin(
            |budget| cas.check_rational_enclosure(&contract, budget),
            costs.enclosure,
        )?;
        assert_eq!(cas.canonical_snapshot(&mut ledger()?)?, before);
    }
    Ok(())
}

/// Pin the independent Machin check and its contribution to the root enclosure ledger.
/// ´test:integration:constant-square-root-breakdown´
/// ´claim:cas:constant-square-root-breakdown´
#[test]
fn constant_square_root_breakdown() -> TestResult {
    let certificate = certificate()?;
    pin(|budget| certificate.check(budget), 18_179)?;
    let mut budget = ledger()?;
    let checked = certificate.check(&mut budget)?;
    assert_eq!(budget.usage().work, 18_179);
    assert_eq!(checked.retained_bytes(), 680);
    let validation = budget.usage().work - checked.retained_bytes();
    assert_eq!(validation, 17_499);
    assert_eq!(82_954 - 57_657 - validation, 7_798);
    Ok(())
}

/// The rational LC period needs only the existing exact-rational enclosure origin.
fn lc_enclosure() -> TestResult<(Cas, RationalEnclosure)> {
    let (cas, target) = lc()?;
    let (cas, mut contract) = enclosure_for(cas, target)?;
    let value = Rational::from_ratio(1, 2_000, &mut ledger()?)?;
    contract.lower = value.clone();
    contract.upper = value.clone();
    contract.origin = EnclosureOrigin::ExactRational {
        expression: contract.target.clone(),
        value,
    };
    Ok((cas, contract))
}

/// The exact LC radical replays as a singleton rational enclosure of the original product.
/// ´test:integration:constant-square-root-lc-enclosure´
/// ´claim:cas:constant-square-root-lc-enclosure´
#[test]
fn constant_square_root_lc_enclosure() -> TestResult {
    let (cas, contract) = lc_enclosure()?;
    cas.check_rational_enclosure(&contract, &mut ledger()?)?;
    let mut changed = contract;
    changed.origin = EnclosureOrigin::ExactRational {
        expression: changed.target.clone(),
        value: Rational::from_ratio(1, 1_000, &mut ledger()?)?,
    };
    let Err(fault) = cas.check_rational_enclosure(&changed, &mut ledger()?) else {
        panic!("false LC period");
    };
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::EndpointMismatch)
    );
    assert_eq!(fault.location.operation, "exact rational enclosure");
    Ok(())
}

/// Square-root checking retains arbitrary-precision components beyond machine integers.
/// ´test:integration:constant-square-root-large-components´
/// ´claim:cas:constant-square-root-large-components´
#[test]
fn constant_square_root_large_components() -> TestResult {
    let base = rational(2, 1)?;
    let square = base.pow(128, &mut ledger()?)?;
    let expected = base.pow(64, &mut ledger()?)?;
    let positive = square.positive().ok_or("positive large square")?;
    assert_eq!(positive.sqrt(&mut ledger()?)?, expected);
    Ok(())
}

/// An outer zero cannot hide an undefined root or authorize splitting negative factors.
/// ´test:integration:constant-square-root-hidden-domain´
/// ´claim:cas:constant-square-root-hidden-domain´
#[test]
fn constant_square_root_hidden_domain() -> TestResult {
    let (cas, expression) = original(
        &[
            (
                Operator::Rational(Rational::from_ratio(-1, 1, &mut ledger()?)?),
                vec![],
            ),
            (
                Operator::Rational(Rational::from_ratio(1, 2, &mut ledger()?)?),
                vec![],
            ),
            (Operator::GeneralPower(Branch::StrictReal), vec![0, 1]),
            (Operator::Rational(Rational::zero()), vec![]),
            (Operator::Mul, vec![3, 2]),
        ],
        false,
    )?;
    refused(
        &cas,
        &expression,
        InvalidCode::RefutedDomain,
        "exact constant expression",
    )?;
    let (cas, expression) = rooted(vec![
        (
            Operator::Rational(Rational::from_ratio(-2, 1, &mut ledger()?)?),
            vec![],
        ),
        (
            Operator::Rational(Rational::from_ratio(-3, 1, &mut ledger()?)?),
            vec![],
        ),
        (Operator::Mul, vec![0, 1]),
    ])?;
    assert_eq!(
        ExactConstant::from_expression(&cas, &expression, &mut ledger()?)?,
        root(6, 1)?
    );
    Ok(())
}

/// Fractional radicands use squared rational bounds, including numerator-denominator scaling.
/// ´test:integration:constant-square-root-fractional-enclosure´
/// ´claim:cas:constant-square-root-fractional-enclosure´
#[test]
fn constant_square_root_fractional_enclosure() -> TestResult {
    let (cas, target) = rooted(vec![(
        Operator::Rational(Rational::from_ratio(2, 3, &mut ledger()?)?),
        vec![],
    )])?;
    let (cas, mut contract) = enclosure_for(cas, target)?;
    contract.lower = Rational::from_ratio(2, 3, &mut ledger()?)?;
    contract.upper = Rational::one();
    cas.check_rational_enclosure(&contract, &mut ledger()?)?;
    Ok(())
}

/// Positive nested radicals outside the rational-radicand fragment never acquire denesting rules.
/// ´test:integration:constant-square-root-nested-boundary´
/// ´claim:cas:constant-square-root-nested-boundary´
#[test]
fn constant_square_root_nested_boundary() -> TestResult {
    let (cas, expression) = rooted(vec![
        (
            Operator::Rational(Rational::from_ratio(2, 1, &mut ledger()?)?),
            vec![],
        ),
        (
            Operator::Rational(Rational::from_ratio(1, 2, &mut ledger()?)?),
            vec![],
        ),
        (Operator::GeneralPower(Branch::StrictReal), vec![0, 1]),
        (Operator::Mul, vec![0, 2]),
        (
            Operator::Rational(Rational::from_ratio(3, 1, &mut ledger()?)?),
            vec![],
        ),
        (Operator::Add, vec![4, 3]),
    ])?;
    refused(
        &cas,
        &expression,
        InvalidCode::UnsupportedFragment,
        "finite coefficient jets",
    )?;
    Ok(())
}
