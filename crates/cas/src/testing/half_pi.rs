// SPDX-License-Identifier: AGPL-3.0-only
//! Independent original syntax and expected claims for concrete half-Pi recognition.

use super::common::{TestResult, default_budget, real};
use crate::test_api::domain::{
    AmbientDomain, AssumptionDraft, ContextDraft, HeaderDraft, HypothesisRef, Layer, RegionDraft,
    RegionExpr,
};
use crate::test_api::proof::{
    ApplicationDraft, Certificate, DomainContract, Judgment, Primitive, Statement,
};
use crate::test_api::syntax::{
    AtomicSort, Constant, Declaration, Draft, Environment, ExpressionDraft, Field, Operator,
    PredicateKind, Sort,
};
use crate::test_api::{Cas, Handle, Rational, Reference};

/// Syntax preparation never calls a proof checker, including in receiving stores.
pub(crate) struct Fixture {
    pub(crate) cas: Cas,
    pub(crate) header: Handle,
    pub(crate) context: Handle,
    pub(crate) domain: Handle,
    pub(crate) steps: Vec<ApplicationDraft>,
    pub(crate) expected: Judgment,
}

fn admit(cas: &mut Cas, draft: Draft) -> TestResult<Handle> {
    cas.admit(&[draft], &mut default_budget()?)?
        .pop()
        .ok_or_else(|| "missing half-Pi object".into())
}

pub(crate) fn term(
    cas: &mut Cas,
    header: &Handle,
    operator: Operator,
    operands: &[Handle],
) -> TestResult<Handle> {
    admit(
        cas,
        Draft::Expression(ExpressionDraft {
            header: Reference::Existing(header.clone()),
            space: None,
            operator,
            sort: real(),
            scope: Vec::new(),
            binding: None,
            operands: operands.iter().cloned().map(Reference::Existing).collect(),
        }),
    )
}

/// The coefficient is supplied independently, rather than derived from the candidate integer.
pub(crate) fn prepared(
    integer: &str,
    coefficient: &str,
    zero: bool,
    reordered: bool,
) -> TestResult<Fixture> {
    prepared_with_opaque(integer, coefficient, zero, reordered, false)
}

pub(crate) fn prepared_with_opaque(
    integer: &str,
    coefficient: &str,
    zero: bool,
    reordered: bool,
    opaque: bool,
) -> TestResult<Fixture> {
    let mut cas = Cas::new(default_budget()?.limits().clone())?;
    let mut declarations = vec![Declaration {
        key: "x".into(),
        display_name: "x".into(),
        sort: real(),
    }];
    if opaque {
        declarations.push(Declaration {
            key: "sine".into(),
            display_name: "sine".into(),
            sort: Sort::Function {
                inputs: vec![AtomicSort::Scalar(Field::Real)],
                output: AtomicSort::Scalar(Field::Real),
            },
        });
    }
    let header = admit(
        &mut cas,
        Draft::Header(HeaderDraft::Core(Environment {
            declarations,
            ..Environment::default()
        })),
    )?;
    if reordered {
        term(&mut cas, &header, Operator::Free("x".into()), &[])?;
        term(&mut cas, &header, Operator::Constant(Constant::E), &[])?;
    }
    let domain = admit(
        &mut cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Real("x".into()),
            expression: RegionExpr::Ambient,
        }),
    )?;
    let pi = term(&mut cas, &header, Operator::Constant(Constant::Pi), &[])?;
    let q = term(
        &mut cas,
        &header,
        Operator::Rational(Rational::parse(coefficient, &mut default_budget()?)?),
        &[],
    )?;
    let product = term(&mut cas, &header, Operator::Mul, &[pi, q])?;
    let u = if zero {
        term(&mut cas, &header, Operator::Rational(Rational::zero()), &[])?
    } else {
        product.clone()
    };
    let predicate = admit(
        &mut cas,
        Draft::Condition {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            kind: PredicateKind::Defined,
            arguments: vec![Reference::Existing(u.clone())],
        },
    )?;
    let context = admit(
        &mut cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header.clone()),
            admissible: Reference::Existing(domain.clone()),
            assumptions: vec![AssumptionDraft {
                key: "original-defined".into(),
                condition: Reference::Existing(predicate.clone()),
            }],
        }),
    )?;
    let k = Box::new(Rational::parse(integer, &mut default_budget()?)?);
    let conclusion = Statement::HalfPiWitness {
        u,
        k,
        context: context.clone(),
        domain: domain.clone(),
    };
    let steps = if zero {
        zero_proposals(&conclusion, predicate, product, &mut cas, &header)?
    } else {
        proposals(&conclusion, predicate, product, false)?
    };
    let expected = Judgment {
        context: context.clone(),
        layer: Layer::Pointwise,
        space: None,
        statement: conclusion,
        residuals: Vec::new(),
    };
    Ok(Fixture {
        cas,
        header,
        context,
        domain,
        steps,
        expected,
    })
}

