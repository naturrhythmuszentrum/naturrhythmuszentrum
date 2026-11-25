// SPDX-License-Identifier: AGPL-3.0-only
//! Exact special values retain original operands and reject changed evidence.

use crate::testing::common::{TestResult, default_budget};
use crate::testing::half_pi::{Fixture, term};
use crate::testing::trig_values::{atan, circular};
use nrz_cas::proof::{Certificate, HypothesisKind, Primitive, Statement};
use nrz_cas::syntax::{Constant, Operator};
use nrz_cas::{FaultKind, InvalidCode, Rational};

mod original;

fn alternate_zero(f: &mut Fixture, join: usize) -> TestResult<nrz_cas::proof::VerifiedWitness> {
    use nrz_cas::Reference;
    use nrz_cas::domain::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
    use nrz_cas::proof::{ApplicationDraft, DomainContract};
    use nrz_cas::syntax::Draft;
    let zero = term(
        &mut f.cas,
        &f.header,
        Operator::Rational(Rational::zero()),
        &[],
    )?;
    let one = term(
        &mut f.cas,
        &f.header,
        Operator::Rational(Rational::one()),
        &[],
    )?;
    let other = term(&mut f.cas, &f.header, Operator::Mul, &[one, zero.clone()])?;
    let domain = if join == 2 {
        f.cas
            .admit(
                &[Draft::Region(RegionDraft {
                    header: Reference::Existing(f.header.clone()),
                    scope: Vec::new(),
                    ambient: AmbientDomain::Real("x".into()),
                    expression: RegionExpr::Empty,
                })],
                &mut default_budget()?,
            )?
            .pop()
            .ok_or("other domain")?
    } else {
        f.domain.clone()
    };
    let context = if join != 0 {
        f.cas
            .admit(
                &[Draft::Context(ContextDraft::Root {
                    header: Reference::Existing(f.header.clone()),
                    admissible: Reference::Existing(domain.clone()),
                    assumptions: Vec::new(),
                })],
                &mut default_budget()?,
            )?
            .pop()
            .ok_or("other context")?
    } else {
        f.context.clone()
    };
    Ok(f.cas.check_proof(
        &context,
        &[ApplicationDraft {
            rule: if join == 0 {
                Primitive::MulZero
            } else {
                Primitive::Reflexivity
            },
            context: context.clone(),
            conclusion: Statement::Equality {
                left: if join == 0 { other } else { zero.clone() },
                right: zero,
                domain,
                domain_contract: DomainContract::DefinedAndEqual,
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::None,
        }],
        &mut default_budget()?,
    )?)
}

/// Valid equalities with different operands, contexts or domains cannot replace the exact join.
/// ´test:integration:trig-values-valid-premise-joins´
#[test]
fn trig_values_valid_premise_joins() -> TestResult {
    for join in 0..3 {
        let mut f = atan("0", false, false)?;
        let original = f.cas.check_proof(
            &f.context,
            std::slice::from_ref(f.steps.get(1).ok_or("equality")?),
            &mut default_budget()?,
        )?;
        let guard = f.cas.check_proof(
            &f.context,
            f.steps.get(..1).ok_or("guard")?,
            &mut default_budget()?,
        )?;
        let alternate = alternate_zero(&mut f, join)?;
        assert!(matches!(
            alternate.judgment().statement,
            Statement::Equality { .. }
        ));
        assert_ne!(alternate.judgment(), original.judgment());
        let Statement::Equality { left, .. } = &f.expected.statement else {
            return Err("equality".into());
        };
        let snapshot = f.cas.canonical_snapshot(&mut default_budget()?)?;
        let fault = f
            .cas
            .trig_special_value(
                left,
                &[guard.proof().clone(), alternate.proof().clone()],
                &mut default_budget()?,
            )
            .err()
            .ok_or("alternate accepted")?;
        assert_eq!(
            fault.kind,
            FaultKind::Invalid(InvalidCode::EndpointMismatch)
        );
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, snapshot);
        let value = f
            .cas
            .trig_special_value(
                left,
                &[guard.proof().clone(), original.proof().clone()],
                &mut default_budget()?,
            )?
            .ok_or("original join")?;
        assert_eq!(value.judgment(), &f.expected);
    }
    Ok(())
}

