// SPDX-License-Identifier: AGPL-3.0-only
//! Public syntax for independent source and target axes and hand-derived pullback expectations.

use super::budgets::derivation_budget as budget;
use super::builders::{self, admit};
use super::cases;
use super::common::{TestResult, real};
use crate::test_api::domain::{
    AmbientDomain, ContextDraft, HeaderDraft, Layer, RegionDraft, RegionExpr, SpaceDraft,
};
use crate::test_api::proof::{
    DistributionStatement, Judgment, PullbackMode, PullbackValue, Statement,
};
use crate::test_api::request::{PullbackKind, Task};
use crate::test_api::syntax::{Draft, ExpressionDraft, Field, Operator, PredicateKind, Sort};
use crate::test_api::{Cas, Handle, Rational, Reference};

/// Two exact real axes with a target-axis context and a shared explicit declaration signature.
pub(crate) struct Fixture {
    /// Ordinary scalar constructors and covered requests.
    pub(crate) base: cases::Fixture,
    /// Original distribution on the y axis.
    pub(crate) source_space: Handle,
    /// Pullback distribution on the x axis.
    pub(crate) target_space: Handle,
    /// Inverse-map coordinate.
    pub(crate) y: Handle,
}

impl Fixture {
    /// Construct spaces without using any calculus producer.
    pub(crate) fn new() -> TestResult<Self> {
        let mut cas = Cas::new(budget()?.limits().clone())?;
        let header = admit(
            &mut cas,
            Draft::Header(HeaderDraft::Core(builders::environment(
                Field::Real,
                &["x", "y"],
            ))),
            &mut budget()?,
        )?;
        let (domain, target_space) = axis(&mut cas, &header, "x")?;
        let (_, source_space) = axis(&mut cas, &header, "y")?;
        let context = admit(
            &mut cas,
            Draft::Context(ContextDraft::Root {
                header: Reference::Existing(header.clone()),
                admissible: Reference::Existing(domain.clone()),
                assumptions: Vec::new(),
            }),
            &mut budget()?,
        )?;
        let source = admit(
            &mut cas,
            Draft::Source {
                key: "pullback-source".into(),
                text: "explicit delta and polynomial map".into(),
            },
            &mut budget()?,
        )?;
        let x = builders::expression(
            &mut cas,
            &context,
            Operator::Free("x".into()),
            real(),
            &[],
            &mut budget()?,
        )?;
        let y = builders::expression(
            &mut cas,
            &context,
            Operator::Free("y".into()),
            real(),
            &[],
            &mut budget()?,
        )?;
        let zero = builders::number(&mut cas, &context, real(), "0", budget)?;
        let one = builders::number(&mut cas, &context, real(), "1", budget)?;
        let base = cases::Fixture {
            cas,
            header,
            context,
            domain,
            source,
            x,
            zero,
            one,
        };
        Ok(Self {
            base,
            source_space,
            target_space,
            y,
        })
    }

    /// Explicit distribution syntax in the caller-selected exact space.
    pub(crate) fn node(
        &mut self,
        space: &Handle,
        operator: Operator,
        operands: &[Handle],
    ) -> TestResult<Handle> {
        admit(
            &mut self.base.cas,
            Draft::Expression(ExpressionDraft {
                header: Reference::Existing(self.base.header.clone()),
                space: Some(Reference::Existing(space.clone())),
                scope: Vec::new(),
                sort: Sort::Distribution,
                operator,
                operands: operands.iter().cloned().map(Reference::Existing).collect(),
                binding: None,
            }),
            &mut budget()?,
        )
    }

    /// Replace one declared whole-axis space with an exact finite open interval.
    pub(crate) fn interval(&mut self, source: bool, lower: &str, upper: &str) -> TestResult {
        let coordinate = if source { "y" } else { "x" };
        let variable = if source {
            self.y.clone()
        } else {
            self.base.x.clone()
        };
        let lower = self.base.number(lower)?;
        let upper = self.base.number(upper)?;
        let lower = self.base.term(Operator::Sub, &[variable.clone(), lower])?;
        let upper = self.base.term(Operator::Sub, &[upper, variable])?;
        let mut regions = Vec::new();
        let (domain, _) = axis(&mut self.base.cas, &self.base.header, coordinate)?;
        for difference in [lower, upper] {
            let predicate = self
                .base
                .predicate(PredicateKind::Positive, &[difference])?;
            let guard = RegionExpr::GuardTrue {
                base: Reference::Existing(domain.clone()),
                predicate: Reference::Existing(predicate),
            };
            regions.push(Reference::Existing(self.region(coordinate, guard)?));
        }
        let domain = self.region(coordinate, RegionExpr::Meet(regions))?;
        let space = admit(
            &mut self.base.cas,
            Draft::Space(SpaceDraft {
                header: Reference::Existing(self.base.header.clone()),
                domain: Reference::Existing(domain),
                coordinate: coordinate.into(),
                field: Field::Real,
                test_family: "CompactSmooth".into(),
            }),
            &mut budget()?,
        )?;
        if source {
            self.source_space = space;
        } else {
            self.target_space = space;
        }
        Ok(())
    }

