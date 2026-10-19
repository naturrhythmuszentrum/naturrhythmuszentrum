// SPDX-License-Identifier: AGPL-3.0-only
//! Shared coefficient syntax and arithmetic fixtures for both test binaries.

use crate::test_api::domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
use crate::test_api::limits::{Budget, Limits};
use crate::test_api::solving::{
    CoefficientDomainData, CoefficientValueData, CoefficientValueObject, Coordinate,
    CoordinateRole, CoordinateSpaceData, PolynomialData, PolynomialTerm, SolveDraft,
};
use crate::test_api::syntax::{AtomicSort, Branch, Declaration, Draft, Environment, Field, Sort};
use crate::test_api::{Cas, Handle, Rational, Reference};
use crate::testing::common::TestResult;
use crate::testing::solve_graph::admit;

/// A two-parameter coefficient context and its exact ground/fraction domains.
#[derive(Debug)]
pub(crate) struct Fixture {
    /// Owning mathematical graph.
    pub(crate) cas: Cas,
    /// Full declaration header.
    pub(crate) header: Handle,
    /// Whole coefficient region.
    pub(crate) region: Handle,
    /// Empty-assumption context on that region.
    pub(crate) context: Handle,
    /// Rational ground domain.
    pub(crate) rational: Handle,
    /// Gaussian ground domain.
    pub(crate) gaussian: Handle,
    /// Independent rational-function field in x and y.
    pub(crate) fractions: Handle,
}

impl Fixture {
    /// Construct the shared scope using only ordinary public graph admission.
    pub(crate) fn new() -> TestResult<Self> {
        let mut cas = Cas::new(Limits::blind_cubic())?;
        let declarations = ["x", "y"]
            .into_iter()
            .map(|key| Declaration {
                key: key.into(),
                display_name: key.into(),
                sort: Sort::Value(AtomicSort::Scalar(Field::Real)),
            })
            .collect();
        let header = admit(
            &mut cas,
            &Draft::Header(HeaderDraft::Core(Environment {
                field: Field::Real,
                branch: Branch::StrictReal,
                declarations,
            })),
        )?;
        let coordinates = admit(
            &mut cas,
            &Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
                scalar_spaces: Vec::new(),
                header: Reference::Existing(header.clone()),
                coordinates: ["x", "y"]
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
            &mut cas,
            &Draft::Region(RegionDraft {
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Product {
                    coordinates: coordinates.clone(),
                },
                expression: RegionExpr::Ambient,
            }),
        )?;
        let context = admit(
            &mut cas,
            &Draft::Context(ContextDraft::Root {
                header: Reference::Existing(header.clone()),
                admissible: Reference::Existing(region.clone()),
                assumptions: Vec::new(),
            }),
        )?;
        let rational = admit(
            &mut cas,
            &Draft::Solve(SolveDraft::CoefficientDomain(
                CoefficientDomainData::Rational,
            )),
        )?;
        let gaussian = admit(
            &mut cas,
            &Draft::Solve(SolveDraft::CoefficientDomain(
                CoefficientDomainData::Gaussian,
            )),
        )?;
        let fractions = admit(
            &mut cas,
            &Draft::Solve(SolveDraft::CoefficientDomain(
                CoefficientDomainData::Fraction {
                    base: Reference::Existing(rational.clone()),
                    coordinates: Reference::Existing(coordinates),
                },
            )),
        )?;
        Ok(Self {
            cas,
            header,
            region,
            context,
            rational,
            gaussian,
            fractions,
        })
    }

    /// An independent integer fixture is parsed through the existing exact public API.
    pub(crate) fn integer(&mut self, value: i64) -> TestResult<Handle> {
        let scalar = Rational::from_ratio(value, 1, &mut Budget::new(Limits::default())?)?;
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
                domain: Reference::Existing(self.rational.clone()),
                value: CoefficientValueData::Rational { value: scalar },
            })),
        )
    }

    /// Supply canonical decreasing graded-lex integer terms without using coefficient arithmetic.
    pub(crate) fn polynomial(
        &mut self,
        variables: usize,
        entries: &[(Vec<u32>, i64)],
    ) -> TestResult<Handle> {
        let mut terms = Vec::new();
        for (exponents, coefficient) in entries {
            terms.push(PolynomialTerm {
                exponents: exponents.clone(),
                coefficient: Reference::Existing(self.integer(*coefficient)?),
            });
        }
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::Polynomial(PolynomialData {
                domain: Reference::Existing(self.rational.clone()),
                variables: vec![Field::Real; variables],
                terms,
            })),
        )
    }

    /// A supplied fraction remains an untrusted proposed canonical pair.
    pub(crate) fn fraction(&self, numerator: &Handle, denominator: &Handle) -> Draft {
        Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
            domain: Reference::Existing(self.fractions.clone()),
            value: CoefficientValueData::Fraction {
                numerator: Reference::Existing(numerator.clone()),
                denominator: Reference::Existing(denominator.clone()),
            },
        }))
    }
}

