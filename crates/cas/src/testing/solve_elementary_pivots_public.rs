// SPDX-License-Identifier: AGPL-3.0-only
//! Independently multiplied exact algebraic inverse and embedded-unit fixtures.

use super::{BinomialFixture, Fixture, admit};
use crate::test_api::limits::{Budget, Limits};
use crate::test_api::proof::{ApplicationDraft, DomainContract, Statement};
use crate::test_api::solving::{
    CertificateExactLinearEquivalence, CoefficientValueData, CoefficientValueObject, RowOperation,
    SolveCertificate, SolveDraft, SolveSchema, SolveStatement, SolveView, StatementRank,
};
use crate::test_api::syntax::{Draft, Field, Operator};
use crate::test_api::{Handle, Rational, Reference};
use crate::testing::common::TestResult;

/// Small rational parent coefficients are independently supplied, not computed by the inverse engine.
fn rational(base: &mut Fixture, numerator: i64, denominator: u64) -> TestResult<Handle> {
    let value = Rational::from_ratio(
        numerator,
        denominator,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    admit(
        &mut base.cas,
        &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
            domain: Reference::Existing(base.algebra.clone()),
            value: CoefficientValueData::Rational { value },
        })),
    )
}

/// Rank one is proved by a supplied lower minor and an independently checked upper matrix.
fn rank(
    base: &mut Fixture,
    entry: &Handle,
    inverse: Option<&Handle>,
    one: &Handle,
) -> TestResult<ApplicationDraft> {
    let matrix = base.matrix(1, 1, std::slice::from_ref(entry))?;
    let minor = if inverse.is_some() { one } else { entry };
    let reduced = base.matrix(1, 1, std::slice::from_ref(minor))?;
    let operations = inverse.map_or_else(Vec::new, |value| {
        vec![RowOperation::Scale {
            row: 0,
            value: value.clone(),
            unit: 0,
        }]
    });
    Ok(base.step(
        SolveSchema::ExactLinearEquivalence,
        SolveStatement::Rank(StatementRank {
            matrix: matrix.clone(),
            domain: base.algebra.clone(),
            rank: 1,
            pivots: vec![0],
            minor: minor.clone(),
            elimination: reduced.clone(),
        }),
        SolveCertificate::ExactLinearEquivalence(CertificateExactLinearEquivalence {
            matrix,
            augmented_columns: 0,
            operations,
            pivots: vec![0],
            minor: minor.clone(),
            reduced,
            region: base.region.clone(),
        }),
    ))
}

/// In Q[w]/(w²+w+1), (w-1)(-(w+2)/3)=1; no irreducibility assertion is needed.
pub(crate) fn inverse_pivot(scale: bool) -> TestResult<BinomialFixture> {
    inverse_pivot_mutation(scale, u8::MAX)
}

/// Each mutation changes one mathematically relevant input to the public proof lifecycle.
pub(crate) fn inverse_pivot_mutation(scale: bool, mutation: u8) -> TestResult<BinomialFixture> {
    let mut base = Fixture::in_field(&Field::Complex)?;
    let unity = base.unity(3)?;
    let negative = base.coefficient(-1)?;
    let linear = base.coefficient(1)?;
    let pivot_value = base.tower_value(&unity.domain, &[negative, linear])?;
    let constant = rational(&mut base, if mutation == 0 { -1 } else { -2 }, 3)?;
    let linear = rational(&mut base, -1, 3)?;
    let inverse = base.tower_value(&unity.domain, &[constant, linear])?;
    let pivot = base.literal(&pivot_value, &unity.map)?;
    let inverse = base.literal(&inverse, &unity.map)?;
    let one = base.literal(&unity.one, &unity.map)?;
    let product = base.expression(Operator::Mul, &[pivot.clone(), inverse.clone()], false)?;
    let mut identity = base.identity(&product, &one, &unity.map);
    identity.conclusion = Statement::Equality {
        left: product,
        right: one.clone(),
        domain: base.region.clone(),
        domain_contract: DomainContract::DefinedAndEqual,
    };
    identity.premises = vec![0, 1, 2];
    let matrix_pivot = if mutation == 1 {
        let different = alternate_map(&mut base, &unity.map)?;
        base.literal(&pivot_value, &different)?
    } else {
        pivot
    };
    let mut steps = unity.steps;
    steps.push(identity);
    base.algebra = unity.domain;
    let mut step = rank(&mut base, &matrix_pivot, scale.then_some(&inverse), &one)?;
    step.premises = vec![3, 1, 2];
    steps.push(step);
    let mut fixture = BinomialFixture { base, steps };
    mutate_scope(&mut fixture, mutation, &one)?;
    Ok(fixture)
}