    /// The ambient coordinate stays explicit even when both axes share a header.
    fn region(&mut self, coordinate: &str, expression: RegionExpr) -> TestResult<Handle> {
        admit(
            &mut self.base.cas,
            Draft::Region(RegionDraft {
                header: Reference::Existing(self.base.header.clone()),
                scope: Vec::new(),
                ambient: AmbientDomain::Real(coordinate.into()),
                expression,
            }),
            &mut budget()?,
        )
    }

    /// Original delta at zero in the source coordinate.
    pub(crate) fn source(&mut self) -> TestResult<Handle> {
        let zero = self.base.zero.clone();
        self.node(
            &self.source_space.clone(),
            Operator::Delta { order: 0 },
            std::slice::from_ref(&zero),
        )
    }

    /// The forward map a*x+b and independently written inverse (y-b)/a.
    pub(crate) fn affine(&mut self, scale: &str, offset: &str) -> TestResult<(Handle, Handle)> {
        let scale = self.base.number(scale)?;
        let offset = self.base.number(offset)?;
        let linear = self
            .base
            .term(Operator::Mul, &[scale.clone(), self.base.x.clone()])?;
        let map = self.base.term(Operator::Add, &[linear, offset.clone()])?;
        let numerator = self.base.term(Operator::Sub, &[self.y.clone(), offset])?;
        let inverse = self.base.term(Operator::Div, &[numerator, scale])?;
        Ok((map, inverse))
    }

    /// A finite factorization with independently supplied simple or repeated roots.
    pub(crate) fn quadratic(&mut self, repeated: bool) -> TestResult<Handle> {
        let left = self
            .base
            .term(Operator::Sub, &[self.base.x.clone(), self.base.one.clone()])?;
        let right = if repeated {
            left.clone()
        } else {
            self.base
                .term(Operator::Add, &[self.base.x.clone(), self.base.one.clone()])?
        };
        self.base.term(Operator::Mul, &[left, right])
    }

    /// Expected sums use caller-supplied locations and weights, with no pullback computation.
    pub(crate) fn weighted(&mut self, pairs: &[(&str, &str)]) -> TestResult<Handle> {
        let space = self.target_space.clone();
        let mut result = None;
        for (point, coefficient) in pairs {
            let point = self.base.number(point)?;
            let delta = self.node(&space, Operator::Delta { order: 0 }, &[point])?;
            let coefficient = self.base.number(coefficient)?;
            let term = self.node(&space, Operator::DistributionScale, &[coefficient, delta])?;
            result = Some(match result {
                None => term,
                Some(previous) => {
                    self.node(&space, Operator::DistributionAdd, &[previous, term])?
                }
            });
        }
        result.ok_or_else(|| "nonempty expected delta sum".into())
    }

    /// Typed pullback intent keeps both spaces regardless of coordinate spelling.
    pub(crate) fn task(&self, map_kind: PullbackKind) -> Task {
        Task::Pullback {
            source_space: self.source_space.clone(),
            target_space: self.target_space.clone(),
            map_kind,
        }
    }

    /// Assemble a complete independent expected theorem from explicit syntax.
    pub(crate) fn expected(
        &self,
        source: Handle,
        map: Handle,
        inverse: Option<Handle>,
        result: Handle,
    ) -> Judgment {
        let mode = if inverse.is_some() {
            PullbackMode::Affine
        } else {
            PullbackMode::SimpleZeros
        };
        Judgment {
            context: self.base.context.clone(),
            layer: Layer::Distribution,
            space: Some(self.target_space.clone()),
            residuals: Vec::new(),
            statement: Statement::Distribution(Box::new(DistributionStatement::Pullback(
                Box::new(PullbackValue {
                    source,
                    result,
                    map,
                    inverse,
                    source_space: self.source_space.clone(),
                    target_space: self.target_space.clone(),
                    mode,
                }),
            ))),
        }
    }
}

/// One whole-axis region and its exact compact-smooth test space.
fn axis(cas: &mut Cas, header: &Handle, coordinate: &str) -> TestResult<(Handle, Handle)> {
    let domain = admit(
        cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            ambient: AmbientDomain::Real(coordinate.into()),
            expression: RegionExpr::Ambient,
        }),
        &mut budget()?,
    )?;
    let space = admit(
        cas,
        Draft::Space(SpaceDraft {
            header: Reference::Existing(header.clone()),
            domain: Reference::Existing(domain.clone()),
            coordinate: coordinate.into(),
            field: Field::Real,
            test_family: "CompactSmooth".into(),
        }),
        &mut budget()?,
    )?;
    Ok((domain, space))
}

/// Rational roots are test data, never extracted from a produced certificate.
pub(crate) fn roots(values: &[&str]) -> TestResult<PullbackKind> {
    let mut ledger = budget()?;
    Ok(PullbackKind::SimpleZeros(
        values
            .iter()
            .map(|value| Rational::parse(value, &mut ledger))
            .collect::<Result<_, _>>()?,
    ))
}