impl Fixture {
    /// Admit a supplied exact rational independently of coefficient arithmetic.
    pub(crate) fn rational_value(&mut self, text: &str) -> TestResult<Handle> {
        let value = Rational::parse(text, &mut Budget::new(Limits::default())?)?;
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
                domain: Reference::Existing(self.rational.clone()),
                value: CoefficientValueData::Rational { value },
            })),
        )
    }

    /// Construct a sparse univariate polynomial from independent ascending degree/coefficient pairs.
    pub(crate) fn univariate(
        &mut self,
        domain: &Handle,
        entries: &[(u32, Handle)],
    ) -> TestResult<Handle> {
        let terms = entries
            .iter()
            .rev()
            .map(|(power, coefficient)| PolynomialTerm {
                exponents: vec![*power],
                coefficient: Reference::Existing(coefficient.clone()),
            })
            .collect();
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::Polynomial(PolynomialData {
                domain: Reference::Existing(domain.clone()),
                variables: vec![Field::Real],
                terms,
            })),
        )
    }

    /// A monic quotient has no constructor field or reducedness assertion.
    pub(crate) fn tower(
        &mut self,
        parent: &Handle,
        polynomial: &Handle,
    ) -> TestResult<(Handle, Handle)> {
        use crate::test_api::solving::{ExtensionLevelData, SquareFreeRequest};
        let level = admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::ExtensionLevel(ExtensionLevelData {
                parent: Reference::Existing(parent.clone()),
                polynomial: Reference::Existing(polynomial.clone()),
                generator: vec![AtomicSort::Scalar(Field::Real)],
                square_free: SquareFreeRequest::Required,
            })),
        )?;
        let domain = admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::CoefficientDomain(
                CoefficientDomainData::Quotient {
                    level: Reference::Existing(level.clone()),
                },
            )),
        )?;
        Ok((level, domain))
    }

    /// Admit a canonical ascending reduced tower vector.
    pub(crate) fn tower_value(
        &mut self,
        domain: &Handle,
        entries: &[Handle],
    ) -> TestResult<Handle> {
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
                domain: Reference::Existing(domain.clone()),
                value: CoefficientValueData::Tower {
                    coefficients: entries.iter().cloned().map(Reference::Existing).collect(),
                },
            })),
        )
    }

    /// Canonical identity coefficient map on the shared coefficient region.
    pub(crate) fn identity_map(&mut self, domain: &Handle) -> TestResult<Handle> {
        use crate::test_api::solving::{CoefficientMapData, MapMode, MapTarget};
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
                source: Reference::Existing(domain.clone()),
                target: MapTarget::CoefficientDomain {
                    domain: Reference::Existing(domain.clone()),
                },
                mode: MapMode::Embedding,
                region: Reference::Existing(self.region.clone()),
                images: Vec::new(),
                denominators: Vec::new(),
            })),
        )
    }

    /// A fully supplied normalization certificate with no hidden producer arithmetic.
    pub(crate) fn identity(
        &self,
        left: &Handle,
        right: &Handle,
        map: &Handle,
    ) -> crate::test_api::proof::ApplicationDraft {
        use crate::test_api::proof::{ApplicationDraft, Certificate, Primitive, Statement};
        use crate::test_api::solving::{
            CertificateCoefficientIdentity, IdentityMode, SolveCertificate, SolveSchema,
            SolveStatement, StatementCoefficientEquality,
        };
        ApplicationDraft {
            rule: Primitive::Solve(SolveSchema::CoefficientIdentity),
            context: self.context.clone(),
            conclusion: Statement::Solve(Box::new(SolveStatement::CoefficientEquality(
                StatementCoefficientEquality {
                    left: left.clone(),
                    right: right.clone(),
                    map: map.clone(),
                    region: self.region.clone(),
                },
            ))),
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::Solve(Box::new(SolveCertificate::CoefficientIdentity(
                CertificateCoefficientIdentity {
                    mode: IdentityMode::Normalize,
                    left: left.clone(),
                    right: right.clone(),
                    map: map.clone(),
                    region: self.region.clone(),
                    combinations: Vec::new(),
                    denominators: Vec::new(),
                },
            ))),
        }
    }
}

