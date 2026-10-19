// SPDX-License-Identifier: AGPL-3.0-only
//! Hand-authored finite evaluation witnesses, independent of producer recipes.

use super::common::TestResult;
use super::solve_coefficients::{AdjunctionFixture, Fixture};
use super::sorts::{admit, expression, reference};
use crate::test_api::domain::{AssumptionDraft, ContextDraft, HypothesisRef};
use crate::test_api::limits::{Budget, Limits};
use crate::test_api::proof::{ApplicationDraft, Certificate, Primitive, Statement};
use crate::test_api::solving::{
    AdjunctionMode, CertificateCoefficientEvaluation, CoefficientEvaluationData, Coordinate,
    CoordinateRole, CoordinateSpaceData, EvaluationBinding, EvaluationTarget, SolveCertificate,
    SolveDraft, SolveSchema, SolveStatement, SolveView, StatementCoefficientEvaluation,
};
use crate::test_api::syntax::{Draft, Field, Operator, PredicateKind};
use crate::test_api::{Cas, Handle, Rational, Reference};

/// A complete supplied proof fragment and its exact original context.
pub(crate) struct EvaluationFixture {
    /// Owning mathematical graph.
    pub(crate) cas: Cas,
    /// Exact conditional assertion context.
    pub(crate) context: Handle,
    /// Earlier facts followed by the evaluation claim.
    pub(crate) steps: Vec<ApplicationDraft>,
}

/// Construct exact scalar syntax in the retained Real header.
fn scalar(base: &mut Fixture, operator: Operator, operands: &[Handle]) -> TestResult<Handle> {
    expression(
        &mut base.cas,
        &base.header,
        Field::Real,
        operator,
        Vec::new(),
        operands.iter().map(reference).collect(),
    )
}

/// Original source slot x stays Real even when its image is zero or an algebraic root.
fn source(base: &mut Fixture) -> TestResult<Handle> {
    admit(
        &mut base.cas,
        Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
            scalar_spaces: Vec::new(),
            header: reference(&base.header),
            coordinates: vec![Coordinate {
                key: "x".into(),
                field: Field::Real,
                role: CoordinateRole::Coefficient,
            }],
        })),
    )
}

