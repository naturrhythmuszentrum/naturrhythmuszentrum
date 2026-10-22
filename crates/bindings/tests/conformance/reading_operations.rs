// SPDX-License-Identifier: AGPL-3.0-only
//! Closed physical operation relations retain original coordinates and native matrix order.

use crate::testing::readings::{Fixture, TestResult, ledger, map, matrix, meaning, state_case};
use nrz_bindings::readings::{
    AffineOperation, CoordinateFamily, LinearOperator, OperatorApplication, ReadingAxis,
    ReadingOperationError, ReadingShape,
};
use nrz_cas::{
    limits::Budget,
    readings::{AffineMap, SelectionSpec},
    source::SyntaxNode,
    syntax::{AtomicSort, Operator, Sort},
};

/// Original mixed operator and its ordered voltage/current column operand.
struct Product {
    /// Exact original declarations, with no original product proof.
    fixture: Fixture,
    /// Formal ordered coefficient meanings, including typed diagonal zeros.
    operator: LinearOperator,
    /// Original mixed physical input with two checked axes.
    input: ReadingShape,
}

/// Declare a bounded mixed operator without inventing scalar quotient kinds.
fn product() -> TestResult<Product> {
    let integer = Sort::Value(AtomicSort::Integer);
    let mut fixture = Fixture::new(&[
        ("z", matrix(2, 1)),
        ("x", matrix(2, 1)),
        ("A", matrix(2, 2)),
        ("other", matrix(2, 2)),
        ("state", integer.clone()),
        ("rate", integer.clone()),
        ("batch", integer),
    ])?;
    let domain = vec![
        meaning("voltage", [2, 1, -3, -1, 0, 0, 0])?,
        meaning("current", [0, 0, 0, 1, 0, 0, 0])?,
    ];
    let codomain = vec![
        meaning("voltage_rate", [2, 1, -4, -1, 0, 0, 0])?,
        meaning("current_rate", [0, 0, -1, 1, 0, 0, 0])?,
    ];
    let family = CoordinateFamily::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol("z")?,
        domain.clone(),
        &mut ledger()?,
    )?;
    let reading = fixture.define(
        &family,
        "x",
        SelectionSpec::Column(0),
        AffineMap::identity(2, &mut ledger()?)?,
        domain.clone(),
    )?;
    let state = axis(&fixture, "state", 2)?;
    let rate = axis(&fixture, "rate", 2)?;
    let batch = axis(&fixture, "batch", 1)?;
    let input = ReadingShape::new(
        &reading,
        [state.clone(), batch],
        &[SelectionSpec::Block {
            row: 0,
            column: 0,
            rows: 2,
            columns: 1,
        }],
        &mut ledger()?,
    )?;
    let operator = LinearOperator::declare(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol("A")?,
        [rate, state],
        domain,
        codomain,
        &mut ledger()?,
    )?;
    Ok(Product {
        fixture,
        operator,
        input,
    })
}

/// Exact index identities matter independently of matching axis extent.
fn axis(fixture: &Fixture, key: &str, extent: u32) -> TestResult<ReadingAxis> {
    Ok(ReadingAxis::new(
        &fixture.cas,
        &fixture.context,
        &fixture.symbol(key)?,
        (0..extent).collect(),
        vec![extent],
        &mut ledger()?,
    )?)
}

/// Mixed-state projection is a checked affine operation with fresh expected-judgment replay.
/// ´test:integration:affine-reading-operation-replays´
#[test]
fn affine_reading_operation_replays() -> TestResult {
    let mut sender = state_case(0)?;
    let operation = AffineOperation::prove(
        &mut sender.fixture.cas,
        &sender.whole,
        &sender.component,
        map(2, 1, &[1, 0], &[0])?,
        &mut ledger()?,
    )?;
    let bytes = sender
        .fixture
        .cas
        .export_witness(operation.witness(), &mut ledger()?)?;
    let mut receiver = state_case(0)?;
    let checked = AffineOperation::replay(
        &mut receiver.fixture.cas,
        &receiver.whole,
        &receiver.component,
        map(2, 1, &[1, 0], &[0])?,
        &bytes,
        &mut ledger()?,
    )?;
    assert!(checked.witness().is_discharged());
    assert!(
        AffineOperation::replay(
            &mut receiver.fixture.cas,
            &receiver.whole,
            &receiver.component,
            map(2, 1, &[-1, 0], &[0])?,
            &bytes,
            &mut ledger()?
        )
        .is_err()
    );
    Ok(())
}

