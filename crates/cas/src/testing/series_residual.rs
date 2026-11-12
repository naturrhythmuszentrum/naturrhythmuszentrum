// SPDX-License-Identifier: AGPL-3.0-only
//! Independent full-polynomial oracles for finite residual client operations.

use super::common::{TestResult, default_budget, real};
use crate::test_api::domain::{
    AmbientDomain, AssumptionDraft, ContextDraft, HeaderDraft, HypothesisRef, RegionDraft,
    RegionExpr,
};
use crate::test_api::limits::Limits;
use crate::test_api::proof::{
    ApplicationDraft, Certificate, Primitive, Statement, VerifiedWitness,
};
use crate::test_api::series::{
    ResidualReference as R, ResidualRow, ResidualSolve, SeriesObject, SeriesOperation,
    SourceClause, SourceRequest,
};
use crate::test_api::solving::{
    CertificateMembership, CoefficientDomainData, Coordinate, CoordinateRole, CoordinateSpaceData,
    SolveCertificate, SolveDraft, SolveStatement, StatementMember,
};
use crate::test_api::syntax::{
    Branch, Declaration, Draft, Environment, ExpressionDraft, Field, Operator, PredicateKind,
};
use crate::test_api::{Cas, Handle, Rational, Reference};

/// Original syntax, separate from the admitted graph and production evaluator.
#[derive(Clone)]
pub(crate) enum Term {
    /// Rational constant.
    Q(&'static str),
    /// Original independent coordinate.
    X,
    /// Dependent jet coordinate of the indicated derivative order.
    Y(u32),
    /// Binary polynomial operation.
    Binary(Operator, Box<Self>, Box<Self>),
    /// Repeated multiplication, including its dead operand.
    Power(Box<Self>, u32),
}

/// Keep original operands ordered while building a hand expression tree.
pub(crate) fn binary(op: Operator, left: Term, right: Term) -> Term {
    Term::Binary(op, Box::new(left), Box::new(right))
}

/// Parse only independently supplied exact fixture values.
pub(crate) fn rational(value: &str) -> TestResult<Rational> {
    Ok(Rational::parse(value, &mut default_budget()?)?)
}

/// Independent declaration frames used by positive and boundary controls.
#[derive(Clone, Copy)]
enum FrameHeader {
    /// Original Real declarations.
    Core,
    /// Structural totality uses production defaults throughout.
    Totality,
    /// An additional Complex declaration.
    Complex,
    /// A conservative child of the original Real header.
    Extended,
    /// Consistently renamed declarations for independent source portability.
    Renamed,
}

/// Shared public frame with an explicit coefficient product and a distinct dependent.
pub(crate) struct Fixture {
    /// Owning graph.
    pub(crate) cas: Cas,
    /// Immutable declaration header.
    pub(crate) header: Handle,
    /// Exact coefficient region.
    pub(crate) region: Handle,
    /// Exact proof context.
    pub(crate) context: Handle,
    /// Coordinate x.
    pub(crate) x: Handle,
    /// Formal placeholder y.
    pub(crate) y: Handle,
    /// Q interpretation.
    pub(crate) field: Handle,
    /// Exact centre literal.
    pub(crate) centre: Handle,
    /// Independent full candidate coefficients.
    pub(crate) candidate: Vec<Rational>,
    /// Independent centre value.
    pub(crate) centre_value: Rational,
}

/// Admit one ordinary graph object without mathematical checking.
pub(crate) fn admit(cas: &mut Cas, draft: Draft) -> TestResult<Handle> {
    Ok(cas
        .admit(&[draft], &mut default_budget()?)?
        .pop()
        .ok_or("admission")?)
}

impl Fixture {
    /// The ordinary scalar checker can distinguish strict definedness from polynomial interpretation.
    pub(crate) fn defined(
        &mut self,
        value: &Handle,
    ) -> TestResult<crate::test_api::proof::ApplicationDraft> {
        use crate::test_api::solving::{SolveProblemData, SolveSchema, SolveView};
        let (_, expected) = self.expectations(&Term::Q("0"), &[Rational::zero()], 0)?;
        let Statement::Solve(statement) = expected.statement else {
            return Err("solve".into());
        };
        let SolveStatement::Member(mut statement) = *statement else {
            return Err("member".into());
        };
        let SolveView::SolveProblem(original) = self.cas.solve_view(&statement.problem)? else {
            return Err("problem".into());
        };
        let original = original.clone();
        let predicate = admit(
            &mut self.cas,
            Draft::Condition {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                kind: PredicateKind::Defined,
                arguments: vec![Reference::Existing(value.clone())],
            },
        )?;
        statement.problem = admit(
            &mut self.cas,
            Draft::Solve(SolveDraft::SolveProblem(SolveProblemData {
                header: Reference::Existing(original.header),
                scope: original.scope,
                predicate: Reference::Existing(predicate),
                coefficients: Reference::Existing(original.coefficients),
                unknowns: Reference::Existing(original.unknowns),
                region: Reference::Existing(original.region),
                domain: Reference::Existing(original.domain),
                coefficient_domain: Reference::Existing(original.coefficient_domain),
                ambient: original.ambient,
                denominators: Vec::new(),
            })),
        )?;
        let certificate = CertificateMembership {
            problem: statement.problem.clone(),
            tuple: statement.tuple.clone(),
            region: statement.region.clone(),
            substitutions: Vec::new(),
            definedness: Vec::new(),
        };
        Ok(ApplicationDraft {
            rule: Primitive::Solve(SolveSchema::Membership),
            context: self.context.clone(),
            conclusion: Statement::Solve(Box::new(SolveStatement::Member(statement))),
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::Solve(Box::new(SolveCertificate::Membership(certificate))),
        })
    }
    /// Build both expectations entirely from syntax, without admitting a proof in the receiver.
    #[expect(
        clippy::too_many_lines,
        reason = "independent expected syntax retains all metadata together"
    )]
    pub(crate) fn expectations(
        &mut self,
        term: &Term,
        residual: &[Rational],
        derivative_order: u32,
    ) -> TestResult<(
        crate::test_api::proof::Judgment,
        crate::test_api::proof::Judgment,
    )> {
        use crate::test_api::domain::Layer;
        use crate::test_api::series::{ResidualSubject, SeriesStatement};
        use crate::test_api::solving::{SolveProblemData, TupleData};
        use crate::test_api::syntax::{AtomicSort, Sort};
        let candidate = self.candidate.clone();
        let left = self.object(&candidate)?;
        let mut identity = vec![Rational::zero(); candidate.len()];
        if let Some(linear) = identity.get_mut(1) {
            *linear = Rational::one();
        }
        let right = self.object(&identity)?;
        let subject = ResidualSubject::Compose {
            left: left.clone(),
            right,
            output: left,
            order: u32::try_from(candidate.len())?,
            default_order: true,
        };
        let (expression, _) = self.term(term, residual.len())?;
        let output = self.object(residual)?;
        let zero = self.node(Operator::Rational(Rational::zero()), &[])?;
        let coefficients = self.cas.series_object(&output)?.coefficients.clone();
        let (kind, arguments) = if residual.len() == 1 {
            (PredicateKind::Equal, vec![coefficients[0].clone(), zero])
        } else {
            let mut equations = Vec::new();
            for coefficient in coefficients {
                equations.push(admit(
                    &mut self.cas,
                    Draft::Expression(ExpressionDraft {
                        header: Reference::Existing(self.header.clone()),
                        space: None,
                        scope: Vec::new(),
                        binding: None,
                        sort: Sort::Value(AtomicSort::Boolean),
                        operator: Operator::Predicate(PredicateKind::Equal),
                        operands: vec![
                            Reference::Existing(coefficient),
                            Reference::Existing(zero.clone()),
                        ],
                    }),
                )?);
            }
            (PredicateKind::And, equations)
        };
        let predicate = admit(
            &mut self.cas,
            Draft::Condition {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                kind,
                arguments: arguments.into_iter().map(Reference::Existing).collect(),
            },
        )?;
        let coefficients = admit(
            &mut self.cas,
            Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
                header: Reference::Existing(self.header.clone()),
                scalar_spaces: Vec::new(),
                coordinates: vec![Coordinate {
                    key: "x".into(),
                    field: Field::Real,
                    role: CoordinateRole::Coefficient,
                }],
            })),
        )?;
        let unknowns = admit(
            &mut self.cas,
            Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
                header: Reference::Existing(self.header.clone()),
                scalar_spaces: Vec::new(),
                coordinates: Vec::new(),
            })),
        )?;
        let domain = admit(
            &mut self.cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Product {
                    coordinates: unknowns.clone(),
                },
                expression: RegionExpr::Ambient,
            }),
        )?;
        let problem = admit(
            &mut self.cas,
            Draft::Solve(SolveDraft::SolveProblem(SolveProblemData {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                predicate: Reference::Existing(predicate),
                coefficients: Reference::Existing(coefficients),
                unknowns: Reference::Existing(unknowns.clone()),
                region: Reference::Existing(self.region.clone()),
                domain: Reference::Existing(domain),
                coefficient_domain: Reference::Existing(self.field.clone()),
                ambient: Field::Real,
                denominators: Vec::new(),
            })),
        )?;
        let tuple = admit(
            &mut self.cas,
            Draft::Solve(SolveDraft::Tuple(TupleData {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                coordinates: Reference::Existing(unknowns),
                entries: Vec::new(),
            })),
        )?;
        let residual = crate::test_api::proof::Judgment {
            context: self.context.clone(),
            layer: Layer::Logic,
            space: None,
            statement: Statement::Series(Box::new(SeriesStatement::EvaluatedResidual {
                subject,
                expression,
                dependent: self.y.clone(),
                output,
                problem: problem.clone(),
                derivative_order,
                order: u32::try_from(residual.len())?,
            })),
            residuals: Vec::new(),
        };
        let solve = crate::test_api::proof::Judgment {
            statement: Statement::Solve(Box::new(SolveStatement::Member(StatementMember {
                problem,
                tuple,
                region: self.region.clone(),
            }))),
            ..residual.clone()
        };
        Ok((residual, solve))
    }
    /// Original composition twin with independently chosen coefficients and order.
    pub(crate) fn compose_pair(
        &mut self,
        left: &[&str],
        right: &[&str],
    ) -> TestResult<VerifiedWitness> {
        let left = left
            .iter()
            .map(|v| rational(v))
            .collect::<TestResult<Vec<_>>>()?;
        let right = right
            .iter()
            .map(|v| rational(v))
            .collect::<TestResult<Vec<_>>>()?;
        let left = self.object(&left)?;
        let right = self.object(&right)?;
        Ok(self.cas.compute_series(
            &SeriesOperation::Compose {
                left,
                right,
                order: None,
            },
            &[],
            &mut default_budget()?,
        )?)
    }

    /// Supplied constant echelon evidence, with no production elimination involved.
    pub(crate) fn linear(&mut self, residual: &[Rational]) -> TestResult<ResidualSolve> {
        use crate::test_api::solving::{
            CertificateExactLinearEquivalence, RowOperation, StatementLinearSystem,
        };
        use crate::test_api::syntax::{AtomicSort, Extent, Sort};
        let mut values = residual
            .iter()
            .map(|v| Ok(v.negated(&mut default_budget()?)?))
            .collect::<TestResult<Vec<_>>>()?;
        let mut operations = Vec::new();
        let pivot = values.iter().position(|v| !v.is_zero());
        if let Some(index) = pivot
            && index != 0
        {
            values.swap(0, index);
            operations.push(RowOperation::Swap {
                left: 0,
                right: u32::try_from(index)?,
            });
        }
        let mut entries = Vec::new();
        for value in &values {
            entries.push(Reference::Existing(
                self.node(Operator::Rational(value.clone()), &[])?,
            ));
        }
        let order = u32::try_from(values.len())?;
        let matrix = admit(
            &mut self.cas,
            Draft::Expression(ExpressionDraft {
                header: Reference::Existing(self.header.clone()),
                space: None,
                scope: Vec::new(),
                binding: None,
                sort: Sort::Value(AtomicSort::Matrix {
                    field: Field::Real,
                    rows: Extent::Literal(order),
                    columns: Extent::Literal(1),
                }),
                operator: Operator::Matrix {
                    rows: order,
                    columns: 1,
                },
                operands: entries,
            }),
        )?;
        let minor = self.node(
            Operator::Rational(if pivot.is_some() {
                values[0].clone()
            } else {
                Rational::one()
            }),
            &[],
        )?;
        Ok(ResidualSolve {
            statement: SolveStatement::LinearSystem(StatementLinearSystem {
                problem: R::Problem,
                reduced: R::Existing(matrix.clone()),
                rank: 0,
                augmented_rank: u32::from(pivot.is_some()),
                region: R::Region,
                free_columns: Vec::new(),
            }),
            certificate: SolveCertificate::ExactLinearEquivalence(
                CertificateExactLinearEquivalence {
                    matrix: R::Matrix,
                    augmented_columns: 1,
                    operations,
                    pivots: if pivot.is_some() { vec![0] } else { Vec::new() },
                    minor: R::Existing(minor),
                    reduced: R::Existing(matrix),
                    region: R::Region,
                },
            ),
            premises: Vec::new(),
        })
    }
    /// Premise-free Product source fixture retains all production ceilings.
    pub(crate) fn totality(reversed: bool) -> TestResult<Self> {
        Self::framed(
            &["0", "1/3", "0", "-1/45", "0"],
            "0",
            reversed,
            false,
            FrameHeader::Totality,
        )
    }

    /// Add structural definedness beside the assumption-retention source controls.
    pub(crate) fn total_source(&mut self, source: &Term) -> TestResult<VerifiedWitness> {
        let (expression, _) = self.term(source, self.candidate.len())?;
        let defined = self
            .cas
            .defined_total(&expression, &self.context, &mut default_budget()?)?;
        assert!(defined.is_unconditional());
        Ok(self.cas.expand_series_source(
            &SourceRequest {
                clause: SourceClause::PolynomialSeed,
                source: expression,
                domain: self.region.clone(),
                field: self.field.clone(),
                centre: self.centre.clone(),
                variable: self.x.clone(),
                scope: Vec::new(),
                context: self.context.clone(),
                order: u32::try_from(self.candidate.len())?,
            },
            std::slice::from_ref(defined.proof()),
            &mut default_budget()?,
        )?)
    }

    /// Both allocation orders preserve exactly the same mathematical frame.
    pub(crate) fn new(
        values: &[&str],
        centre: &str,
        reversed: bool,
        empty: bool,
    ) -> TestResult<Self> {
        Self::with_complex(values, centre, reversed, empty, false)
    }

    /// Rename all three declarations without changing their mathematical roles.
    pub(crate) fn renamed(values: &[&str], reversed: bool) -> TestResult<Self> {
        Self::framed(values, "0", reversed, false, FrameHeader::Renamed)
    }

    /// A matching complex declaration distinguishes scalar derivative admission from this grammar.
    pub(crate) fn with_complex(
        values: &[&str],
        centre: &str,
        reversed: bool,
        empty: bool,
        complex: bool,
    ) -> TestResult<Self> {
        Self::framed(
            values,
            centre,
            reversed,
            empty,
            if complex {
                FrameHeader::Complex
            } else {
                FrameHeader::Core
            },
        )
    }

    /// An ancestor header supplies a distinct zero identity for exact-frame controls.
    pub(crate) fn with_extension(values: &[&str]) -> TestResult<Self> {
        Self::framed(values, "0", false, false, FrameHeader::Extended)
    }

    /// Build the complete frame before admitting any candidate coefficients.
    #[expect(
        clippy::too_many_lines,
        reason = "the complete independent frame is explicit"
    )]
    fn framed(
        values: &[&str],
        centre: &str,
        reversed: bool,
        empty: bool,
        frame: FrameHeader,
    ) -> TestResult<Self> {
        let names = if matches!(frame, FrameHeader::Renamed) {
            ["u", "v", "w"]
        } else {
            ["x", "y", "z"]
        };
        let mut cas = Cas::new(if matches!(frame, FrameHeader::Totality) {
            Limits::default()
        } else {
            Limits {
                work: 20_000_000,
                replay_work: 20_000_000,
                ..Limits::default()
            }
        })?;
        let header = admit(
            &mut cas,
            Draft::Header(HeaderDraft::Core(Environment {
                field: Field::Real,
                branch: Branch::StrictReal,
                declarations: names
                    .into_iter()
                    .map(|key| Declaration {
                        key: key.into(),
                        display_name: key.into(),
                        sort: real(),
                    })
                    .chain(matches!(frame, FrameHeader::Complex).then(|| Declaration {
                        key: "c".into(),
                        display_name: "c".into(),
                        sort: crate::test_api::syntax::Sort::Value(
                            crate::test_api::syntax::AtomicSort::Scalar(Field::Complex),
                        ),
                    }))
                    .collect(),
            })),
        )?;
        let header = if matches!(frame, FrameHeader::Extended) {
            admit(
                &mut cas,
                Draft::Header(HeaderDraft::Extend {
                    parent: Reference::Existing(header),
                    declarations: vec![Declaration {
                        key: "extension".into(),
                        display_name: "extension".into(),
                        sort: real(),
                    }],
                    spaces: Vec::new(),
                }),
            )?
        } else {
            header
        };
        let product = admit(
            &mut cas,
            Draft::Solve(SolveDraft::CoordinateSpace(CoordinateSpaceData {
                header: Reference::Existing(header.clone()),
                scalar_spaces: Vec::new(),
                coordinates: vec![Coordinate {
                    key: names[0].into(),
                    field: Field::Real,
                    role: CoordinateRole::Coefficient,
                }],
            })),
        )?;
        let mut region = admit(
            &mut cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Product {
                    coordinates: product.clone(),
                },
                expression: RegionExpr::Ambient,
            }),
        )?;
        let literal = |cas: &mut Cas, text: &str| {
            admit(
                cas,
                Draft::Expression(ExpressionDraft {
                    header: Reference::Existing(header.clone()),
                    space: None,
                    scope: Vec::new(),
                    binding: None,
                    sort: real(),
                    operator: Operator::Rational(rational(text)?),
                    operands: Vec::new(),
                }),
            )
        };
        if empty {
            let zero = literal(&mut cas, "0")?;
            let one = literal(&mut cas, "1")?;
            let predicate = admit(
                &mut cas,
                Draft::Condition {
                    header: Reference::Existing(header.clone()),
                    scope: Vec::new(),
                    kind: PredicateKind::Equal,
                    arguments: vec![Reference::Existing(zero), Reference::Existing(one)],
                },
            )?;
            region = admit(
                &mut cas,
                Draft::Region(RegionDraft {
                    header: Reference::Existing(header.clone()),
                    scope: Vec::new(),
                    ambient: AmbientDomain::Product {
                        coordinates: product,
                    },
                    expression: RegionExpr::GuardTrue {
                        base: Reference::Existing(region),
                        predicate: Reference::Existing(predicate),
                    },
                }),
            )?;
        }
        let context = admit(
            &mut cas,
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(header.clone()),
                admissible: Reference::Existing(region.clone()),
                assumptions: Vec::new(),
            }),
        )?;
        let mut declarations = Vec::new();
        for key in if reversed {
            [names[1], names[0]]
        } else {
            [names[0], names[1]]
        } {
            declarations.push(admit(
                &mut cas,
                Draft::Expression(ExpressionDraft {
                    header: Reference::Existing(header.clone()),
                    space: None,
                    scope: Vec::new(),
                    binding: None,
                    sort: real(),
                    operator: Operator::Free(key.into()),
                    operands: Vec::new(),
                }),
            )?);
        }
        if reversed {
            declarations.reverse();
        }
        let field = admit(
            &mut cas,
            Draft::Solve(SolveDraft::CoefficientDomain(
                CoefficientDomainData::Rational,
            )),
        )?;
        let centre_handle = literal(&mut cas, centre)?;
        Ok(Self {
            cas,
            header,
            region,
            context,
            x: declarations[0].clone(),
            y: declarations[1].clone(),
            field,
            centre: centre_handle,
            candidate: values
                .iter()
                .map(|value| rational(value))
                .collect::<TestResult<_>>()?,
            centre_value: rational(centre)?,
        })
    }

    /// Original expression admission is independent of oracle arithmetic.
    pub(crate) fn node(&mut self, operator: Operator, operands: &[Handle]) -> TestResult<Handle> {
        admit(
            &mut self.cas,
            Draft::Expression(ExpressionDraft {
                header: Reference::Existing(self.header.clone()),
                space: None,
                scope: Vec::new(),
                binding: None,
                sort: real(),
                operator,
                operands: operands.iter().cloned().map(Reference::Existing).collect(),
            }),
        )
    }

    /// Complete admitted candidate syntax supplies no theorem.
    pub(crate) fn object(&mut self, values: &[Rational]) -> TestResult<Handle> {
        let mut coefficients = Vec::new();
        for value in values {
            coefficients.push(Reference::Existing(
                self.node(Operator::Rational(value.clone()), &[])?,
            ));
        }
        admit(
            &mut self.cas,
            Draft::SeriesObject(SeriesObject {
                field: Reference::Existing(self.field.clone()),
                centre: Reference::Existing(self.centre.clone()),
                variable: Reference::Existing(self.x.clone()),
                scope: Vec::new(),
                context: Reference::Existing(self.context.clone()),
                order: u32::try_from(values.len())?,
                coefficients,
            }),
        )
    }

    /// Identity composition proves the full fixed candidate with no source claim.
    pub(crate) fn compose(&mut self) -> TestResult<VerifiedWitness> {
        let values = self.candidate.clone();
        let left = self.object(&values)?;
        let mut identity = vec![Rational::zero(); values.len()];
        if let Some(linear) = identity.get_mut(1) {
            *linear = Rational::one();
        }
        let right = self.object(&identity)?;
        Ok(self.cas.compute_series(
            &SeriesOperation::Compose {
                left,
                right,
                order: None,
            },
            &[],
            &mut default_budget()?,
        )?)
    }

    /// A consistent explicit definedness premise remains visible through the source closure.
    pub(crate) fn source(
        &mut self,
        source: &Term,
        clause: SourceClause,
    ) -> TestResult<VerifiedWitness> {
        let (mut expression, _) = self.term(source, self.candidate.len())?;
        match clause {
            SourceClause::SineOrigin => expression = self.node(Operator::Sine, &[expression])?,
            SourceClause::ArctangentOrigin => {
                expression = self.node(Operator::Arctangent, &[expression])?;
            }
            _ => {}
        }
        let predicate = admit(
            &mut self.cas,
            Draft::Condition {
                header: Reference::Existing(self.header.clone()),
                scope: Vec::new(),
                kind: PredicateKind::Defined,
                arguments: vec![Reference::Existing(expression.clone())],
            },
        )?;
        self.context = admit(
            &mut self.cas,
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(self.header.clone()),
                admissible: Reference::Existing(self.region.clone()),
                assumptions: vec![AssumptionDraft {
                    key: "source-defined".into(),
                    condition: Reference::Existing(predicate.clone()),
                }],
            }),
        )?;
        let proof = self.cas.check_proof(
            &self.context,
            &[ApplicationDraft {
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
            }],
            &mut default_budget()?,
        )?;
        Ok(self.cas.expand_series_source(
            &SourceRequest {
                clause,
                source: expression,
                domain: self.region.clone(),
                field: self.field.clone(),
                centre: self.centre.clone(),
                variable: self.x.clone(),
                scope: Vec::new(),
                context: self.context.clone(),
                order: u32::try_from(self.candidate.len())?,
            },
            &[proof.proof().clone()],
            &mut default_budget()?,
        )?)
    }

    /// Full polynomials are evaluated before truncation, independently of the finite interpreter.
    pub(crate) fn term(
        &mut self,
        term: &Term,
        order: usize,
    ) -> TestResult<(Handle, Vec<ResidualRow>)> {
        let mut rows = Vec::new();
        let (expression, _) = self.walk(term, order, &mut rows)?;
        Ok((expression, rows))
    }

    /// Emit the explicit schedule while retaining full polynomial intermediates in the oracle.
    fn walk(
        &mut self,
        term: &Term,
        order: usize,
        rows: &mut Vec<ResidualRow>,
    ) -> TestResult<(Handle, Vec<Rational>)> {
        let (expression, values) = match term {
            Term::Q(value) => {
                let value = rational(value)?;
                (
                    self.node(Operator::Rational(value.clone()), &[])?,
                    vec![value],
                )
            }
            Term::X => (
                self.x.clone(),
                vec![self.centre_value.clone(), Rational::one()],
            ),
            Term::Y(0) => (self.y.clone(), self.candidate.clone()),
            Term::Y(depth) => {
                let (child, values) = self.walk(&Term::Y(depth - 1), order, rows)?;
                let values = values
                    .iter()
                    .enumerate()
                    .skip(1)
                    .map(|(degree, value)| {
                        Ok(value.mul(
                            &Rational::from_ratio(
                                i64::try_from(degree)?,
                                1,
                                &mut default_budget()?,
                            )?,
                            &mut default_budget()?,
                        )?)
                    })
                    .collect::<TestResult<Vec<_>>>()?;
                (
                    self.node(Operator::Derivative("x".into()), &[child])?,
                    values,
                )
            }
            Term::Binary(op, left, right) => {
                let (left, a) = self.walk(left, order, rows)?;
                let (right, b) = self.walk(right, order, rows)?;
                let values = polynomial(op, &a, &b)?;
                (self.node(op.clone(), &[left, right])?, values)
            }
            Term::Power(child, exponent) => {
                let (child, base) = self.walk(child, order, rows)?;
                let mut value = vec![Rational::one()];
                for _ in 0..*exponent {
                    value = polynomial(&Operator::Mul, &value, &base)?;
                    row(rows, &value, order)?;
                }
                (
                    self.node(Operator::IntegerPower(i64::from(*exponent)), &[child])?,
                    value,
                )
            }
        };
        row(rows, &values, order)?;
        Ok((expression, values))
    }
}