fn large_inputs() -> TestResult<(nrz_cas::Cas, Vec<nrz_cas::Handle>)> {
    use crate::testing::common::real;
    use nrz_cas::domain::HeaderDraft;
    use nrz_cas::syntax::{
        AtomicSort, Declaration, Draft, Environment, ExpressionDraft, Field, Sort,
    };
    use nrz_cas::{Cas, Reference};
    let mut cas = Cas::new(default_budget()?.limits().clone())?;
    let header = cas
        .admit(
            &[Draft::Header(HeaderDraft::Core(Environment {
                declarations: vec![Declaration {
                    key: "many".into(),
                    display_name: "many".into(),
                    sort: Sort::Function {
                        inputs: vec![AtomicSort::Scalar(Field::Real); 256],
                        output: AtomicSort::Scalar(Field::Real),
                    },
                }],
                ..Environment::default()
            }))],
            &mut default_budget()?,
        )?
        .pop()
        .ok_or("header")?;
    let literal = term(
        &mut cas,
        &header,
        Operator::Rational(Rational::parse(&"9".repeat(200), &mut default_budget()?)?),
        &[],
    )?;
    let call = term(
        &mut cas,
        &header,
        Operator::Call("many".into()),
        &vec![literal.clone(); 256],
    )?;
    let scoped = cas
        .admit(
            &[Draft::Expression(ExpressionDraft {
                header: Reference::Existing(header),
                space: None,
                operator: Operator::Rational(Rational::zero()),
                sort: real(),
                scope: vec![vec![AtomicSort::Scalar(Field::Real); 8]; 64],
                binding: None,
                operands: Vec::new(),
            })],
            &mut default_budget()?,
        )?
        .pop()
        .ok_or("scoped literal")?;
    Ok((cas, vec![literal, call, scoped]))
}

/// Large admitted payloads remain borrowed through unsupported and malformed-shape exits.
/// ´test:integration:trig-values-borrowed-early-exits´
#[test]
fn trig_values_borrowed_early_exits() -> TestResult {
    use nrz_cas::Location;
    use nrz_cas::limits::{BudgetKind, Limits};
    let (mut cas, inputs) = large_inputs()?;
    let snapshot = cas.canonical_snapshot(&mut default_budget()?)?;
    for input in inputs {
        for extra in [false, true] {
            // 192 checkpoint bytes plus 19 retained caller bytes; no input payload is copied.
            for (work, staging) in [(293, 211), (294, 211), (294, 210)] {
                let proofs = if extra {
                    vec![input.clone(); 3]
                } else {
                    Vec::new()
                };
                let mut budget = nrz_cas::limits::Budget::new(Limits {
                    work,
                    staged_payload_bytes: staging,
                    ..Limits::default()
                })?;
                budget.work(37, Location::new("prior early-exit Work"))?;
                let result = budget.with_retained_payload(
                    19,
                    Location::new("prior early-exit payload"),
                    |budget| cas.trig_special_value(&input, &proofs, budget),
                );
                if staging == 210 {
                    assert!(matches!(
                        result.err().ok_or("staging refusal")?.kind,
                        FaultKind::Exhausted {
                            budget: BudgetKind::StagedPayloadBytes,
                            ..
                        }
                    ));
                } else if work == 293 {
                    assert!(matches!(
                        result.err().ok_or("work refusal")?.kind,
                        FaultKind::Exhausted {
                            budget: BudgetKind::Work,
                            ..
                        }
                    ));
                } else if extra {
                    assert_eq!(
                        result.err().ok_or("shape refusal")?.kind,
                        FaultKind::Invalid(InvalidCode::EndpointMismatch)
                    );
                } else {
                    assert!(result?.is_none());
                }
                assert_eq!(budget.usage().work, if staging == 210 { 293 } else { work });
                assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, snapshot);
            }
        }
    }
    Ok(())
}