/// Ordered matrix application keeps native algebra, formal units and original axis identities.
/// ´test:integration:mixed-operator-application-replays-native-order´
#[test]
fn mixed_operator_application_replays_native_order() -> TestResult {
    let mut sender = product()?;
    let operation = OperatorApplication::define(
        &mut sender.fixture.cas,
        &sender.operator,
        &sender.input,
        &mut ledger()?,
    )?;
    let syntax = sender
        .fixture
        .cas
        .describe_syntax(operation.node(), &mut ledger()?)?;
    let SyntaxNode::Expression(expression) = syntax.node else {
        return Err("operation expression".into());
    };
    assert_eq!(expression.operator, Operator::MatrixMultiply);
    assert_eq!(syntax.dependencies[0].target, *sender.operator.node());
    assert_eq!(
        syntax.dependencies[1].target,
        *sender.input.reading().node()
    );
    assert_eq!(operation.entries(), sender.operator.codomain());
    assert!(
        sender
            .operator
            .entries()
            .iter()
            .all(|entry| entry.pairing().uncertainty_descriptor().is_none())
    );
    let bytes = sender
        .fixture
        .cas
        .export_witness(operation.witness(), &mut ledger()?)?;
    let mut receiver = product()?;
    let replayed = OperatorApplication::replay(
        &mut receiver.fixture.cas,
        &receiver.operator,
        &receiver.input,
        &bytes,
        &mut ledger()?,
    )?;
    assert!(replayed.witness().is_discharged());
    Ok(())
}

/// Equal dimensions do not permit a changed ordered contraction or substituted coefficient declaration.
/// ´test:integration:mixed-operator-refuses-axis-and-operand-substitution´
#[test]
fn mixed_operator_refuses_axis_and_operand_substitution() -> TestResult {
    let mut case = product()?;
    let operation = OperatorApplication::define(
        &mut case.fixture.cas,
        &case.operator,
        &case.input,
        &mut ledger()?,
    )?;
    let bytes = case
        .fixture
        .cas
        .export_witness(operation.witness(), &mut ledger()?)?;
    let wrong = LinearOperator::declare(
        &case.fixture.cas,
        &case.fixture.context,
        &case.fixture.symbol("other")?,
        case.operator.axes().clone(),
        case.operator.domain().to_vec(),
        case.operator.codomain().to_vec(),
        &mut ledger()?,
    )?;
    assert!(
        OperatorApplication::replay(
            &mut case.fixture.cas,
            &wrong,
            &case.input,
            &bytes,
            &mut ledger()?
        )
        .is_err()
    );
    let axes = [
        case.operator.axes()[0].clone(),
        axis(&case.fixture, "rate", 2)?,
    ];
    let wrong = LinearOperator::declare(
        &case.fixture.cas,
        &case.fixture.context,
        &case.fixture.symbol("A")?,
        axes,
        case.operator.domain().to_vec(),
        case.operator.codomain().to_vec(),
        &mut ledger()?,
    )?;
    assert!(matches!(
        OperatorApplication::define(&mut case.fixture.cas, &wrong, &case.input, &mut ledger()?),
        Err(ReadingOperationError::Axis)
    ));
    let mut domain = case.operator.domain().to_vec();
    domain.swap(0, 1);
    let wrong = LinearOperator::declare(
        &case.fixture.cas,
        &case.fixture.context,
        &case.fixture.symbol("A")?,
        case.operator.axes().clone(),
        domain,
        case.operator.codomain().to_vec(),
        &mut ledger()?,
    )?;
    assert!(matches!(
        OperatorApplication::define(&mut case.fixture.cas, &wrong, &case.input, &mut ledger()?),
        Err(ReadingOperationError::Input)
    ));
    Ok(())
}