/// One valid unit of the reducible algebra Q[t]/(t²-1), with independent supplied products.
#[derive(Debug)]
pub(crate) struct UnitFixture {
    /// Shared graph and scope.
    pub(crate) base: Fixture,
    /// Source extension level.
    pub(crate) level: Handle,
    /// Source quotient domain.
    pub(crate) domain: Handle,
    /// Its defining polynomial t²-1.
    pub(crate) modulus: Handle,
    /// Generator t.
    pub(crate) generator: Handle,
    /// Zero in the quotient, for certificate mutations.
    pub(crate) zero: Handle,
    /// Supplied derivative-unit proof establishing square-freeness.
    pub(crate) derivative: crate::test_api::proof::ApplicationDraft,
}

/// Package a complete finite gcd/unit certificate; each entry is untrusted syntax.
pub(super) fn unit_step(
    base: &Fixture,
    value: &Handle,
    inverse: &Handle,
    domain: &Handle,
    polynomials: [&Handle; 6],
) -> crate::test_api::proof::ApplicationDraft {
    use crate::test_api::proof::{ApplicationDraft, Certificate, Primitive, Statement};
    use crate::test_api::solving::{
        CertificateExtensionUnit, SolveCertificate, SolveSchema, SolveStatement, StatementUnit,
    };
    let [modulus, gcd, quotient, first, second, modulus_quotient] = polynomials;
    ApplicationDraft {
        rule: Primitive::Solve(SolveSchema::ExtensionUnit),
        context: base.context.clone(),
        conclusion: Statement::Solve(Box::new(SolveStatement::Unit(StatementUnit {
            value: value.clone(),
            inverse: inverse.clone(),
            domain: domain.clone(),
            region: base.region.clone(),
        }))),
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::Solve(Box::new(SolveCertificate::ExtensionUnit(
            CertificateExtensionUnit {
                element: value.clone(),
                modulus: modulus.clone(),
                gcd: gcd.clone(),
                quotients: vec![quotient.clone(), modulus_quotient.clone()],
                bezout: vec![first.clone(), second.clone()],
                region: base.region.clone(),
            },
        ))),
    }
}

impl UnitFixture {
    /// t*t - (t²-1) = 1; (2t)*(t/2) - (t²-1) = 1.
    pub(crate) fn new() -> TestResult<Self> {
        let mut base = Fixture::new()?;
        let rational = base.rational.clone();
        let zero_scalar = base.integer(0)?;
        let one = base.integer(1)?;
        let negative = base.integer(-1)?;
        let two = base.integer(2)?;
        let half = base.rational_value("1/2")?;
        let modulus = base.univariate(&rational, &[(0, negative.clone()), (2, one.clone())])?;
        let gcd = base.univariate(&rational, &[(0, one.clone())])?;
        let minus_one = base.univariate(&rational, &[(0, negative)])?;
        let twice_t = base.univariate(&rational, &[(1, two.clone())])?;
        let half_t = base.univariate(&rational, &[(1, half.clone())])?;
        let (level, domain) = base.tower(&rational, &modulus)?;
        let generator = base.tower_value(&domain, &[zero_scalar.clone(), one])?;
        let zero = base.tower_value(&domain, &[])?;
        let derivative_value = base.tower_value(&domain, &[zero_scalar.clone(), two])?;
        let derivative_inverse = base.tower_value(&domain, &[zero_scalar, half])?;
        let derivative = unit_step(
            &base,
            &derivative_value,
            &derivative_inverse,
            &domain,
            [&modulus, &gcd, &twice_t, &half_t, &minus_one, &modulus],
        );
        Ok(Self {
            base,
            level,
            domain,
            modulus,
            generator,
            zero,
            derivative,
        })
    }
}

