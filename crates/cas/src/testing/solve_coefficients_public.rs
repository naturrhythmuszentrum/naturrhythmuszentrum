// SPDX-License-Identifier: AGPL-3.0-only
//! Public coefficient proof fixtures reuse the shared arithmetic syntax setup.

#[path = "solve_coefficients.rs"]
#[expect(
    clippy::redundant_pub_crate,
    reason = "shared coefficient fixture types remain crate-visible inside the public harness"
)]
mod shared;

use crate::test_api::domain::ContextDraft;
use crate::test_api::limits::{Budget, Limits};
use crate::test_api::solving::{CoefficientValueData, CoefficientValueObject, SolveDraft};
use crate::test_api::syntax::{AtomicSort, Draft, Field, Sort};
use crate::test_api::{Handle, Rational, Reference};
use crate::testing::common::TestResult;
use crate::testing::solve_graph::admit;
use shared::unit_step;
pub(crate) use shared::{Fixture, SplitFixture, UnitFixture};

impl UnitFixture {
    /// Independently supplied t*t - (t²-1) = 1 is needed only by public replay tests.
    pub(crate) fn step(&mut self) -> TestResult<crate::test_api::proof::ApplicationDraft> {
        let rational = self.base.rational.clone();
        let one = self.base.integer(1)?;
        let negative = self.base.integer(-1)?;
        let gcd = self.base.univariate(&rational, &[(0, one.clone())])?;
        let polynomial_t = self.base.univariate(&rational, &[(1, one)])?;
        let minus_one = self.base.univariate(&rational, &[(0, negative)])?;
        Ok(unit_step(
            &self.base,
            &self.generator,
            &self.generator,
            &self.domain,
            [
                &self.modulus,
                &gcd,
                &polynomial_t,
                &polynomial_t,
                &minus_one,
                &self.modulus,
            ],
        ))
    }
}

impl SplitFixture {
    /// Borrow the actual supplied component certificate, without duplicating its selection list.
    fn certificate(&self) -> TestResult<&crate::test_api::solving::CertificateComponentSplit> {
        use crate::test_api::proof::Certificate;
        use crate::test_api::solving::SolveCertificate;
        let step = self.steps.last().ok_or("missing component cover")?;
        let Certificate::Solve(certificate) = &step.certificate else {
            return Err("wrong certificate".into());
        };
        let SolveCertificate::ComponentSplit(split) = certificate.as_ref() else {
            return Err("wrong component certificate".into());
        };
        Ok(split)
    }

    /// Child domains come from the graph-owned split used by the supplied certificate.
    pub(crate) fn children(&self) -> TestResult<[Handle; 2]> {
        use crate::test_api::solving::SolveView;
        let certificate = self.certificate()?;
        let SolveView::ComponentSplit(split) = self.unit.base.cas.solve_view(&certificate.split)?
        else {
            return Err("wrong split".into());
        };
        Ok([split.left_domain.clone(), split.right_domain.clone()])
    }

    /// Retain the certificate's actual ordered root selections for public transport mutations.
    pub(crate) fn selections(&self) -> TestResult<&[Handle]> {
        Ok(self.certificate()?.selections.as_slice())
    }
}

impl Fixture {
    /// Canonical constant map into an explicitly named scalar ambient.
    pub(crate) fn scalar_map(&mut self, domain: &Handle, field: &Field) -> TestResult<Handle> {
        use crate::test_api::solving::{CoefficientMapData, MapMode, MapTarget};
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
                source: Reference::Existing(domain.clone()),
                target: MapTarget::ScalarAmbient {
                    header: Reference::Existing(self.header.clone()),
                    field: field.clone(),
                },
                mode: MapMode::Embedding,
                region: Reference::Existing(self.region.clone()),
                images: Vec::new(),
                denominators: Vec::new(),
            })),
        )
    }

    /// A coefficient literal is linked to its exact scalar interpretation map.
    pub(crate) fn literal(&mut self, value: &Handle, map: &Handle) -> TestResult<Handle> {
        admit(
            &mut self.cas,
            &Draft::ExactCoefficient {
                header: Reference::Existing(self.header.clone()),
                value: Reference::Existing(value.clone()),
                map: Reference::Existing(map.clone()),
            },
        )
    }
}

