// SPDX-License-Identifier: AGPL-3.0-only
//! Independent rational and validated-fixture truth is read from the published masked result.

use super::{
    ArithmeticContext, Counts, Fixture, Policy, TestResult, broad, check_reading, ledger, oracles,
    point, represented,
};
use nrz_bindings::{BindingSymbols, Budget, Context, Descriptor, ExponentPolicy, Registry};
use nrz_matrix::{
    ApplicationEvaluation, ApplicationPublication, Axis, AxisEntry, BlockDraft, Coverage,
    EntryDraft, ExecutionAccount, ExecutionLimits, Expm1ApplicationRequest, Heap, HeapLimits,
    HostChannel, HostTileDeclaration, OperationDraft, Partition, PointwiseExpm1Request,
    PreparedExpm1Operation, PublishedTile, RegionContract, RetainedExpm1, ShapeDraft, ShapeLimits,
    StructureScope,
};
use std::rc::Rc;

/// Every public numerical reader retains this same result bundle and original account.
struct Published {
    heap: Heap,
    account: ExecutionAccount,
    tile: PublishedTile<f64>,
    authority: RetainedExpm1,
}
/// A coherent assembly supplies original checked region witnesses, without retagging a Square.
fn region(cells: usize) -> TestResult<RegionContract> {
    let limits = ShapeLimits::default();
    let owner = Rc::new(
        Context::new(
            Registry::new(ExponentPolicy::Rational),
            nrz_bindings::readings::algebra::limits::Limits::default(),
            nrz_bindings::readings::algebra::syntax::Environment::default(),
        )?
        .freeze()?,
    );
    let domain = Axis::new(
        vec![AxisEntry::new("component", Descriptor::ratio())?],
        Partition::whole(1, limits)?,
        limits,
    )?;
    let entries = (0..cells)
        .map(|cell| AxisEntry::new(format!("cell-{cell}"), Descriptor::ratio()))
        .collect::<Result<Vec<_>, _>>()?;
    let codomain = Axis::new(entries, Partition::whole(cells, limits)?, limits)?;
    let blocks = if cells == 0 {
        Vec::new()
    } else {
        vec![BlockDraft::new(
            0..cells,
            0..1,
            vec![EntryDraft::Declared(Descriptor::ratio()); cells],
        )]
    };
    let shape = ShapeDraft::new(domain, codomain, blocks, StructureScope::WholeFamily)
        .check(owner, limits)?;
    let checked = OperationDraft::copy(vec![shape]).check(ExecutionLimits::default())?;
    Ok(checked.inputs()[0].clone())
}
/// Preparation and host publication precede the original-account checked masked application.
fn publish(values: &[Option<f64>], domain: [f64; 2]) -> TestResult<Published> {
    let mut fixture = Fixture::new()?;
    let original = broad(&mut fixture, 40)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = ExecutionAccount::new(ledger()?)?;
    let law = heap
        .prepare_retained_expm1(
            fixture.cas,
            &original,
            domain,
            Policy::default(),
            &ArithmeticContext::new()?,
            &mut Counts::default(),
            &mut account,
        )
        .map_err(|(error, _)| error)?;
    let authority = heap.retain_retained_expm1(&law, &mut account)?;
    let mut symbolic = ledger()?;
    let shape = [u32::try_from(values.len())?, 1];
    let source = BindingSymbols::declare(&Descriptor::ratio(), shape, [false; 2], &mut symbolic)?;
    let destination =
        BindingSymbols::declare(&Descriptor::ratio(), shape, [true; 2], &mut symbolic)?;
    let axis = heap.create_cell_axis(values.len(), &mut symbolic)?;
    let input = HostTileDeclaration::check(
        source.clone(),
        axis,
        Coverage::RepresentedInputs,
        &mut account,
    )?;
    let output = HostTileDeclaration::check(
        destination.clone(),
        axis,
        Coverage::RepresentedInputs,
        &mut account,
    )?;
    let (source_tile, receipt) = publish_host(&mut heap, &mut account, input, values)?;
    let layout = receipt
        .filled(HostChannel::Center)
        .ok_or("filled")?
        .native()
        .layout();
    let demand = heap.create_mask(
        layout,
        &(0..values.len()).collect::<Vec<_>>(),
        &mut symbolic,
    )?;
    let input = region(values.len())?;
    let operation = PreparedExpm1Operation::check(
        &law,
        &original.original.target,
        &original.original.context,
        &original.original.coordinates[0].coordinate,
        domain,
        40,
        1,
        input.clone(),
        input,
        source,
        destination,
        axis,
        &mut account,
    )?;
    let owner = match heap.request_pointwise_expm1(
        PointwiseExpm1Request {
            operation,
            law,
            input: source_tile.into_pin(),
            destination: output,
            demand,
            limits: ExecutionLimits::default(),
        },
        &mut account,
    ) {
        Expm1ApplicationRequest::ApplicationReady(owner) => owner,
        other @ Expm1ApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    let owner = match owner.evaluate(&mut account) {
        ApplicationEvaluation::ApplicationCandidate(owner) => owner,
        other => return Err(format!("{other:?}").into()),
    };
    let tile = match heap.publish_application(owner, &mut account) {
        ApplicationPublication::ApplicationPublished(tile, _) => tile,
        other => return Err(format!("{other:?}").into()),
    };
    Ok(Published {
        heap,
        account,
        tile,
        authority,
    })
}
/// Filled host values and cleared invalid holes use only the public host protocol.
fn publish_host(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    input: HostTileDeclaration<f64>,
    values: &[Option<f64>],
) -> TestResult<(PublishedTile<f64>, nrz_matrix::PublicationReceipt)> {
    let mut loan = match heap.request_loan(input, account) {
        nrz_matrix::LoanRequest::Granted(loan) => loan,
        other @ nrz_matrix::LoanRequest::Refused(_) => return Err(format!("{other:?}").into()),
    };
    for (cell, value) in values.iter().enumerate() {
        loan.write_component(
            HostChannel::Center,
            cell,
            0,
            value.unwrap_or(f64::NAN),
            account,
        )?;
        if value.is_none() {
            loan.clear_cell(HostChannel::Center, cell, account)?;
        }
    }
    Ok(match heap.handback(loan, account) {
        nrz_matrix::HandbackOutcome::Published(tile, receipt) => (tile, receipt),
        other => return Err(format!("{other:?}").into()),
    })
}
/// Only published selected readers produce these five data values.
fn reading(result: &mut Published, cell: usize) -> TestResult<nrz_scalar::Expm1Output> {
    let reader = result.tile.reader(&result.heap, &mut result.account)?;
    Ok(nrz_scalar::Expm1Output {
        endpoints: reader.expm1_endpoints(cell, 0, &mut result.account)?,
        center: reader.read_component(HostChannel::Center, cell, 0, &mut result.account)?,
        total_radius: reader.read_component(HostChannel::Total, cell, 0, &mut result.account)?,
        numerical_error: reader.read_component(HostChannel::Error, cell, 0, &mut result.account)?,
    })
}
/// Rational alternating-series bounds, exact nearest rounding and export reconstruction are independent.
/// ´test:integration:masked-prepared-expm1-published-rational-oracles´
#[test]
fn masked_prepared_expm1_published_rational_oracles() -> TestResult {
    let values = [
        -2.0,
        (-2.0_f64).next_up(),
        -1.0,
        -0.25,
        -2.0_f64.powi(-10),
        -2.0_f64.powi(-40),
        -f64::MIN_POSITIVE,
        -f64::MIN_POSITIVE.next_up(),
        0.0,
    ];
    let mut result = publish(&values.map(Some), [-2.0, 0.0])?;
    let mut budget = Budget::new(nrz_bindings::Limits {
        work: 40_000_000_000,
        arithmetic_steps: 100_000_000,
        component_bits: 32_768,
        intermediate_bits: 131_072,
        integer_exponent: 16_384,
        ..nrz_bindings::Limits::default()
    })?;
    for (cell, value) in values.into_iter().enumerate() {
        let output = reading(&mut result, cell)?;
        check_reading(
            result.authority.authority()?,
            point(value),
            output,
            &mut budget,
        )?;
        let center = represented(output.center, &mut budget)?;
        let total = represented(output.total_radius, &mut budget)?;
        let lower = represented(output.endpoints[0], &mut budget)?;
        let upper = represented(output.endpoints[1], &mut budget)?;
        oracles::le(&center.sub(&total, &mut budget)?, &lower, &mut budget)?;
        oracles::le(&upper, &center.add(&total, &mut budget)?, &mut budget)?;
        assert!(
            nrz_scalar::Enclosure::new(output.endpoints[0], output.endpoints[1])?
                .displacement_from(
                    nrz_scalar::Finite::new(output.center)?,
                    &ArithmeticContext::new()?
                )?
                .get()
                <= output.total_radius
        );
    }
    Ok(())
}
/// Golden identities select their own input, target and rational interval from the same validated row.
/// ´test:integration:masked-prepared-expm1-published-validated-fixture´
#[test]
fn masked_prepared_expm1_published_validated_fixture() -> TestResult {
    let payload = nrz_docgen::golden::read(
        include_bytes!("../../../../data/golden/prepared-binary64.json").as_slice(),
        nrz_docgen::golden::Limits::default(),
        &mut ledger()?,
    )?;
    let rows = payload.value()["entries"]
        .as_array()
        .ok_or("entries")?
        .iter()
        .filter(|row| row["operation"] == "expm1")
        .collect::<Vec<_>>();
    let mut values = Vec::new();
    for row in &rows {
        let id = row["id"].as_str().ok_or("id")?;
        let spelling = row["input"]["point"].as_str().ok_or("point")?;
        assert_eq!(
            spelling,
            match id {
                "M1" => "-1/1024",
                "M2" => "-1/1099511627776",
                "M0" => "0",
                _ => return Err("golden identity".into()),
            }
        );
        assert_eq!(row["target"]["kind"], "relative");
        assert_eq!(row["target"]["bits"], 40);
        let (n, d) = spelling.split_once('/').unwrap_or((spelling, "1"));
        values.push(Some(n.parse::<f64>()? / d.parse::<f64>()?));
    }
    let mut result = publish(&values, [-2.0, 0.0])?;
    let mut identities = Vec::new();
    for (cell, row) in rows.iter().enumerate() {
        let output = reading(&mut result, cell)?;
        let golden = row["golden"]["point"].as_array().ok_or("golden")?;
        let lower = nrz_bindings::readings::algebra::Rational::parse(
            golden[0].as_str().ok_or("lower")?,
            &mut ledger()?,
        )?;
        let upper = nrz_bindings::readings::algebra::Rational::parse(
            golden[1].as_str().ok_or("upper")?,
            &mut ledger()?,
        )?;
        oracles::le(
            &represented(output.endpoints[0], &mut ledger()?)?,
            &lower,
            &mut ledger()?,
        )?;
        oracles::le(
            &upper,
            &represented(output.endpoints[1], &mut ledger()?)?,
            &mut ledger()?,
        )?;
        identities.push(row["id"].as_str().ok_or("id")?);
    }
    assert_eq!(identities, ["M1", "M2", "M0"]);
    Ok(())
}
/// Admitted domain validation precedes zero, and holes never expose either numerical family.
#[test]
fn masked_prepared_expm1_published_narrow_and_normal_boundaries() -> TestResult {
    for (domain, values, failures) in [
        (
            [-1.0, 0.0],
            vec![
                Some(-2.0),
                Some(-f64::MIN_POSITIVE.next_down()),
                None,
                Some(-1.0),
                Some(0.0),
            ],
            vec![
                Some(nrz_scalar::Expm1Failure::OutsideDomain),
                Some(nrz_scalar::Expm1Failure::BelowNormalOperand),
                None,
                None,
                None,
            ],
        ),
        (
            [-0.75, -0.5],
            vec![
                Some(0.0),
                Some((-0.75_f64).next_down()),
                Some((-0.5_f64).next_up()),
                Some(-0.75),
                Some(-0.5),
            ],
            vec![Some(nrz_scalar::Expm1Failure::OutsideDomain); 3]
                .into_iter()
                .chain([None, None])
                .collect(),
        ),
    ] {
        let mut result = publish(&values, domain)?;
        let reader = result.tile.reader(&result.heap, &mut result.account)?;
        for (cell, failure) in failures.into_iter().enumerate() {
            let outcome = reader.application_outcome(cell, &mut result.account)?;
            if let Some(expected) = failure {
                assert!(
                    matches!(outcome, nrz_matrix::CellOutcome::PreparedRefused { component: 0, reason } if *reason == expected)
                );
            } else if values[cell].is_none() {
                assert!(matches!(outcome, nrz_matrix::CellOutcome::MissingInput));
            } else {
                assert!(matches!(outcome, nrz_matrix::CellOutcome::Completed));
            }
            assert_eq!(
                reader.expm1_endpoints(cell, 0, &mut result.account).is_ok(),
                failure.is_none() && values[cell].is_some()
            );
        }
    }
    Ok(())
}

/// A valid retained constant proof has no free parameter for the closed component substitution.
#[test]
fn masked_prepared_expm1_closed_constant_correspondence_refuses() -> TestResult {
    let mut fixture = Fixture::new()?;
    let argument = fixture.term(super::Operator::Rational(super::number(-1, 2)?), &[])?;
    let original = super::expression(&mut fixture, argument.clone(), 40)?;
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = ExecutionAccount::new(ledger()?)?;
    let law = heap
        .prepare_retained_expm1(
            fixture.cas,
            &original,
            [-0.5, -0.5],
            Policy::default(),
            &ArithmeticContext::new()?,
            &mut Counts::default(),
            &mut account,
        )
        .map_err(|(error, _)| error)?;
    assert_eq!(law.authority()?.request().original.coordinates.len(), 0);
    let mut symbolic = ledger()?;
    let source = BindingSymbols::declare(&Descriptor::ratio(), [1, 1], [false; 2], &mut symbolic)?;
    let destination =
        BindingSymbols::declare(&Descriptor::ratio(), [1, 1], [true; 2], &mut symbolic)?;
    let axis = heap.create_cell_axis(1, &mut symbolic)?;
    let input = region(1)?;
    let before = account.usage().work;
    assert!(matches!(
        PreparedExpm1Operation::check(
            &law,
            &original.original.target,
            &original.original.context,
            &argument,
            [-0.5, -0.5],
            40,
            1,
            input.clone(),
            input,
            source,
            destination,
            axis,
            &mut account
        ),
        Err(nrz_matrix::ApplicationError::Loan(
            nrz_matrix::LoanError::UnsupportedContract
        ))
    ));
    assert_eq!(account.usage().work - before, 1);
    Ok(())
}
