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

impl Fixture {
    /// Independent ground-unit certificate 1*1+X*0=1 supplies the exact leading-unit hypothesis.
    pub(crate) fn ground_unit(&mut self) -> TestResult<crate::test_api::proof::ApplicationDraft> {
        let rational = self.rational.clone();
        let one = self.integer(1)?;
        let modulus = self.univariate(&rational, &[(1, one.clone())])?;
        let constant = self.univariate(&rational, &[(0, one.clone())])?;
        let zero = self.univariate(&rational, &[])?;
        Ok(unit_step(
            self,
            &one,
            &one,
            &rational,
            [&modulus, &constant, &constant, &constant, &zero, &modulus],
        ))
    }

    /// A root-adjunction step explicitly names its theorem mode and exact leading-unit premise slot.
    pub(crate) fn adjunction(
        &self,
        relation: &Handle,
        selection: Option<&Handle>,
        mode: crate::test_api::solving::AdjunctionMode,
        order: Option<u32>,
    ) -> crate::test_api::proof::ApplicationDraft {
        use crate::test_api::proof::{ApplicationDraft, Certificate, Primitive, Statement};
        use crate::test_api::solving::{
            CertificateRootAdjunction, SolveCertificate, SolveSchema, SolveStatement,
            StatementRootExistence,
        };
        ApplicationDraft {
            rule: Primitive::Solve(SolveSchema::RootAdjunction),
            context: self.context.clone(),
            conclusion: Statement::Solve(Box::new(SolveStatement::RootExistence(
                StatementRootExistence {
                    relation: selection.unwrap_or(relation).clone(),
                    region: self.region.clone(),
                },
            ))),
            premises: vec![0],
            substitutions: Vec::new(),
            certificate: Certificate::Solve(Box::new(SolveCertificate::RootAdjunction(
                CertificateRootAdjunction {
                    mode,
                    relation: relation.clone(),
                    selection: selection.cloned(),
                    leading_unit: 0,
                    order,
                    splitting_chain: Vec::new(),
                },
            ))),
        }
    }

    /// A canonical parent constant has a direct independent unit certificate in a named quotient.
    fn quotient_one_unit(
        &mut self,
        domain: &Handle,
        modulus: &Handle,
    ) -> TestResult<(Handle, crate::test_api::proof::ApplicationDraft)> {
        let rational = self.rational.clone();
        let one = self.integer(1)?;
        let value = self.tower_value(domain, std::slice::from_ref(&one))?;
        let constant = self.univariate(&rational, &[(0, one)])?;
        let zero = self.univariate(&rational, &[])?;
        let step = unit_step(
            self,
            &value,
            &value,
            domain,
            [modulus, &constant, &constant, &constant, &zero, modulus],
        );
        Ok((value, step))
    }

    /// Select u in this quotient, then make v's polynomial evaluate through that same u.
    fn linked_pair(
        &mut self,
        domain: &Handle,
        modulus: &Handle,
        minus_generator: &Handle,
    ) -> TestResult<Vec<crate::test_api::proof::ApplicationDraft>> {
        use crate::test_api::solving::{AdjunctionMode, CoefficientMapData, MapMode, MapTarget};
        let ground = self.ground_unit()?;
        let (first_relation, first_selection) = self.root(modulus, &Field::Complex)?;
        let first = self.adjunction(
            &first_relation,
            Some(&first_selection),
            AdjunctionMode::ComplexFTA,
            None,
        );
        let selected = admit(
            &mut self.cas,
            &Draft::RootValue {
                header: Reference::Existing(self.header.clone()),
                selection: Reference::Existing(first_selection),
                slot: 0,
            },
        )?;
        let map = admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
                source: Reference::Existing(domain.clone()),
                target: MapTarget::ScalarAmbient {
                    header: Reference::Existing(self.header.clone()),
                    field: Field::Complex,
                },
                mode: MapMode::Specialization,
                region: Reference::Existing(self.region.clone()),
                images: vec![Reference::Existing(selected)],
                denominators: Vec::new(),
            })),
        )?;
        let (one, parent_unit) = self.quotient_one_unit(domain, modulus)?;
        let polynomial = self.univariate(domain, &[(0, minus_generator.clone()), (1, one)])?;
        let (second_relation, second_selection) =
            self.mapped_root(&polynomial, &map, &Field::Complex)?;
        let mut second = self.adjunction(
            &second_relation,
            Some(&second_selection),
            AdjunctionMode::ComplexFTA,
            None,
        );
        second.premises = vec![2, 1];
        Ok(vec![ground, first, parent_unit, second])
    }

    /// One is a unit in every admitted coefficient algebra; the auxiliary ground modulus is X.
    fn fraction_one_unit(
        &mut self,
        one: &Handle,
    ) -> TestResult<crate::test_api::proof::ApplicationDraft> {
        let domain = self.fractions.clone();
        let modulus = self.univariate(&domain, &[(1, one.clone())])?;
        let constant = self.univariate(&domain, &[(0, one.clone())])?;
        let zero = self.univariate(&domain, &[])?;
        Ok(unit_step(
            self,
            one,
            one,
            &domain,
            [&modulus, &constant, &constant, &constant, &zero, &modulus],
        ))
    }
}

