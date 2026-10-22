// SPDX-License-Identifier: AGPL-3.0-only
//! Public hand-authored Real witnesses; expected signs and quotients never use solver arithmetic.

use crate::test_api::limits::{Budget, Limits};
use crate::test_api::proof::{ApplicationDraft, Certificate, Primitive, Statement};
use crate::test_api::solving::{self, SolveCertificate, SolveDraft, SolveSchema, SolveStatement};
use crate::test_api::syntax::{AtomicSort, Draft, Field, Operator, PredicateKind};
use crate::test_api::{Handle, Rational, Reference};
use crate::testing::common::TestResult;
use crate::testing::solve_blind::Fixture as Base;

/// One actual graph and its independently authored ordered proof proposal.
pub(crate) struct Fixture {
    /// Original Real declaration frame.
    pub(crate) base: Base,
    /// Earlier applications, followed by the requested theorem.
    pub(crate) steps: Vec<ApplicationDraft>,
}

/// Public admission is the sole syntax route used by this fixture.
fn admit(base: &mut Base, draft: Draft) -> TestResult<Handle> {
    base.cas
        .admit(&[draft], &mut Budget::new(Limits::blind_cubic())?)?
        .into_iter()
        .next()
        .ok_or_else(|| "Real fixture admission".into())
}

/// Keep the exact existing handle without duplicating mathematical declarations.
fn reference(handle: &Handle) -> Reference {
    Reference::Existing(handle.clone())
}

impl Fixture {
    /// The original frame contains only Real coefficients and an independently bound root slot.
    pub(crate) fn new() -> TestResult<Self> {
        Ok(Self {
            base: Base::with_parameters(&Field::Real, &["a"], Limits::blind_cubic())?,
            steps: Vec::new(),
        })
    }

    /// Append a typed proposal using the full already authored premise sequence.
    pub(crate) fn push(
        &mut self,
        schema: SolveSchema,
        statement: SolveStatement,
        certificate: SolveCertificate,
    ) -> TestResult<u32> {
        let index = u32::try_from(self.steps.len())?;
        self.steps.push(ApplicationDraft {
            rule: Primitive::Solve(schema),
            context: self.base.context.clone(),
            conclusion: Statement::Solve(Box::new(statement)),
            premises: (0..index).collect(),
            substitutions: Vec::new(),
            certificate: Certificate::Solve(Box::new(certificate)),
        });
        Ok(index)
    }