impl Fixture {
    /// A root relation and unrestricted single-slot selection with an exact scoped tautology.
    pub(crate) fn root(
        &mut self,
        polynomial: &Handle,
        ambient: &Field,
    ) -> TestResult<(Handle, Handle)> {
        use crate::test_api::solving::{CoefficientMapData, MapMode, MapTarget, SolveView};
        let SolveView::Polynomial(stored) = self.cas.solve_view(polynomial)? else {
            return Err("wrong root polynomial".into());
        };
        let source = stored.domain.clone();
        let map = admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
                source: Reference::Existing(source),
                target: MapTarget::ScalarAmbient {
                    header: Reference::Existing(self.header.clone()),
                    field: ambient.clone(),
                },
                mode: MapMode::Specialization,
                region: Reference::Existing(self.region.clone()),
                images: Vec::new(),
                denominators: Vec::new(),
            })),
        )?;
        self.mapped_root(polynomial, &map, ambient)
    }

    /// A root relation can depend on an explicitly selected earlier algebraic value through its map.
    pub(crate) fn mapped_root(
        &mut self,
        polynomial: &Handle,
        map: &Handle,
        ambient: &Field,
    ) -> TestResult<(Handle, Handle)> {
        use crate::test_api::solving::RootRelationData;
        let relation = admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::RootRelation(RootRelationData {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                ambient: ambient.clone(),
                polynomial: Reference::Existing(polynomial.clone()),
                binder: vec![AtomicSort::Scalar(ambient.clone())],
                map: Reference::Existing(map.clone()),
                region: Reference::Existing(self.region.clone()),
                multiplicity: None,
            })),
        )?;
        let selected = self.any_selection(&relation, ambient)?;
        Ok((relation, selected))
    }

    /// The bound argument is always slot zero of the relation's actual ambient.
    fn bound_root(&mut self, ambient: &Field) -> TestResult<Handle> {
        use crate::test_api::syntax::{ExpressionDraft, Operator};
        admit(
            &mut self.cas,
            &Draft::Expression(ExpressionDraft {
                header: Reference::Existing(self.header.clone()),
                operator: Operator::Bound { depth: 0, slot: 0 },
                sort: Sort::Value(AtomicSort::Scalar(ambient.clone())),
                space: None,
                scope: vec![vec![AtomicSort::Scalar(ambient.clone())]],
                binding: None,
                operands: Vec::new(),
            }),
        )
    }

    /// An unrestricted root selection states only r=r inside the joint root binder.
    fn any_selection(&mut self, relation: &Handle, ambient: &Field) -> TestResult<Handle> {
        use crate::test_api::solving::{RootSelectionData, SelectionMode};
        use crate::test_api::syntax::PredicateKind;
        let bound = self.bound_root(ambient)?;
        let predicate = admit(
            &mut self.cas,
            &Draft::Condition {
                header: Reference::Existing(self.header.clone()),
                scope: vec![vec![AtomicSort::Scalar(ambient.clone())]],
                kind: PredicateKind::Equal,
                arguments: vec![
                    Reference::Existing(bound.clone()),
                    Reference::Existing(bound),
                ],
            },
        )?;
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::RootSelection(RootSelectionData {
                cell: None,
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                relations: vec![Reference::Existing(relation.clone())],
                predicate: Reference::Existing(predicate),
                mode: SelectionMode::Any,
                convention: None,
                interval: None,
            })),
        )
    }

    /// A component projection explicitly sends the parent generator to the child generator.
    pub(crate) fn projection(
        &mut self,
        source: &Handle,
        target: &Handle,
        image: &Handle,
    ) -> TestResult<Handle> {
        use crate::test_api::solving::{CoefficientMapData, MapMode, MapTarget};
        admit(
            &mut self.cas,
            &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
                source: Reference::Existing(source.clone()),
                target: MapTarget::CoefficientDomain {
                    domain: Reference::Existing(target.clone()),
                },
                mode: MapMode::Specialization,
                region: Reference::Existing(self.region.clone()),
                images: vec![Reference::Existing(image.clone())],
                denominators: Vec::new(),
            })),
        )
    }
}

/// Full public Chinese-remainder fixture for (t-1)(t+1).
#[derive(Debug)]
pub(crate) struct SplitFixture {
    /// Shared tower and derivative-unit witnesses.
    pub(crate) unit: UnitFixture,
    /// Ordered proof DAG: product identity, square-free unit, component cover.
    pub(crate) steps: Vec<crate::test_api::proof::ApplicationDraft>,
}