/// A supplied theorem fixture with an independently certified leading unit.
#[derive(Debug)]
pub(crate) struct AdjunctionFixture {
    /// Public graph and scoped hypotheses.
    pub(crate) base: Fixture,
    /// Exact root equation.
    pub(crate) relation: Handle,
    /// Ordered root selection, without a uniqueness theorem.
    pub(crate) selection: Handle,
    /// Leading unit followed by the requested root theorem.
    pub(crate) steps: Vec<crate::test_api::proof::ApplicationDraft>,
}

impl AdjunctionFixture {
    /// Ground coefficients are supplied independently as small signed integers.
    pub(crate) fn new(
        entries: &[(Vec<u32>, i64)],
        ambient: &Field,
        mode: crate::test_api::solving::AdjunctionMode,
        order: Option<u32>,
    ) -> TestResult<Self> {
        let mut base = Fixture::new()?;
        let polynomial = base.polynomial(1, entries)?;
        let (relation, any) = base.root(&polynomial, ambient)?;
        let selection = if mode == crate::test_api::solving::AdjunctionMode::PositiveRealSquareRoot
        {
            base.positive_selection(&relation, true)?
        } else {
            any
        };
        let unit = base.ground_unit()?;
        let step = base.adjunction(&relation, Some(&selection), mode, order);
        Ok(Self {
            base,
            relation,
            selection,
            steps: vec![unit, step],
        })
    }
}

impl SplitFixture {
    /// Both components retain an actual dependent relation v-u after projection to u=1 or u=-1.
    fn linked_children(
        &mut self,
        cover: &mut crate::test_api::proof::ApplicationDraft,
        children: &[Handle; 2],
    ) -> TestResult {
        use crate::test_api::proof::Certificate;
        use crate::test_api::solving::{CoefficientDomainData, SolveCertificate, SolveView};
        let mut selections = Vec::new();
        for (child, sign) in children.iter().zip([-1, 1]) {
            let SolveView::CoefficientDomain(CoefficientDomainData::Quotient { level }) =
                self.unit.base.cas.solve_view(child)?
            else {
                return Err("wrong child domain".into());
            };
            let SolveView::ExtensionLevel(extension) = self.unit.base.cas.solve_view(level)? else {
                return Err("wrong child level".into());
            };
            let modulus = extension.polynomial.clone();
            let negative = self.unit.base.integer(sign)?;
            let minus_generator = self
                .unit
                .base
                .tower_value(child, std::slice::from_ref(&negative))?;
            let fragment = self
                .unit
                .base
                .linked_pair(child, &modulus, &minus_generator)?;
            cover
                .premises
                .push(append_steps(&mut self.steps, fragment)?);
            let (_, selected) = self.unit.base.root(&modulus, &Field::Complex)?;
            selections.push(selected);
        }
        let Certificate::Solve(certificate) = &mut cover.certificate else {
            return Err("wrong cover certificate".into());
        };
        let SolveCertificate::ComponentSplit(split) = certificate.as_mut() else {
            return Err("wrong component cover".into());
        };
        split.selections = selections;
        Ok(())
    }

    /// One proof closure retains linked adjunctions and their reconstructed images on both exhaustive components.
    pub(crate) fn linked() -> TestResult<Self> {
        let mut fixture = Self::new()?;
        let children = fixture.children()?;
        let mut cover = fixture.steps.pop().ok_or("missing component cover")?;
        let zero = fixture.unit.base.integer(0)?;
        let negative = fixture.unit.base.integer(-1)?;
        let minus_generator = fixture
            .unit
            .base
            .tower_value(&fixture.unit.domain, &[zero, negative])?;
        let fragment = fixture.unit.base.linked_pair(
            &fixture.unit.domain,
            &fixture.unit.modulus,
            &minus_generator,
        )?;
        cover
            .premises
            .push(append_steps(&mut fixture.steps, fragment)?);
        fixture.linked_children(&mut cover, &children)?;
        fixture.steps.push(cover);
        Ok(fixture)
    }
}

