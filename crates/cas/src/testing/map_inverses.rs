// SPDX-License-Identifier: AGPL-3.0-only
//! Public exact-map fixtures and independent inverse coefficient oracles, compiled in public tests.

use super::builders;
use super::common::{TestResult, real};
use super::readings::Fixture;
use crate::test_api::domain::{AssumptionDraft, ContextDraft};
use crate::test_api::limits::{Budget, Limits};
use crate::test_api::readings::ExactAffineMap;
use crate::test_api::syntax::{Draft, Operator, PredicateKind};
use crate::test_api::{Cas, ExactConstant, Fault, Handle, Rational, Reference};

/// Explicit shared construction/proof profile; no operation resets the caller's ledger.
pub(crate) fn limits() -> Limits {
    Limits {
        work: 100_000_000,
        replay_work: 100_000_000,
        ..Limits::default()
    }
}

/// A fresh setup or independently measured operation ledger.
pub(crate) fn ledger() -> Result<Budget, Fault> {
    Budget::new(limits())
}

/// Declare exact scalar parameters without checking a theorem during setup.
pub(crate) fn fixture(keys: &[&str]) -> TestResult<Fixture> {
    let mut cas = Cas::new(limits())?;
    let environment = builders::environment(crate::test_api::syntax::Field::Real, keys);
    let context = builders::core_context(&mut cas, environment, Vec::new(), &mut ledger()?)?;
    let mut symbols = Vec::new();
    for key in keys {
        let value = builders::expression(
            &mut cas,
            &context,
            Operator::Free((*key).into()),
            real(),
            &[],
            &mut ledger()?,
        )?;
        symbols.push(((*key).into(), value));
    }
    Ok(Fixture {
        cas,
        context,
        symbols,
    })
}

/// Parse exact independent rational coefficient spellings.
pub(crate) fn constants(values: &[&str]) -> TestResult<Vec<ExactConstant>> {
    values
        .iter()
        .map(|value| Ok(Rational::parse(value, &mut ledger()?)?.into()))
        .collect()
}

/// Define a square map from an independent literal coefficient table.
pub(crate) fn constant_map(
    case: &mut Fixture,
    count: u32,
    values: &[&str],
    offsets: &[&str],
) -> TestResult<ExactAffineMap> {
    Ok(case.cas.constant_reading_map(
        &case.context,
        (count, count),
        &constants(values)?,
        &constants(offsets)?,
        &mut ledger()?,
    )?)
}

/// Original and inverse coefficients are supplied independently of the inversion producer.
pub(crate) fn rational_case(kind: u32) -> TestResult<(Fixture, ExactAffineMap, ExactAffineMap)> {
    let mut case = fixture(&[])?;
    let (count, values, offsets, inverse, origin): (u32, &[&str], &[&str], &[&str], &[&str]) =
        match kind {
            0 => (
                2,
                &["5", "0", "0", "1/10"],
                &["0", "0"],
                &["1/5", "0", "0", "10"],
                &["0", "0"],
            ),
            1 => (
                3,
                &["1", "2", "3", "0", "1", "4", "0", "0", "1"],
                &["2", "3", "5"],
                &["1", "-2", "5", "0", "1", "-4", "0", "0", "1"],
                &["-21", "17", "-5"],
            ),
            2 => (
                2,
                &["2", "3", "5", "7"],
                &["11", "13"],
                &["-7", "3", "5", "-2"],
                &["38", "-29"],
            ),
            3 => (
                2,
                &["0", "-1", "1", "0"],
                &["0", "0"],
                &["0", "1", "-1", "0"],
                &["0", "0"],
            ),
            _ => (0, &[], &[], &[], &[]),
        };
    let original = constant_map(&mut case, count, values, offsets)?;
    let expected = constant_map(&mut case, count, inverse, origin)?;
    Ok((case, original, expected))
}

/// Add a named exact positivity premise to a fresh context over the same declarations.
pub(crate) fn positive(case: &mut Fixture, term: &Handle, key: &str) -> TestResult {
    let header = case.cas.context_header(&case.context)?;
    let domain = case.cas.admissible_region(&case.context)?;
    let predicate = builders::admit(
        &mut case.cas,
        Draft::Condition {
            kind: PredicateKind::Positive,
            header: Reference::Existing(header.clone()),
            scope: Vec::new(),
            arguments: vec![Reference::Existing(term.clone())],
        },
        &mut ledger()?,
    )?;
    case.context = builders::admit(
        &mut case.cas,
        Draft::Context(ContextDraft::Root {
            header: Reference::Existing(header),
            admissible: Reference::Existing(domain),
            assumptions: vec![AssumptionDraft {
                key: key.into(),
                condition: Reference::Existing(predicate),
            }],
        }),
        &mut ledger()?,
    )?;
    Ok(())
}

