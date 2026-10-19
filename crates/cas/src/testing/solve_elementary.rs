// SPDX-License-Identifier: AGPL-3.0-only
//! Crate-only elementary recipe fixtures use the public graph admission boundary.

use super::common::TestResult;
use crate::budget::{Budget, Limits};
use crate::graph::{Cas, Draft, Handle, Reference};
use crate::model::Field;
use crate::solving::{
    CoefficientValueData, CoefficientValueObject, PolynomialData, PolynomialTerm, SolveDraft,
};

/// A polynomial and its exact coefficient context, for the unsealed recipe interaction tests.
pub(crate) struct RecipeFixture {
    /// Original graph store.
    pub(crate) cas: Cas,
    /// Ground or rational-function algebra.
    pub(crate) domain: Handle,
    /// Exact coefficient region.
    pub(crate) region: Handle,
    /// Complete hypothesis context.
    pub(crate) context: Handle,
    /// Canonical original polynomial.
    pub(crate) polynomial: Handle,
    /// Explicit regional leading-unit hypotheses.
    pub(crate) premises: Vec<crate::proof_model::VerifiedWitness>,
    /// Independent symbolic expected shift, scale and radicand, when supplied.
    pub(crate) expected: Vec<Handle>,
}

/// One bounded graph admission keeps the fixture from constructing trusted arithmetic scratch.
fn admit(cas: &mut Cas, draft: &Draft) -> TestResult<Handle> {
    cas.admit(
        std::slice::from_ref(draft),
        &mut Budget::new(Limits::blind_cubic())?,
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "missing object".into())
}

/// A canonical polynomial with scalar rational coefficients and a fixed variable frame.
fn polynomial(
    cas: &mut Cas,
    domain: &Handle,
    variables: usize,
    terms: &[(Vec<u32>, crate::number::Rational)],
) -> TestResult<Handle> {
    let mut coefficients = Vec::new();
    for (exponents, value) in terms {
        let coefficient = admit(
            cas,
            &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
                domain: Reference::Existing(domain.clone()),
                value: CoefficientValueData::Rational {
                    value: value.clone(),
                },
            })),
        )?;
        coefficients.push(PolynomialTerm {
            exponents: exponents.clone(),
            coefficient: Reference::Existing(coefficient),
        });
    }
    admit(
        cas,
        &Draft::Solve(SolveDraft::Polynomial(PolynomialData {
            domain: Reference::Existing(domain.clone()),
            variables: vec![Field::Real; variables],
            terms: coefficients,
        })),
    )
}

impl RecipeFixture {
    /// Independent ascending integer coefficients, including an explicitly empty zero polynomial.
    pub(crate) fn ground(coefficients: &[i64]) -> TestResult<Self> {
        let mut fixture = super::solve_coefficients::Fixture::new()?;
        let mut terms = Vec::new();
        for (index, value) in coefficients.iter().enumerate().rev() {
            if *value != 0 {
                terms.push((
                    vec![u32::try_from(index)?],
                    crate::number::Rational::from_ratio(
                        *value,
                        1,
                        &mut Budget::new(Limits::blind_cubic())?,
                    )?,
                ));
            }
        }
        let original = polynomial(&mut fixture.cas, &fixture.rational, 1, &terms)?;
        Ok(Self {
            cas: fixture.cas,
            domain: fixture.rational,
            region: fixture.region,
            context: fixture.context,
            polynomial: original,
            premises: Vec::new(),
            expected: Vec::new(),
        })
    }

