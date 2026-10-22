// SPDX-License-Identifier: AGPL-3.0-only
//! Small original Real/Complex fixtures for current structural syntax.

use super::common::TestResult;
use crate::test_api::domain::{AmbientDomain, HeaderDraft, RegionDraft, RegionExpr};
use crate::test_api::limits::{Budget, Limits};
use crate::test_api::solving::{
    Coordinate, CoordinateRole, CoordinateSpaceData, FamilyArgument, FamilyArgumentRole,
    FamilyExpressionData, LiesOver, ParameterBaseData, ScalarPieceData, SolveDraft,
    SpaceFamilyData, SpacePlacementData,
};
use crate::test_api::syntax::{
    AtomicSort, Branch, Declaration, Draft, Environment, ExpressionDraft, Field, Operator,
    PredicateKind, Scope, Sort,
};
use crate::test_api::{Cas, Handle, Reference};

/// Admit one reversible structural object with the standard ceilings.
pub(crate) fn admit(cas: &mut Cas, draft: Draft) -> TestResult<Handle> {
    cas.admit(&[draft], &mut Budget::new(Limits::default())?)?
        .pop()
        .ok_or_else(|| "missing admitted object".into())
}

/// Preserve a handle's exact original identity.
pub(crate) fn reference(value: &Handle) -> Reference {
    Reference::Existing(value.clone())
}

/// Original symbols and their independently checked own-space identities.
pub(crate) struct Fixture {
    /// Original cas retained by the fixture.
    pub(crate) cas: Cas,
    /// Original header retained by the fixture.
    pub(crate) header: Handle,
    /// Original real retained by the fixture.
    pub(crate) real: Handle,
    /// Original complex retained by the fixture.
    pub(crate) complex: Handle,
    /// Original real space retained by the fixture.
    pub(crate) real_space: Handle,
    /// Original complex space retained by the fixture.
    pub(crate) complex_space: Handle,
}

impl Fixture {
    /// Keep all declarations in one immutable header, with distinct scalar sorts.
    pub(crate) fn new() -> TestResult<Self> {
        Self::with_limits(Limits::default())
    }

    /// Mixed solving shares the unchanged acceptance profile.
    pub(crate) fn with_limits(limits: Limits) -> TestResult<Self> {
        let mut cas = Cas::new(limits)?;
        let declarations = [
            ("r", Field::Real),
            ("c", Field::Complex),
            ("z", Field::Complex),
        ]
        .into_iter()
        .map(|(key, field)| Declaration {
            key: key.into(),
            display_name: key.into(),
            sort: Sort::Value(AtomicSort::Scalar(field)),
        })
        .collect();
        let header = admit(
            &mut cas,
            Draft::Header(HeaderDraft::Core(Environment {
                field: Field::Complex,
                branch: Branch::PrincipalComplex,
                declarations,
            })),
        )?;
        let real = expression(
            &mut cas,
            &header,
            Field::Real,
            Operator::Free("r".into()),
            Vec::new(),
            Vec::new(),
        )?;
        let complex = expression(
            &mut cas,
            &header,
            Field::Complex,
            Operator::Free("c".into()),
            Vec::new(),
            Vec::new(),
        )?;
        let real_space = cas.scalar_ambient(&real)?.ok_or("missing Real ambient")?;
        let complex_space = cas
            .scalar_ambient(&complex)?
            .ok_or("missing Complex ambient")?;
        Ok(Self {
            cas,
            header,
            real,
            complex,
            real_space,
            complex_space,
        })
    }

    /// Construct exact base/fibre products without replacing any declaration.
    pub(crate) fn family(&mut self) -> TestResult<Family> {
        let parameters = self.coordinates(&[("r", Field::Real, CoordinateRole::Coefficient)])?;
        let coefficients =
            self.coordinates(&[("c", Field::Complex, CoordinateRole::Coefficient)])?;
        let unknowns = self.coordinates(&[("z", Field::Complex, CoordinateRole::Unknown)])?;
        let total = self.coordinates(&[
            ("r", Field::Real, CoordinateRole::Coefficient),
            ("c", Field::Complex, CoordinateRole::Coefficient),
        ])?;
        let base_region = self.region(&parameters)?;
        let base = admit(
            &mut self.cas,
            Draft::Solve(SolveDraft::ParameterBase(ParameterBaseData {
                header: reference(&self.header),
                coordinates: reference(&parameters),
                domain: reference(&base_region),
            })),
        )?;
        let family = admit(
            &mut self.cas,
            Draft::Solve(SolveDraft::SpaceFamily(SpaceFamilyData {
                base: reference(&base),
                fibre_space: reference(&self.complex_space),
                coefficients: reference(&coefficients),
                unknowns: reference(&unknowns),
            })),
        )?;
        let region = admit(
            &mut self.cas,
            Draft::FamilyRegion {
                region: RegionDraft {
                    header: reference(&self.header),
                    scope: Vec::new(),
                    ambient: AmbientDomain::Product {
                        coordinates: total.clone(),
                    },
                    expression: RegionExpr::Ambient,
                },
                lies_over: LiesOver {
                    family: reference(&family),
                    stratum: reference(&base_region),
                },
            },
        )?;
        Ok(Family {
            parameters,
            base_region,
            handle: family,
            region,
            total,
        })
    }

