// SPDX-License-Identifier: AGPL-3.0-only
//! Public-contract checks for proofs.

use nrz_cas::limits::{Budget, BudgetKind, Limits};
use nrz_cas::proof::{GuardKind, Judgment, Primitive, Statement};
use nrz_cas::syntax::{
    AtomicSort, Declaration, Draft, ExpressionDraft, Field, Operator, Sort, Substitution,
};
use nrz_cas::{Cas, FaultKind, InvalidCode, Rational, Reference};

use crate::testing::budgets::proof_budget as budget;
use crate::testing::builders::at;
use crate::testing::common::{TestResult, real};
use crate::testing::proofs::{Fixture, cancellation};
use crate::testing::wire::canonical_fixture;
use crate::testing::witness::{derivative, equality};

/// Cancellation retains its guard and publishes both application and proof graph vertices.
/// ´test:integration:conditional-cancellation´
#[test]
fn conditional_cancellation() -> TestResult {
    let (mut fixture, _, step) = cancellation()?;
    let before = fixture.cas.counts();
    let witness = fixture.cas.check_proof(
        &fixture.context,
        &[step],
        &mut Budget::new(Limits::default())?,
    )?;
    assert!(!witness.is_discharged());
    assert_eq!(
        witness.judgment().residuals,
        vec![crate::testing::witness::guarded(
            &mut fixture.cas,
            &fixture.context,
            GuardKind::NonZero,
            &(fixture.x)
        )?],
    );
    let no_hypotheses: &[nrz_cas::domain::HypothesisRef] = &[];
    assert_eq!(witness.hypotheses(), no_hypotheses);
    assert!(fixture.cas.counts().vertices >= before.vertices + 3);
    assert_eq!(
        witness.proof().logical_id().kind,
        nrz_cas::syntax::VertexKind::Proof
    );
    Ok(())
}

/// A false equality cannot publish any candidate record or consume a logical serial.
/// ´test:integration:false-proof-rollback´
#[test]
fn false_proof_rollback() -> TestResult {
    let (mut fixture, quotient, _) = cancellation()?;
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    let bad = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::Cancel,
        &quotient,
        &fixture.zero,
    );
    let fault = fixture
        .cas
        .check_proof(&fixture.context, &[bad], &mut budget()?)
        .err()
        .ok_or("accepted false cancellation")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Rule));
    assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// A zero product cannot erase the undefined quotient that appears in its original operand.
/// ´test:integration:proof-retains-partial-operands´
#[test]
fn proof_retains_partial_operands() -> TestResult {
    let mut fixture = Fixture::new()?;
    let bad = fixture.term(Operator::Div, &[fixture.one.clone(), fixture.zero.clone()])?;
    let product = fixture.term(Operator::Mul, &[bad, fixture.zero.clone()])?;
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    let fault = fixture
        .cas
        .check_proof(
            &fixture.context,
            &[equality(
                &fixture.context,
                &fixture.domain,
                Primitive::MulZero,
                &product,
                &fixture.zero,
            )],
            &mut budget()?,
        )
        .err()
        .ok_or("accepted an undefined original operand")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::RefutedDomain));
    assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// Exact rational arithmetic is recomputed, and an altered result is refused.
/// ´test:integration:exact-proof-arithmetic´
#[test]
fn exact_proof_arithmetic() -> TestResult {
    let mut fixture = Fixture::new()?;
    let sum = fixture.term(Operator::Add, &[fixture.one.clone(), fixture.two.clone()])?;
    let three = fixture.term(
        Operator::Rational(Rational::from_ratio(3, 1, &mut budget()?)?),
        &[],
    )?;
    let witness = fixture.cas.check_proof(
        &fixture.context,
        &[equality(
            &fixture.context,
            &fixture.domain,
            Primitive::Exact,
            &sum,
            &three,
        )],
        &mut budget()?,
    )?;
    assert!(witness.is_discharged());
    assert!(
        fixture
            .cas
            .check_proof(
                &fixture.context,
                &[equality(
                    &fixture.context,
                    &fixture.domain,
                    Primitive::Exact,
                    &sum,
                    &fixture.two
                )],
                &mut budget()?
            )
            .is_err()
    );
    Ok(())
}

