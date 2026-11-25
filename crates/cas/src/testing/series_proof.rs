// SPDX-License-Identifier: AGPL-3.0-only
//! Hand arithmetic and independently admitted receiving-store expectations.

use super::cases::Fixture as Base;
use super::common::{TestResult, default_budget};
use crate::test_api::domain::{AmbientDomain, Layer, RegionDraft, RegionExpr};
use crate::test_api::proof::{
    ApplicationDraft, CaseCertificate, Certificate, Judgment, Primitive, RegionCertificate,
    Statement,
};
use crate::test_api::series::{
    SeriesBinaryRow, SeriesCertificate as C, SeriesObject, SeriesProductRow, SeriesStatement as S,
};
use crate::test_api::solving::{CoefficientDomainData, SolveDraft};
use crate::test_api::syntax::{Draft, PredicateKind};
use crate::test_api::{Handle, Rational, Reference};

/// One complete supplied-candidate computation with an independent expected statement.
pub(crate) struct Fixture {
    /// Original semantic store and coordinate frame.
    pub(crate) base: Base,
    /// Exact interpretation retained by all objects.
    pub(crate) field: Handle,
    /// Complete ordered checked expansion.
    pub(crate) steps: Vec<ApplicationDraft>,
    /// Independently constructed caller expectation.
    pub(crate) expected: Judgment,
}

/// Source-only production fixture; the result object and result-only literals are absent.
pub(crate) struct Production {
    /// Original store and frame.
    pub(crate) base: Base,
    /// Finite request with explicit common-order semantics.
    pub(crate) operation: crate::test_api::series::SeriesOperation,
    /// Checked ordinary definedness or explicit truncation premises.
    pub(crate) proofs: Vec<Handle>,
}