/// The canonical constant vector one is nonzero without an inverse-identity premise.
pub(crate) fn embedded_tower_one() -> TestResult<BinomialFixture> {
    let mut base = Fixture::in_field(&Field::Complex)?;
    let unity = base.unity(3)?;
    let one = base.literal(&unity.one, &unity.map)?;
    base.algebra = unity.domain;
    let mut step = rank(&mut base, &one, None, &one)?;
    step.premises = vec![1, 2];
    let mut steps = unity.steps;
    steps.push(step);
    Ok(BinomialFixture { base, steps })
}

/// A parameter literal is defined on both split sides; its numerator is not a formal unit.
pub(crate) fn embedded_fraction(nonzero: bool) -> TestResult<BinomialFixture> {
    let mut base = Fixture::new()?;
    if nonzero {
        let region = base.coefficient_guard(true)?;
        base.enter_region(region)?;
    }
    let parameter = base.free("a")?;
    let (polynomial, map) = base.parameter_binomial(&parameter)?;
    let SolveView::Polynomial(polynomial) = base.cas.solve_view(&polynomial)? else {
        return Err("missing parameter polynomial".into());
    };
    let domain = polynomial.domain.clone();
    let coefficient = polynomial
        .terms
        .first()
        .ok_or("missing coefficient")?
        .coefficient
        .clone();
    let literal = base.literal(&coefficient, &map)?;
    base.algebra = domain;
    let partition = base.split(&literal)?;
    let unknown = base.free("x")?;
    let predicate = base.equation(&unknown, &literal)?;
    let problem = base.problem("x", &predicate, &[])?;
    let mut member = base.member(&problem, &literal)?;
    member.premises = vec![0];
    let one = base.integer(1, false)?;
    let mut step = rank(&mut base, &literal, None, &one)?;
    step.premises = vec![0, 1];
    Ok(BinomialFixture {
        base,
        steps: vec![partition, member, step],
    })
}

/// Two valid maps differ only in an explicit, harmless localization obligation.
fn alternate_map(base: &mut Fixture, original: &Handle) -> TestResult<Handle> {
    use crate::test_api::solving::{CoefficientMapData, MapTarget};
    let one = base.coefficient(1)?;
    let algebra = base.algebra.clone();
    let denominator = base.polynomial(&algebra, &[(0, one)])?;
    let SolveView::CoefficientMap(map) = base.cas.solve_view(original)? else {
        return Err("missing coefficient map".into());
    };
    let map = map.clone();
    let MapTarget::ScalarAmbient { header, field } = map.target else {
        return Err("missing scalar target".into());
    };
    admit(
        &mut base.cas,
        &Draft::Solve(SolveDraft::CoefficientMap(CoefficientMapData {
            source: Reference::Existing(map.source),
            target: MapTarget::ScalarAmbient {
                header: Reference::Existing(header),
                field,
            },
            mode: map.mode,
            region: Reference::Existing(map.region),
            images: map.images.into_iter().map(Reference::Existing).collect(),
            denominators: vec![Reference::Existing(denominator)],
        })),
    )
}