/// The finite-jet identity uses exact coefficients and independently retains the quotient guard.
/// ´test:integration:finite-jet-difference-quotient´
#[test]
fn finite_jet_difference_quotient() -> TestResult {
    let mut fixture = Fixture::new()?;
    let sum = fixture.term(Operator::Add, &[fixture.x.clone(), fixture.y.clone()])?;
    let square = fixture.term(Operator::IntegerPower(2), &[sum])?;
    let x_square = fixture.term(Operator::IntegerPower(2), &[fixture.x.clone()])?;
    let difference = fixture.term(Operator::Sub, &[square, x_square])?;
    let quotient = fixture.term(Operator::Div, &[difference, fixture.y.clone()])?;
    let twice_x = fixture.term(Operator::Mul, &[fixture.two.clone(), fixture.x.clone()])?;
    let target = fixture.term(Operator::Add, &[twice_x, fixture.y.clone()])?;
    let witness = fixture.cas.check_proof(
        &fixture.context,
        &[equality(
            &fixture.context,
            &fixture.domain,
            Primitive::RationalIdentity,
            &quotient,
            &target,
        )],
        &mut budget()?,
    )?;
    assert_eq!(
        witness.judgment().residuals,
        vec![crate::testing::witness::guarded(
            &mut fixture.cas,
            &fixture.context,
            GuardKind::NonZero,
            &(fixture.y.clone())
        )?],
    );
    assert!(
        fixture
            .cas
            .check_proof(
                &fixture.context,
                &[equality(
                    &fixture.context,
                    &fixture.domain,
                    Primitive::Jets,
                    &quotient,
                    &target
                )],
                &mut budget()?
            )
            .is_err()
    );
    assert!(
        fixture
            .cas
            .check_proof(
                &fixture.context,
                &[equality(
                    &fixture.context,
                    &fixture.domain,
                    Primitive::Jets,
                    &fixture.x,
                    &fixture.y
                )],
                &mut budget()?
            )
            .is_err()
    );
    Ok(())
}

/// Strict zero-power differentiation retains nonzero, while the one-power case adds no guard.
/// ´test:integration:derivative-power-domain-cases´
#[test]
fn derivative_power_domain_cases() -> TestResult {
    let mut fixture = Fixture::new()?;
    let power_zero = fixture.term(Operator::IntegerPower(0), &[fixture.x.clone()])?;
    let power_one = fixture.term(Operator::IntegerPower(1), &[fixture.x.clone()])?;
    let variable = derivative(
        &fixture.context,
        &fixture.domain,
        Primitive::DifferentiateVariable,
        &fixture.x,
        &fixture.one,
        &[],
    );
    let zero_steps = [
        variable.clone(),
        derivative(
            &fixture.context,
            &fixture.domain,
            Primitive::DifferentiatePower,
            &power_zero,
            &fixture.zero,
            &[0],
        ),
    ];
    let zero = fixture
        .cas
        .check_proof(&fixture.context, &zero_steps, &mut budget()?)?;
    assert_eq!(
        zero.judgment().residuals,
        vec![crate::testing::witness::guarded(
            &mut fixture.cas,
            &fixture.context,
            GuardKind::NonZero,
            &(fixture.x.clone())
        )?],
    );
    let one_steps = [
        variable,
        derivative(
            &fixture.context,
            &fixture.domain,
            Primitive::DifferentiatePower,
            &power_one,
            &fixture.one,
            &[0],
        ),
    ];
    let one = fixture
        .cas
        .check_proof(&fixture.context, &one_steps, &mut budget()?)?;
    assert!(one.is_discharged());
    Ok(())
}

/// Simultaneous equality instantiation transports every old guard before discharging it.
/// ´test:integration:substituted-proof-guards´
#[test]
fn substituted_proof_guards() -> TestResult {
    let (mut fixture, _, first) = cancellation()?;
    let two_over_two = fixture.term(Operator::Div, &[fixture.two.clone(), fixture.two.clone()])?;
    let mut second = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::Substitute,
        &two_over_two,
        &fixture.one,
    );
    second.premises = vec![0];
    second.substitutions = vec![Substitution {
        key: "x".into(),
        replacement: fixture.two.clone(),
    }];
    let witness =
        fixture
            .cas
            .check_proof(&fixture.context, &[first.clone(), second], &mut budget()?)?;
    assert!(witness.is_discharged());
    let zero_over_zero =
        fixture.term(Operator::Div, &[fixture.zero.clone(), fixture.zero.clone()])?;
    let mut bad = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::Substitute,
        &zero_over_zero,
        &fixture.one,
    );
    bad.premises = vec![0];
    bad.substitutions = vec![Substitution {
        key: "x".into(),
        replacement: fixture.zero.clone(),
    }];
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    let fault = fixture
        .cas
        .check_proof(&fixture.context, &[first, bad], &mut budget()?)
        .err()
        .ok_or("accepted zero substitution")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::RefutedDomain));
    assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// Every truncation and trailing suffix refuses without publishing its decoded prefix.