/// Supply exact conditional image evidence without claiming an automatic definedness producer.
fn assumptions(
    cas: &mut Cas,
    header: &Handle,
    region: &Handle,
    entries: &[(PredicateKind, Handle)],
) -> TestResult<(Handle, Vec<ApplicationDraft>)> {
    let mut predicates = Vec::new();
    let mut assumptions = Vec::new();
    for (index, (kind, argument)) in entries.iter().enumerate() {
        let predicate = admit(
            cas,
            Draft::Condition {
                kind: kind.clone(),
                header: reference(header),
                scope: Vec::new(),
                arguments: vec![reference(argument)],
            },
        )?;
        assumptions.push(AssumptionDraft {
            key: format!("given-{index}"),
            condition: reference(&predicate),
        });
        predicates.push(predicate);
    }
    let context = admit(
        cas,
        Draft::Context(ContextDraft::Root {
            header: reference(header),
            admissible: reference(region),
            assumptions,
        }),
    )?;
    let mut steps = Vec::new();
    for (slot, predicate) in predicates.into_iter().enumerate() {
        steps.push(ApplicationDraft {
            rule: Primitive::ContextHypothesis,
            context: context.clone(),
            conclusion: Statement::PredicateFact {
                region: region.clone(),
                predicate,
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::Hypothesis(HypothesisRef {
                context: context.clone(),
                slot: u32::try_from(slot)?,
            }),
        });
    }
    Ok((context, steps))
}

/// Explicit premise inventories, never inferred from an unrelated earlier fact.
#[derive(Default)]
struct Evidence {
    /// Ordered application dependencies.
    premises: Vec<u32>,
    /// Exact image-definedness positions.
    definedness: Vec<u32>,
    /// Exact specialized localization positions.
    units: Vec<u32>,
    /// Exact earlier root selections.
    existence: Vec<u32>,
}

/// The certificate repeats finite support and actual earlier premise positions.
fn step(
    cas: &Cas,
    context: &Handle,
    mapping: Handle,
    left: Handle,
    right: Handle,
    evidence: Evidence,
) -> TestResult<ApplicationDraft> {
    let SolveView::CoefficientEvaluation(evaluation) = cas.solve_view(&mapping)? else {
        return Err("missing evaluation".into());
    };
    let region = evaluation.region.clone();
    let SolveView::Polynomial(polynomial) = cas.solve_view(&left)? else {
        return Err("missing finite support".into());
    };
    let support = polynomial
        .terms
        .iter()
        .map(|term| term.coefficient.clone())
        .collect();
    let statement = StatementCoefficientEvaluation {
        evaluation: mapping.clone(),
        left: left.clone(),
        right: right.clone(),
        region: region.clone(),
    };
    let certificate = CertificateCoefficientEvaluation {
        evaluation: mapping,
        left,
        right,
        support,
        definedness: evidence.definedness,
        units: evidence.units,
        existence: evidence.existence,
        region,
    };
    Ok(ApplicationDraft {
        rule: Primitive::Solve(SolveSchema::CoefficientEvaluation),
        context: context.clone(),
        conclusion: Statement::Solve(Box::new(SolveStatement::CoefficientEvaluation(statement))),
        premises: evidence.premises,
        substitutions: Vec::new(),
        certificate: Certificate::Solve(Box::new(SolveCertificate::CoefficientEvaluation(
            certificate,
        ))),
    })
}

/// Bind one original Real coordinate to an exact scalar image and localization inventory.
fn scalar_mapping(
    base: &mut Fixture,
    original: &Handle,
    image: &Handle,
    localization: Vec<Reference>,
) -> TestResult<Handle> {
    let space = base
        .cas
        .scalar_ambient(image)?
        .ok_or("missing own ambient")?;
    admit(
        &mut base.cas,
        Draft::Solve(SolveDraft::CoefficientEvaluation(
            CoefficientEvaluationData {
                source: reference(original),
                target: EvaluationTarget::Scalar {
                    header: reference(&base.header),
                    space: reference(&space),
                },
                region: reference(&base.region),
                bindings: vec![EvaluationBinding {
                    slot: 0,
                    image: reference(image),
                }],
                localization,
            },
        )),
    )
}

/// Finite specialization may send x to zero without an injectivity claim.
pub(crate) fn finite(zero: bool, localized: bool, unit: bool) -> TestResult<EvaluationFixture> {
    let mut base = Fixture::new()?;
    let image = if zero {
        scalar(&mut base, Operator::Rational(Rational::zero()), &[])?
    } else {
        base.free("y")?
    };
    let right = scalar(&mut base, Operator::Mul, &[image.clone(), image.clone()])?;
    let polynomial = base.polynomial(1, &[(vec![2], 1)])?;
    let mut localization = Vec::new();
    if localized {
        localization.push(reference(&base.polynomial(1, &[(vec![1], 1)])?));
    }
    let original = source(&mut base)?;
    let mapping = scalar_mapping(&mut base, &original, &image, localization)?;
    let mut entries = vec![(PredicateKind::Defined, image)];
    if unit {
        entries.push((PredicateKind::NonZero, base.free("y")?));
    }
    let (context, mut steps) = assumptions(&mut base.cas, &base.header, &base.region, &entries)?;
    let slots = (0..u32::try_from(steps.len())?).collect();
    let units = if localized && unit {
        vec![1]
    } else {
        Vec::new()
    };
    steps.push(step(
        &base.cas,
        &context,
        mapping,
        polynomial,
        right,
        Evidence {
            premises: slots,
            definedness: vec![0],
            units,
            existence: Vec::new(),
        },
    )?);
    Ok(EvaluationFixture {
        cas: base.cas,
        context,
        steps,
    })
}

/// An actual divisor is checked even when the image simplifies algebraically to one.
pub(crate) fn canceled_divisor(unit: bool) -> TestResult<EvaluationFixture> {
    let mut base = Fixture::new()?;
    let parameter = base.free("y")?;
    let image = scalar(
        &mut base,
        Operator::Div,
        &[parameter.clone(), parameter.clone()],
    )?;
    let right = scalar(&mut base, Operator::Rational(Rational::one()), &[])?;
    let polynomial = base.polynomial(1, &[(vec![1], 1)])?;
    let original = source(&mut base)?;
    let mapping = scalar_mapping(&mut base, &original, &image, Vec::new())?;
    let mut entries = vec![(PredicateKind::Defined, image)];
    if unit {
        entries.push((PredicateKind::NonZero, parameter));
    }
    let (context, mut steps) = assumptions(&mut base.cas, &base.header, &base.region, &entries)?;
    let slots = (0..u32::try_from(steps.len())?).collect();
    steps.push(step(
        &base.cas,
        &context,
        mapping,
        polynomial,
        right,
        Evidence {
            premises: slots,
            definedness: vec![0],
            ..Evidence::default()
        },
    )?);
    Ok(EvaluationFixture {
        cas: base.cas,
        context,
        steps,
    })
}

/// A Real selected-root image needs the exact earlier Real existence theorem.
pub(crate) fn selected_root() -> TestResult<EvaluationFixture> {
    let mut fixture = AdjunctionFixture::new(
        &[(vec![3], 1), (vec![0], -2)],
        &Field::Real,
        AdjunctionMode::RealOddRoot,
        Some(3),
    )?;
    let base = &mut fixture.base;
    let image = admit(
        &mut base.cas,
        Draft::RootValue {
            header: reference(&base.header),
            selection: reference(&fixture.selection),
            slot: 0,
        },
    )?;
    let original = source(base)?;
    let polynomial = base.polynomial(1, &[(vec![1], 1)])?;
    let mapping = scalar_mapping(base, &original, &image, Vec::new())?;
    let (context, supplied) = assumptions(
        &mut base.cas,
        &base.header,
        &base.region,
        &[(PredicateKind::Defined, image.clone())],
    )?;
    for earlier in &mut fixture.steps {
        earlier.context = context.clone();
    }
    fixture.steps.extend(supplied);
    fixture.steps.push(step(
        &base.cas,
        &context,
        mapping,
        polynomial,
        image,
        Evidence {
            premises: vec![0, 1, 2],
            definedness: vec![2],
            units: Vec::new(),
            existence: vec![1],
        },
    )?);
    Ok(EvaluationFixture {
        cas: fixture.base.cas,
        context,
        steps: fixture.steps,
    })
}

/// Check a complete supplied fragment with the public kernel and ordinary finite ceilings.
pub(crate) fn verify(
    mut fixture: EvaluationFixture,
) -> TestResult<(Cas, crate::test_api::proof::VerifiedWitness)> {
    let proof = fixture.cas.check_proof(
        &fixture.context,
        &fixture.steps,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    Ok((fixture.cas, proof))
}

/// Build a sparse rational polynomial whose displayed terms all have coefficient one.
fn unit_polynomial(
    cas: &mut Cas,
    variables: Vec<Field>,
    powers: Vec<Vec<u32>>,
) -> TestResult<Handle> {
    use crate::test_api::solving::{
        CoefficientDomainData, CoefficientValueData, CoefficientValueObject, PolynomialData,
        PolynomialTerm,
    };
    let domain = admit(
        cas,
        Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Rational,
        )),
    )?;
    let one = admit(
        cas,
        Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
            domain: reference(&domain),
            value: CoefficientValueData::Rational {
                value: Rational::one(),
            },
        })),
    )?;
    let terms = powers
        .into_iter()
        .map(|exponents| PolynomialTerm {
            exponents,
            coefficient: reference(&one),
        })
        .collect();
    admit(
        cas,
        Draft::Solve(SolveDraft::Polynomial(PolynomialData {
            domain: reference(&domain),
            variables,
            terms,
        })),
    )
}