/// Construct both actual quotient projections without assuming a theorem from their syntax.
fn component_graph(unit: &mut UnitFixture, left: &Handle, right: &Handle) -> TestResult<Handle> {
    use crate::test_api::solving::ComponentSplitData;
    let rational = unit.base.rational.clone();
    let (_, first) = unit.base.tower(&rational, left)?;
    let (_, second) = unit.base.tower(&rational, right)?;
    let one = unit.base.integer(1)?;
    let negative = unit.base.integer(-1)?;
    let first_image = unit.base.tower_value(&first, std::slice::from_ref(&one))?;
    let second_image = unit
        .base
        .tower_value(&second, std::slice::from_ref(&negative))?;
    let first_map = unit.base.projection(&unit.domain, &first, &first_image)?;
    let second_map = unit.base.projection(&unit.domain, &second, &second_image)?;
    let split = admit(
        &mut unit.base.cas,
        &Draft::Solve(SolveDraft::ComponentSplit(ComponentSplitData {
            parent: Reference::Existing(unit.level.clone()),
            left_factor: Reference::Existing(left.clone()),
            right_factor: Reference::Existing(right.clone()),
            left_domain: Reference::Existing(first.clone()),
            right_domain: Reference::Existing(second.clone()),
            left_map: Reference::Existing(first_map),
            right_map: Reference::Existing(second_map),
            region: Reference::Existing(unit.base.region.clone()),
        })),
    )?;
    Ok(split)
}

/// The public cover references independently supplied product, coprimality and derivative-unit data.
fn cover_step(
    unit: &UnitFixture,
    split: Handle,
    bezout: Vec<Handle>,
    selections: &[Handle; 2],
) -> crate::test_api::proof::ApplicationDraft {
    use crate::test_api::proof::{ApplicationDraft, Certificate, Primitive, Statement};
    use crate::test_api::solving::{
        CertificateComponentSplit, SolveCertificate, SolveSchema, SolveStatement,
        StatementComponentCover,
    };
    ApplicationDraft {
        rule: Primitive::Solve(SolveSchema::ComponentSplit),
        context: unit.base.context.clone(),
        conclusion: Statement::Solve(Box::new(SolveStatement::ComponentCover(
            StatementComponentCover {
                split: split.clone(),
                region: unit.base.region.clone(),
            },
        ))),
        premises: vec![0, 1],
        substitutions: Vec::new(),
        certificate: Certificate::Solve(Box::new(SolveCertificate::ComponentSplit(
            CertificateComponentSplit {
                split,
                product: 0,
                bezout,
                square_free: 1,
                selections: selections.to_vec(),
            },
        ))),
    }
}

impl SplitFixture {
    /// Independent factors and Bezout multipliers: -(t-1)/2 + (t+1)/2 = 1.
    pub(crate) fn new() -> TestResult<Self> {
        let mut unit = UnitFixture::new()?;
        let left = unit.base.polynomial(1, &[(vec![1], 1), (vec![0], -1)])?;
        let right = unit.base.polynomial(1, &[(vec![1], 1), (vec![0], 1)])?;
        let split = component_graph(&mut unit, &left, &right)?;
        let (_, first_selection) = unit.base.root(&left, &Field::Real)?;
        let (_, second_selection) = unit.base.root(&right, &Field::Real)?;
        let first_scalar = unit.base.rational_value("-1/2")?;
        let second_scalar = unit.base.rational_value("1/2")?;
        let rational = unit.base.rational.clone();
        let first = unit.base.univariate(&rational, &[(0, first_scalar)])?;
        let second = unit.base.univariate(&rational, &[(0, second_scalar)])?;
        let map = unit.base.identity_map(&rational)?;
        let product = unit.base.identity(&unit.modulus, &unit.modulus, &map);
        let selections = [first_selection, second_selection];
        let cover = cover_step(&unit, split, vec![first, second], &selections);
        let derivative = unit.derivative.clone();
        Ok(Self {
            unit,
            steps: vec![product, derivative, cover],
        })
    }
}

impl Fixture {
    /// Build a scalar guard expression under the complete fixture header.
    pub(crate) fn guard_expression(
        &mut self,
        operator: crate::test_api::syntax::Operator,
        operands: &[Handle],
    ) -> TestResult<Handle> {
        admit(
            &mut self.cas,
            &crate::testing::solve_graph::expression(
                Reference::Existing(self.header.clone()),
                operator,
                Sort::Value(AtomicSort::Scalar(Field::Real)),
                operands.iter().cloned().map(Reference::Existing).collect(),
            ),
        )
    }

    /// Admit an exact product region without assuming any general region decision.
    pub(crate) fn region_expression(&mut self, expression: RegionExpr) -> TestResult<Handle> {
        use crate::test_api::solving::SolveView;
        let SolveView::CoefficientDomain(CoefficientDomainData::Fraction { coordinates, .. }) =
            self.cas.solve_view(&self.fractions)?
        else {
            return Err("missing parameter frame".into());
        };
        let ambient = AmbientDomain::Product {
            coordinates: coordinates.clone(),
        };
        admit(
            &mut self.cas,
            &Draft::Region(RegionDraft {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                ambient,
                expression,
            }),
        )
    }