/// ´test:integration:canonical-wire-refusals´
#[test]
fn canonical_wire_refusals() -> TestResult {
    let (mut fixture, _, step) = cancellation()?;
    let witness = fixture
        .cas
        .check_proof(&fixture.context, &[step], &mut budget()?)?;
    let bytes = fixture.cas.export_witness(&witness, &mut budget()?)?;
    let mut receiver = Cas::new(budget()?.limits().clone())?;
    let before = receiver.canonical_snapshot(&mut budget()?)?;
    for length in 0..bytes.len() {
        assert!(
            receiver
                .inspect(bytes.get(..length).ok_or("bad test slice")?, &mut budget()?,)
                .is_err()
        );
        assert_eq!(receiver.counts().vertices, 0);
    }
    let mut extended = bytes.clone();
    extended.push(0);
    assert!(receiver.inspect(&extended, &mut budget()?).is_err());
    let mut version = bytes.clone();
    let version_position = 16_usize
        .checked_add(Primitive::Cancel.specification().key.len())
        .ok_or("position overflow")?;
    *version.get_mut(version_position).ok_or("missing version")? = 2;
    let fault = receiver
        .inspect(&version, &mut budget()?)
        .err()
        .ok_or("accepted unknown rule version")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedVersion)
    );
    assert_eq!(receiver.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// Explicit forward premises, unused proof nodes and wrong-store inputs all refuse.
/// ´test:integration:proof-reference-refusals´
#[test]
fn proof_reference_refusals() -> TestResult {
    let (mut fixture, _, step) = cancellation()?;
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    let mut recursive = step.clone();
    recursive.premises = vec![0];
    assert!(
        fixture
            .cas
            .check_proof(&fixture.context, &[recursive], &mut budget()?)
            .is_err()
    );
    assert!(
        fixture
            .cas
            .check_proof(
                &fixture.context,
                &[step.clone(), step.clone()],
                &mut budget()?,
            )
            .is_err()
    );
    let foreign = Fixture::new()?;
    let mut bad = step;
    let Statement::Equality { right, .. } = &mut bad.conclusion else {
        return Err("expected equality proposal".into());
    };
    *right = foreign.one;
    assert!(
        fixture
            .cas
            .check_proof(&fixture.context, &[bad], &mut budget()?)
            .is_err()
    );
    assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// Shortcut admission requires a positive strict improvement and preserves the older proof.
/// ´test:integration:derived-shortcut-improvement´
#[test]
fn derived_shortcut_improvement() -> TestResult {
    let mut fixture = Fixture::new()?;
    let sum = fixture.term(Operator::Add, &[fixture.x.clone(), fixture.x.clone()])?;
    let product = fixture.term(Operator::Mul, &[fixture.two.clone(), fixture.x.clone()])?;
    let witness = fixture.cas.check_proof(
        &fixture.context,
        &[equality(
            &fixture.context,
            &fixture.domain,
            Primitive::Jets,
            &sum,
            &product,
        )],
        &mut budget()?,
    )?;
    let handle = fixture
        .cas
        .register_derived(&witness, witness.judgment(), 1, &mut budget()?)?;
    assert_eq!(
        handle.logical_id().kind,
        nrz_cas::syntax::VertexKind::Application
    );
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    assert!(
        fixture
            .cas
            .register_derived(&witness, witness.judgment(), 1, &mut budget()?)
            .is_err()
    );
    assert!(
        fixture
            .cas
            .register_derived(&witness, witness.judgment(), 0, &mut budget()?)
            .is_err()
    );
    assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// Witness bytes are reproducible, and exact byte/proof/replay ceilings refuse before publication.
/// ´test:integration:proof-resource-boundaries´
#[test]
fn proof_resource_boundaries() -> TestResult {
    let (mut a, _, first) = cancellation()?;
    let (mut b, _, second) = cancellation()?;
    let x = a.cas.check_proof(&a.context, &[first], &mut budget()?)?;
    let y = b
        .cas
        .check_proof(&b.context, std::slice::from_ref(&second), &mut budget()?)?;
    let bytes = a.cas.export_witness(&x, &mut budget()?)?;
    assert_eq!(bytes, b.cas.export_witness(&y, &mut budget()?)?);
    let length = u64::try_from(bytes.len())?;
    let mut exact = Budget::new(Limits {
        witness_bytes: length,
        ..Limits::default()
    })?;
    assert_eq!(a.cas.export_witness(&x, &mut exact)?, bytes);
    let mut short = Budget::new(Limits {
        witness_bytes: length.checked_sub(1).ok_or("empty witness")?,
        ..Limits::default()
    })?;
    assert!(a.cas.export_witness(&x, &mut short).is_err());
    let before = b.cas.canonical_snapshot(&mut budget()?)?;
    let mut no_proofs = Budget::new(Limits {
        proof_nodes: 0,
        work: 100_000_000,
        ..Limits::default()
    })?;
    assert!(
        b.cas
            .check_proof(&b.context, &[second], &mut no_proofs)
            .is_err()
    );
    let mut no_replay = Budget::new(Limits {
        replay_work: 0,
        work: 100_000_000,
        ..Limits::default()
    })?;
    let fault = b
        .cas
        .inspect(&bytes, &mut no_replay)
        .err()
        .ok_or("unmetered replay")?;
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::ReplayWork,
            ..
        }
    ));
    assert_eq!(b.cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// Field schemas preserve operand order except at the one explicitly checked law.
/// ´test:integration:closed-field-schemas´
#[test]
fn closed_field_schemas() -> TestResult {
    let mut fixture = Fixture::new()?;
    let xy = fixture.term(Operator::Add, &[fixture.x.clone(), fixture.y.clone()])?;
    let yx = fixture.term(Operator::Add, &[fixture.y.clone(), fixture.x.clone()])?;
    let product = fixture.term(Operator::Mul, &[fixture.x.clone(), xy.clone()])?;
    let xx = fixture.term(Operator::Mul, &[fixture.x.clone(), fixture.x.clone()])?;
    let times_y = fixture.term(Operator::Mul, &[fixture.x.clone(), fixture.y.clone()])?;
    let distributed = fixture.term(Operator::Add, &[xx, times_y])?;
    let x_zero = fixture.term(Operator::Add, &[fixture.x.clone(), fixture.zero.clone()])?;
    let x_one = fixture.term(Operator::Mul, &[fixture.x.clone(), fixture.one.clone()])?;
    let negative = fixture.term(Operator::Neg, &[fixture.x.clone()])?;
    let opposite = fixture.term(Operator::Add, &[fixture.x.clone(), negative])?;
    for step in [
        equality(
            &fixture.context,
            &fixture.domain,
            Primitive::AddCommute,
            &xy,
            &yx,
        ),
        equality(
            &fixture.context,
            &fixture.domain,
            Primitive::Distribute,
            &product,
            &distributed,
        ),
        equality(
            &fixture.context,
            &fixture.domain,
            Primitive::AddZero,
            &x_zero,
            &fixture.x,
        ),
        equality(
            &fixture.context,
            &fixture.domain,
            Primitive::MulOne,
            &x_one,
            &fixture.x,
        ),
        equality(
            &fixture.context,
            &fixture.domain,
            Primitive::AddInverse,
            &opposite,
            &fixture.zero,
        ),
    ] {
        let witness = fixture
            .cas
            .check_proof(&fixture.context, &[step], &mut budget()?)?;
        assert!(witness.is_discharged());
    }
    Ok(())
}

/// Congruence changes one ordered operand, and transitivity requires the exact middle term.
/// ´test:integration:proof-composition-paths´
#[test]
fn proof_composition_paths() -> TestResult {
    let mut fixture = Fixture::new()?;
    let xy = fixture.term(Operator::Add, &[fixture.x.clone(), fixture.y.clone()])?;
    let yx = fixture.term(Operator::Add, &[fixture.y.clone(), fixture.x.clone()])?;
    let left = fixture.term(Operator::Mul, &[fixture.two.clone(), xy.clone()])?;
    let right = fixture.term(Operator::Mul, &[fixture.two.clone(), yx.clone()])?;
    let first = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::AddCommute,
        &xy,
        &yx,
    );
    let mut second = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::Congruence,
        &left,
        &right,
    );
    second.certificate = nrz_cas::proof::Certificate::Congruence(vec![1]);
    second.premises = vec![0];
    let mut reverse = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::Symmetry,
        &right,
        &left,
    );
    reverse.premises = vec![1];
    let mut final_step = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::Transitivity,
        &left,
        &left,
    );
    final_step.premises = vec![1, 2];
    let steps = [first, second, reverse, final_step];
    assert!(
        fixture
            .cas
            .check_proof(&fixture.context, &steps, &mut budget()?)?
            .is_discharged()
    );
    let mut bad = steps.to_vec();
    bad.get_mut(1).ok_or("missing congruence")?.certificate =
        nrz_cas::proof::Certificate::Congruence(vec![0]);
    assert!(
        fixture
            .cas
            .check_proof(&fixture.context, &bad, &mut budget()?)
            .is_err()
    );
    Ok(())
}