/// Mixed finite support is evaluated through the explicit base/fibre family operation.
pub(crate) fn family() -> TestResult<EvaluationFixture> {
    let mut fixture = super::sorts::Fixture::new()?;
    let shape = fixture.family()?;
    let right = fixture.body(&shape)?;
    let polynomial = unit_polynomial(
        &mut fixture.cas,
        vec![Field::Real, Field::Complex],
        vec![vec![1, 0], vec![0, 1]],
    )?;
    let mapping = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::CoefficientEvaluation(
            CoefficientEvaluationData {
                source: reference(&shape.total),
                target: EvaluationTarget::Family {
                    family: reference(&shape.handle),
                },
                region: reference(&shape.region),
                bindings: vec![
                    EvaluationBinding {
                        slot: 0,
                        image: reference(&fixture.real),
                    },
                    EvaluationBinding {
                        slot: 1,
                        image: reference(&fixture.complex),
                    },
                ],
                localization: Vec::new(),
            },
        )),
    )?;
    let entries = [
        (PredicateKind::Defined, fixture.real),
        (PredicateKind::Defined, fixture.complex),
    ];
    let (context, mut steps) =
        assumptions(&mut fixture.cas, &fixture.header, &shape.region, &entries)?;
    steps.push(step(
        &fixture.cas,
        &context,
        mapping,
        polynomial,
        right,
        Evidence {
            premises: vec![0, 1],
            definedness: vec![0, 1],
            ..Evidence::default()
        },
    )?);
    Ok(EvaluationFixture {
        cas: fixture.cas,
        context,
        steps,
    })
}