/// The independent public Gaussian cube fixture has no coefficient producer dependency.
pub(crate) fn gaussian_identity(
    imaginary: i64,
) -> TestResult<(Fixture, crate::test_api::proof::ApplicationDraft)> {
    use crate::test_api::syntax::Operator;
    let mut fixture = Fixture::new()?;
    let gaussian = fixture.gaussian.clone();
    let mut budget = Budget::new(Limits::default())?;
    let real = Rational::from_ratio(2, 1, &mut budget)?;
    let value = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
            domain: Reference::Existing(gaussian.clone()),
            value: CoefficientValueData::Gaussian {
                real: real.clone(),
                imaginary: Rational::one(),
            },
        })),
    )?;
    let expected = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
            domain: Reference::Existing(gaussian.clone()),
            value: CoefficientValueData::Gaussian {
                real,
                imaginary: Rational::from_ratio(imaginary, 1, &mut budget)?,
            },
        })),
    )?;
    let map = fixture.scalar_map(&gaussian, &Field::Complex)?;
    let left = fixture.literal(&value, &map)?;
    let right = fixture.literal(&expected, &map)?;
    let cube = admit(
        &mut fixture.cas,
        &super::solve_graph::expression(
            Reference::Existing(fixture.header.clone()),
            Operator::IntegerPower(3),
            Sort::Value(AtomicSort::Scalar(Field::Complex)),
            vec![Reference::Existing(left)],
        ),
    )?;
    let step = fixture.identity(&cube, &right, &map);
    Ok((fixture, step))
}

impl Fixture {
    /// A positive or nonnegative root predicate is explicit in a single real root binder.
    pub(crate) fn positive_selection(
        &mut self,
        relation: &Handle,
        strict: bool,
    ) -> TestResult<Handle> {
        use crate::test_api::solving::{RootSelectionData, SelectionMode};
        use crate::test_api::syntax::{ExpressionDraft, Operator, PredicateKind};
        let scope = vec![vec![AtomicSort::Scalar(Field::Real)]];
        let bound = admit(
            &mut self.cas,
            &Draft::Expression(ExpressionDraft {
                header: Reference::Existing(self.header.clone()),
                operator: Operator::Bound { depth: 0, slot: 0 },
                sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
                space: None,
                scope: scope.clone(),
                binding: None,
                operands: Vec::new(),
            }),
        )?;
        let predicate = admit(
            &mut self.cas,
            &Draft::Condition {
                header: Reference::Existing(self.header.clone()),
                scope,
                kind: if strict {
                    PredicateKind::Positive
                } else {
                    PredicateKind::NonNegative
                },
                arguments: vec![Reference::Existing(bound)],
            },
        )?;
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::RootSelection(RootSelectionData {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                relations: vec![Reference::Existing(relation.clone())],
                predicate: Reference::Existing(predicate),
                mode: SelectionMode::Constrained,
                convention: None,
                interval: None,
            })),
        )
    }
}

impl Fixture {
    /// A declared parameter expression carries the fixture's exact semantic header.
    pub(crate) fn free(&mut self, key: &str) -> TestResult<Handle> {
        use crate::test_api::syntax::Operator;
        admit(
            &mut self.cas,
            &super::solve_graph::expression(
                Reference::Existing(self.header.clone()),
                Operator::Free(key.into()),
                Sort::Value(AtomicSort::Scalar(Field::Real)),
                Vec::new(),
            ),
        )
    }