/// Rational subexpression guards discharge after exact evaluation, including nominal sums.
/// ´test:integration:rational-condition-closure´
#[test]
fn rational_condition_closure() -> TestResult {
    let mut fixture = Fixture::new()?;
    let one_plus_zero =
        fixture.term(Operator::Add, &[fixture.one.clone(), fixture.zero.clone()])?;
    let ratio = fixture.term(Operator::Div, &[one_plus_zero.clone(), one_plus_zero])?;
    assert!(
        fixture
            .cas
            .check_proof(
                &fixture.context,
                &[equality(
                    &fixture.context,
                    &fixture.domain,
                    Primitive::Cancel,
                    &ratio,
                    &fixture.one
                )],
                &mut budget()?
            )?
            .is_discharged()
    );
    let difference = fixture.term(Operator::Sub, &[fixture.two.clone(), fixture.two.clone()])?;
    let false_ratio = fixture.term(Operator::Div, &[difference.clone(), difference])?;
    let fault = fixture
        .cas
        .check_proof(
            &fixture.context,
            &[equality(
                &fixture.context,
                &fixture.domain,
                Primitive::Cancel,
                &false_ratio,
                &fixture.one,
            )],
            &mut budget()?,
        )
        .err()
        .ok_or("accepted a rational zero denominator")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::RefutedDomain));
    Ok(())
}