/// A changed context or region cannot borrow the original identity's authority.
fn mutate_scope(fixture: &mut BinomialFixture, mutation: u8, one: &Handle) -> TestResult {
    use crate::test_api::domain::{AssumptionDraft, ContextDraft};
    use crate::test_api::proof::Certificate;
    use crate::test_api::syntax::PredicateKind;
    if mutation == 2 {
        let predicate = admit(
            &mut fixture.base.cas,
            &Draft::Condition {
                header: Reference::Existing(fixture.base.header.clone()),
                scope: Vec::new(),
                kind: PredicateKind::Defined,
                arguments: vec![Reference::Existing(one.clone())],
            },
        )?;
        fixture.base.context = admit(
            &mut fixture.base.cas,
            &Draft::Context(ContextDraft::Root {
                header: Reference::Existing(fixture.base.header.clone()),
                admissible: Reference::Existing(fixture.base.region.clone()),
                assumptions: vec![AssumptionDraft {
                    key: "different-context".into(),
                    condition: Reference::Existing(predicate),
                }],
            }),
        )?;
        fixture
            .steps
            .last_mut()
            .ok_or("missing rank")?
            .context
            .clone_from(&fixture.base.context);
    } else if mutation == 3 {
        let region = fixture.base.coefficient_guard(true)?;
        let Certificate::Solve(certificate) =
            &mut fixture.steps.last_mut().ok_or("missing rank")?.certificate
        else {
            return Err("wrong certificate".into());
        };
        let SolveCertificate::ExactLinearEquivalence(value) = certificate.as_mut() else {
            return Err("wrong rank".into());
        };
        value.region = region;
    }
    Ok(())
}

/// A formal polynomial equality cannot name a binary scalar inverse product.
fn polynomial_identity(
    base: &mut Fixture,
    scalar: &ApplicationDraft,
) -> TestResult<ApplicationDraft> {
    let crate::test_api::proof::Certificate::Solve(certificate) = &scalar.certificate else {
        return Err("missing scalar certificate".into());
    };
    let SolveCertificate::CoefficientIdentity(identity) = certificate.as_ref() else {
        return Err("missing scalar identity".into());
    };
    let SolveView::CoefficientDomain(crate::test_api::solving::CoefficientDomainData::Quotient {
        level,
    }) = base.cas.solve_view(&base.algebra)?
    else {
        return Err("missing quotient".into());
    };
    let SolveView::ExtensionLevel(extension) = base.cas.solve_view(level)? else {
        return Err("missing extension".into());
    };
    let parent = extension.parent.clone();
    let unit = admit(
        &mut base.cas,
        &Draft::Solve(SolveDraft::CoefficientValue(CoefficientValueObject {
            domain: Reference::Existing(parent),
            value: CoefficientValueData::Rational {
                value: Rational::one(),
            },
        })),
    )?;
    let domain = base.algebra.clone();
    let one = base.tower_value(&domain, &[unit])?;
    let polynomial = base.polynomial(&domain, &[(1, one)])?;
    let mut step = base.identity(&polynomial, &polynomial, &identity.map);
    step.premises = vec![0, 1, 2];
    Ok(step)
}

/// Compare one rank claim with and without an earlier unrelated polynomial identity.
pub(crate) fn inverse_pivot_scan(
    with_scalar: bool,
) -> TestResult<(BinomialFixture, Vec<ApplicationDraft>)> {
    let mut fixture = inverse_pivot(false)?;
    let mut rank = fixture.steps.pop().ok_or("missing rank")?;
    let mut scalar = fixture.steps.pop().ok_or("missing scalar inverse")?;
    let mut baseline = fixture.steps.clone();
    if with_scalar {
        baseline.push(scalar.clone());
    }
    rank.premises = if with_scalar {
        vec![3, 1, 2]
    } else {
        vec![1, 2]
    };
    baseline.push(rank.clone());
    let polynomial = polynomial_identity(&mut fixture.base, &scalar)?;
    fixture.steps.push(polynomial);
    if with_scalar {
        scalar.premises.push(3);
        fixture.steps.push(scalar);
    }
    rank.premises = if with_scalar {
        vec![3, 4, 1, 2]
    } else {
        vec![3, 1, 2]
    };
    fixture.steps.push(rank);
    Ok((fixture, baseline))
}