/// A native one-arm case exposes the exact family body's definedness on its active region.
fn wrapped(
    fixture: &mut super::sorts::Fixture,
    shape: &super::sorts::Family,
    body: &Handle,
) -> TestResult<Handle> {
    use crate::test_api::domain::{CaseArmDraft, CasesDraft};
    use crate::test_api::solving::PlacementUse;
    let piece = fixture.piece(&shape.region)?;
    let arm = admit(
        &mut fixture.cas,
        Draft::CaseArm(CaseArmDraft {
            placements: vec![PlacementUse {
                operand: reference(&fixture.real),
                piece: reference(&piece),
            }],
            header: reference(&fixture.header),
            scope: Vec::new(),
            region: reference(&shape.region),
            value: reference(body),
        }),
    )?;
    admit(
        &mut fixture.cas,
        Draft::Cases(CasesDraft {
            header: reference(&fixture.header),
            scope: Vec::new(),
            domain: reference(&shape.region),
            arms: vec![reference(&arm)],
        }),
    )
}

/// A Complex coefficient image may be a local family body with an original Real base dependency.
pub(crate) fn family_image() -> TestResult<EvaluationFixture> {
    let mut fixture = super::sorts::Fixture::new()?;
    let shape = fixture.family()?;
    let image = fixture.body(&shape)?;
    let wrapper = wrapped(&mut fixture, &shape, &image)?;
    let SolveView::SpaceFamily(family) = fixture.cas.solve_view(&shape.handle)? else {
        return Err("missing family".into());
    };
    let source = family.coefficients.clone();
    let polynomial = unit_polynomial(&mut fixture.cas, vec![Field::Complex], vec![vec![1]])?;
    let mapping = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::CoefficientEvaluation(
            CoefficientEvaluationData {
                source: reference(&source),
                target: EvaluationTarget::Family {
                    family: reference(&shape.handle),
                },
                region: reference(&shape.region),
                bindings: vec![EvaluationBinding {
                    slot: 0,
                    image: reference(&image),
                }],
                localization: Vec::new(),
            },
        )),
    )?;
    let (context, mut steps) = assumptions(
        &mut fixture.cas,
        &fixture.header,
        &shape.region,
        &[(PredicateKind::Defined, wrapper)],
    )?;
    steps.push(step(
        &fixture.cas,
        &context,
        mapping,
        polynomial,
        image,
        Evidence {
            premises: vec![0],
            definedness: vec![0],
            ..Evidence::default()
        },
    )?);
    Ok(EvaluationFixture {
        cas: fixture.cas,
        context,
        steps,
    })
}

