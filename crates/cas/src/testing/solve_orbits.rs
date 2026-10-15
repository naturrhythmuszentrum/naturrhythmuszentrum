// SPDX-License-Identifier: AGPL-3.0-only
//! Generic arithmetic and scoped coefficient-map setup for crate-only resolvent producers.

use crate::domain::{AmbientDomain, ContextDraft, HeaderDraft, RegionDraft, RegionExpr};
use crate::limits::{Budget, Limits};
use crate::proof::{ApplicationDraft, Certificate, Primitive, Statement};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::model::Element;
use crate::solving::symmetric::algebra::Polynomial;
use crate::solving::symmetric::proof::{CertifiedReduction, Evaluation, certify};
use crate::solving::symmetric::reduction::Reduction;
use crate::solving::towers::{arithmetic, cyclotomic, dense::Dense, materialize};
use crate::solving::{
    AdjunctionMode, CertificateRootAdjunction, CoefficientMapData, MapMode, MapTarget,
    SolveCertificate, SolveDraft, SolveSchema, SolveStatement, StatementRootExistence,
};
use crate::syntax::{AtomicSort, Branch, Declaration, Draft, Environment, Field, Operator, Sort};
use crate::testing::common::TestResult;
use crate::testing::solve_coefficients::{Fixture as Coefficients, unit_step};
use crate::testing::solve_graph::{admit, expression};
use crate::{Handle, Reference};

/// A generic formal root frame and an explicitly selected primitive unity, with no formula oracle.
#[derive(Debug)]
pub(crate) struct Fixture {
    /// Ordinary graph and complete coefficient context.
    pub(crate) base: Coefficients,
    /// Empty polynomial carrying the ordered formal root slots and exact algebra.
    pub(crate) frame: Polynomial,
    /// Canonical generator in the computed cyclotomic quotient.
    pub(crate) unity: Element,
    /// Explicit scalar evaluation of that generator at its selected complex root.
    map: Handle,
    /// Declared independent root images used only to state polynomial expansion identities.
    roots: Vec<Handle>,
    /// Independent parameter images used only for coefficient substitution identities.
    parameters: Vec<Handle>,
    /// Supplied leading-unit and root-existence evidence, rechecked for every derived proof.
    prefix: Vec<ApplicationDraft>,
}

/// Create an explicit complex context with separate root and parameter declarations.
fn context(base: &mut Coefficients, degree: u32) -> TestResult {
    let declarations = ["root", "parameter"]
        .into_iter()
        .flat_map(|prefix| {
            (0..degree).map(move |slot| {
                let key = format!("{prefix}{slot}");
                Declaration {
                    display_name: key.clone(),
                    key,
                    sort: Sort::Value(AtomicSort::Scalar(Field::Complex)),
                }
            })
        })
        .collect();
    base.header = admit(
        &mut base.cas,
        &Draft::Header(HeaderDraft::Core(Environment {
            field: Field::Complex,
            branch: Branch::PrincipalComplex,
            declarations,
        })),
    )?;
    base.region = admit(
        &mut base.cas,
        &Draft::Region(RegionDraft {
            header: Reference::Existing(base.header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Assignments,
            expression: RegionExpr::Ambient,
        }),
    )?;
    base.context = admit(
        &mut base.cas,
        &Draft::Context(ContextDraft::Root {
            header: Reference::Existing(base.header.clone()),
            admissible: Reference::Existing(base.region.clone()),
            assumptions: Vec::new(),
        }),
    )?;
    Ok(())
}

/// Exact independent variable expressions, never extra adjoined tower levels.
fn variables(base: &mut Coefficients, prefix: &str, degree: u32) -> TestResult<Vec<Handle>> {
    (0..degree)
        .map(|slot| {
            admit(
                &mut base.cas,
                &expression(
                    Reference::Existing(base.header.clone()),
                    Operator::Free(format!("{prefix}{slot}")),
                    Sort::Value(AtomicSort::Scalar(Field::Complex)),
                    Vec::new(),
                ),
            )
        })
        .collect()
}

/// Supply the independent ground identity 1*1 + X*0 = 1 used by root existence.
fn unit(base: &mut Coefficients) -> TestResult<ApplicationDraft> {
    let rational = base.rational.clone();
    let one = base.integer(1)?;
    let modulus = base.univariate(&rational, &[(1, one.clone())])?;
    let constant = base.univariate(&rational, &[(0, one.clone())])?;
    let zero = base.univariate(&rational, &[])?;
    Ok(unit_step(
        base,
        &one,
        &one,
        &rational,
        [&modulus, &constant, &constant, &constant, &zero, &modulus],
    ))
}

/// Existence names the actual computed cyclotomic relation and its single joint selection.
fn adjunction(
    base: &Coefficients,
    relation: &Handle,
    selection: &Handle,
    degree: u32,
) -> ApplicationDraft {
    ApplicationDraft {
        rule: Primitive::Solve(SolveSchema::RootAdjunction),
        context: base.context.clone(),
        conclusion: Statement::Solve(Box::new(SolveStatement::RootExistence(
            StatementRootExistence {
                relation: selection.clone(),
                region: base.region.clone(),
            },
        ))),
        premises: vec![0],
        substitutions: Vec::new(),
        certificate: Certificate::Solve(Box::new(SolveCertificate::RootAdjunction(
            CertificateRootAdjunction {
                mode: AdjunctionMode::PrimitiveUnity,
                relation: relation.clone(),
                selection: Some(selection.clone()),
                leading_unit: 0,
                order: Some(degree),
                splitting_chain: Vec::new(),
            },
        ))),
    }
}

/// Canonical selected-root evaluation is explicit, including its original relation's parent map.
fn scalar_map(base: &mut Coefficients, domain: &Handle, selection: &Handle) -> TestResult<Handle> {
    let root = admit(
        &mut base.cas,
        &Draft::RootValue {
            header: Reference::Existing(base.header.clone()),
            selection: Reference::Existing(selection.clone()),
            slot: 0,
        },
    )?;
    admit(
        &mut base.cas,
        &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
            source: Reference::Existing(domain.clone()),
            target: MapTarget::ScalarAmbient {
                header: Reference::Existing(base.header.clone()),
                field: Field::Complex,
            },
            mode: MapMode::Specialization,
            region: Reference::Existing(base.region.clone()),
            images: vec![Reference::Existing(root)],
            denominators: Vec::new(),
        })),
    )
}

