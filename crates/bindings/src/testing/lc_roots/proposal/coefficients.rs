// SPDX-License-Identifier: AGPL-3.0-only
//! Candidate coefficient syntax retains its complete original algebra and interpretation.

use super::{Importer, TestResult};
use nrz_bindings::readings::algebra::{
    Handle,
    solving::{
        CoefficientDomainData, CoefficientMapData, CoefficientValueData, MapTarget, PolynomialData,
        PolynomialTerm,
    },
};

impl Importer<'_> {
    pub(super) fn domain(
        &mut self,
        data: &CoefficientDomainData<Handle>,
    ) -> TestResult<CoefficientDomainData> {
        Ok(match data {
            CoefficientDomainData::Rational => CoefficientDomainData::Rational,
            CoefficientDomainData::Gaussian => CoefficientDomainData::Gaussian,
            CoefficientDomainData::Fraction { base, coordinates } => {
                CoefficientDomainData::Fraction {
                    base: self.reference(base)?,
                    coordinates: self.reference(coordinates)?,
                }
            }
            CoefficientDomainData::Quotient { level } => CoefficientDomainData::Quotient {
                level: self.reference(level)?,
            },
        })
    }

    pub(super) fn value(
        &mut self,
        data: &CoefficientValueData<Handle>,
    ) -> TestResult<CoefficientValueData> {
        Ok(match data {
            CoefficientValueData::Rational { value } => CoefficientValueData::Rational {
                value: value.clone(),
            },
            CoefficientValueData::Gaussian { real, imaginary } => CoefficientValueData::Gaussian {
                real: real.clone(),
                imaginary: imaginary.clone(),
            },
            CoefficientValueData::Fraction {
                numerator,
                denominator,
            } => CoefficientValueData::Fraction {
                numerator: self.reference(numerator)?,
                denominator: self.reference(denominator)?,
            },
            CoefficientValueData::Tower { coefficients } => CoefficientValueData::Tower {
                coefficients: self.references(coefficients)?,
            },
        })
    }

    pub(super) fn polynomial(
        &mut self,
        data: &PolynomialData<Handle>,
    ) -> TestResult<PolynomialData> {
        let terms = data
            .terms
            .iter()
            .map(|term| {
                Ok(PolynomialTerm {
                    exponents: term.exponents.clone(),
                    coefficient: self.reference(&term.coefficient)?,
                })
            })
            .collect::<TestResult<Vec<_>>>()?;
        Ok(PolynomialData {
            domain: self.reference(&data.domain)?,
            variables: data.variables.clone(),
            terms,
        })
    }

    pub(super) fn mapping(
        &mut self,
        data: &CoefficientMapData<Handle>,
    ) -> TestResult<CoefficientMapData> {
        let target = match &data.target {
            MapTarget::Family { family } => MapTarget::Family {
                family: self.reference(family)?,
            },
            MapTarget::CoefficientDomain { domain } => MapTarget::CoefficientDomain {
                domain: self.reference(domain)?,
            },
            MapTarget::ScalarAmbient { header, field } => MapTarget::ScalarAmbient {
                header: self.reference(header)?,
                field: field.clone(),
            },
        };
        Ok(CoefficientMapData {
            source: self.reference(&data.source)?,
            target,
            mode: data.mode,
            region: self.reference(&data.region)?,
            images: self.references(&data.images)?,
            denominators: self.references(&data.denominators)?,
        })
    }
}