/// A variable ceiling covers the union of a polynomial's monomials, and degree is finite.
/// ´test:integration:coefficient-view-limits´
#[test]
fn coefficient_view_limits() -> TestResult {
    let mut fixture = Fixture::new()?;
    let sum = fixture.term(Operator::Add, &[fixture.x.clone(), fixture.y.clone()])?;
    let step = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::Jets,
        &sum,
        &sum,
    );
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    let mut one_variable = Budget::new(Limits {
        polynomial_variables: 1,
        work: 100_000_000,
        ..Limits::default()
    })?;
    let fault = fixture
        .cas
        .check_proof(&fixture.context, &[step], &mut one_variable)
        .err()
        .ok_or("ignored distinct-variable ceiling")?;
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::PolynomialVariables,
            ..
        },
    ));
    assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    let square = fixture.term(Operator::IntegerPower(2), &[fixture.x.clone()])?;
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    let mut degree_one = Budget::new(Limits {
        polynomial_degree: 1,
        work: 100_000_000,
        ..Limits::default()
    })?;
    assert!(
        fixture
            .cas
            .check_proof(
                &fixture.context,
                &[equality(
                    &fixture.context,
                    &fixture.domain,
                    Primitive::Jets,
                    &square,
                    &square
                )],
                &mut degree_one
            )
            .is_err()
    );
    assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// The expected-claim boundary rejects a caller's mismatched branch interpretation.
