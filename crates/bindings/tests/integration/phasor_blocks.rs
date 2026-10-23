// SPDX-License-Identifier: AGPL-3.0-only
//! The closed block law retains original component readings and checks every ordered entry.

use crate::testing::readings::{Fixture, TestResult, ledger, map, matrix, meaning};
use nrz_bindings::readings::{
    CoordinateFamily, PhasorBlockCorrespondence, PhasorConvention, ReadingError, Representation,
    RepresentationDefinition,
};
use nrz_cas::{
    Rational,
    limits::{Budget, Limits},
    readings::SelectionSpec,
    source::SyntaxNode,
    syntax::{AtomicSort, Field, Operator, Sort},
};

/// Independent operands select disjoint component pairs from one retained exact source family.
fn block_operands() -> TestResult<(Fixture, [Representation; 2])> {
    let complex = Sort::Value(AtomicSort::Scalar(Field::Complex));
    let mut fixture = Fixture::new(&[
        ("root", matrix(4, 1)),
        ("left", complex.clone()),
        ("right", complex),
        ("left_block", matrix(2, 2)),
        ("right_block", matrix(2, 2)),
    ])?;
    let entry = meaning("coordinate", [0; 7])?;
    let root = CoordinateFamily::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol("root")?,
        vec![entry.clone(); 4],
        &mut ledger()?,
    )?;
    let mut result = Vec::new();
    for (key, block, coefficients) in [
        (
            "left",
            "left_block",
            [1, 0, 0, 0, 0, 1, 0, 0, 0, -1, 0, 0, 1, 0, 0, 0],
        ),
        (
            "right",
            "right_block",
            [0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, -1, 0, 0, 1, 0],
        ),
    ] {
        let symbol = fixture.symbol(block)?;
        let family = root.expand(
            &mut fixture.cas,
            &symbol,
            &map(4, 4, &coefficients, &[0; 4])?,
            vec![entry.clone(); 4],
            &mut ledger()?,
        )?;
        let reading = fixture.define(
            &family,
            key,
            SelectionSpec::Block {
                row: 0,
                column: 0,
                rows: 2,
                columns: 2,
            },
            map(4, 2, &[1, 0, 0, 0, 0, 1, 0, 0], &[0; 2])?,
            vec![entry.clone(); 2],
        )?;
        result.push(Representation::prove(
            &mut fixture.cas,
            &reading,
            RepresentationDefinition::ComplexBlock(symbol),
            &mut ledger()?,
        )?);
    }
    Ok((fixture, result.try_into().map_err(|_| "two operands")?))
}

/// Whole-family multiplication is proved componentwise and retains native ordered operation syntax.
/// ´test:integration:phasor-block-correspondence-keeps-original-operations´
#[test]
fn phasor_block_correspondence_keeps_original_operations() -> TestResult {
    let (mut fixture, operands) = block_operands()?;
    let checked = PhasorBlockCorrespondence::prove(
        &mut fixture.cas,
        operands.clone(),
        PhasorConvention::PositiveTimePassive,
        &mut ledger()?,
    )?;
    checked.check_operands(&operands, &mut ledger()?)?;
    for (node, operator) in [
        (checked.original(), Operator::Mul),
        (checked.lowered(), Operator::MatrixMultiply),
    ] {
        let syntax = fixture.cas.describe_syntax(node, &mut ledger()?)?;
        let SyntaxNode::Expression(expression) = syntax.node else {
            return Err("operation".into());
        };
        assert_eq!(expression.operator, operator);
    }
    assert_eq!(checked.equations().len(), 4);
    for (components, operand) in checked.components().iter().zip(&operands) {
        for component in components {
            assert_eq!(component.reading(), operand.reading());
            assert_eq!(
                component.reading().family().root(),
                operand.reading().family().root()
            );
        }
    }
    oracle(checked.coefficients().coefficients())
}

/// Exact integer samples independently compare all four entries with the scalar Complex product.
fn oracle(coefficients: &[Rational]) -> TestResult {
    for [a, b, u, v] in [
        [2, 3, 5, 7],
        [-3, 0, 0, 5],
        [0, -75, 3, 0],
        [0, 0, 3, 0],
        [0, 75, 3, 0],
    ] {
        let monomials = [a * u, a * v, b * u, b * v];
        let expected = [a * u - b * v, b * u + a * v, -a * v - b * u, a * u - b * v];
        for (row, target) in coefficients.as_chunks::<4>().0.iter().zip(expected) {
            let mut value = Rational::zero();
            for (coefficient, product) in row.iter().zip(monomials) {
                value = value.add(
                    &coefficient.mul(
                        &Rational::from_ratio(product, 1, &mut ledger()?)?,
                        &mut ledger()?,
                    )?,
                    &mut ledger()?,
                )?;
            }
            assert_eq!(value, Rational::from_ratio(target, 1, &mut ledger()?)?);
        }
    }
    Ok(())
}