impl Fixture {
    /// Build any supported degree from the generic cyclotomic product, without degree-specific constants.
    pub(crate) fn new(degree: u32) -> TestResult<Self> {
        let mut base = Coefficients::new()?;
        context(&mut base, degree)?;
        let mut budget = Budget::new(Limits::blind_cubic())?;
        let parent = Engine {
            state: &base.cas.state,
            domain: &base.rational,
        };
        let cyclotomic = cyclotomic::polynomial(&parent, degree, &mut budget)?;
        let variable = Dense::new(
            &parent,
            vec![
                parent.constant(false, &mut budget)?,
                parent.constant(true, &mut budget)?,
            ],
            &mut budget,
        )?;
        let modulus =
            materialize::polynomial(&mut base.cas, &cyclotomic, &[Field::Real], &mut budget)?;
        let rational = base.rational.clone();
        let (_, domain) = base.tower(&rational, &modulus)?;
        let engine = Engine {
            state: &base.cas.state,
            domain: &domain,
        };
        let unity = arithmetic::reduced(&engine, variable, &mut budget)?;
        let fields = vec![Field::Complex; usize::try_from(degree)?];
        let frame = Polynomial::constant(
            &domain,
            &fields,
            engine.constant(false, &mut budget)?,
            &base.cas.state,
            &mut budget,
        )?;
        let roots = variables(&mut base, "root", degree)?;
        let parameters = variables(&mut base, "parameter", degree)?;
        let (relation, selection) = base.root(&modulus, &Field::Complex)?;
        let map = scalar_map(&mut base, &domain, &selection)?;
        let prefix = vec![
            unit(&mut base)?,
            adjunction(&base, &relation, &selection, degree),
        ];
        Ok(Self {
            base,
            frame,
            unity,
            map,
            roots,
            parameters,
            prefix,
        })
    }

    /// All emitted symmetry and substitution applications are independently checked before publication.
    pub(crate) fn certify(
        &mut self,
        reduction: &Reduction,
        images: &[Polynomial],
    ) -> TestResult<CertifiedReduction> {
        self.certify_with(
            reduction,
            images,
            &mut Budget::new(Limits::blind_cubic())?,
            &mut Budget::new(Limits::blind_cubic())?,
        )
    }

    /// Separate caller-owned generation and replay ledgers expose exact boundary behavior.
    pub(crate) fn certify_with(
        &mut self,
        reduction: &Reduction,
        images: &[Polynomial],
        generation: &mut Budget,
        checker: &mut Budget,
    ) -> TestResult<CertifiedReduction> {
        let evaluation = Evaluation {
            context: &self.base.context,
            map: &self.map,
            roots: &self.roots,
            parameters: &self.parameters,
            prefix: self.prefix.clone(),
        };
        Ok(certify(
            &mut self.base.cas,
            reduction,
            images,
            evaluation,
            generation,
            checker,
        )?)
    }

    /// A deliberate missing map hypothesis is used only by the outer atomicity test.
    pub(crate) fn without_existence(&mut self) {
        self.prefix.clear();
    }
}