/// ´test:integration:expected-context-mismatch´
#[test]
fn expected_context_mismatch() -> TestResult {
    let (mut fixture, quotient, step) = cancellation()?;
    let witness = fixture
        .cas
        .check_proof(&fixture.context, &[step], &mut budget()?)?;
    let bytes = fixture.cas.export_witness(&witness, &mut budget()?)?;
    let mut environment = fixture.environment.clone();
    environment.branch = nrz_cas::syntax::Branch::Declared("different-powers".into());
    let handles = fixture.cas.admit(
        &[
            Draft::Header(nrz_cas::domain::HeaderDraft::Core(environment)),
            Draft::Region(nrz_cas::domain::RegionDraft {
                header: Reference::Local(0),
                scope: Vec::new(),
                ambient: nrz_cas::domain::AmbientDomain::Assignments,
                expression: nrz_cas::domain::RegionExpr::Ambient,
            }),
            Draft::Context(nrz_cas::domain::ContextDraft::Root {
                header: Reference::Local(0),
                admissible: Reference::Local(1),
                assumptions: Vec::new(),
            }),
            Draft::Expression(ExpressionDraft {
                space: None,
                operator: Operator::Free("x".into()),
                sort: real(),
                header: Reference::Local(0),
                scope: Vec::new(),
                binding: None,
                operands: Vec::new(),
            }),
        ],
        &mut budget()?,
    )?;
    let wrong = Judgment {
        context: fixture.context.clone(),
        layer: nrz_cas::domain::Layer::Pointwise,
        space: None,
        statement: Statement::Equality {
            left: quotient,
            right: fixture.one.clone(),
            domain: fixture.domain.clone(),
            domain_contract: nrz_cas::proof::DomainContract::DefinedAndEqual,
        },
        residuals: vec![crate::testing::witness::guarded(
            &mut fixture.cas,
            &fixture.context,
            GuardKind::NonZero,
            &(at(&handles, 3)?),
        )?],
    };
    let fault = fixture
        .cas
        .replay(&bytes, &wrong, &mut budget()?)
        .err()
        .ok_or("reinterpreted an expected guard")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Context));
    Ok(())
}

/// A supplied Jacobian proves each ordered cell and refuses an altered variable order.
/// ´test:integration:checked-jacobian-assembly´
#[test]
fn checked_jacobian_assembly() -> TestResult {
    let mut fixture = Fixture::new()?;
    let syntax_header = fixture.cas.context_header(&fixture.context)?;
    let matrix = |rows, columns, operands: Vec<Reference>| {
        Draft::Expression(ExpressionDraft {
            space: None,
            operator: Operator::Matrix { rows, columns },
            sort: Sort::Value(AtomicSort::Matrix {
                field: Field::Real,
                rows: nrz_cas::syntax::Extent::Literal(rows),
                columns: nrz_cas::syntax::Extent::Literal(columns),
            }),
            header: Reference::Existing(syntax_header.clone()),
            scope: Vec::new(),
            binding: None,
            operands,
        })
    };
    let handles = fixture.cas.admit(
        &[
            matrix(1, 1, vec![Reference::Existing(fixture.x.clone())]),
            matrix(
                1,
                2,
                vec![
                    Reference::Existing(fixture.one.clone()),
                    Reference::Existing(fixture.zero.clone()),
                ],
            ),
        ],
        &mut budget()?,
    )?;
    let first = derivative(
        &fixture.context,
        &fixture.domain,
        Primitive::DifferentiateVariable,
        &fixture.x,
        &fixture.one,
        &[],
    );
    let mut second = derivative(
        &fixture.context,
        &fixture.domain,
        Primitive::DifferentiateVariable,
        &fixture.x,
        &fixture.zero,
        &[],
    );
    second.conclusion = Statement::DerivativeOn {
        function: fixture.x.clone(),
        derivative: fixture.zero.clone(),
        coordinate: "y".into(),
        domain: fixture.domain.clone(),
    };
    let mut assembly = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::Jacobian,
        &at(&handles, 0)?,
        &at(&handles, 1)?,
    );
    assembly.conclusion = Statement::Jacobian {
        input: at(&handles, 0)?,
        output: at(&handles, 1)?,
        domain: fixture.domain.clone(),
        variables: vec!["x".into(), "y".into()],
    };
    assembly.premises = vec![0, 1];
    let witness = fixture.cas.check_proof(
        &fixture.context,
        &[first.clone(), second.clone(), assembly.clone()],
        &mut budget()?,
    )?;
    assert!(witness.is_discharged());
    assembly.conclusion = Statement::Jacobian {
        input: at(&handles, 0)?,
        output: at(&handles, 1)?,
        domain: fixture.domain.clone(),
        variables: vec!["y".into(), "x".into()],
    };
    assert!(
        fixture
            .cas
            .check_proof(&fixture.context, &[first, second, assembly], &mut budget()?,)
            .is_err()
    );
    Ok(())
}