fn accept(f: &mut Fixture) -> TestResult {
    let witness = f
        .cas
        .check_proof(&f.context, &f.steps, &mut default_budget()?)?;
    assert_eq!(witness.judgment(), &f.expected);
    assert_eq!(witness.hypotheses().len(), 1);
    assert!(!witness.is_unconditional());
    assert_eq!(
        f.cas.statement_hypothesis_kind(
            &witness.judgment().statement,
            &mut crate::testing::budgets::graph_budget()?
        )?,
        HypothesisKind::Equality
    );
    Ok(())
}

fn reject(f: &mut Fixture) -> TestResult {
    let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
    let counts = f.cas.counts();
    let error = f
        .cas
        .check_proof(&f.context, &f.steps, &mut default_budget()?)
        .err()
        .ok_or("forged special value")?;
    assert!(matches!(error.kind, FaultKind::Invalid(_)), "{error:?}");
    assert_eq!(f.cas.counts(), counts);
    assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
    Ok(())
}

/// Independently transcribed signed residues and exact output tables exceed machine integers.
/// ´test:integration:trig-values-residues´
#[test]
fn trig_values_residues() -> TestResult {
    for (k, q, sine, cosine) in [
        ("0", "0", 0, 1),
        ("1", "1/2", 1, 0),
        ("2", "1", 0, -1),
        ("3", "3/2", -1, 0),
        ("-4", "-2", 0, 1),
        ("-3", "-3/2", 1, 0),
        ("-2", "-1", 0, -1),
        ("-1", "-1/2", -1, 0),
        ("18446744073709551616", "9223372036854775808", 0, 1),
        ("18446744073709551617", "18446744073709551617/2", 1, 0),
        ("18446744073709551618", "9223372036854775809", 0, -1),
        ("18446744073709551619", "18446744073709551619/2", -1, 0),
        ("-18446744073709551616", "-9223372036854775808", 0, 1),
        ("-18446744073709551617", "-18446744073709551617/2", -1, 0),
        ("-18446744073709551618", "-9223372036854775809", 0, -1),
        ("-18446744073709551619", "-18446744073709551619/2", 1, 0),
    ] {
        for (cos, expected) in [(false, sine), (true, cosine)] {
            accept(&mut circular(k, q, cos, expected, false, false)?)?;
        }
    }
    for (cos, expected) in [(false, 0), (true, 1)] {
        accept(&mut circular("0", "0", cos, expected, true, false)?)?;
    }
    accept(&mut atan("0", false, false)?)?;
    accept(&mut atan("1", true, false)?)
}

/// Wrong residues, integers, integrality, ordered slots and original operands cannot establish values.
/// ´test:integration:trig-values-mutations´
#[test]
fn trig_values_mutations() -> TestResult {
    for cos in [false, true] {
        for mutation in 0..7 {
            let expected = if cos { 0 } else { -1 };
            let mut f = circular("3", "3/2", cos, expected, false, false)?;
            accept(&mut f)?;
            match mutation {
                0 => {
                    let right = term(
                        &mut f.cas,
                        &f.header,
                        Operator::Rational(Rational::one()),
                        &[],
                    )?;
                    let Statement::Equality { right: target, .. } =
                        &mut f.steps.last_mut().ok_or("value")?.conclusion
                    else {
                        return Err("equality".into());
                    };
                    *target = right;
                }
                1 | 2 => {
                    let k = Rational::parse(
                        if mutation == 1 { "2" } else { "3/2" },
                        &mut default_budget()?,
                    )?;
                    let recognition = f.steps.get_mut(2).ok_or("recognition")?;
                    let Statement::HalfPiWitness { k: integer, .. } = &mut recognition.conclusion
                    else {
                        return Err("witness".into());
                    };
                    **integer = k.clone();
                    let Certificate::HalfPiWitness { k: integer, .. } =
                        &mut recognition.certificate
                    else {
                        return Err("certificate".into());
                    };
                    **integer = k;
                }
                3 => f.steps.last_mut().ok_or("value")?.premises = vec![1],
                4 => {
                    f.steps.last_mut().ok_or("value")?.certificate =
                        f.steps.get(2).ok_or("witness")?.certificate.clone();
                }
                5 => f.steps.last_mut().ok_or("value")?.premises.clear(),
                _ => {
                    let zero = term(
                        &mut f.cas,
                        &f.header,
                        Operator::Rational(Rational::zero()),
                        &[],
                    )?;
                    let left = term(
                        &mut f.cas,
                        &f.header,
                        if cos {
                            Operator::Cosine
                        } else {
                            Operator::Sine
                        },
                        &[zero],
                    )?;
                    let Statement::Equality { left: target, .. } =
                        &mut f.steps.last_mut().ok_or("value")?.conclusion
                    else {
                        return Err("equality".into());
                    };
                    *target = left;
                }
            }
            reject(&mut f)?;
        }
    }
    Ok(())
}