/// Ordered operand identities and the time/passive-sign convention remain explicit obligations.
/// ´test:integration:phasor-block-correspondence-refuses-order-and-convention-changes´
#[test]
fn phasor_block_correspondence_refuses_order_and_convention_changes() -> TestResult {
    let (mut fixture, operands) = block_operands()?;
    assert!(matches!(
        PhasorBlockCorrespondence::prove(
            &mut fixture.cas,
            operands.clone(),
            PhasorConvention::NegativeTimePassive,
            &mut ledger()?
        ),
        Err(ReadingError::Unsupported)
    ));
    let checked = PhasorBlockCorrespondence::prove(
        &mut fixture.cas,
        operands.clone(),
        PhasorConvention::PositiveTimePassive,
        &mut ledger()?,
    )?;
    assert!(matches!(
        checked.check_operands(&[operands[1].clone(), operands[0].clone()], &mut ledger()?),
        Err(ReadingError::Relation)
    ));
    let (_, foreign) = block_operands()?;
    assert!(checked.check_operands(&foreign, &mut ledger()?).is_err());
    Ok(())
}

/// Shared operands retain the same projections; the component basis never asserts independence.
/// ´test:integration:phasor-block-correspondence-preserves-shared-operands´
#[test]
fn phasor_block_correspondence_preserves_shared_operands() -> TestResult {
    let (mut fixture, operands) = block_operands()?;
    let repeated = [operands[0].clone(), operands[0].clone()];
    let checked = PhasorBlockCorrespondence::prove(
        &mut fixture.cas,
        repeated,
        PhasorConvention::PositiveTimePassive,
        &mut ledger()?,
    )?;
    for index in 0..2 {
        assert_eq!(
            checked.components()[0][index].node(),
            checked.components()[1][index].node()
        );
    }
    assert!(
        checked
            .conjugation()
            .iter()
            .all(|proof| proof.operand().witness().is_discharged())
    );
    Ok(())
}

/// One cumulative allowance covers syntax, coefficient checking and both conjugation witnesses.
/// ´test:integration:phasor-block-correspondence-resource-boundaries´
#[test]
fn phasor_block_correspondence_resource_boundaries() -> TestResult {
    let (mut fixture, operands) = block_operands()?;
    let mut budget = ledger()?;
    PhasorBlockCorrespondence::prove(
        &mut fixture.cas,
        operands,
        PhasorConvention::PositiveTimePassive,
        &mut budget,
    )?;
    let needed = budget.usage().work;
    for (limit, prior, success) in [
        (needed, 0, true),
        (needed - 1, 0, false),
        (needed, 1, false),
    ] {
        let (mut fixture, original) = block_operands()?;
        let mut budget = Budget::new(Limits {
            work: limit,
            ..Limits::default()
        })?;
        budget.work(prior, nrz_cas::Location::new("phasor precharge"))?;
        assert_eq!(
            PhasorBlockCorrespondence::prove(
                &mut fixture.cas,
                original,
                PhasorConvention::PositiveTimePassive,
                &mut budget
            )
            .is_ok(),
            success
        );
    }
    Ok(())
}

/// Equal exact values through another repeated copy do not replace a declared operand identity.
/// ´test:integration:phasor-block-correspondence-refuses-copy-and-sign-mutations´
#[test]
fn phasor_block_correspondence_refuses_copy_and_sign_mutations() -> TestResult {
    let (mut fixture, operands) = block_operands()?;
    let checked = PhasorBlockCorrespondence::prove(
        &mut fixture.cas,
        operands.clone(),
        PhasorConvention::PositiveTimePassive,
        &mut ledger()?,
    )?;
    let family = operands[0].reading().family().clone();
    let meanings = operands[0].reading().entries().to_vec();
    let selection = SelectionSpec::Block {
        row: 0,
        column: 0,
        rows: 2,
        columns: 2,
    };
    let other = fixture.define(
        &family,
        "left",
        selection.clone(),
        map(4, 2, &[0, 0, 0, 1, 0, 0, -1, 0], &[0; 2])?,
        meanings.clone(),
    )?;
    let symbol = fixture.symbol("left_block")?;
    let representation = Representation::prove(
        &mut fixture.cas,
        &other,
        RepresentationDefinition::ComplexBlock(symbol.clone()),
        &mut ledger()?,
    )?;
    assert!(
        checked
            .check_operands(&[representation, operands[1].clone()], &mut ledger()?)
            .is_err()
    );
    let wrong = fixture.define(
        &family,
        "left",
        selection,
        map(4, 2, &[1, 0, 0, 0, 0, -1, 0, 0], &[0; 2])?,
        meanings,
    )?;
    assert!(
        Representation::prove(
            &mut fixture.cas,
            &wrong,
            RepresentationDefinition::ComplexBlock(symbol),
            &mut ledger()?
        )
        .is_err()
    );
    Ok(())
}