/// Changes to a guard, coefficient, reduced magnitude or forward reference cannot become proof.
/// ´test:integration:witness-payload-tampering´
#[test]
fn witness_payload_tampering() -> TestResult {
    let original = canonical_fixture()?;
    let mut receiver = Cas::new(budget()?.limits().clone())?;
    for (position, byte) in [(247, 0), (211, 2), (173, 4), (304, 255)] {
        let mut changed = original.clone();
        *changed
            .get_mut(position)
            .ok_or("bad independent wire offset")? = byte;
        assert!(receiver.inspect(&changed, &mut budget()?).is_err());
        assert_eq!(receiver.counts().vertices, 0);
    }
    let mut unreduced = original;
    *unreduced.get_mut(211).ok_or("missing numerator")? = 2;
    *unreduced.get_mut(216).ok_or("missing denominator")? = 2;
    let fault = receiver
        .inspect(&unreduced, &mut budget()?)
        .err()
        .ok_or("normalized noncanonical wire")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Encoding));
    assert_eq!(receiver.counts().vertices, 0);
    Ok(())
}

/// Permanent name ceilings and a lowered expression-depth limit also constrain proof APIs.
/// ´test:integration:proof-permanent-limits´
#[test]
fn proof_permanent_limits() -> TestResult {
    let mut small = Cas::new(Limits {
        name_bytes: 3,
        ..Limits::default()
    })?;
    let mut ledger = budget()?;
    let fault = small
        .inspect(&canonical_fixture()?, &mut ledger)
        .err()
        .ok_or("ignored store name limit")?;
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::NameBytes,
            ..
        }
    ));
    assert_eq!(small.counts().vertices, 0);
    assert_eq!(ledger.limits().name_bytes, Limits::default().name_bytes);
    let mut fixture = Fixture::new()?;
    let negative = fixture.term(Operator::Neg, &[fixture.x.clone()])?;
    let mut shallow = Budget::new(Limits {
        depth: 1,
        work: 100_000_000,
        ..Limits::default()
    })?;
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    let fault = fixture
        .cas
        .check_proof(
            &fixture.context,
            &[equality(
                &fixture.context,
                &fixture.domain,
                Primitive::Reflexivity,
                &negative,
                &negative,
            )],
            &mut shallow,
        )
        .err()
        .ok_or("ignored expression depth during proof")?;
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::Depth,
            ..
        }
    ));
    assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// The scalar commutativity checker cannot be reused for ordered matrix multiplication.
/// ´test:integration:matrix-proof-boundary´
#[test]
fn matrix_proof_boundary() -> TestResult {
    let mut fixture = Fixture::new()?;
    let syntax_header = fixture.cas.context_header(&fixture.context)?;
    let sort = Sort::Value(AtomicSort::Matrix {
        field: Field::Real,
        rows: nrz_cas::syntax::Extent::Literal(1),
        columns: nrz_cas::syntax::Extent::Literal(1),
    });
    let matrix = |operator, operands| {
        Draft::Expression(ExpressionDraft {
            space: None,
            operator,
            sort: sort.clone(),
            header: Reference::Existing(syntax_header.clone()),
            scope: Vec::new(),
            binding: None,
            operands,
        })
    };
    let handles = fixture.cas.admit(
        &[
            matrix(
                Operator::Matrix {
                    rows: 1,
                    columns: 1,
                },
                vec![Reference::Existing(fixture.x.clone())],
            ),
            matrix(
                Operator::Matrix {
                    rows: 1,
                    columns: 1,
                },
                vec![Reference::Existing(fixture.y.clone())],
            ),
            matrix(
                Operator::MatrixMultiply,
                vec![Reference::Local(0), Reference::Local(1)],
            ),
            matrix(
                Operator::MatrixMultiply,
                vec![Reference::Local(1), Reference::Local(0)],
            ),
        ],
        &mut budget()?,
    )?;
    let fault = fixture
        .cas
        .check_proof(
            &fixture.context,
            &[equality(
                &fixture.context,
                &fixture.domain,
                Primitive::MulCommute,
                &at(&handles, 2)?,
                &at(&handles, 3)?,
            )],
            &mut budget()?,
        )
        .err()
        .ok_or("used scalar commutativity on matrices")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedFragment)
    );
    Ok(())
}