/// Every operation constructor, proof and independent replay has exact resource boundaries.
/// ´test:integration:reading-operation-resource-boundaries´
#[test]
fn reading_operation_resource_boundaries() -> TestResult {
    for operation in 0..5 {
        crate::testing::readings::boundaries(|limits, precharge| {
            let mut budget = Budget::new(limits)?;
            budget.work(
                precharge,
                nrz_cas::Location::new("operation boundary precharge"),
            )?;
            if operation < 2 {
                affine_attempt(operation == 1, &mut budget)?;
            } else {
                product_attempt(operation, &mut budget)?;
            }
            Ok(budget.usage().work)
        })?;
    }
    Ok(())
}

/// Build all original physical declarations independently before measuring the affine operation.
fn affine_attempt(replay: bool, budget: &mut Budget) -> TestResult {
    let mut case = state_case(0)?;
    let map = map(2, 1, &[1, 0], &[0])?;
    if replay {
        let mut sender = state_case(0)?;
        let proof = AffineOperation::prove(
            &mut sender.fixture.cas,
            &sender.whole,
            &sender.component,
            map.clone(),
            &mut ledger()?,
        )?;
        let bytes = sender
            .fixture
            .cas
            .export_witness(proof.witness(), &mut ledger()?)?;
        AffineOperation::replay(
            &mut case.fixture.cas,
            &case.whole,
            &case.component,
            map,
            &bytes,
            budget,
        )?;
    } else {
        AffineOperation::prove(
            &mut case.fixture.cas,
            &case.whole,
            &case.component,
            map,
            budget,
        )?;
    }
    Ok(())
}

/// Native operator declaration, application and replay retain independent accounting.
fn product_attempt(operation: u32, budget: &mut Budget) -> TestResult {
    let mut case = product()?;
    if operation == 2 {
        LinearOperator::declare(
            &case.fixture.cas,
            &case.fixture.context,
            &case.fixture.symbol("A")?,
            case.operator.axes().clone(),
            case.operator.domain().to_vec(),
            case.operator.codomain().to_vec(),
            budget,
        )?;
    } else if operation == 3 {
        OperatorApplication::define(&mut case.fixture.cas, &case.operator, &case.input, budget)?;
    } else {
        let mut sender = product()?;
        let proof = OperatorApplication::define(
            &mut sender.fixture.cas,
            &sender.operator,
            &sender.input,
            &mut ledger()?,
        )?;
        let bytes = sender
            .fixture
            .cas
            .export_witness(proof.witness(), &mut ledger()?)?;
        OperatorApplication::replay(
            &mut case.fixture.cas,
            &case.operator,
            &case.input,
            &bytes,
            budget,
        )?;
    }
    Ok(())
}

/// Formal coefficients preserve nonunit exact references rather than silently using unit measures.
/// ´test:integration:operator-coefficients-retain-reference-quotients´
#[test]
fn operator_coefficients_retain_reference_quotients() -> TestResult {
    let case = product()?;
    let mut outputs = case.operator.codomain().to_vec();
    let descriptor = outputs[0].descriptor().clone();
    let reference = nrz_bindings::ReferenceDefinition::new(
        "ten volts per second",
        &descriptor,
        nrz_bindings::Scale::Exact(nrz_cas::Rational::from_ratio(10, 1, &mut ledger()?)?),
    )?;
    outputs[0] =
        nrz_bindings::readings::EntryMeaning::new(descriptor, reference, None, &mut ledger()?)?;
    let operator = LinearOperator::declare(
        &case.fixture.cas,
        &case.fixture.context,
        case.operator.node(),
        case.operator.axes().clone(),
        case.operator.domain().to_vec(),
        outputs,
        &mut ledger()?,
    )?;
    assert_eq!(
        operator.entries()[0].reference().as_rational(),
        Some(&nrz_cas::Rational::from_ratio(10, 1, &mut ledger()?)?)
    );
    assert_eq!(operator.entries()[0].input(), &case.operator.domain()[0]);
    assert_eq!(operator.entries()[2].output(), &operator.codomain()[0]);
    Ok(())
}