/// A scale fact retains an assumed scalar condition, optionally closed as a residual.
#[expect(
    clippy::too_many_lines,
    reason = "The complete qualified hypothesis and close expansion stays visible in one fixture"
)]
pub(crate) fn conditioned(residual: bool, reordered: bool) -> TestResult<Fixture> {
    use crate::test_api::domain::{AssumptionDraft, ContextDraft, HypothesisRef};
    use crate::test_api::proof::{ConstructorLaw, GuardedObligation};
    let mut f = fixture(2, reordered)?;
    let Statement::Series(value) = &f.expected.statement else {
        return Err("scale".into());
    };
    let S::Scale { scalar, .. } = value.as_ref() else {
        return Err("scale".into());
    };
    let scalar = scalar.clone();
    let predicate = f
        .base
        .predicate(PredicateKind::Defined, std::slice::from_ref(&scalar))?;
    let parent = f.base.context.clone();
    let draft = if residual {
        ContextDraft::Local {
            parent: Reference::Existing(parent.clone()),
            region: Reference::Existing(f.base.domain.clone()),
            hypotheses: vec![AssumptionDraft {
                key: "scalar-defined".into(),
                condition: Reference::Existing(predicate.clone()),
            }],
        }
    } else {
        ContextDraft::Root {
            header: Reference::Existing(f.base.header.clone()),
            admissible: Reference::Existing(f.base.domain.clone()),
            assumptions: vec![AssumptionDraft {
                key: "scalar-defined".into(),
                condition: Reference::Existing(predicate.clone()),
            }],
        }
    };
    let frame = f
        .base
        .cas
        .admit(&[Draft::Context(draft)], &mut default_budget()?)?
        .pop()
        .ok_or("frame")?;
    let region = f
        .base
        .cas
        .admissible_region(&frame, &mut crate::testing::budgets::graph_budget()?)?;
    let fact = Statement::PredicateFact {
        region: region.clone(),
        predicate: predicate.clone(),
    };
    let hypothesis = HypothesisRef {
        context: frame.clone(),
        slot: u32::from(residual),
    };
    let mut steps = vec![ApplicationDraft {
        rule: Primitive::ContextHypothesis,
        context: frame.clone(),
        conclusion: fact.clone(),
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Hypothesis(hypothesis),
    }];
    f.base.context = frame.clone();
    f.base.domain = region.clone();
    if residual {
        steps.push(ApplicationDraft {
            rule: Primitive::RegionConstruct,
            context: parent.clone(),
            conclusion: Statement::Constructor {
                region: region.clone(),
                law: ConstructorLaw::Membership,
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::None,
        });
        steps.push(ApplicationDraft {
            rule: Primitive::ContextClose,
            context: parent,
            conclusion: fact.clone(),
            premises: vec![0, 1],
            substitutions: Vec::new(),
            certificate: Certificate::Close { frame },
        });
        f.base.context = f
            .base
            .cas
            .admit(
                &[Draft::Context(ContextDraft::Root {
                    header: Reference::Existing(f.base.header.clone()),
                    admissible: Reference::Existing(region.clone()),
                    assumptions: Vec::new(),
                })],
                &mut default_budget()?,
            )?
            .pop()
            .ok_or("receiving root")?;
        steps.push(ApplicationDraft {
            rule: Primitive::RegionConstruct,
            context: f.base.context.clone(),
            conclusion: Statement::Subset {
                subset: region.clone(),
                superset: region.clone(),
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::None,
        });
        steps.push(ApplicationDraft {
            rule: Primitive::ContextWeaken,
            context: f.base.context.clone(),
            conclusion: fact,
            premises: vec![2, 3],
            substitutions: Vec::new(),
            certificate: Certificate::None,
        });
    }
    let input = object(&mut f.base, &f.field, &["1", "2", "3"])?;
    let output = object(&mut f.base, &f.field, &["-1/2", "-1", "-3/2"])?;
    let mut scale = f.steps.pop().ok_or("scale")?;
    scale.context = f.base.context.clone();
    scale.conclusion = Statement::Series(Box::new(S::Scale {
        input,
        scalar,
        output,
    }));
    scale.premises = vec![u32::try_from(steps.len() - 1)?];
    f.expected = Judgment {
        context: f.base.context.clone(),
        layer: Layer::Logic,
        space: None,
        statement: scale.conclusion.clone(),
        residuals: if residual {
            vec![GuardedObligation {
                home: f.base.context.clone(),
                scope: Vec::new(),
                layer: Layer::Logic,
                space: None,
                region,
                predicate,
            }]
        } else {
            Vec::new()
        },
    };
    steps.push(scale);
    f.steps = steps;
    Ok(f)
}

/// Reuse the same qualified ordinary fact while changing only the finite operation and rows.
pub(crate) fn conditioned_integral(residual: bool, reordered: bool) -> TestResult<Fixture> {
    let mut f = conditioned(residual, reordered)?;
    let mut root = f.steps.pop().ok_or("scale root")?;
    let Statement::Series(value) = &root.conclusion else {
        return Err("series".into());
    };
    let S::Scale { input, scalar, .. } = value.as_ref() else {
        return Err("scale".into());
    };
    let input = input.clone();
    let constant = scalar.clone();
    let original_context = f.base.context.clone();
    f.base.context = root.context.clone();
    let output = object(&mut f.base, &f.field, &["-1/2", "1", "1", "1"])?;
    f.base.context = original_context;
    root.rule = Primitive::SeriesIntegrate;
    root.conclusion = Statement::Series(Box::new(S::Integrate {
        input,
        constant,
        output,
    }));
    let mut rows = Vec::new();
    for (index, integer, inverse, input) in [
        (0, "1", "1", "1"),
        (1, "2", "1/2", "2"),
        (2, "3", "1/3", "3"),
    ] {
        rows.push(crate::test_api::series::SeriesIntegralRow {
            index,
            integer: rational(integer)?,
            inverse: rational(inverse)?,
            input: rational(input)?,
            result: rational("1")?,
        });
    }
    root.certificate = Certificate::Series(Box::new(C::Integrate { form: 0, rows }));
    f.expected.statement = root.conclusion.clone();
    f.steps.push(root);
    Ok(f)
}

/// Build only original syntax and required evidence, independently of the expected result store.
pub(crate) fn production(kind: usize) -> TestResult<Production> {
    use crate::test_api::series::SeriesOperation as O;
    if kind >= 13 {
        return super::series_compose::production(kind);
    }
    if kind >= 7 {
        return super::series_calculus::production(kind);
    }
    let mut base = Base::new()?;
    let field = base
        .cas
        .admit(
            &[Draft::Solve(SolveDraft::CoefficientDomain(
                CoefficientDomainData::Rational,
            ))],
            &mut default_budget()?,
        )?
        .pop()
        .ok_or("Q")?;
    let input = object(&mut base, &field, &["1", "2", "3"])?;
    let mut proofs = Vec::new();
    let operation = match kind {
        0 => O::Truncate { input, order: 1 },
        1 | 3 | 4 => {
            let right = object(
                &mut base,
                &field,
                if kind == 4 {
                    &["4", "5"]
                } else {
                    &["4", "5", "0"]
                },
            )?;
            if kind == 4 {
                let restricted = object(&mut base, &field, &["1", "2"])?;
                let draft = step(
                    &base,
                    S::Truncate {
                        input: input.clone(),
                        output: restricted,
                    },
                    C::Truncate {
                        form: 0,
                        coefficients: vec![rational("1")?, rational("2")?],
                    },
                    Vec::new(),
                );
                proofs.push(
                    base.cas
                        .check_proof(&base.context, &[draft], &mut default_budget()?)?
                        .proof()
                        .clone(),
                );
            }
            if kind == 1 {
                O::Add {
                    left: input,
                    right,
                    order: None,
                }
            } else {
                O::Mul {
                    left: input,
                    right,
                    order: None,
                }
            }
        }
        2 => {
            let scalar = base.number("-1/2")?;
            let draft = defined(&mut base, &scalar)?;
            proofs.push(
                base.cas
                    .check_proof(&base.context, &[draft], &mut default_budget()?)?
                    .proof()
                    .clone(),
            );
            O::Scale { input, scalar }
        }
        5 | 6 => O::Coefficient {
            input,
            index: if kind == 5 { 0 } else { 2 },
        },
        _ => return Err("production kind".into()),
    };
    Ok(Production {
        base,
        operation,
        proofs,
    })
}

/// Parse an oracle literal without using the operation recurrence under test.
pub(crate) fn rational(text: &str) -> TestResult<Rational> {
    Ok(Rational::parse(text, &mut default_budget()?)?)
}

/// Admit one explicitly supplied vector; missing entries are never synthesized.
pub(crate) fn object(base: &mut Base, field: &Handle, values: &[&str]) -> TestResult<Handle> {
    let mut coefficients = Vec::new();
    for value in values {
        coefficients.push(Reference::Existing(base.number(value)?));
    }
    Ok(base
        .cas
        .admit(
            &[Draft::SeriesObject(SeriesObject {
                field: Reference::Existing(field.clone()),
                centre: Reference::Existing(base.zero.clone()),
                variable: Reference::Existing(base.x.clone()),
                scope: Vec::new(),
                context: Reference::Existing(base.context.clone()),
                order: u32::try_from(values.len())?,
                coefficients,
            })],
            &mut default_budget()?,
        )?
        .pop()
        .ok_or("series object")?)
}

/// The existing case law proves the scalar fact with no assumed hypothesis.
pub(crate) fn defined(base: &mut Base, scalar: &Handle) -> TestResult<ApplicationDraft> {
    let predicate = base.predicate(PredicateKind::Defined, std::slice::from_ref(scalar))?;
    let regions = base.cas.admit(
        &[
            Draft::Region(RegionDraft {
                header: Reference::Existing(base.header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Real("x".into()),
                expression: RegionExpr::Meet(vec![Reference::Existing(base.domain.clone())]),
            }),
            Draft::Region(RegionDraft {
                header: Reference::Existing(base.header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Real("x".into()),
                expression: RegionExpr::Meet(vec![
                    Reference::Existing(base.domain.clone()),
                    Reference::Local(0),
                ]),
            }),
        ],
        &mut default_budget()?,
    )?;
    Ok(ApplicationDraft {
        rule: Primitive::CaseDomain,
        context: base.context.clone(),
        conclusion: Statement::PredicateFact {
            region: base.domain.clone(),
            predicate,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Cases(Box::new(CaseCertificate {
            regions: vec![base.domain.clone(), regions[1].clone()],
            arms: Vec::new(),
            frames: Vec::new(),
            cells: vec![RegionCertificate {
                cuts: Vec::new(),
                membership: vec![vec![true], vec![true]],
            }],
        })),
    })
}

/// Construct one typed application without asking a producer for any expected arithmetic.
pub(crate) fn step(
    base: &Base,
    statement: S,
    certificate: C,
    premises: Vec<u32>,
) -> ApplicationDraft {
    ApplicationDraft {
        rule: statement.primitive(),
        context: base.context.clone(),
        conclusion: Statement::Series(Box::new(statement)),
        premises,
        substitutions: Vec::new(),
        certificate: Certificate::Series(Box::new(certificate)),
    }
}

/// Two-operand rows are explicit hand arithmetic, independent of both checkers and producers.
fn binary(values: &[(&str, &str, &str)]) -> TestResult<Vec<SeriesBinaryRow>> {
    values
        .iter()
        .enumerate()
        .map(|(index, (left, right, result))| {
            Ok(SeriesBinaryRow {
                index: u32::try_from(index)?,
                left: rational(left)?,
                right: rational(right)?,
                result: rational(result)?,
            })
        })
        .collect()
}

/// Kinds cover truncation, addition, scaling, both convolution orders and both endpoint indices.
#[expect(
    clippy::too_many_lines,
    reason = "Independent hand fixtures state every endpoint and arithmetic row explicitly"
)]
pub(crate) fn fixture(kind: usize, reordered: bool) -> TestResult<Fixture> {
    if kind >= 13 {
        return super::series_compose::fixture_kind(kind, reordered);
    }
    if kind >= 7 {
        return super::series_calculus::fixture(kind, reordered);
    }
    let mut base = Base::new()?;
    if reordered {
        for value in ["-3/2", "22", "13", "5", "4", "3", "2", "-1/2"] {
            base.number(value)?;
        }
    }
    let field = base
        .cas
        .admit(
            &[Draft::Solve(SolveDraft::CoefficientDomain(
                CoefficientDomainData::Rational,
            ))],
            &mut default_budget()?,
        )?
        .pop()
        .ok_or("Q")?;
    let input = object(&mut base, &field, &["1", "2", "3"])?;
    let mut steps = Vec::new();
    let root = match kind {
        0 => {
            let output = object(&mut base, &field, &["1"])?;
            step(
                &base,
                S::Truncate { input, output },
                C::Truncate {
                    form: 0,
                    coefficients: vec![rational("1")?],
                },
                Vec::new(),
            )
        }
        1 => {
            let right = object(&mut base, &field, &["4", "5", "0"])?;
            let output = object(&mut base, &field, &["5", "7", "3"])?;
            step(
                &base,
                S::Add {
                    left: input,
                    right,
                    output,
                    order: 3,
                    default_order: true,
                },
                C::Add {
                    form: 0,
                    order: 3,
                    default_order: true,
                    rows: binary(&[("1", "4", "5"), ("2", "5", "7"), ("3", "0", "3")])?,
                },
                Vec::new(),
            )
        }
        2 => {
            let scalar = base.number("-1/2")?;
            steps.push(defined(&mut base, &scalar)?);
            let output = object(&mut base, &field, &["-1/2", "-1", "-3/2"])?;
            step(
                &base,
                S::Scale {
                    input,
                    scalar,
                    output,
                },
                C::Scale {
                    form: 0,
                    rows: binary(&[
                        ("-1/2", "1", "-1/2"),
                        ("-1/2", "2", "-1"),
                        ("-1/2", "3", "-3/2"),
                    ])?,
                },
                vec![0],
            )
        }
        3 | 4 => {
            let short = kind == 4;
            let order = if short { 2 } else { 3 };
            let right = object(
                &mut base,
                &field,
                if short { &["4", "5"] } else { &["4", "5", "0"] },
            )?;
            let output = object(
                &mut base,
                &field,
                if short {
                    &["4", "13"]
                } else {
                    &["4", "13", "22"]
                },
            )?;
            let mut premises = Vec::new();
            if short {
                let restricted = object(&mut base, &field, &["1", "2"])?;
                steps.push(step(
                    &base,
                    S::Truncate {
                        input: input.clone(),
                        output: restricted,
                    },
                    C::Truncate {
                        form: 0,
                        coefficients: vec![rational("1")?, rational("2")?],
                    },
                    Vec::new(),
                ));
                premises.push(0);
            }
            let hand = [
                (0, 0, 0, "1", "4", "4", "4"),
                (1, 0, 1, "1", "5", "5", "5"),
                (1, 1, 0, "2", "4", "8", "13"),
                (2, 0, 2, "1", "0", "0", "0"),
                (2, 1, 1, "2", "5", "10", "10"),
                (2, 2, 0, "3", "4", "12", "22"),
            ];
            let mut rows = Vec::new();
            for (degree, left_index, right_index, left, right, product, sum) in hand {
                if degree < order {
                    rows.push(SeriesProductRow {
                        degree,
                        left_index,
                        right_index,
                        left: rational(left)?,
                        right: rational(right)?,
                        product: rational(product)?,
                        sum: rational(sum)?,
                    });
                }
            }
            step(
                &base,
                S::Mul {
                    left: input,
                    right,
                    output,
                    order,
                    default_order: true,
                },
                C::Mul {
                    form: 0,
                    order,
                    default_order: true,
                    rows,
                },
                premises,
            )
        }
        5 | 6 => {
            let index = if kind == 5 { 0 } else { 2 };
            let text = if kind == 5 { "1" } else { "3" };
            let value = base.number(text)?;
            step(
                &base,
                S::Coefficient {
                    input,
                    index,
                    value,
                },
                C::Coefficient {
                    form: 0,
                    index,
                    value: rational(text)?,
                },
                Vec::new(),
            )
        }
        _ => return Err("unknown finite fixture".into()),
    };
    let expected = Judgment {
        context: base.context.clone(),
        layer: Layer::Logic,
        space: None,
        statement: root.conclusion.clone(),
        residuals: Vec::new(),
    };
    steps.push(root);
    Ok(Fixture {
        base,
        field,
        steps,
        expected,
    })
}