/// Full polynomial multiplication uses all monomial pairs before any truncation.
fn polynomial(op: &Operator, a: &[Rational], b: &[Rational]) -> TestResult<Vec<Rational>> {
    if *op == Operator::Mul {
        let mut result = vec![Rational::zero(); a.len() + b.len()];
        for (i, left) in a.iter().enumerate() {
            for (j, right) in b.iter().enumerate() {
                result[i + j] = result[i + j].add(
                    &left.mul(right, &mut default_budget()?)?,
                    &mut default_budget()?,
                )?;
            }
        }
        Ok(result)
    } else {
        (0..a.len().max(b.len()))
            .map(|index| {
                let zero = Rational::zero();
                let left = a.get(index).unwrap_or(&zero);
                let right = b.get(index).unwrap_or(&zero);
                Ok(if *op == Operator::Add {
                    left.add(right, &mut default_budget()?)?
                } else {
                    left.sub(right, &mut default_budget()?)?
                })
            })
            .collect()
    }
}

/// Truncation is applied only when recording the independently computed full value.
fn row(rows: &mut Vec<ResidualRow>, values: &[Rational], order: usize) -> TestResult {
    rows.push(ResidualRow {
        ordinal: u32::try_from(rows.len())?,
        coefficients: (0..order)
            .map(|index| values.get(index).cloned().unwrap_or_else(Rational::zero))
            .collect(),
    });
    Ok(())
}

/// No caller-selected problem or substitution is admitted by the public adapter.
pub(crate) const fn member() -> ResidualSolve {
    ResidualSolve {
        statement: SolveStatement::Member(StatementMember {
            problem: R::Problem,
            tuple: R::Tuple,
            region: R::Region,
        }),
        certificate: SolveCertificate::Membership(CertificateMembership {
            problem: R::Problem,
            tuple: R::Tuple,
            region: R::Region,
            substitutions: Vec::new(),
            definedness: Vec::new(),
        }),
        premises: Vec::new(),
    }
}