/// Arctangent independently checks its ordered premises and exact endpoint.
/// ´test:integration:trig-values-atan-mutations´
#[test]
fn trig_values_atan_mutations() -> TestResult {
    for one in [false, true] {
        for mutation in 0..3 {
            let mut f = atan(if one { "1" } else { "0" }, one, false)?;
            accept(&mut f)?;
            if mutation == 0 {
                f.steps.last_mut().ok_or("value")?.premises = vec![1, 0];
            } else {
                let right = if mutation == 1 {
                    term(&mut f.cas, &f.header, Operator::Constant(Constant::E), &[])?
                } else {
                    term(
                        &mut f.cas,
                        &f.header,
                        Operator::Rational(Rational::one()),
                        &[],
                    )?
                };
                let Statement::Equality { right: target, .. } =
                    &mut f.steps.last_mut().ok_or("value")?.conclusion
                else {
                    return Err("equality".into());
                };
                *target = right;
            }
            reject(&mut f)?;
        }
        reject(&mut atan("2", one, false)?)?;
        reject(&mut atan("-1", one, false)?)?;
    }
    Ok(())
}

/// Production consumes checked premises and returns no value for another valid rational input.
/// ´test:integration:trig-values-production´
#[test]
fn trig_values_production() -> TestResult {
    for (input, one, recognized) in [
        ("0", false, true),
        ("1", true, true),
        ("-1", false, false),
        ("2", false, false),
    ] {
        let mut f = atan(input, one, false)?;
        let mut proofs = Vec::new();
        for step in f.steps.get(..2).ok_or("premises")? {
            proofs.push(
                f.cas
                    .check_proof(
                        &f.context,
                        std::slice::from_ref(step),
                        &mut default_budget()?,
                    )?
                    .proof()
                    .clone(),
            );
        }
        let Statement::Equality { left, .. } = &f.expected.statement else {
            return Err("equality".into());
        };
        let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
        let mut malformed = proofs.clone();
        malformed.push(proofs.first().ok_or("premise")?.clone());
        let error = f
            .cas
            .trig_special_value(left, &malformed, &mut default_budget()?)
            .err()
            .ok_or("extra proof accepted")?;
        assert_eq!(
            error.kind,
            FaultKind::Invalid(InvalidCode::EndpointMismatch)
        );
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
        let value = f
            .cas
            .trig_special_value(left, &proofs, &mut default_budget()?)?;
        assert_eq!(value.is_some(), recognized);
        if let Some(value) = value {
            assert_eq!(value.judgment(), &f.expected);
        } else {
            assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
        }
    }
    for cos in [false, true] {
        let mut f = circular("-3", "-3/2", cos, i64::from(!cos), false, false)?;
        let proof = f.cas.check_proof(
            &f.context,
            f.steps.get(..3).ok_or("recognition")?,
            &mut default_budget()?,
        )?;
        let Statement::Equality { left, .. } = &f.expected.statement else {
            return Err("equality".into());
        };
        let value = f
            .cas
            .trig_special_value(left, &[proof.proof().clone()], &mut default_budget()?)?
            .ok_or("special value")?;
        assert_eq!(value.judgment(), &f.expected);
    }
    for primitive in [
        Primitive::SinHalfPi,
        Primitive::CosHalfPi,
        Primitive::AtanZero,
        Primitive::AtanOne,
    ] {
        assert_eq!(
            primitive.specification().premises.hypothesis_kind(),
            HypothesisKind::ExactPremises
        );
    }
    Ok(())
}