    /// A fresh root context on a cell introduces no conditional hypothesis.
    pub(crate) fn on_region(&mut self, region: &Handle) -> TestResult {
        self.region.clone_from(region);
        self.context = admit(
            &mut self.cas,
            &Draft::Context(ContextDraft::Root {
                header: Reference::Existing(self.header.clone()),
                admissible: Reference::Existing(region.clone()),
                assumptions: Vec::new(),
            }),
        )?;
        Ok(())
    }

    /// Ordered zero/nonzero cells share the exact defined equality predicate and base.
    pub(crate) fn guard_cells(&mut self, expression: &Handle) -> TestResult<[Handle; 2]> {
        use crate::test_api::syntax::{Operator, PredicateKind};
        let zero = self.guard_expression(Operator::Rational(Rational::zero()), &[])?;
        let predicate = admit(
            &mut self.cas,
            &Draft::Condition {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                kind: PredicateKind::Equal,
                arguments: vec![
                    Reference::Existing(expression.clone()),
                    Reference::Existing(zero),
                ],
            },
        )?;
        let base = Reference::Existing(self.region.clone());
        let zero = self.region_expression(RegionExpr::GuardTrue {
            base: base.clone(),
            predicate: Reference::Existing(predicate.clone()),
        })?;
        let nonzero = self.region_expression(RegionExpr::GuardFalse {
            base,
            predicate: Reference::Existing(predicate),
        })?;
        Ok([zero, nonzero])
    }
}

impl Fixture {
    /// A supplied reciprocal retains its exact canonical denominator for both test binaries.
    pub(crate) fn reciprocal(&mut self, entries: &[(Vec<u32>, i64)]) -> TestResult<Handle> {
        let denominator = self.polynomial(2, entries)?;
        let one = self.polynomial(2, &[(vec![0, 0], 1)])?;
        let draft = self.fraction(&one, &denominator);
        admit(&mut self.cas, &draft)
    }

    /// Enter a fresh nonzero guard cell with no extra context assumption.
    pub(crate) fn on_nonzero(&mut self, expression: &Handle) -> TestResult {
        let children = self.guard_cells(expression)?;
        self.on_region(&children[1])
    }
}

/// Independently expanded denominators exercise powers, products and failed exact divisions.
pub(crate) fn nonzero_closure_fixture(case: usize) -> TestResult<(Fixture, Handle, bool)> {
    use crate::test_api::syntax::Operator;
    let mut fixture = Fixture::new()?;
    let x = fixture.guard_expression(Operator::Free("x".into()), &[])?;
    let y = fixture.guard_expression(Operator::Free("y".into()), &[])?;
    let sum = fixture.guard_expression(Operator::Add, &[x.clone(), y.clone()])?;
    let entries = match case {
        0..=2 => {
            fixture.on_nonzero(&x)?;
            vec![(vec![u32::try_from(case + 1)?, 0], 1)]
        }
        3 | 4 => {
            fixture.on_nonzero(&x)?;
            if case == 4 {
                fixture.on_nonzero(&y)?;
            }
            vec![(vec![2, 3], 1)]
        }
        5 | 6 => {
            fixture.on_nonzero(&sum)?;
            if case == 5 {
                vec![
                    (vec![3, 0], 1),
                    (vec![2, 1], 3),
                    (vec![1, 2], 3),
                    (vec![0, 3], 1),
                ]
            } else {
                vec![(vec![2, 0], 1), (vec![0, 2], 1)]
            }
        }
        7 => {
            let two = fixture.guard_expression(
                Operator::Rational(Rational::from_ratio(
                    2,
                    1,
                    &mut Budget::new(Limits::default())?,
                )?),
                &[],
            )?;
            let scaled = fixture.guard_expression(Operator::Mul, &[two, x])?;
            fixture.on_nonzero(&scaled)?;
            vec![(vec![3, 0], 1)]
        }
        8 | 9 => {
            fixture.on_nonzero(&x)?;
            if case == 8 {
                vec![(vec![3, 0], 1), (vec![0, 0], 1)]
            } else {
                vec![(vec![0, 2], 1), (vec![0, 0], 1)]
            }
        }
        _ => return Err("unknown closure fixture".into()),
    };
    let value = fixture.reciprocal(&entries)?;
    Ok((fixture, value, matches!(case, 0..=2 | 4 | 5 | 7)))
}
