// SPDX-License-Identifier: AGPL-3.0-only
//! Public fraction-admission fixtures contain only the scope and domains they use.

use super::common::TestResult;
use super::solve_graph::admit;
use crate::test_api::domain::{AmbientDomain, HeaderDraft, RegionDraft, RegionExpr};
use crate::test_api::limits::{Budget, Limits};
use crate::test_api::solving::{
    CoefficientDomainData, CoefficientValueData, CoefficientValueObject, Coordinate,
    CoordinateRole, CoordinateSpaceData, PolynomialData, PolynomialTerm, SolveDraft,
};
use crate::test_api::syntax::{AtomicSort, Branch, Declaration, Draft, Environment, Field, Sort};
use crate::test_api::{Cas, Handle, Rational, Reference};

/// A two-parameter coefficient context and its exact ground/fraction domains.
#[derive(Debug)]
pub(crate) struct Fixture {
    /// Owning mathematical graph.
    pub(crate) cas: Cas,
    /// Full declaration header.
    pub(crate) header: Handle,
    /// Whole coefficient region.
    pub(crate) region: Handle,
    /// Rational ground domain.
    pub(crate) rational: Handle,
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
        let rational = admit(
            &mut cas,
            &Draft::Solve(SolveDraft::CoefficientDomain(
                CoefficientDomainData::Rational,
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
            rational,
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