fn proposals(
    conclusion: &Statement,
    predicate: Handle,
    right: Handle,
    zero: bool,
) -> TestResult<Vec<ApplicationDraft>> {
    let Statement::HalfPiWitness {
        u,
        k,
        context,
        domain,
    } = conclusion
    else {
        return Err("half-Pi statement".into());
    };
    Ok(vec![
        ApplicationDraft {
            rule: Primitive::ContextHypothesis,
            context: context.clone(),
            conclusion: Statement::PredicateFact {
                region: domain.clone(),
                predicate,
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::Hypothesis(HypothesisRef {
                context: context.clone(),
                slot: 0,
            }),
        },
        ApplicationDraft {
            rule: if zero {
                Primitive::MulZero
            } else {
                Primitive::Reflexivity
            },
            context: context.clone(),
            conclusion: Statement::Equality {
                left: u.clone(),
                right,
                domain: domain.clone(),
                domain_contract: DomainContract::DefinedAndEqual,
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::None,
        },
        ApplicationDraft {
            rule: Primitive::HalfPiWitness,
            context: context.clone(),
            conclusion: conclusion.clone(),
            premises: vec![0, 1],
            substitutions: Vec::new(),
            certificate: Certificate::HalfPiWitness {
                u: u.clone(),
                k: k.clone(),
                context: context.clone(),
                domain: domain.clone(),
                defined: 0,
                equality: 1,
            },
        },
    ])
}

/// Rebuild the original definedness assumption when constructing a different original operand.
pub(crate) fn original(f: &mut Fixture, u: Handle) -> TestResult {
    let predicate = admit(
        &mut f.cas,
        Draft::Condition {
            header: Reference::Existing(f.header.clone()),
            scope: Vec::new(),
            kind: PredicateKind::Defined,
            arguments: vec![Reference::Existing(u.clone())],
        },
    )?;
    f.context = admit(
        &mut f.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(f.header.clone()),
            admissible: Reference::Existing(f.domain.clone()),
            assumptions: vec![AssumptionDraft {
                key: "original-defined".into(),
                condition: Reference::Existing(predicate.clone()),
            }],
        }),
    )?;
    let Statement::HalfPiWitness { k, .. } = &f.expected.statement else {
        return Err("recognition".into());
    };
    let zero_integer = k.is_zero();
    let right = match &f.steps.get(1).ok_or("equality")?.conclusion {
        Statement::Equality { right, .. } => right.clone(),
        _ => return Err("equality".into()),
    };
    f.expected.context = f.context.clone();
    f.expected.statement = Statement::HalfPiWitness {
        u,
        k: k.clone(),
        context: f.context.clone(),
        domain: f.domain.clone(),
    };
    f.steps = if zero_integer {
        let pi = term(&mut f.cas, &f.header, Operator::Constant(Constant::Pi), &[])?;
        let zero = term(
            &mut f.cas,
            &f.header,
            Operator::Rational(Rational::zero()),
            &[],
        )?;
        let product = term(&mut f.cas, &f.header, Operator::Mul, &[pi, zero])?;
        zero_proposals(
            &f.expected.statement,
            predicate,
            product,
            &mut f.cas,
            &f.header,
        )?
    } else {
        proposals(&f.expected.statement, predicate, right, true)?
    };
    Ok(())
}

/// The graph retains Mul(Pi, 0); symmetry restores it after the checked zero law.
/// A different original operand first proves u = 0, then composes to Mul(Pi, 0).
fn zero_proposals(
    conclusion: &Statement,
    predicate: Handle,
    product: Handle,
    cas: &mut Cas,
    header: &Handle,
) -> TestResult<Vec<ApplicationDraft>> {
    let zero = term(cas, header, Operator::Rational(Rational::zero()), &[])?;
    assert_ne!(product, zero);
    let Statement::HalfPiWitness {
        u, context, domain, ..
    } = conclusion
    else {
        return Err("recognition conclusion".into());
    };
    let mut steps = proposals(conclusion, predicate, product.clone(), false)?;
    let mut witness = steps.pop().ok_or("recognition step")?;
    steps.pop().ok_or("equality step")?;
    let equality = |rule, left, right, premises| ApplicationDraft {
        rule,
        context: context.clone(),
        conclusion: Statement::Equality {
            left,
            right,
            domain: domain.clone(),
            domain_contract: DomainContract::DefinedAndEqual,
        },
        premises,
        substitutions: Vec::new(),
        certificate: Certificate::None,
    };
    // Mul(Pi, 0) = 0, by the existing zero law.
    steps.push(equality(
        Primitive::MulZero,
        product.clone(),
        zero.clone(),
        Vec::new(),
    ));
    // 0 = Mul(Pi, 0), by checked symmetry of the zero-law equality.
    steps.push(equality(
        Primitive::Symmetry,
        zero.clone(),
        product.clone(),
        vec![1],
    ));
    let endpoint = if *u == zero {
        2
    } else {
        // u = 0 retains every divisor of the original operand.
        steps.push(equality(Primitive::MulZero, u.clone(), zero, Vec::new()));
        // u = Mul(Pi, 0), composing original zero and symmetry equalities.
        steps.push(equality(
            Primitive::Transitivity,
            u.clone(),
            product,
            vec![3, 2],
        ));
        4
    };
    witness.premises = vec![0, endpoint];
    steps.push(witness);
    Ok(steps)
}