/// A valid unknown function call stays outside the admitted derivative schemas even with a child
/// proof.
/// ´test:integration:unknown-function-derivative-refusal´
#[test]
fn unknown_function_derivative_refusal() -> TestResult {
    let mut fixture = Fixture::new()?;
    let mut environment = fixture.environment.clone();
    environment.declarations.push(Declaration {
        key: "f".into(),
        display_name: "f".into(),
        sort: Sort::Function {
            inputs: vec![AtomicSort::Scalar(Field::Real)],
            output: AtomicSort::Scalar(Field::Real),
        },
    });
    let handles = fixture.cas.admit(
        &[
            Draft::Header(nrz_cas::domain::HeaderDraft::Core(environment)),
            Draft::Region(nrz_cas::domain::RegionDraft {
                header: Reference::Local(0),
                scope: Vec::new(),
                ambient: nrz_cas::domain::AmbientDomain::Assignments,
                expression: nrz_cas::domain::RegionExpr::Ambient,
            }),
            Draft::Context(nrz_cas::domain::ContextDraft::Root {
                header: Reference::Local(0),
                admissible: Reference::Local(1),
                assumptions: Vec::new(),
            }),
            Draft::Expression(ExpressionDraft {
                space: None,
                operator: Operator::Free("x".into()),
                sort: real(),
                header: Reference::Local(0),
                scope: Vec::new(),
                binding: None,
                operands: Vec::new(),
            }),
            Draft::Expression(ExpressionDraft {
                space: None,
                operator: Operator::Rational(Rational::one()),
                sort: real(),
                header: Reference::Local(0),
                scope: Vec::new(),
                binding: None,
                operands: Vec::new(),
            }),
            Draft::Expression(ExpressionDraft {
                space: None,
                operator: Operator::Rational(Rational::zero()),
                sort: real(),
                header: Reference::Local(0),
                scope: Vec::new(),
                binding: None,
                operands: Vec::new(),
            }),
            Draft::Expression(ExpressionDraft {
                space: None,
                operator: Operator::Call("f".into()),
                sort: real(),
                header: Reference::Local(0),
                scope: Vec::new(),
                binding: None,
                operands: vec![Reference::Local(3)],
            }),
        ],
        &mut budget()?,
    )?;
    let steps = [
        derivative(
            &at(&handles, 2)?,
            &fixture.cas.admissible_region(&at(&handles, 2)?)?,
            Primitive::DifferentiateVariable,
            &at(&handles, 3)?,
            &at(&handles, 4)?,
            &[],
        ),
        derivative(
            &at(&handles, 2)?,
            &fixture.cas.admissible_region(&at(&handles, 2)?)?,
            Primitive::DifferentiateConstant,
            &at(&handles, 6)?,
            &at(&handles, 5)?,
            &[0],
        ),
    ];
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    let fault = fixture
        .cas
        .check_proof(&at(&handles, 2)?, &steps, &mut budget()?)
        .err()
        .ok_or("manufactured an unknown-function derivative")?;
    assert_eq!(fault.kind, FaultKind::Invalid(InvalidCode::Rule));
    assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}

/// Reflexivity remains unregistered as a checker for square-free judgments: a complete
/// multiplicity certificate is required by the dedicated schema.
/// ´test:integration:square-free-checker-remains-unregistered´
#[test]
fn square_free_checker_remains_unregistered() -> TestResult {
    let mut fixture = Fixture::new()?;
    let mut step = equality(
        &fixture.context,
        &fixture.domain,
        Primitive::Reflexivity,
        &fixture.x,
        &fixture.x,
    );
    step.conclusion = Statement::SquareFree {
        input: fixture.x.clone(),
        output: fixture.x.clone(),
        domain: fixture.domain.clone(),
        variable: "x".into(),
        field: Field::Rational,
    };
    let before = fixture.cas.canonical_snapshot(&mut budget()?)?;
    let fault = fixture
        .cas
        .check_proof(&fixture.context, &[step], &mut budget()?)
        .err()
        .ok_or("accepted square-free judgment by reflexivity")?;
    assert_eq!(
        fault.kind,
        FaultKind::Invalid(InvalidCode::UnsupportedFragment)
    );
    assert_eq!(fixture.cas.canonical_snapshot(&mut budget()?)?, before);
    Ok(())
}