    /// Ordered coordinate claims are resolved to registered spaces by ordinary admission.
    pub(crate) fn coordinates(
        &mut self,
        entries: &[(&str, Field, CoordinateRole)],
    ) -> TestResult<Handle> {
        let coordinates = entries
            .iter()
            .map(|(key, field, role)| Coordinate {
                key: (*key).into(),
                field: field.clone(),
                role: *role,
            })
            .collect();
        admit(
            &mut self.cas,
            Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
                scalar_spaces: Vec::new(),
                header: reference(&self.header),
                coordinates,
            })),
        )
    }

    /// An original product domain makes no claim of nonemptiness.
    fn region(&mut self, coordinates: &Handle) -> TestResult<Handle> {
        admit(
            &mut self.cas,
            Draft::Region(RegionDraft {
                header: reference(&self.header),
                scope: Vec::new(),
                ambient: AmbientDomain::Product {
                    coordinates: coordinates.clone(),
                },
                expression: RegionExpr::Ambient,
            }),
        )
    }

    /// One original base operand and one original fibre operand form a local body.
    pub(crate) fn body(&mut self, shape: &Family) -> TestResult<Handle> {
        let draft = self.body_data(shape);
        admit(
            &mut self.cas,
            Draft::Solve(SolveDraft::FamilyExpression(draft)),
        )
    }

    /// Untrusted local role claims remain convenient to mutate independently.
    pub(crate) fn body_data(&self, shape: &Family) -> FamilyExpressionData {
        FamilyExpressionData {
            family: reference(&shape.handle),
            ambient: reference(&self.complex_space),
            region: reference(&shape.region),
            scope: Vec::new(),
            operator: Operator::Add,
            result: Sort::Value(AtomicSort::Scalar(Field::Complex)),
            arguments: vec![
                FamilyArgument {
                    role: FamilyArgumentRole::Base,
                    value: reference(&self.real),
                },
                FamilyArgument {
                    role: FamilyArgumentRole::Fibre,
                    value: reference(&self.complex),
                },
            ],
        }
    }

    /// Instantiate the installed incidence in its exact local query/value frame.
    pub(crate) fn piece(&mut self, region: &Handle) -> TestResult<Handle> {
        let placement = admit(
            &mut self.cas,
            Draft::Solve(SolveDraft::SpacePlacement(SpacePlacementData {
                source: reference(&self.real_space),
                ambient: reference(&self.complex_space),
                specification: "placement.real-in-complex".into(),
                version: 1,
            })),
        )?;
        let local_frame = incidence(&mut self.cas, &self.header)?;
        admit(
            &mut self.cas,
            Draft::Solve(SolveDraft::ScalarPiece(ScalarPieceData {
                placement: reference(&placement),
                piece: 0,
                region: reference(region),
                local_frame: reference(&local_frame),
            })),
        )
    }
}

/// The exact family and its original parameter and total regions.
pub(crate) struct Family {
    /// Original parameters retained by the fixture.
    pub(crate) parameters: Handle,
    /// Original base region retained by the fixture.
    pub(crate) base_region: Handle,
    /// Original family retained by the fixture.
    pub(crate) handle: Handle,
    /// Original region retained by the fixture.
    pub(crate) region: Handle,
    /// Original total retained by the fixture.
    pub(crate) total: Handle,
}