impl AdjunctionFixture {
    /// Supply the independently known factorization X²-2=(X-t)(X+t) over t²=2.
    pub(crate) fn splitting_chain(sign: i64) -> TestResult<Self> {
        use crate::test_api::proof::Certificate;
        use crate::test_api::solving::{AdjunctionMode, SolveCertificate, SolveView};
        let mut fixture = Self::new(
            &[(vec![2], 1), (vec![0], -2)],
            &Field::Complex,
            AdjunctionMode::ComplexFTA,
            None,
        )?;
        let SolveView::RootRelation(relation) = fixture.base.cas.solve_view(&fixture.relation)?
        else {
            return Err("wrong root relation".into());
        };
        let polynomial = relation.polynomial.clone();
        let rational = fixture.base.rational.clone();
        let (_, domain) = fixture.base.tower(&rational, &polynomial)?;
        let zero = fixture.base.integer(0)?;
        let signed = fixture.base.integer(sign)?;
        let one = fixture.base.integer(1)?;
        let generator = fixture.base.tower_value(&domain, &[zero, signed])?;
        let constant = fixture
            .base
            .tower_value(&domain, std::slice::from_ref(&one))?;
        let quotient = fixture
            .base
            .univariate(&domain, &[(0, generator), (1, constant)])?;
        let step = fixture.steps.last_mut().ok_or("missing adjunction")?;
        let Certificate::Solve(certificate) = &mut step.certificate else {
            return Err("wrong certificate".into());
        };
        let SolveCertificate::RootAdjunction(adjunction) = certificate.as_mut() else {
            return Err("wrong adjunction".into());
        };
        adjunction.splitting_chain.push(quotient);
        Ok(fixture)
    }

    /// A symbolic square root consumes precisely the contextual sign fact on its interpreted radicand.
    pub(crate) fn parameter_square_root() -> TestResult<Self> {
        use crate::test_api::solving::AdjunctionMode;
        use crate::test_api::syntax::{Operator, PredicateKind};
        let mut base = Fixture::new()?;
        let denominator = base.polynomial(2, &[(vec![0, 0], 1)])?;
        let numerator = base.polynomial(2, &[(vec![1, 0], -1)])?;
        let one_draft = base.fraction(&denominator, &denominator);
        let one = admit(&mut base.cas, &one_draft)?;
        let constant_draft = base.fraction(&numerator, &denominator);
        let constant = admit(&mut base.cas, &constant_draft)?;
        let map = base.parameter_map()?;
        let literal = base.literal(&constant, &map)?;
        let radicand = admit(
            &mut base.cas,
            &super::solve_graph::expression(
                Reference::Existing(base.header.clone()),
                Operator::Neg,
                Sort::Value(AtomicSort::Scalar(Field::Real)),
                vec![Reference::Existing(literal)],
            ),
        )?;
        let assumption = base.assumption(&radicand, PredicateKind::NonNegative)?;
        let unit = base.fraction_one_unit(&one)?;
        let domain = base.fractions.clone();
        let polynomial = base.univariate(&domain, &[(0, constant), (2, one)])?;
        let (relation, _) = base.mapped_root(&polynomial, &map, &Field::Real)?;
        let selection = base.positive_selection(&relation, false)?;
        let mut root = base.adjunction(
            &relation,
            Some(&selection),
            AdjunctionMode::PositiveRealSquareRoot,
            Some(2),
        );
        root.premises = vec![1, 0];
        Ok(Self {
            base,
            relation,
            selection,
            steps: vec![assumption, unit, root],
        })
    }
}

/// Append a complete supplied proof fragment, shifting only its explicit earlier-step indices.
fn append_steps(
    target: &mut Vec<crate::test_api::proof::ApplicationDraft>,
    mut fragment: Vec<crate::test_api::proof::ApplicationDraft>,
) -> TestResult<u32> {
    let offset = u32::try_from(target.len())?;
    for step in &mut fragment {
        for premise in &mut step.premises {
            *premise = premise.checked_add(offset).ok_or("premise overflow")?;
        }
    }
    target.extend(fragment);
    Ok(u32::try_from(
        target.len().checked_sub(1).ok_or("empty proof fragment")?,
    )?)
}
