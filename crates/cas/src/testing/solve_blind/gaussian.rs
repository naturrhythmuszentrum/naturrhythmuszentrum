// SPDX-License-Identifier: AGPL-3.0-only
//! Original Gaussian coefficients supplied without candidate roots or expected factorization.

use super::{Fixture, TestResult, input::admit};
use crate::test_api::limits::{Budget, Limits};
use crate::test_api::solving::{
    CoefficientDomainData, CoefficientMapData, CoefficientValueData, CoefficientValueObject,
    MapMode, MapTarget, SolveDraft,
};
use crate::test_api::syntax::{Draft, Field, Operator};
use crate::test_api::{Handle, Rational, Reference};

/// The canonical Gaussian embedding fixes i before any root choice is made.
fn mapping(fixture: &mut Fixture) -> TestResult<Handle> {
    fixture.algebra = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::CoefficientDomain(
            CoefficientDomainData::Gaussian,
        )),
    )?;
    admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
            source: Reference::Existing(fixture.algebra.clone()),
            target: MapTarget::ScalarAmbient {
                header: Reference::Existing(fixture.header.clone()),
                field: Field::Complex,
            },
            mode: MapMode::Embedding,
            region: Reference::Existing(fixture.region.clone()),
            images: Vec::new(),
            denominators: Vec::new(),
        })),
    )
}

/// Exact nonreal source syntax has an explicit ground map, with no scalar projection.
fn literal(fixture: &mut Fixture, map: &Handle, parts: &(i64, i64)) -> TestResult<Handle> {
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let value = admit(
        &mut fixture.cas,
        &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
            domain: Reference::Existing(fixture.algebra.clone()),
            value: CoefficientValueData::Gaussian {
                real: Rational::from_ratio(parts.0, 1, &mut budget)?,
                imaginary: Rational::from_ratio(parts.1, 1, &mut budget)?,
            },
        })),
    )?;
    admit(
        &mut fixture.cas,
        &Draft::ExactCoefficient {
            header: Reference::Existing(fixture.header.clone()),
            value: Reference::Existing(value),
            map: Reference::Existing(map.clone()),
        },
    )
}

/// Horner syntax uses only the requested coefficient list; expected roots are supplied after search.
pub(crate) fn polynomial(
    coefficients: &[(i64, i64)],
) -> TestResult<(Fixture, crate::test_api::request::Request)> {
    let limits = Limits::blind_cubic();
    let mut fixture =
        Fixture::with_parameters(&Field::Complex, &["a", "b", "c", "d"], limits.clone())?;
    let map = mapping(&mut fixture)?;
    let unknown = fixture.free("x")?;
    let mut expression = fixture.integer(0, false)?;
    for parts in coefficients.iter().rev() {
        let product = fixture.expression(Operator::Mul, &[expression, unknown.clone()], false)?;
        let constant = literal(&mut fixture, &map, parts)?;
        expression = fixture.expression(Operator::Add, &[product, constant], false)?;
    }
    super::request(fixture, expression, format!("{coefficients:?}"), &limits)
}