/// One varying diagonal scale and its independently written reciprocal.
pub(crate) fn parameter_case(
    guarded: bool,
) -> TestResult<(Fixture, ExactAffineMap, ExactAffineMap)> {
    let mut case = fixture(&["p"])?;
    let parameter = case.symbol("p")?;
    if guarded {
        positive(&mut case, &parameter, "positive-scale")?;
    }
    let zero = case.term(Operator::Rational(Rational::zero()), real(), &[])?;
    let one = case.term(Operator::Rational(Rational::one()), real(), &[])?;
    let reciprocal = case.term(Operator::Div, real(), &[one, parameter.clone()])?;
    let original = case.cas.exact_reading_map(
        &case.context,
        (1, 1),
        &[parameter],
        std::slice::from_ref(&zero),
        &mut ledger()?,
    )?;
    let expected = case.cas.exact_reading_map(
        &case.context,
        (1, 1),
        &[reciprocal],
        &[zero],
        &mut ledger()?,
    )?;
    Ok((case, original, expected))
}

/// The complete independently built right-inverse claim for fresh replay.
pub(crate) fn expected(
    case: &mut Fixture,
    original: &ExactAffineMap,
    inverse: &ExactAffineMap,
) -> TestResult<crate::test_api::proof::Judgment> {
    let product = case
        .cas
        .compose_reading_maps(original, inverse, &mut ledger()?)?;
    let count = original.inputs();
    let mut coefficients = Vec::new();
    for row in 0..count {
        for column in 0..count {
            coefficients.push(if row == column {
                ExactConstant::one()
            } else {
                ExactConstant::zero()
            });
        }
    }
    let offsets = vec![ExactConstant::zero(); usize::try_from(count)?];
    let identity = case.cas.constant_reading_map(
        &case.context,
        (count, count),
        &coefficients,
        &offsets,
        &mut ledger()?,
    )?;
    Ok(crate::test_api::proof::Judgment {
        context: case.context.clone(),
        layer: crate::test_api::domain::Layer::Pointwise,
        space: None,
        statement: crate::test_api::proof::Statement::Equality {
            left: product.expression().clone(),
            right: identity.expression().clone(),
            domain: case.cas.admissible_region(&case.context)?,
            domain_contract: crate::test_api::proof::DomainContract::DefinedAndEqual,
        },
        residuals: Vec::new(),
    })
}

/// Independently spell the determinant oracle for each exact inverse fixture.
pub(crate) fn determinant_oracle(case: &mut Fixture, kind: u32) -> TestResult<Handle> {
    if kind == 5 {
        return case.symbol("p");
    }
    let value = if kind == 6 {
        let pi = ExactConstant::named(crate::test_api::syntax::Constant::Pi, &mut ledger()?)?;
        pi.div(
            &Rational::parse("180", &mut ledger()?)?.into(),
            &mut ledger()?,
        )?
    } else {
        let text = match kind {
            0 => "1/2",
            2 => "-1",
            _ => "1",
        };
        Rational::parse(text, &mut ledger()?)?.into()
    };
    let header = case.cas.context_header(&case.context)?;
    let drafts = value.expression_drafts(Reference::Existing(header), &mut ledger()?)?;
    let drafts: Vec<_> = drafts.into_iter().map(Draft::Expression).collect();
    case.cas
        .admit(&drafts, &mut ledger()?)?
        .pop()
        .ok_or_else(|| "determinant oracle".into())
}

/// The expected determinant quotient includes its precise original map and denominator.
pub(crate) fn determinant_expected(
    case: &mut Fixture,
    original: &ExactAffineMap,
    value: &Handle,
) -> TestResult<crate::test_api::proof::Judgment> {
    let actual = case.term(
        Operator::MatrixMap(crate::test_api::syntax::MatrixOperation::Determinant),
        real(),
        std::slice::from_ref(original.expression()),
    )?;
    let left = case.term(Operator::Div, real(), &[actual, value.clone()])?;
    let right = case.term(Operator::Rational(Rational::one()), real(), &[])?;
    Ok(crate::test_api::proof::Judgment {
        context: case.context.clone(),
        layer: crate::test_api::domain::Layer::Pointwise,
        space: None,
        statement: crate::test_api::proof::Statement::Equality {
            left,
            right,
            domain: case.cas.admissible_region(&case.context)?,
            domain_contract: crate::test_api::proof::DomainContract::DefinedAndEqual,
        },
        residuals: Vec::new(),
    })
}