    /// Three independent coefficients enter a fresh product, with an explicit nonzero leading hypothesis.
    pub(crate) fn symbolic() -> TestResult<Self> {
        use crate::domain_model::{
            AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr,
        };
        use crate::model::{AtomicSort, Declaration, Operator, PredicateKind, Sort};
        use crate::solving::{
            CoefficientDomainData, Coordinate, CoordinateRole, CoordinateSpaceData,
        };
        let mut base = super::solve_coefficients::Fixture::new()?;
        let header = admit(
            &mut base.cas,
            &Draft::Header(HeaderDraft::Extend {
                parent: Reference::Existing(base.header),
                declarations: vec![Declaration {
                    key: "z".into(),
                    display_name: "z".into(),
                    sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
                }],
                spaces: Vec::new(),
            }),
        )?;
        let coordinates = admit(
            &mut base.cas,
            &Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
                scalar_spaces: Vec::new(),
                header: Reference::Existing(header.clone()),
                coordinates: ["x", "y", "z"]
                    .into_iter()
                    .map(|key| Coordinate {
                        key: key.into(),
                        field: Field::Real,
                        role: CoordinateRole::Coefficient,
                    })
                    .collect(),
            })),
        )?;
        let region = admit(
            &mut base.cas,
            &Draft::Region(RegionDraft {
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Product {
                    coordinates: coordinates.clone(),
                },
                expression: RegionExpr::Ambient,
            }),
        )?;
        let variable = admit(
            &mut base.cas,
            &super::solve_graph::expression(
                Reference::Existing(header.clone()),
                Operator::Free("x".into()),
                Sort::Value(AtomicSort::Scalar(Field::Real)),
                Vec::new(),
            ),
        )?;
        let predicate = admit(
            &mut base.cas,
            &Draft::Condition {
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                kind: PredicateKind::NonZero,
                arguments: vec![Reference::Existing(variable)],
            },
        )?;
        let context = admit(
            &mut base.cas,
            &Draft::Context(ContextDraft::Root {
                header: Reference::Existing(header),
                admissible: Reference::Existing(region.clone()),
                assumptions: vec![crate::graph::AssumptionDraft {
                    key: "leading".into(),
                    condition: Reference::Existing(predicate.clone()),
                }],
            }),
        )?;
        let domain = admit(
            &mut base.cas,
            &Draft::Solve(SolveDraft::CoefficientDomain(
                CoefficientDomainData::Fraction {
                    base: Reference::Existing(base.rational.clone()),
                    coordinates: Reference::Existing(coordinates),
                },
            )),
        )?;
        Self::symbolic_values(base.cas, &base.rational, domain, region, context, predicate)
    }

    /// The oracle is used only by tests: shift=y/(2x), scale=x, radicand=(y²/4-xz)/x².
    fn symbolic_values(
        mut cas: Cas,
        rational: &Handle,
        domain: Handle,
        region: Handle,
        context: Handle,
        predicate: Handle,
    ) -> TestResult<Self> {
        let one = crate::number::Rational::one();
        let constant = polynomial(&mut cas, rational, 3, &[(vec![0, 0, 0], one.clone())])?;
        let leading = polynomial(&mut cas, rational, 3, &[(vec![1, 0, 0], one.clone())])?;
        let middle = polynomial(&mut cas, rational, 3, &[(vec![0, 1, 0], one.clone())])?;
        let trailing = polynomial(&mut cas, rational, 3, &[(vec![0, 0, 1], one)])?;
        let mut values = Vec::new();
        for numerator in [&leading, &middle, &trailing] {
            values.push(fraction(&mut cas, &domain, numerator, &constant)?);
        }
        let terms = values
            .iter()
            .enumerate()
            .map(|(index, coefficient)| {
                Ok(PolynomialTerm {
                    exponents: vec![
                        2_u32
                            .checked_sub(u32::try_from(index)?)
                            .ok_or("bad degree")?,
                    ],
                    coefficient: Reference::Existing(coefficient.clone()),
                })
            })
            .collect::<TestResult<Vec<_>>>()?;
        let original = admit(
            &mut cas,
            &Draft::Solve(SolveDraft::Polynomial(PolynomialData {
                domain: Reference::Existing(domain.clone()),
                variables: vec![Field::Real],
                terms,
            })),
        )?;
        let half =
            crate::number::Rational::from_ratio(1, 2, &mut Budget::new(Limits::blind_cubic())?)?;
        let quarter =
            crate::number::Rational::from_ratio(1, 4, &mut Budget::new(Limits::blind_cubic())?)?;
        let negative =
            crate::number::Rational::from_ratio(-1, 1, &mut Budget::new(Limits::blind_cubic())?)?;
        let shift_numerator = polynomial(&mut cas, rational, 3, &[(vec![0, 1, 0], half)])?;
        let shift = fraction(&mut cas, &domain, &shift_numerator, &leading)?;
        let radicand_numerator = polynomial(
            &mut cas,
            rational,
            3,
            &[(vec![1, 0, 1], negative), (vec![0, 2, 0], quarter)],
        )?;
        let squared = polynomial(
            &mut cas,
            rational,
            3,
            &[(vec![2, 0, 0], crate::number::Rational::one())],
        )?;
        let radicand = fraction(&mut cas, &domain, &radicand_numerator, &squared)?;
        let hypothesis = assumed(&mut cas, &context, &region, predicate)?;
        Ok(Self {
            cas,
            domain,
            region,
            context,
            polynomial: original,
            premises: vec![hypothesis],
            expected: vec![
                shift,
                values.first().ok_or("missing scale")?.clone(),
                radicand,
            ],
        })
    }
}

/// Canonical rational functions are admitted independently of the translation producer.
fn fraction(
    cas: &mut Cas,
    domain: &Handle,
    numerator: &Handle,
    denominator: &Handle,
) -> TestResult<Handle> {
    admit(
        cas,
        &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
            domain: Reference::Existing(domain.clone()),
            value: CoefficientValueData::Fraction {
                numerator: Reference::Existing(numerator.clone()),
                denominator: Reference::Existing(denominator.clone()),
            },
        })),
    )
}

/// Hypothesis provenance remains visible to the recipe's regional inversion interface.
fn assumed(
    cas: &mut Cas,
    context: &Handle,
    region: &Handle,
    predicate: Handle,
) -> TestResult<crate::proof_model::VerifiedWitness> {
    let step = crate::proof_model::ApplicationDraft {
        rule: crate::rule::Primitive::ContextHypothesis,
        context: context.clone(),
        conclusion: crate::proof_model::Statement::PredicateFact {
            region: region.clone(),
            predicate,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: crate::proof_model::Certificate::Hypothesis(
            crate::domain_model::HypothesisRef {
                context: context.clone(),
                slot: 0,
            },
        ),
    };
    Ok(cas.check_proof(
        context,
        std::slice::from_ref(&step),
        &mut Budget::new(Limits::blind_cubic())?,
    )?)
}