/// Empty coordinate support isolates the declared family fibre from unused header declarations.
fn closed_family(
    fixture: &mut super::sorts::Fixture,
    space: &Handle,
) -> TestResult<(Handle, Handle, Handle)> {
    use crate::test_api::domain::{AmbientDomain, RegionDraft, RegionExpr};
    use crate::test_api::solving::{LiesOver, ParameterBaseData, SpaceFamilyData};
    let coordinates = fixture.coordinates(&[])?;
    let draft = RegionDraft {
        header: reference(&fixture.header),
        scope: Vec::new(),
        ambient: AmbientDomain::Product {
            coordinates: coordinates.clone(),
        },
        expression: RegionExpr::Ambient,
    };
    let stratum = admit(&mut fixture.cas, Draft::Region(draft.clone()))?;
    let base = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::ParameterBase(ParameterBaseData {
            header: reference(&fixture.header),
            coordinates: reference(&coordinates),
            domain: reference(&stratum),
        })),
    )?;
    let family = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::SpaceFamily(SpaceFamilyData {
            base: reference(&base),
            fibre_space: reference(space),
            coefficients: reference(&coordinates),
            unknowns: reference(&coordinates),
        })),
    )?;
    let region = admit(
        &mut fixture.cas,
        Draft::FamilyRegion {
            region: draft,
            lies_over: LiesOver {
                family: reference(&family),
                stratum: reference(&stratum),
            },
        },
    )?;
    Ok((coordinates, family, region))
}

/// The same exact constant is represented over either ground coefficient domain.
fn constant_two(cas: &mut Cas, gaussian: bool) -> TestResult<Handle> {
    use crate::test_api::solving::{
        CoefficientDomainData, CoefficientValueData, CoefficientValueObject, PolynomialData,
        PolynomialTerm,
    };
    let domain = if gaussian {
        CoefficientDomainData::Gaussian
    } else {
        CoefficientDomainData::Rational
    };
    let domain = admit(cas, Draft::Solve(SolveDraft::CoefficientDomain(domain)))?;
    let two = Rational::one().add(&Rational::one(), &mut Budget::new(Limits::default())?)?;
    let value = if gaussian {
        CoefficientValueData::Gaussian {
            real: two,
            imaginary: Rational::zero(),
        }
    } else {
        CoefficientValueData::Rational { value: two }
    };
    let coefficient = admit(
        cas,
        Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
            domain: reference(&domain),
            value,
        })),
    )?;
    admit(
        cas,
        Draft::Solve(SolveDraft::Polynomial(PolynomialData {
            domain: reference(&domain),
            variables: Vec::new(),
            terms: vec![PolynomialTerm {
                exponents: Vec::new(),
                coefficient: reference(&coefficient),
            }],
        })),
    )
}

/// Ground domains must match the family fibre even when this particular value is real.
pub(crate) fn family_ground_constants(gaussian: bool, real: bool) -> TestResult<EvaluationFixture> {
    use crate::test_api::solving::{FamilyArgument, FamilyArgumentRole, FamilyExpressionData};
    use crate::test_api::syntax::{AtomicSort, Sort};
    let mut fixture = super::sorts::Fixture::new()?;
    let space = if real {
        fixture.real_space.clone()
    } else {
        fixture.complex_space.clone()
    };
    let field = if real { Field::Real } else { Field::Complex };
    let (source, family, region) = closed_family(&mut fixture, &space)?;
    let one = expression(
        &mut fixture.cas,
        &fixture.header,
        field.clone(),
        Operator::Rational(Rational::one()),
        Vec::new(),
        Vec::new(),
    )?;
    let argument = FamilyArgument {
        role: FamilyArgumentRole::Fibre,
        value: reference(&one),
    };
    let right = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::FamilyExpression(FamilyExpressionData {
            family: reference(&family),
            ambient: reference(&space),
            region: reference(&region),
            scope: Vec::new(),
            operator: Operator::Add,
            result: Sort::Value(AtomicSort::Scalar(field)),
            arguments: vec![argument.clone(), argument],
        })),
    )?;
    let mapping = admit(
        &mut fixture.cas,
        Draft::Solve(SolveDraft::CoefficientEvaluation(
            CoefficientEvaluationData {
                source: reference(&source),
                target: EvaluationTarget::Family {
                    family: reference(&family),
                },
                region: reference(&region),
                bindings: Vec::new(),
                localization: Vec::new(),
            },
        )),
    )?;
    let polynomial = constant_two(&mut fixture.cas, gaussian)?;
    let (context, mut steps) = assumptions(&mut fixture.cas, &fixture.header, &region, &[])?;
    steps.push(step(
        &fixture.cas,
        &context,
        mapping,
        polynomial,
        right,
        Evidence::default(),
    )?);
    Ok(EvaluationFixture {
        cas: fixture.cas,
        context,
        steps,
    })
}