/// Build a scalar expression while retaining its exact requested field and scope.
pub(crate) fn expression(
    cas: &mut Cas,
    header: &Handle,
    field: Field,
    operator: Operator,
    scope: Scope,
    operands: Vec<Reference>,
) -> TestResult<Handle> {
    admit(
        cas,
        Draft::Expression(ExpressionDraft {
            space: None,
            operator,
            sort: Sort::Value(AtomicSort::Scalar(field)),
            header: reference(header),
            scope,
            binding: None,
            operands,
        }),
    )
}

/// Build one typed Boolean equation inside an explicit local frame.
fn equality(
    cas: &mut Cas,
    header: &Handle,
    scope: &Scope,
    left: &Handle,
    right: &Handle,
) -> TestResult<Handle> {
    admit(
        cas,
        Draft::Expression(ExpressionDraft {
            space: None,
            operator: Operator::Predicate(PredicateKind::Equal),
            sort: Sort::Value(AtomicSort::Boolean),
            header: reference(header),
            scope: scope.clone(),
            binding: None,
            operands: vec![reference(left), reference(right)],
        }),
    )
}

/// Build the incidence payload for independent structural tests.
fn incidence(cas: &mut Cas, header: &Handle) -> TestResult<Handle> {
    let scope = vec![vec![
        AtomicSort::Scalar(Field::Complex),
        AtomicSort::Scalar(Field::Real),
    ]];
    let query = expression(
        cas,
        header,
        Field::Complex,
        Operator::Bound { depth: 0, slot: 0 },
        scope.clone(),
        Vec::new(),
    )?;
    let local = expression(
        cas,
        header,
        Field::Real,
        Operator::Bound { depth: 0, slot: 1 },
        scope.clone(),
        Vec::new(),
    )?;
    let imaginary = expression(
        cas,
        header,
        Field::Real,
        Operator::ImagPart,
        scope.clone(),
        vec![reference(&query)],
    )?;
    let real = expression(
        cas,
        header,
        Field::Real,
        Operator::RealPart,
        scope.clone(),
        vec![reference(&query)],
    )?;
    let zero = expression(
        cas,
        header,
        Field::Real,
        Operator::Rational(crate::test_api::Rational::zero()),
        scope.clone(),
        Vec::new(),
    )?;
    let first = equality(cas, header, &scope, &imaginary, &zero)?;
    let second = equality(cas, header, &scope, &real, &local)?;
    admit(
        cas,
        Draft::Condition {
            kind: PredicateKind::And,
            header: reference(header),
            scope,
            arguments: vec![reference(&first), reference(&second)],
        },
    )
}

/// A named assumption carries family syntax without asserting an unproved family identity.
pub(crate) fn assumed(
    cas: &mut Cas,
    header: &Handle,
    region: &Handle,
    value: &Handle,
) -> TestResult<crate::test_api::proof::VerifiedWitness> {
    use crate::test_api::domain::{AssumptionDraft, ContextDraft, HypothesisRef};
    use crate::test_api::proof::{ApplicationDraft, Certificate, Primitive, Statement};
    let predicate = admit(
        cas,
        Draft::Condition {
            kind: PredicateKind::Named("family-property".into()),
            header: reference(header),
            scope: Vec::new(),
            arguments: vec![reference(value)],
        },
    )?;
    let context = admit(
        cas,
        Draft::Context(ContextDraft::Root {
            header: reference(header),
            admissible: reference(region),
            assumptions: vec![AssumptionDraft {
                key: "supplied".into(),
                condition: reference(&predicate),
            }],
        }),
    )?;
    let step = ApplicationDraft {
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
            slot: 0,
        }),
    };
    Ok(cas.check_proof(&context, &[step], &mut Budget::new(Limits::default())?)?)
}

/// Independently reconstruct the same current family case and its open hypothesis.
pub(crate) fn family_witness() -> TestResult<(Cas, crate::test_api::proof::VerifiedWitness)> {
    use crate::test_api::domain::{CaseArmDraft, CasesDraft};
    use crate::test_api::solving::PlacementUse;
    let mut fixture = Fixture::new()?;
    let shape = fixture.family()?;
    let body = fixture.body(&shape)?;
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
            value: reference(&body),
        }),
    )?;
    let cases = admit(
        &mut fixture.cas,
        Draft::Cases(CasesDraft {
            header: reference(&fixture.header),
            scope: Vec::new(),
            domain: reference(&shape.region),
            arms: vec![reference(&arm)],
        }),
    )?;
    let witness = assumed(&mut fixture.cas, &fixture.header, &shape.region, &cases)?;
    Ok((fixture.cas, witness))
}