    /// Install one explicit contextual assumption, retaining its hypothesis in every downstream witness.
    pub(crate) fn assumption(
        &mut self,
        argument: &Handle,
        kind: crate::test_api::syntax::PredicateKind,
    ) -> TestResult<crate::test_api::proof::ApplicationDraft> {
        use crate::test_api::domain::{AssumptionDraft, HypothesisRef};
        use crate::test_api::proof::{ApplicationDraft, Certificate, Primitive, Statement};
        let predicate = admit(
            &mut self.cas,
            &Draft::Condition {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                kind,
                arguments: vec![Reference::Existing(argument.clone())],
            },
        )?;
        self.context = admit(
            &mut self.cas,
            &Draft::Context(ContextDraft::Root {
                header: Reference::Existing(self.header.clone()),
                admissible: Reference::Existing(self.region.clone()),
                assumptions: vec![AssumptionDraft {
                    key: "nonzero-parameter".into(),
                    condition: Reference::Existing(predicate.clone()),
                }],
            }),
        )?;
        Ok(ApplicationDraft {
            rule: Primitive::ContextHypothesis,
            context: self.context.clone(),
            conclusion: Statement::PredicateFact {
                region: self.region.clone(),
                predicate,
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::Hypothesis(HypothesisRef {
                context: self.context.clone(),
                slot: 0,
            }),
        })
    }
}

impl Fixture {
    /// Canonical parameter interpretation, retaining the exact declared coordinate order.
    pub(crate) fn parameter_map(&mut self) -> TestResult<Handle> {
        use crate::test_api::solving::{CoefficientMapData, MapMode, MapTarget};
        let first = self.free("x")?;
        let second = self.free("y")?;
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
                source: Reference::Existing(self.fractions.clone()),
                target: MapTarget::ScalarAmbient {
                    header: Reference::Existing(self.header.clone()),
                    field: Field::Real,
                },
                mode: MapMode::Specialization,
                region: Reference::Existing(self.region.clone()),
                images: vec![Reference::Existing(first), Reference::Existing(second)],
                denominators: Vec::new(),
            })),
        )
    }
}

/// The independent fraction product (x/y)(y/x)=1 retains the exact nonzero hypothesis on x/y.
pub(crate) fn fraction_identity(
    reciprocal: bool,
) -> TestResult<(Fixture, Vec<crate::test_api::proof::ApplicationDraft>)> {
    use crate::test_api::syntax::{Operator, PredicateKind};
    let mut fixture = Fixture::new()?;
    let numerator = fixture.polynomial(2, &[(vec![1, 0], 1)])?;
    let denominator = fixture.polynomial(2, &[(vec![0, 1], 1)])?;
    let constant = fixture.polynomial(2, &[(vec![0, 0], 1)])?;
    let first_draft = fixture.fraction(&numerator, &denominator);
    let second_draft = if reciprocal {
        fixture.fraction(&denominator, &numerator)
    } else {
        first_draft.clone()
    };
    let one_draft = fixture.fraction(&constant, &constant);
    let first_value = admit(&mut fixture.cas, &first_draft)?;
    let second_value = admit(&mut fixture.cas, &second_draft)?;
    let one = admit(&mut fixture.cas, &one_draft)?;
    let map = fixture.parameter_map()?;
    let first = fixture.literal(&first_value, &map)?;
    let second = fixture.literal(&second_value, &map)?;
    let expected = fixture.literal(&one, &map)?;
    let assumption = fixture.assumption(&first, PredicateKind::NonZero)?;
    let product = admit(
        &mut fixture.cas,
        &super::solve_graph::expression(
            Reference::Existing(fixture.header.clone()),
            Operator::Mul,
            Sort::Value(AtomicSort::Scalar(Field::Real)),
            vec![Reference::Existing(first), Reference::Existing(second)],
        ),
    )?;
    let mut identity = fixture.identity(&product, &expected, &map);
    identity.premises = vec![0];
    Ok((fixture, vec![assumption, identity]))
}