    /// Explicit small rational scalar syntax supplies an independent expected sign.
    pub(crate) fn literal(&mut self, numerator: i64, denominator: i64) -> TestResult<Handle> {
        let number = Rational::from_ratio(
            numerator,
            u64::try_from(denominator)?,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        self.base.expression(Operator::Rational(number), &[], false)
    }

    /// Exact rational coefficient object; no producer or normalization search computes its value.
    fn coefficient(&mut self, numerator: i64, denominator: i64) -> TestResult<Handle> {
        let number = Rational::from_ratio(
            numerator,
            u64::try_from(denominator)?,
            &mut Budget::new(Limits::blind_cubic())?,
        )?;
        let domain = reference(&self.base.algebra);
        admit(
            &mut self.base,
            Draft::Solve(SolveDraft::CoefficientValue(
                solving::CoefficientValueObject {
                    domain,
                    value: solving::CoefficientValueData::Rational { value: number },
                },
            )),
        )
    }

    /// Supply ascending rational coefficients directly; omitted zeros are test-side integers.
    fn polynomial(&mut self, coefficients: &[(i64, i64)]) -> TestResult<Handle> {
        let mut terms = Vec::new();
        for (index, (numerator, denominator)) in coefficients.iter().enumerate().rev() {
            if *numerator != 0 {
                terms.push(solving::PolynomialTerm {
                    exponents: vec![u32::try_from(index)?],
                    coefficient: reference(&self.coefficient(*numerator, *denominator)?),
                });
            }
        }
        let domain = reference(&self.base.algebra);
        admit(
            &mut self.base,
            Draft::Solve(SolveDraft::Polynomial(solving::PolynomialData {
                domain,
                variables: vec![Field::Real],
                terms,
            })),
        )
    }

    /// A literal sign is a one-node ordered-field derivation with a pinned numerical expectation.
    pub(crate) fn sign(&mut self, numerator: i64, denominator: i64) -> TestResult<u32> {
        let subject = self.literal(numerator, denominator)?;
        let conclusion = match numerator.cmp(&0) {
            std::cmp::Ordering::Less => solving::RealSignConclusion::Negative,
            std::cmp::Ordering::Equal => solving::RealSignConclusion::Zero,
            std::cmp::Ordering::Greater => solving::RealSignConclusion::Positive,
        };
        self.sign_steps(vec![solving::RealSignStep {
            rule: solving::RealSignRule::Rational,
            subject,
            conclusion,
            arguments: Vec::new(),
            premises: Vec::new(),
            exponent: 0,
        }])
    }

    /// The final sign-node subject and conclusion are part of the independently expected judgment.
    pub(crate) fn sign_steps(&mut self, steps: Vec<solving::RealSignStep>) -> TestResult<u32> {
        let root = u32::try_from(steps.len().checked_sub(1).ok_or("empty sign derivation")?)?;
        let last = steps.last().ok_or("missing final sign")?;
        let statement = solving::StatementRealSign {
            subject: last.subject.clone(),
            region: self.base.region.clone(),
            conclusion: last.conclusion,
        };
        let certificate = solving::CertificateRealSign {
            subject: last.subject.clone(),
            region: self.base.region.clone(),
            steps,
            root,
        };
        self.push(
            SolveSchema::RealSign,
            SolveStatement::RealSign(statement),
            SolveCertificate::RealSign(certificate),
        )
    }

    /// Ground unit evidence reconstructs q*(1/q)=1 and the auxiliary modulus X exactly.
    fn unit(&mut self, numerator: i64, denominator: i64) -> TestResult<u32> {
        let value = self.coefficient(numerator, denominator)?;
        let inverse = self.coefficient(denominator, numerator)?;
        let modulus = self.polynomial(&[(0, 1), (1, 1)])?;
        let gcd = self.polynomial(&[(1, 1)])?;
        let quotient = self.polynomial(&[(numerator, denominator)])?;
        let first = self.polynomial(&[(denominator, numerator)])?;
        let second = self.polynomial(&[])?;
        self.push(
            SolveSchema::ExtensionUnit,
            SolveStatement::Unit(solving::StatementUnit {
                value: value.clone(),
                inverse,
                domain: self.base.algebra.clone(),
                region: self.base.region.clone(),
            }),
            SolveCertificate::ExtensionUnit(solving::CertificateExtensionUnit {
                element: value,
                modulus: modulus.clone(),
                gcd,
                quotients: vec![quotient, modulus],
                bezout: vec![first, second],
                region: self.base.region.clone(),
            }),
        )
    }

    /// This actual Real relation retains the exact source polynomial and same-sort map.
    fn mapping(&mut self) -> TestResult<Handle> {
        let mapping = solving::CoefficientMapData {
            source: reference(&self.base.algebra),
            target: solving::MapTarget::ScalarAmbient {
                header: reference(&self.base.header),
                field: Field::Real,
            },
            mode: solving::MapMode::Specialization,
            region: reference(&self.base.region),
            images: Vec::new(),
            denominators: Vec::new(),
        };
        admit(
            &mut self.base,
            Draft::Solve(SolveDraft::CoefficientMap(mapping)),
        )
    }

    /// Keep the defining polynomial in its actual Real scalar frame.
    fn relation(&mut self, polynomial: &Handle) -> TestResult<Handle> {
        let map = self.mapping()?;
        let relation = solving::RootRelationData {
            header: reference(&self.base.header),
            scope: Vec::new(),
            ambient: Field::Real,
            polynomial: reference(polynomial),
            binder: vec![AtomicSort::Scalar(Field::Real)],
            map: reference(&map),
            region: reference(&self.base.region),
            multiplicity: None,
        };
        admit(
            &mut self.base,
            Draft::Solve(SolveDraft::RootRelation(relation)),
        )
    }

    /// A cell is unproved syntax, independent of the root subsequently selected in it.
    fn cell(
        &mut self,
        left: solving::RealEndpoint<Reference>,
        right: solving::RealEndpoint<Reference>,
        closed: bool,
    ) -> TestResult<Handle> {
        let cell = solving::RealCellData {
            header: reference(&self.base.header),
            scope: Vec::new(),
            region: reference(&self.base.region),
            left,
            right,
            left_open: !closed,
            right_open: !closed,
        };
        admit(&mut self.base, Draft::Solve(SolveDraft::RealCell(cell)))
    }
}

/// Independent exact division: f=x^3-3x+1, f'=3x^2-3, s2=2x-1, s3=9/4.
pub(crate) fn cubic() -> TestResult<Fixture> {
    let mut fixture = Fixture::new()?;
    let original = fixture.polynomial(&[(1, 1), (-3, 1), (0, 1), (1, 1)])?;
    let derivative = fixture.polynomial(&[(-3, 1), (0, 1), (3, 1)])?;
    let linear = fixture.polynomial(&[(-1, 1), (2, 1)])?;
    let constant = fixture.polynomial(&[(9, 4)])?;
    let relation = fixture.relation(&original)?;
    let gcd = fixture.polynomial(&[(1, 1)])?;
    let quotients = vec![
        derivative.clone(),
        fixture.polynomial(&[(-1, 3), (-2, 3)])?,
        fixture.polynomial(&[(-4, 9), (1, 9), (2, 9)])?,
        fixture.polynomial(&[(0, 1), (1, 3)])?,
        fixture.polynomial(&[(3, 4), (3, 2)])?,
        fixture.polynomial(&[(-4, 9), (8, 9)])?,
    ];
    let units = vec![
        fixture.unit(1, 1)?,
        fixture.unit(3, 1)?,
        fixture.unit(2, 1)?,
        fixture.unit(9, 4)?,
    ];
    let signs = cubic_signs(&mut fixture)?;
    let cells = cubic_cells(&mut fixture)?;
    let certificate = solving::CertificateRealRootCount {
        relation: relation.clone(),
        region: fixture.base.region.clone(),
        cells: cells.clone(),
        square_free: original.clone(),
        derivative: derivative.clone(),
        gcd,
        chain: vec![original, derivative, linear, constant],
        quotients,
        rescalings: Vec::new(),
        identities: Vec::new(),
        units,
        signs,
        counts: vec![1, 1, 1],
        exterior_left: 0,
        exterior_right: 0,
    };
    let statement = solving::StatementRealRootCount {
        relation,
        region: fixture.base.region.clone(),
        cells,
        counts: vec![1, 1, 1],
        exterior_left: 0,
        exterior_right: 0,
    };
    fixture.push(
        SolveSchema::RealRootCount,
        SolveStatement::RealRootCount(statement),
        SolveCertificate::RealRootCount(certificate),
    )?;
    Ok(fixture)
}

/// These exact endpoint values are calculated from the displayed integer polynomial and derivatives.
fn cubic_signs(fixture: &mut Fixture) -> TestResult<Vec<u32>> {
    let mut signs = Vec::new();
    for (numerator, denominator) in [
        (1, 1),
        (3, 1),
        (2, 1),
        (9, 4),
        (1, 1),
        (3, 1),
        (2, 1),
        (9, 4),
        (3, 1),
        (0, 1),
        (-6, 1),
        (-3, 1),
        (9, 4),
        (2, 1),
        (3, 1),
        (0, 1),
        (-6, 1),
        (-3, 1),
        (9, 4),
        (-1, 1),
        (0, 1),
        (6, 1),
        (1, 1),
        (9, 4),
        (-1, 1),
        (0, 1),
        (6, 1),
        (1, 1),
        (9, 4),
    ] {
        signs.push(fixture.sign(numerator, denominator)?);
    }
    Ok(signs)
}

/// The critical cells are ordered, disjoint and cover all other points except two checked nonroots.
fn cubic_cells(fixture: &mut Fixture) -> TestResult<Vec<Handle>> {
    use solving::RealEndpoint::{NegativeInfinity, PositiveInfinity, RealExpression};
    let negative = fixture.literal(-1, 1)?;
    let positive = fixture.literal(1, 1)?;
    Ok(vec![
        fixture.cell(
            NegativeInfinity,
            RealExpression(reference(&negative)),
            false,
        )?,
        fixture.cell(
            RealExpression(reference(&negative)),
            RealExpression(reference(&positive)),
            false,
        )?,
        fixture.cell(
            RealExpression(reference(&positive)),
            PositiveInfinity,
            false,
        )?,
    ])
}

/// An isolated selection uses its cell metadata and the exact tautological bound-slot predicate.
pub(crate) fn existence() -> TestResult<Fixture> {
    let mut fixture = cubic()?;
    let count = u32::try_from(fixture.steps.len() - 1)?;
    let Statement::Solve(statement) = &fixture.steps.last().ok_or("count")?.conclusion else {
        return Err("count statement".into());
    };
    let SolveStatement::RealRootCount(statement) = statement.as_ref() else {
        return Err("count shape".into());
    };
    let relation = statement.relation.clone();
    let cell = statement.cells.first().ok_or("cell")?.clone();
    let bound = fixture
        .base
        .expression(Operator::Bound { depth: 0, slot: 0 }, &[], true)?;
    let draft = Draft::Condition {
        header: reference(&fixture.base.header),
        scope: vec![vec![AtomicSort::Scalar(Field::Real)]],
        kind: PredicateKind::Equal,
        arguments: vec![reference(&bound), reference(&bound)],
    };
    let predicate = admit(&mut fixture.base, draft)?;
    let selected = solving::RootSelectionData {
        cell: Some(reference(&cell)),
        header: reference(&fixture.base.header),
        scope: Vec::new(),
        relations: vec![reference(&relation)],
        predicate: reference(&predicate),
        mode: solving::SelectionMode::Isolated,
        convention: None,
        interval: None,
    };
    let selection = admit(
        &mut fixture.base,
        Draft::Solve(SolveDraft::RootSelection(selected)),
    )?;
    fixture.push(
        SolveSchema::RealCellExists,
        SolveStatement::RootExistence(solving::StatementRootExistence {
            relation: selection,
            region: fixture.base.region.clone(),
        }),
        SolveCertificate::RealCellExists(solving::CertificateRealCellExists {
            relation,
            cell,
            predicate,
            region: fixture.base.region.clone(),
            count,
            definedness: vec![count],
            order: vec![count],
        }),
    )?;
    Ok(fixture)
}

/// Defined equality is independently checked before a sign can be transported across it.
impl Fixture {
    /// Supply a ground identity using the ordinary coefficient checker, never a sign shortcut.
    pub(crate) fn equality(&mut self, left: &Handle, right: &Handle) -> TestResult<u32> {
        let map = self.mapping()?;
        let index = u32::try_from(self.steps.len())?;
        self.steps.push(ApplicationDraft {
            rule: Primitive::Solve(SolveSchema::CoefficientIdentity),
            context: self.base.context.clone(),
            conclusion: Statement::Equality {
                left: left.clone(),
                right: right.clone(),
                domain: self.base.region.clone(),
                domain_contract: crate::test_api::proof::DomainContract::DefinedAndEqual,
            },
            premises: (0..index).collect(),
            substitutions: Vec::new(),
            certificate: Certificate::Solve(Box::new(SolveCertificate::CoefficientIdentity(
                solving::CertificateCoefficientIdentity {
                    mode: solving::IdentityMode::Normalize,
                    left: left.clone(),
                    right: right.clone(),
                    map,
                    region: self.base.region.clone(),
                    combinations: Vec::new(),
                    denominators: Vec::new(),
                },
            ))),
        });
        Ok(index)
    }
}

/// Supply a repeated-root original, its actual gcd, and open intervals interleaved with root singletons.
pub(crate) fn repeated() -> TestResult<Fixture> {
    let mut fixture = Fixture::new()?;
    let original = fixture.polynomial(&[(2, 1), (-3, 1), (0, 1), (1, 1)])?;
    let derivative = fixture.polynomial(&[(-3, 1), (0, 1), (3, 1)])?;
    let square_free = fixture.polynomial(&[(-2, 1), (1, 1), (1, 1)])?;
    let gradient = fixture.polynomial(&[(1, 1), (2, 1)])?;
    let constant = fixture.polynomial(&[(9, 4)])?;
    let relation = fixture.relation(&original)?;
    let gcd = fixture.polynomial(&[(-1, 1), (1, 1)])?;
    let quotients = vec![
        fixture.polynomial(&[(3, 1), (3, 1)])?,
        fixture.polynomial(&[(-1, 2)])?,
        fixture.polynomial(&[(0, 1), (1, 6)])?,
        fixture.polynomial(&[(1, 4), (1, 2)])?,
        fixture.polynomial(&[(4, 9), (8, 9)])?,
    ];
    let units = vec![
        fixture.unit(1, 1)?,
        fixture.unit(2, 1)?,
        fixture.unit(9, 4)?,
    ];
    let signs = repeated_signs(&mut fixture)?;
    let cells = repeated_cells(&mut fixture)?;
    let certificate = solving::CertificateRealRootCount {
        relation,
        region: fixture.base.region.clone(),
        cells,
        square_free: square_free.clone(),
        derivative,
        gcd,
        chain: vec![square_free, gradient, constant],
        quotients,
        rescalings: Vec::new(),
        identities: Vec::new(),
        units,
        signs,
        counts: vec![0, 1, 0, 1, 0],
        exterior_left: 0,
        exterior_right: 0,
    };
    count_statement(&mut fixture, certificate)?;
    Ok(fixture)
}

/// The two original roots have distinct singleton cells even though one has multiplicity two.
fn repeated_cells(fixture: &mut Fixture) -> TestResult<Vec<Handle>> {
    use solving::RealEndpoint::{NegativeInfinity, PositiveInfinity, RealExpression};
    let negative = fixture.literal(-2, 1)?;
    let positive = fixture.literal(1, 1)?;
    Ok(vec![
        fixture.cell(
            NegativeInfinity,
            RealExpression(reference(&negative)),
            false,
        )?,
        fixture.cell(
            RealExpression(reference(&negative)),
            RealExpression(reference(&negative)),
            true,
        )?,
        fixture.cell(
            RealExpression(reference(&negative)),
            RealExpression(reference(&positive)),
            false,
        )?,
        fixture.cell(
            RealExpression(reference(&positive)),
            RealExpression(reference(&positive)),
            true,
        )?,
        fixture.cell(
            RealExpression(reference(&positive)),
            PositiveInfinity,
            false,
        )?,
    ])
}

/// At either root the support's first derivative gives the two exact one-sided signs.
fn repeated_signs(fixture: &mut Fixture) -> TestResult<Vec<u32>> {
    let mut signs = Vec::new();
    for (numerator, denominator) in [(1, 1), (2, 1), (9, 4), (1, 1), (2, 1), (9, 4)] {
        signs.push(fixture.sign(numerator, denominator)?);
    }
    for endpoint in [-2, -2, -2, -2, 1, 1, 1, 1] {
        if signs.len() == 18 {
            signs.push(fixture.sign(3, 1)?);
        }
        for (numerator, denominator) in
            [(0, 1), (2 * endpoint + 1, 1), (2 * endpoint + 1, 1), (9, 4)]
        {
            signs.push(fixture.sign(numerator, denominator)?);
        }
    }
    Ok(signs)
}

/// The exact output counts are authored independently from any checker result.
fn count_statement(
    fixture: &mut Fixture,
    certificate: solving::CertificateRealRootCount,
) -> TestResult {
    let statement = solving::StatementRealRootCount {
        relation: certificate.relation.clone(),
        region: certificate.region.clone(),
        cells: certificate.cells.clone(),
        counts: certificate.counts.clone(),
        exterior_left: certificate.exterior_left,
        exterior_right: certificate.exterior_right,
    };
    fixture.push(
        SolveSchema::RealRootCount,
        SolveStatement::RealRootCount(statement),
        SolveCertificate::RealRootCount(certificate),
    )?;
    Ok(())
}
