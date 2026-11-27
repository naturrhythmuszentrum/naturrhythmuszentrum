// SPDX-License-Identifier: AGPL-3.0-only
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`dyadic_application_correspondence`] | matrix | Original and closed-member admission guards reject substituted bindings before loads. |
//! | [`dyadic_application_masks_and_readings`] | matrix | Public masked selection preserves requested, absent, refused and completed outcomes across word boundaries. |
//! | [`dyadic_application_resources`] | matrix | Independent application inventories and public Work/read authorization boundaries retain their typed refusals. |
//! | [`dyadic_application_lifecycle`] | matrix | Public result retention and detached ready, candidate and fault-entered repair drop/unwind preserve the exact funded owner. |
//! | [`dyadic_application_failures_and_pairing`] | matrix | Flat causes, allocation prefixes and selected private pairing guards retain their distinct evidence. |
//! | [`dyadic_application_observation`] | matrix | Scoped application reuse and repair preserve Work without reconstructing the law. |

use super::*;
use crate::loan::retained_dyadic_power::tests::fixture as laws;
use crate::{ExecutionLimits, HeapLimits, RetainedDyadicPower};
use nrz_bindings::{BindingSymbols, Descriptor};
use nrz_scalar::ArithmeticContext;

/// Test failures retain their original typed cause.
type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Work, native calls, stores and readings from one observed reuse window.
type ObservedCallsResult = Result<(u64, usize, usize, Vec<[f64; 2]>), Box<dyn std::error::Error>>;

/// Observed reuse adds the count of scoped tracing spans to the run readings.
type ObservedReuseResult =
    Result<(u64, usize, usize, Vec<[f64; 2]>, usize), Box<dyn std::error::Error>>;

/// Prepare an actual dyadic law on the original account of an existing declared host source.
fn law_on(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result<RetainedDyadicPower, Box<dyn std::error::Error>> {
    law_for(heap, account, 1, 2, [1.0, 2.0])
}

/// Prepare an original dyadic exponent and represented domain on its existing account.
fn law_for(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    numerator: i64,
    denominator: u64,
    domain: [f64; 2],
) -> Result<RetainedDyadicPower, Box<dyn std::error::Error>> {
    let (cas, target, context) = if domain[0] <= f64::MIN_POSITIVE.next_up() {
        laws::threshold_original(numerator, denominator)?
    } else {
        laws::original(numerator, denominator)?
    };
    let law = heap
        .prepare_retained_dyadic_power(
            cas,
            &target,
            &context,
            domain,
            laws::policy(),
            laws::scalar_limits(),
            &ArithmeticContext::new()?,
            &mut |_| false,
            account,
        )
        .map_err(|(error, _)| error)?;
    Ok(law)
}

/// Bind a real dyadic use to the source and result declarations of a host fixture.
fn checked_on(
    law: &RetainedDyadicPower,
    setup: &mut super::super::tests::expm1::Fixture,
) -> Result<PreparedDyadicPowerOperation, Box<dyn std::error::Error>> {
    let original = law.authority()?.judgment().request();
    let prepared = &setup.request.operation;
    Ok(PreparedDyadicPowerOperation::check(
        law,
        &original.target,
        &original.context,
        &original.coordinates[0].coordinate,
        [1.0, 2.0],
        ArithmeticContext::new()?.revision(),
        prepared.operation.inputs()[0].clone(),
        prepared.operation.outputs()[0].clone(),
        prepared.source.clone(),
        prepared.destination.clone(),
        prepared.axis,
        &mut setup.account,
    )?)
}

/// Real host input and separately funded checked power retain the same original account.
struct Fixture {
    /// Heap with the host source and retained owner.
    heap: Heap,
    /// Original ledger shared by construction and application.
    account: ExecutionAccount,
    /// Complete recoverable public application request.
    request: PointwiseDyadicPowerRequest,
}

/// Declare positive or deliberately invalid values under an independent selected demand.
fn fixture(
    cells: usize,
    components: usize,
    selected: &[usize],
    value: impl Fn(usize, usize) -> Option<f64>,
) -> Result<Fixture, Box<dyn std::error::Error>> {
    fixture_for(cells, components, selected, value, 1, 2, [1.0, 2.0], None)
}

/// Build an original host and checked power at a selected exponent, domain and optional test ceiling.
#[expect(
    clippy::too_many_arguments,
    reason = "the alternate test ceiling preserves the original fixture's individual dimensions"
)]
fn fixture_for(
    cells: usize,
    components: usize,
    selected: &[usize],
    value: impl Fn(usize, usize) -> Option<f64>,
    numerator: i64,
    denominator: u64,
    domain: [f64; 2],
    limits_override: Option<nrz_bindings::Limits>,
) -> Result<Fixture, Box<dyn std::error::Error>> {
    let mut heap = Heap::new(HeapLimits::default())?;
    let limits = limits_override.unwrap_or_else(|| {
        if domain[0] <= f64::MIN_POSITIVE.next_up() {
            laws::threshold_limits()
        } else {
            laws::limits()
        }
    });
    let mut account = ExecutionAccount::new(Budget::new(limits)?)?;
    let mut symbolic = Budget::new(nrz_bindings::Limits::default())?;
    let shape = [u32::try_from(cells)?, u32::try_from(components)?];
    let source = BindingSymbols::declare(&Descriptor::ratio(), shape, [false; 2], &mut symbolic)?;
    let destination =
        BindingSymbols::declare(&Descriptor::ratio(), shape, [true; 2], &mut symbolic)?;
    let axis = heap.create_cell_axis(cells, &mut symbolic)?;
    let source_declaration = crate::HostTileDeclaration::check(
        source.clone(),
        axis,
        crate::Coverage::RepresentedInputs,
        &mut account,
    )?;
    let destination_declaration = crate::HostTileDeclaration::check(
        destination.clone(),
        axis,
        crate::Coverage::RepresentedInputs,
        &mut account,
    )?;
    let mut loan = match heap.request_loan(source_declaration, &mut account) {
        crate::LoanRequest::Granted(loan) => loan,
        other @ crate::LoanRequest::Refused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    for cell in 0..cells {
        for component in 0..components {
            if let Some(center) = value(cell, component) {
                loan.write_component(
                    crate::HostChannel::Center,
                    cell,
                    component,
                    center,
                    &mut account,
                )?;
            }
        }
    }
    let (tile, receipt) = match heap.handback(loan, &mut account) {
        crate::HandbackOutcome::Published(tile, receipt) => (tile, receipt),
        other => return Err(format!("{other:?}").into()),
    };
    let layout = receipt
        .filled(crate::HostChannel::Center)
        .ok_or("source attachment")?
        .native()
        .layout();
    let demand = heap.create_mask(layout, selected, &mut account.ledger)?;
    let law = law_for(&mut heap, &mut account, numerator, denominator, domain)?;
    let declarations = crate::kernel_testing::owner()?;
    let domain_axis = crate::kernel_testing::axis(components)?;
    let codomain = crate::kernel_testing::axis(cells)?;
    let shape = crate::kernel_testing::shape(&declarations, &domain_axis, &codomain)?;
    let regions = crate::OperationDraft::copy(vec![shape]).check(ExecutionLimits::default())?;
    let original = law.authority()?.judgment().request();
    let operation = PreparedDyadicPowerOperation::check(
        &law,
        &original.target,
        &original.context,
        &original.coordinates[0].coordinate,
        domain,
        ArithmeticContext::new()?.revision(),
        regions.inputs()[0].clone(),
        regions.outputs()[0].clone(),
        source,
        destination,
        axis,
        &mut account,
    )?;
    Ok(Fixture {
        heap,
        account,
        request: PointwiseDyadicPowerRequest {
            operation,
            law,
            input: tile.into_pin(),
            destination: destination_declaration,
            demand,
            limits: ExecutionLimits::default(),
        },
    })
}

/// Fund a distinct use and result declaration while retaining the same frozen original.
fn additional(
    setup: &mut Fixture,
) -> Result<PointwiseDyadicPowerRequest, Box<dyn std::error::Error>> {
    let original = &setup.request;
    let law = setup
        .heap
        .retain_retained_dyadic_power(&original.law, &mut setup.account)?;
    let request = law.authority()?.judgment().request();
    let components = original.operation.operation.inputs()[0]
        .shape()
        .extent()
        .columns();
    let cells = original.operation.operation.inputs()[0]
        .shape()
        .extent()
        .rows();
    let destination = BindingSymbols::declare(
        &Descriptor::ratio(),
        [u32::try_from(cells)?, u32::try_from(components)?],
        [true; 2],
        &mut setup.account.ledger,
    )?;
    let operation = PreparedDyadicPowerOperation::check(
        &law,
        &request.target,
        &request.context,
        &request.coordinates[0].coordinate,
        law.domain()?,
        ArithmeticContext::new()?.revision(),
        original.operation.operation.inputs()[0].clone(),
        original.operation.operation.outputs()[0].clone(),
        original.operation.source.clone(),
        destination.clone(),
        original.operation.axis,
        &mut setup.account,
    )?;
    Ok(PointwiseDyadicPowerRequest {
        operation,
        law,
        input: setup.heap.retain_pin(&original.input, &mut setup.account)?,
        destination: crate::HostTileDeclaration::check(
            destination,
            original.destination.axis,
            crate::Coverage::RepresentedInputs,
            &mut setup.account,
        )?,
        demand: original.demand,
        limits: original.limits,
    })
}

/// Publish one selected dyadic result through the public funded lifecycle.
fn published_one()
-> Result<(Heap, ExecutionAccount, crate::loan::PublishedTile<f64>), Box<dyn std::error::Error>> {
    published_one_for(None)
}

/// Publish one selected result with enough independent allowance for a foreign-law fixture.
fn published_one_for(
    limits: Option<nrz_bindings::Limits>,
) -> Result<(Heap, ExecutionAccount, crate::loan::PublishedTile<f64>), Box<dyn std::error::Error>> {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture_for(1, 1, &[0], |_, _| Some(1.0), 1, 2, [1.0, 2.0], limits)?;
    let owner = match heap.request_pointwise_dyadic_power(request, &mut account) {
        DyadicPowerApplicationRequest::ApplicationReady(owner) => owner,
        other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    let candidate = match owner.evaluate(&mut account) {
        super::super::ApplicationEvaluation::ApplicationCandidate(candidate) => candidate,
        other => return Err(format!("{other:?}").into()),
    };
    let tile = match heap.publish_application(candidate, &mut account) {
        super::super::ApplicationPublication::ApplicationPublished(tile, _) => tile,
        other => return Err(format!("{other:?}").into()),
    };
    Ok((heap, account, tile))
}

/// Leave precisely the requested Work on the original account without replacing its limits.
fn leave_work(account: &mut ExecutionAccount, remaining: u64) -> TestResult {
    let spend = account.limits().work - account.usage().work - remaining;
    account.ledger.work(spend, crate::loan::AT)?;
    Ok(())
}

/// Construct a checked one-cell binary contract from distinct original host pins.
fn binary_checked(
    function: crate::BinaryElementwise,
) -> Result<crate::CheckedOperation, Box<dyn std::error::Error>> {
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = ExecutionAccount::new(Budget::new(laws::limits())?)?;
    let axis = heap.create_cell_axis(1, &mut account.ledger)?;
    let destination =
        BindingSymbols::declare(&Descriptor::ratio(), [1, 1], [true; 2], &mut account.ledger)?;
    let mut pins = Vec::new();
    for _ in 0..2 {
        let symbols =
            BindingSymbols::declare(&Descriptor::ratio(), [1, 1], [true; 2], &mut account.ledger)?;
        let declaration = crate::HostTileDeclaration::check(
            symbols,
            axis,
            crate::Coverage::RepresentedInputs,
            &mut account,
        )?;
        let mut loan = match heap.request_loan(declaration, &mut account) {
            crate::LoanRequest::Granted(loan) => loan,
            other @ crate::LoanRequest::Refused(_) => {
                return Err(format!("{other:?}").into());
            }
        };
        for (channel, value) in [
            (crate::HostChannel::Center, 1.0),
            (crate::HostChannel::Total, 0.0),
            (crate::HostChannel::Error, 0.0),
        ] {
            loan.write_component(channel, 0, 0, value, &mut account)?;
        }
        let tile = match heap.handback(loan, &mut account) {
            crate::HandbackOutcome::Published(tile, _) => tile,
            other => return Err(format!("{other:?}").into()),
        };
        pins.push(tile.into_pin());
    }
    let right = pins.pop().ok_or("right pin")?;
    let left = pins.pop().ok_or("left pin")?;
    let declarations = crate::kernel_testing::owner()?;
    let domain = crate::kernel_testing::axis(1)?;
    let codomain = crate::kernel_testing::axis(1)?;
    let region = crate::RegionContract::check(crate::kernel_testing::shape(
        &declarations,
        &domain,
        &codomain,
    )?)?;
    let operation = crate::PointwiseBinaryOperation::check(
        [region.clone(), region.clone()],
        region,
        [&left, &right],
        destination,
        axis,
        function,
        ExecutionLimits::default(),
        &mut account,
    )?;
    Ok(operation.operation)
}

/// Original handles and selected closed member guards reject substitutions before reads.
/// # Panics
/// Panics only if a substituted original, member, symbol or use passes a private check.
/// ´claim:matrix:dyadic-application-correspondence´
/// ´test:unit:dyadic-application-correspondence´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "private correspondence and recoverable public refusal share one control"
)]
fn dyadic_application_correspondence() -> TestResult {
    let mut setup = super::super::tests::expm1::fixture(1, 1, [-2.0, 0.0], 1, false)?;
    let law = law_on(&mut setup.heap, &mut setup.account)?;
    let checked = checked_on(&law, &mut setup)?;
    assert!(!checked.operation.carries_statistics());
    assert!(!setup.request.operation.operation.carries_statistics());
    // The checked alternatives are real closed recipes; only the private member guards are crossed.
    let input_shape = checked.operation.inputs()[0].shape().clone();
    let output_shape = checked.operation.outputs()[0].shape().clone();
    let transform = crate::OperationDraft::transform_entries(
        input_shape.clone(),
        input_shape.domain().clone(),
        input_shape.codomain().clone(),
        crate::EntryMap::reshape(
            input_shape.extent(),
            input_shape.extent(),
            crate::ShapeLimits::default(),
        )?,
        crate::ShapeLimits::default(),
    )?
    .check(ExecutionLimits::default())?;
    let outer = setup.heap.create_cell_axis(1, &mut setup.account.ledger)?;
    let inner = setup.heap.create_cell_axis(1, &mut setup.account.ledger)?;
    let product = setup
        .heap
        .create_cell_product(outer, inner, &mut setup.account.ledger)?;
    let projection = crate::EntryMap::product_cells(
        product,
        crate::CellFactor::Outer,
        1,
        crate::ShapeLimits::default(),
    )?;
    assert!(
        super::super::member::Member::check(&transform, crate::KernelKind::TransformEntries, None,)
            .is_ok()
    );
    assert!(matches!(
        super::super::member::Member::check_dyadic(
            &transform,
            crate::KernelKind::PreparedDyadicPower,
            &checked.binding,
        ),
        Err(LoanError::UnsupportedContract)
    ));
    assert!(matches!(
        super::super::member::Member::check(
            &checked.operation,
            crate::KernelKind::TransformEntries,
            None,
        ),
        Err(LoanError::UnsupportedContract)
    ));
    for function in [
        crate::Elementwise::Square,
        crate::Elementwise::Reciprocal,
        crate::Elementwise::SquareRoot,
    ] {
        let alternative = crate::OperationDraft::map_elementwise(
            input_shape.clone(),
            output_shape.clone(),
            function,
        )?
        .check(ExecutionLimits::default())?;
        let expected = crate::KernelKind::Elementwise(function);
        assert!(super::super::member::Member::check(&alternative, expected, None).is_ok());
        assert!(matches!(
            super::super::member::Member::check_dyadic(
                &alternative,
                crate::KernelKind::PreparedDyadicPower,
                &checked.binding,
            ),
            Err(LoanError::UnsupportedContract)
        ));
        assert!(matches!(
            super::super::member::Member::check(&checked.operation, expected, None),
            Err(LoanError::UnsupportedContract)
        ));
        let projected = crate::OperationDraft::map_projected_elementwise(
            input_shape.clone(),
            output_shape.clone(),
            projection.clone(),
            function,
        )?
        .check(ExecutionLimits::default())?;
        let projected_kind = crate::KernelKind::Elementwise(function);
        assert!(super::super::member::Member::check(&projected, projected_kind, None).is_ok());
        assert!(matches!(
            super::super::member::Member::check_dyadic(
                &projected,
                crate::KernelKind::PreparedDyadicPower,
                &checked.binding,
            ),
            Err(LoanError::UnsupportedContract)
        ));
        assert!(matches!(
            super::super::member::Member::check(&checked.operation, projected_kind, None),
            Err(LoanError::UnsupportedContract)
        ));
    }
    for function in [
        crate::BinaryElementwise::Product,
        crate::BinaryElementwise::Quotient,
    ] {
        let alternative = binary_checked(function)?;
        let expected = crate::KernelKind::BinaryElementwise(function);
        assert!(super::super::member::Member::check(&alternative, expected, None).is_ok());
        assert!(matches!(
            super::super::member::Member::check_dyadic(
                &alternative,
                crate::KernelKind::PreparedDyadicPower,
                &checked.binding,
            ),
            Err(LoanError::UnsupportedContract)
        ));
        assert!(matches!(
            super::super::member::Member::check(&checked.operation, expected, None),
            Err(LoanError::UnsupportedContract)
        ));
    }
    let mut affine_budget = Budget::new(laws::limits())?;
    let affine_context = ArithmeticContext::new()?;
    let tolerance = nrz_scalar::Finite::new(
        <f64 as nrz_scalar::Scalar>::parse_nearest("0.0000000000000005")
            .ok_or("affine tolerance")?,
    )?;
    let circle = crate::PiConstant::new(
        tolerance,
        nrz_scalar::PiLimits {
            work: 2054,
            steps: 11,
            staged_bytes: 256,
            output_bytes: 80,
        },
        &affine_context,
        &mut affine_budget,
    )?;
    let ratio = Descriptor::ratio();
    let coefficient = crate::ExactCoefficient::unit(&ratio)?;
    let prelude = crate::AffinePrelude::new(
        std::rc::Rc::new([coefficient.clone(), coefficient]),
        &circle,
        &nrz_bindings::LimitsOverride {
            staged_payload_bytes: Some(65_536),
            output_bytes: Some(16_384),
            ..nrz_bindings::LimitsOverride::default()
        },
        nrz_scalar::LiteralLimits::default(),
        &affine_context,
        &mut affine_budget,
    )?;
    let affine = crate::OperationDraft::map_projected_affine(
        input_shape,
        output_shape,
        projection,
        &prelude,
    )?
    .check(ExecutionLimits::default())?;
    assert!(
        super::super::member::Member::check(&affine, crate::KernelKind::LinearMap, None).is_ok()
    );
    assert!(matches!(
        super::super::member::Member::check_dyadic(
            &affine,
            crate::KernelKind::PreparedDyadicPower,
            &checked.binding,
        ),
        Err(LoanError::UnsupportedContract)
    ));
    assert!(matches!(
        super::super::member::Member::check(&checked.operation, crate::KernelKind::LinearMap, None),
        Err(LoanError::UnsupportedContract)
    ));
    assert!(
        super::super::member::Member::check(
            &setup.request.operation.operation,
            crate::KernelKind::PreparedExpm1,
            Some(&setup.request.operation),
        )
        .is_ok()
    );
    assert!(matches!(
        super::super::member::Member::check_dyadic(
            &setup.request.operation.operation,
            crate::KernelKind::PreparedDyadicPower,
            &checked.binding,
        ),
        Err(LoanError::UnsupportedContract)
    ));
    assert!(matches!(
        super::super::member::Member::check(
            &checked.operation,
            crate::KernelKind::PreparedExpm1,
            Some(&setup.request.operation),
        ),
        Err(LoanError::UnsupportedContract)
    ));
    let dense_source = setup.heap.import_represented(
        crate::Extent::new(1, 1)?,
        &Descriptor::ratio(),
        &[1.0_f64],
    )?;
    let dense = crate::Binding::new(
        checked.operation,
        vec![dense_source.value()],
        ExecutionLimits::default(),
    );
    let injection = crate::loan::fault::Injection::after(0);
    assert!(matches!(
        dense.validate(&mut setup.heap),
        Err(crate::OperationError::Kernel(crate::KernelError::Contract))
    ));
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    drop(injection);
    let request = law.authority()?.judgment().request();
    let target = &request.target;
    let context = &request.context;
    let argument = &request.coordinates[0].coordinate;
    law.authenticate_account(&setup.account)?;
    let binding = check_original(
        &law,
        target,
        context,
        argument,
        [1.0, 2.0],
        ArithmeticContext::new()?.revision(),
        &mut setup.account.ledger,
    )?;
    for (other_target, other_context, other_argument, other_domain, other_revision) in [
        (context, context, argument, [1.0, 2.0], 1),
        (target, target, argument, [1.0, 2.0], 1),
        (target, context, target, [1.0, 2.0], 1),
        (target, context, argument, [1.0, 1.0], 1),
        (target, context, argument, [1.0, 2.0], 2),
    ] {
        assert!(matches!(
            check_original(
                &law,
                other_target,
                other_context,
                other_argument,
                other_domain,
                other_revision,
                &mut setup.account.ledger,
            ),
            Err(ApplicationError::Loan(LoanError::WrongLoan))
        ));
    }
    let operation = &setup.request.operation;
    check_regions_and_symbols(
        &operation.operation.inputs()[0],
        &operation.operation.outputs()[0],
        &operation.source,
        &operation.destination,
        operation.axis,
        &mut setup.account.ledger,
    )?;
    assert!(matches!(
        check_regions_and_symbols(
            &operation.operation.inputs()[0],
            &operation.operation.outputs()[0],
            &operation.destination,
            &operation.source,
            operation.axis,
            &mut setup.account.ledger,
        ),
        Err(ApplicationError::Loan(LoanError::UnsupportedContract))
    ));
    check_current(&setup.heap, &law, binding, setup.account.identity)?;
    let second = setup
        .heap
        .retain_retained_dyadic_power(&law, &mut setup.account)?;
    assert!(matches!(
        check_current(&setup.heap, &second, binding, setup.account.identity),
        Err(ApplicationError::Loan(LoanError::WrongLoan))
    ));
    let foreign = crate::ExecutionAccount::new(Budget::new(laws::limits())?)?;
    assert!(matches!(
        check_current(&setup.heap, &law, binding, foreign.identity),
        Err(ApplicationError::Loan(LoanError::ForeignAccount))
    ));
    let other = crate::Heap::new(HeapLimits::default())?;
    assert!(check_current(&other, &law, binding, setup.account.identity).is_err());
    let Fixture {
        mut heap,
        mut account,
        mut request,
    } = fixture(1, 1, &[0], |_, _| Some(1.0))?;
    let second = heap.retain_retained_dyadic_power(&request.law, &mut account)?;
    request.operation.binding = Binding::of(&second)?;
    let input_ticket = request.input.ticket()?;
    let injection = crate::loan::fault::Injection::after(0);
    crate::loan::fault::reset_reads();
    let returned = match heap.request_pointwise_dyadic_power(request, &mut account) {
        DyadicPowerApplicationRequest::ApplicationRefused(
            ApplicationError::Loan(LoanError::WrongLoan),
            returned,
        ) => returned,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(returned.input.ticket()?, input_ticket);
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    assert_eq!(crate::loan::fault::reads(), 0);
    drop((injection, second, returned));
    Ok(())
}

/// Derive admission Work from the dyadic request's common geometry and retained-use charges.
/// The three-channel, 23-debit, five-role fixture has six replacement allocations.
fn dyadic_admission_work(cells: usize, components: usize) -> usize {
    let length = cells * components;
    let cell_words = cells.div_ceil(64);
    let component_words = length.div_ceil(64);
    // Plan::new's thirteen common owners, five roles per occupied word class,
    // two nonempty occupations, the attachment shell and six vacant replacements.
    let occupied = usize::from(cells != 0);
    let allocations = 13 + 5 * (3 * occupied) + 2 * occupied + 1 + 6;
    // admission/plan.rs::new charges the channels, member debits and six frame records.
    let records = 3 + 23 + 6;
    // admission/plan.rs::new counts destination, source and prepared-owner edges.
    let edges = 3 + 3 + 1 + 7 + 7;
    // The final two terms are the dyadic use check and its twice-funded validation.
    let retained_use = crate::loan::retained_dyadic_power::Attachment::WORK;
    // contract.rs::check and admission/plan.rs::new charge source, loan and slot checks.
    let checks = 3 + 24 + 1 + (3 + 2 * 3) + 1 + (1 + 2 + 2 * 9) + retained_use + 2 * retained_use;
    // admission/plan.rs::prepay charges acquired-prefix cleanup and preparation.
    let cleanup = allocations + records + edges + (15 + 4 * 3);
    let preparation = records + edges + 1 + (1 + 1 + 2);
    // admission/plan.rs::prepay charges role, mask and attachment initialization.
    checks
        + cleanup
        + preparation
        + 6
        + 1
        + 2 * length
        + 5 * (length + component_words + cell_words)
        + 5 * cell_words
        + cells
}

/// Derive the charged reader visits for a three-channel retained publication.
fn dyadic_reader_work(historical: bool) -> usize {
    if historical {
        // Pin, retained-law presence, and prepared correspondence.
        3 + 1 + 3
    } else {
        // Pin record, owner header, endpoint record, descriptor keys, mask and reader.
        let channels = 3;
        // RetainedLawAttachment::validation_work selects this dyadic charge at the shared reader.
        let owner = channels + 2 + 5 + 1 + crate::loan::retained_dyadic_power::Attachment::WORK;
        3 + owner + (channels + 1 + 3) + channels + 3 + 3
    }
}

/// Checked inventories, isolated internal byte thresholds with active reservations and fresh public reader Work have exact neighbours.
/// # Panics
/// Panics only if a stage, admission or reader authorization boundary changes.
/// ´claim:matrix:dyadic-application-resources´
/// ´test:unit:dyadic-application-resources´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "independent inventory and public admission boundaries share one resource control"
)]
fn dyadic_application_resources() -> TestResult {
    use crate::loan::retained_expm1::tests::memos;
    use nrz_bindings::readings::algebra::{Exhaustion, FaultKind, Location, limits::BudgetKind};
    /// Fixed location of the active caller reservation.
    const BYTE_AT: Location = Location::new("dyadic application caller prefix");
    /// Active caller bytes outside the persistent and staged admission peak.
    const PREFIX: u64 = 1_000_000;
    /// Fixed caller location for the public admission Work boundary.
    const AT: Location = Location::new("dyadic application caller Work");
    assert_eq!(invocation_units([0, 0, 0])?, 6);
    assert_eq!(invocation_units([6, 3, 2])?, 17);
    assert!(matches!(
        invocation_units([usize::MAX, 0, 0]),
        Err(LoanError::CountOverflow)
    ));
    assert_eq!(allowances([0, 0, 0], 0, 1)?, (0, 6, 6));
    assert_eq!(allowances([0, 0, 0], 1, 1)?, (6, 6, 6));
    assert_eq!(allowances([6, 3, 2], 3, 2)?, (102, 34, 34));
    assert_eq!(allowances([0, 0, 0], 65, 1)?, (390, 6, 325));
    assert!(matches!(
        allowances([6, 3, 2], usize::MAX, 2),
        Err(LoanError::CountOverflow)
    ));
    let (_, one_cell, need) = allowances([0, 0, 0], 0, 1)?;
    assert_eq!(one_cell, 6);
    assert!(ExecutionLimits::new(1, need, need).check(1, need).is_ok());
    assert!(
        ExecutionLimits::new(1, need - 1, need)
            .check(1, need)
            .is_err()
    );
    assert!(
        ExecutionLimits::new(1, need, need - 1)
            .check(1, need)
            .is_err()
    );
    assert!(ExecutionLimits::new(1, 0, 0).check(1, need).is_err());
    let mut heap = crate::Heap::new(HeapLimits::default())?;
    let mut account = crate::ExecutionAccount::new(Budget::new(laws::limits())?)?;
    let law = law_on(&mut heap, &mut account)?;
    assert_eq!(law.schedule_inventory()?, [1, 0, 0]);
    assert_eq!(invocation_units(law.schedule_inventory()?)?, 7);
    let mut setup = super::super::tests::expm1::fixture(1, 1, [-1.0, 0.0], 1, false)?;
    let law = law_on(&mut setup.heap, &mut setup.account)?;
    let operation = checked_on(&law, &mut setup)?;
    let crate::binding::NativeRecipe::Kernel(numerical) = &operation.operation.recipe else {
        return Err("dyadic kernel".into());
    };
    let injection = crate::loan::fault::Injection::after(0);
    assert!(matches!(
        numerical.physical_scratch::<f64, f64>(0),
        Err(crate::OperationError::Kernel(crate::KernelError::Contract))
    ));
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    drop(injection);
    // The dyadic plan funds its own use check and validation under
    // ``rule:nrz:matrix-loans-dyadic-application-policy``; native evaluation is later.
    for (cells, components, expected_work) in [
        (0, 1, 233_u64),
        (1, 1, 273),
        (7, 1, 321),
        (8, 1, 329),
        (9, 1, 337),
        (65, 3, 1720),
    ] {
        let selected = if cells == 0 { Vec::new() } else { vec![0] };
        let Fixture {
            mut heap,
            mut account,
            request,
        } = fixture(cells, components, &selected, |_, _| Some(1.0))?;
        let expected = independent_application_bytes(cells, components);
        let before_bytes = heap.resource_bytes()?;
        let before_work = account.usage().work;
        let owner = match heap.request_pointwise_dyadic_power(request, &mut account) {
            DyadicPowerApplicationRequest::ApplicationReady(owner) => owner,
            other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        };
        let measured = heap.resource_bytes()? - before_bytes;
        let work = account.usage().work - before_work;
        assert_eq!(measured, expected);
        let derived_work = u64::try_from(dyadic_admission_work(cells, components))?;
        assert_eq!(derived_work, expected_work);
        assert_eq!(work, derived_work);
        assert!(matches!(
            heap.cancel_application(owner, &mut account),
            crate::ApplicationCancellation::ApplicationCancelled(_)
        ));
        for short in [0, 1] {
            let Fixture {
                mut heap,
                mut account,
                request,
            } = fixture(cells, components, &selected, |_, _| Some(1.0))?;
            let available = expected_work - short;
            let prior = account.limits().work - account.usage().work - available;
            account.ledger.work(prior, AT)?;
            let before = account.usage().work;
            let input_ticket = request.input.ticket()?;
            match heap.request_pointwise_dyadic_power(request, &mut account) {
                DyadicPowerApplicationRequest::ApplicationReady(owner) => {
                    assert_eq!(short, 0);
                    assert_eq!(account.usage().work - before, expected_work);
                    drop(owner);
                }
                DyadicPowerApplicationRequest::ApplicationRefused(
                    ApplicationError::Loan(LoanError::Budget(fault)),
                    returned,
                ) => {
                    assert_eq!(short, 1);
                    assert!(matches!(
                        fault.kind,
                        FaultKind::Exhausted {
                            budget: BudgetKind::Work,
                            ..
                        }
                    ));
                    assert_eq!(returned.input.ticket()?, input_ticket);
                }
                other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
                    return Err(format!("{other:?}").into());
                }
            }
        }
    }
    for (cells, components) in [(0, 1), (1, 1), (8, 1), (9, 1), (65, 3)] {
        let selected = if cells == 0 { Vec::new() } else { vec![0] };
        let Fixture { heap, request, .. } =
            fixture(cells, components, &selected, |_, _| Some(1.0))?;
        let retained = independent_application_bytes(cells, components);
        let old = 256 + crate::loan::abi::group_row() + 3 * 32 + 9 * 264 + 40;
        let temporary = 23 * 264 + 5 * 16 + 3 * 248 + 2 * 24;
        assert_eq!(temporary, 6944);
        let staged = retained + 2 * old + temporary;
        for (is_staged, required) in [(false, retained), (true, staged)] {
            for short in [0, 1] {
                let mut limits = laws::limits();
                if is_staged {
                    limits.staged_payload_bytes = u64::try_from(required - short)?;
                } else {
                    limits.output_bytes = u64::try_from(required - short)?;
                }
                let mut budget = Budget::new(limits)?;
                let injection = crate::loan::fault::Injection::after(0);
                let result =
                    super::super::admission::check_plan_dyadic(&heap, &request, &mut budget);
                assert_eq!(result.is_ok(), short == 0);
                assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                drop(injection);
            }
        }
    }
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture(1, 1, &[0], |_, _| Some(1.0))?;
    let owner = match heap.request_pointwise_dyadic_power(request, &mut account) {
        DyadicPowerApplicationRequest::ApplicationReady(owner) => owner,
        other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    for short in [0, 1] {
        let mut limits = laws::limits();
        limits.staged_payload_bytes = 64 - short;
        let mut budget = Budget::new(limits)?;
        let injection = crate::loan::fault::Injection::after(0);
        let result = super::super::publication::shell_preflight(&heap, &owner.core, &mut budget);
        assert_eq!(result.is_ok(), short == 0);
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        drop(injection);
    }
    // The same internal byte guards compose with a live caller and populated CAS memos.
    let Fixture {
        heap: planning_heap,
        request: planning_request,
        ..
    } = fixture(1, 1, &[0], |_, _| Some(1.0))?;
    let (mut cas, original, reading, _) = memos::seed()?;
    memos::populate(
        &mut cas,
        &original,
        &reading,
        &mut Budget::new(crate::loan::retained_expm1::tests::limits())?,
    )?;
    let persistent = cas.retained_judgment_bytes() + 64 + memos::memo_bytes()?;
    assert!(persistent > 0);
    for shell in [false, true] {
        let staged = u64::try_from(if shell {
            64
        } else {
            let retained = independent_application_bytes(1, 1);
            let old = 256 + crate::loan::abi::group_row() + 3 * 32 + 9 * 264 + 40;
            let temporary = 23 * 264 + 5 * 16 + 3 * 248 + 2 * 24;
            retained + 2 * old + temporary
        })?;
        for short in [0, 1] {
            let boundary = PREFIX + persistent + staged;
            let mut budget = Budget::new(nrz_bindings::Limits {
                staged_payload_bytes: boundary - short,
                ..crate::loan::retained_expm1::tests::limits()
            })?;
            memos::populate(&mut cas, &original, &reading, &mut budget)?;
            budget.work(7, BYTE_AT)?;
            let before_work = budget.usage().work;
            let before_bytes = if shell {
                heap.resource_bytes()?
            } else {
                planning_heap.resource_bytes()?
            };
            let injection = crate::loan::fault::Injection::after(0);
            let result = budget.with_retained_payload(PREFIX, BYTE_AT, |budget| {
                if shell {
                    super::super::publication::shell_preflight(&heap, &owner.core, budget)
                } else {
                    super::super::admission::check_plan_dyadic(
                        &planning_heap,
                        &planning_request,
                        budget,
                    )
                }
            });
            assert_eq!(result.is_ok(), short == 0);
            if let Err(error) = result {
                let LoanError::Budget(fault) = error else {
                    return Err("ledger-aware dyadic byte refusal".into());
                };
                assert_eq!(
                    fault.kind,
                    FaultKind::Exhausted {
                        reason: Exhaustion::Limit,
                        budget: BudgetKind::StagedPayloadBytes,
                        limit: persistent + staged - 1,
                        needed: persistent + staged,
                    }
                );
                assert_eq!(fault.location, crate::loan::AT);
                assert!(fault.series_evidence.is_none());
            }
            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
            assert!(budget.usage().work > before_work);
            assert_eq!(budget.limits().staged_payload_bytes, boundary - short);
            assert_eq!(
                if shell {
                    heap.resource_bytes()?
                } else {
                    planning_heap.resource_bytes()?
                },
                before_bytes
            );
            drop(injection);
        }
    }
    for (cells, components) in [(1, 1), (65, 3)] {
        let needed = (7 * components).max(5 * cells * components);
        for limit in 0..3 {
            for short in [0, 1] {
                let selected = [0];
                let Fixture {
                    mut heap,
                    mut account,
                    mut request,
                } = fixture(cells, components, &selected, |_, _| Some(1.0))?;
                let mut values = [1, needed, needed];
                values[limit] -= short;
                request.limits = ExecutionLimits::new(values[0], values[1], values[2]);
                let injection = crate::loan::fault::Injection::after(usize::MAX);
                match heap.request_pointwise_dyadic_power(request, &mut account) {
                    DyadicPowerApplicationRequest::ApplicationReady(owner) => {
                        assert_eq!(short, 0);
                        drop(owner);
                    }
                    DyadicPowerApplicationRequest::ApplicationRefused(
                        ApplicationError::Operation(crate::OperationError::ResourceLimit),
                        _,
                    ) => {
                        assert_eq!(short, 1);
                        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                    }
                    other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
                        return Err(format!("{other:?}").into());
                    }
                }
                drop(injection);
            }
        }
    }
    // Each reader starts from a fresh public publication and the original account.
    for historical in [false, true] {
        for short in [0, 1] {
            let (heap, mut account, tile) = published_one()?;
            let required = u64::try_from(dyadic_reader_work(historical))?;
            assert_eq!(required, if historical { 7 } else { 33 });
            let ticket = tile.pin().ticket()?;
            leave_work(&mut account, required - short)?;
            let before = account.usage().work;
            crate::loan::fault::reset_reads();
            let result = if historical {
                tile.pin().historical_reader(&mut account)
            } else {
                tile.reader(&heap, &mut account)
            };
            if short == 0 {
                assert!(result.is_ok());
                assert_eq!(account.usage().work - before, required);
            } else {
                assert!(matches!(
                    result,
                    Err(LoanError::Budget(fault))
                        if matches!(fault.kind, FaultKind::Exhausted { budget: BudgetKind::Work, .. })
                ));
            }
            assert_eq!(crate::loan::fault::reads(), 0);
            assert_eq!(tile.pin().ticket()?, ticket);
        }
    }
    for short in [0, 1] {
        let (heap, mut account, tile) = published_one()?;
        let reader = tile.reader(&heap, &mut account)?;
        leave_work(&mut account, 2 - short)?;
        let before = account.usage().work;
        crate::loan::fault::reset_reads();
        let result = reader.dyadic_power_endpoints(0, 0, &mut account);
        if short == 0 {
            assert_eq!(result?, [1.0, 1.0]);
            assert_eq!(account.usage().work - before, 2);
            assert_eq!(crate::loan::fault::reads(), 2);
        } else {
            assert!(matches!(
                result,
                Err(LoanError::Budget(fault))
                    if matches!(fault.kind, FaultKind::Exhausted { budget: BudgetKind::Work, .. })
            ));
            assert_eq!(crate::loan::fault::reads(), 0);
        }
    }
    for short in [0, 1] {
        let (heap, mut account, tile) = published_one()?;
        let reader = tile.reader(&heap, &mut account)?;
        leave_work(&mut account, 1 - short)?;
        let before = account.usage().work;
        crate::loan::fault::reset_reads();
        let result = reader.application_outcome(0, &mut account);
        if short == 0 {
            assert!(matches!(result?, crate::CellOutcome::Completed));
            assert_eq!(account.usage().work - before, 1);
        } else {
            assert!(matches!(
                result,
                Err(LoanError::Budget(fault))
                    if matches!(fault.kind, FaultKind::Exhausted { budget: BudgetKind::Work, .. })
            ));
        }
        assert_eq!(crate::loan::fault::reads(), 0);
    }
    Ok(())
}

/// Independent rounded backings, five roles, fixed headers and table growth for one application.
fn independent_application_bytes(cells: usize, components: usize) -> usize {
    let length = cells * components;
    let words = cells.div_ceil(64);
    let component_words = length.div_ceil(64);
    let rounded = 8 * length.div_ceil(8);
    let growth = 3 * (256 + crate::loan::abi::group_row()) + 2 * 32 + 21 * 264 + 40;
    5 * (8 * rounded + 8 * component_words + 8 * words)
        + 8 * words
        + cells * crate::loan::abi::outcome_width()
        + crate::loan::abi::application_headers::<f64>()
        + crate::loan::abi::retained_law_attachment_bytes()
        + growth
}

/// A self-valid expm1 attachment cannot replace a dyadic use at either detached guard.
fn private_retained_substitution(publish: bool) -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture_for(
        1,
        1,
        &[0],
        |_, _| Some(1.0),
        1,
        2,
        [1.0, 2.0],
        Some(crate::loan::retained_expm1::tests::limits()),
    )?;
    let foreign = crate::loan::retained_expm1::tests::prepare(&mut heap, &mut account)?;
    let attachment = crate::loan::retained_expm1::tests::attach(foreign, &heap, &mut account)?;
    let mut owner = match heap.request_pointwise_dyadic_power(request, &mut account) {
        DyadicPowerApplicationRequest::ApplicationReady(owner) => owner,
        other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    if publish {
        owner = match owner.evaluate(&mut account) {
            super::super::ApplicationEvaluation::ApplicationCandidate(owner) => owner,
            other => return Err(format!("{other:?}").into()),
        };
    }
    let original = owner
        .core
        .host
        .retained_law
        .take()
        .ok_or("original attachment")?;
    owner.core.host.retained_law = Some(Box::new(
        crate::loan::retained_law::RetainedLawAttachment::wrap_expm1(attachment),
    ));
    crate::loan::fault::reset_reads();
    crate::loan::fault::reset_scalars();
    let injection = crate::loan::fault::Injection::after(0);
    let mut returned = if publish {
        match heap.publish_application(owner, &mut account) {
            super::super::ApplicationPublication::ApplicationRejected(
                ApplicationError::Loan(LoanError::WrongLoan),
                owner,
            ) => owner,
            other => return Err(format!("{other:?}").into()),
        }
    } else {
        match owner.evaluate(&mut account) {
            super::super::ApplicationEvaluation::ApplicationNotStarted(
                ApplicationError::Loan(LoanError::WrongLoan),
                owner,
            ) => owner,
            other => return Err(format!("{other:?}").into()),
        }
    };
    assert_eq!(crate::loan::fault::reads(), 0);
    assert_eq!(crate::loan::fault::scalars(), 0);
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    drop(injection);
    returned.core.host.retained_law = Some(original);
    drop(returned);
    Ok(())
}

/// A separately checked dyadic attachment cannot replace expm1 at either detached guard.
fn private_expm1_substitution(publish: bool) -> TestResult {
    let mut setup = super::super::tests::expm1::fixture(1, 1, [-1.0, 0.0], 1, false)?;
    let foreign = law_on(&mut setup.heap, &mut setup.account)?;
    let mut shell = crate::loan::retained_law::RetainedLawAttachment::empty();
    shell.install_dyadic_power(foreign);
    let mut owner = match setup
        .heap
        .request_pointwise_expm1(setup.request, &mut setup.account)
    {
        crate::Expm1ApplicationRequest::ApplicationReady(owner) => owner,
        other @ crate::Expm1ApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    if publish {
        owner = match owner.evaluate(&mut setup.account) {
            super::super::ApplicationEvaluation::ApplicationCandidate(owner) => owner,
            other => return Err(format!("{other:?}").into()),
        };
    }
    let original = owner
        .core
        .host
        .retained_law
        .take()
        .ok_or("original attachment")?;
    owner.core.host.retained_law = Some(Box::new(shell));
    crate::loan::fault::reset_reads();
    crate::loan::fault::reset_scalars();
    let injection = crate::loan::fault::Injection::after(0);
    let mut returned = if publish {
        match setup.heap.publish_application(owner, &mut setup.account) {
            super::super::ApplicationPublication::ApplicationRejected(
                ApplicationError::Loan(LoanError::WrongLoan),
                owner,
            ) => owner,
            other => return Err(format!("{other:?}").into()),
        }
    } else {
        match owner.evaluate(&mut setup.account) {
            super::super::ApplicationEvaluation::ApplicationNotStarted(
                ApplicationError::Loan(LoanError::WrongLoan),
                owner,
            ) => owner,
            other => return Err(format!("{other:?}").into()),
        }
    };
    assert_eq!(crate::loan::fault::reads(), 0);
    assert_eq!(crate::loan::fault::scalars(), 0);
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    drop(injection);
    returned.core.host.retained_law = Some(original);
    drop(returned);
    Ok(())
}

/// A checked square-root recipe and a checked half-power cannot borrow each other's member seal.
fn private_root_substitution(dyadic_owner: bool, publish: bool) -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture(1, 1, &[0], |_, _| Some(1.0))?;
    let dyadic_operation = request.operation.operation.clone();
    let root_operation = crate::OperationDraft::map_elementwise(
        dyadic_operation.inputs()[0].shape().clone(),
        dyadic_operation.outputs()[0].shape().clone(),
        crate::Elementwise::SquareRoot,
    )?
    .check(ExecutionLimits::default())?;
    let (mut owner, replacement) = if dyadic_owner {
        let owner = match heap.request_pointwise_dyadic_power(request, &mut account) {
            DyadicPowerApplicationRequest::ApplicationReady(owner) => owner,
            other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        };
        (owner, root_operation)
    } else {
        let root_request = crate::PointwiseRequest {
            operation: root_operation,
            input: request.input,
            destination: request.destination,
            demand: request.demand,
            limits: request.limits,
        };
        let owner = match heap.request_pointwise_square_root(root_request, &mut account) {
            super::super::ApplicationRequest::ApplicationReady(owner) => owner,
            other @ super::super::ApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        };
        (owner, dyadic_operation)
    };
    if publish {
        owner = match owner.evaluate(&mut account) {
            super::super::ApplicationEvaluation::ApplicationCandidate(candidate) => candidate,
            other => return Err(format!("{other:?}").into()),
        };
    }
    let original = std::mem::replace(&mut owner.core.evidence.operation, replacement);
    crate::loan::fault::reset_reads();
    crate::loan::fault::reset_scalars();
    let injection = crate::loan::fault::Injection::after(0);
    let mut returned = if publish {
        match heap.publish_application(owner, &mut account) {
            super::super::ApplicationPublication::ApplicationRepair(
                ApplicationError::Loan(LoanError::UnsupportedContract),
                returned,
            ) => returned,
            other => return Err(format!("{other:?}").into()),
        }
    } else {
        match owner.evaluate(&mut account) {
            super::super::ApplicationEvaluation::ApplicationNotStarted(
                ApplicationError::Loan(LoanError::UnsupportedContract),
                returned,
            ) => returned,
            other => return Err(format!("{other:?}").into()),
        }
    };
    assert_eq!(crate::loan::fault::reads(), 0);
    assert_eq!(crate::loan::fault::scalars(), 0);
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    drop(injection);
    returned.core.evidence.operation = original;
    Ok(())
}

/// Replacing the retained law in a published dyadic result refuses both reader routes.
fn private_dyadic_reader_substitution() -> TestResult {
    use std::rc::Rc;
    let (mut heap, mut account, mut tile) =
        published_one_for(Some(crate::loan::retained_expm1::tests::limits()))?;
    let foreign = crate::loan::retained_expm1::tests::prepare(&mut heap, &mut account)?;
    let attachment = crate::loan::retained_expm1::tests::attach(foreign, &heap, &mut account)?;
    let root = tile.pin().ticket()?.identity.root.slot;
    let pin_slot = heap.loans.groups[root].as_ref().ok_or("group")?.pin;
    let mut index = heap.loans.pins[pin_slot].take().ok_or("pin index")?;
    index.witness = None;
    heap.loans.groups[root].as_mut().ok_or("group")?.state = crate::loan::registry::State::Reserved;
    let pin = Rc::get_mut(&mut tile.pin.record)
        .and_then(|record| record.payload.as_mut())
        .ok_or("unique pin")?;
    let bundle = Rc::get_mut(&mut pin.bundle)
        .and_then(|owner| owner.payload.as_mut())
        .ok_or("unique bundle")?;
    let original = bundle.retained_law.replace(Box::new(
        crate::loan::retained_law::RetainedLawAttachment::wrap_expm1(attachment),
    ));
    heap.loans.groups[root].as_mut().ok_or("group")?.state =
        crate::loan::registry::State::Published(Rc::clone(&pin.bundle));
    index.witness = Some(Rc::downgrade(&tile.pin.record));
    heap.loans.pins[pin_slot] = Some(index);
    crate::loan::fault::reset_reads();
    assert!(matches!(
        tile.reader(&heap, &mut account),
        Err(LoanError::WrongLoan)
    ));
    assert_eq!(crate::loan::fault::reads(), 0);
    drop(heap);
    assert!(matches!(
        tile.pin().historical_reader(&mut account),
        Err(LoanError::WrongLoan)
    ));
    assert_eq!(crate::loan::fault::reads(), 0);
    // The owner is restored before the privately substituted bundle is released.
    let record = Rc::get_mut(&mut tile.pin.record).ok_or("unique pin")?;
    let bundle = Rc::get_mut(&mut record.payload.as_mut().ok_or("pin")?.bundle)
        .and_then(|owner| owner.payload.as_mut())
        .ok_or("unique bundle")?;
    bundle.retained_law = original;
    Ok(())
}

/// Replacing a published expm1 law with dyadic authority refuses both reader routes.
fn private_expm1_reader_substitution() -> TestResult {
    use std::rc::Rc;
    let mut setup = super::super::tests::expm1::fixture(1, 1, [-1.0, 0.0], 1, false)?;
    let foreign = law_on(&mut setup.heap, &mut setup.account)?;
    let mut shell = crate::loan::retained_law::RetainedLawAttachment::empty();
    shell.install_dyadic_power(foreign);
    let owner = match setup
        .heap
        .request_pointwise_expm1(setup.request, &mut setup.account)
    {
        crate::Expm1ApplicationRequest::ApplicationReady(owner) => owner,
        other @ crate::Expm1ApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    let candidate = match owner.evaluate(&mut setup.account) {
        super::super::ApplicationEvaluation::ApplicationCandidate(candidate) => candidate,
        other => return Err(format!("{other:?}").into()),
    };
    let mut tile = match setup
        .heap
        .publish_application(candidate, &mut setup.account)
    {
        super::super::ApplicationPublication::ApplicationPublished(tile, _) => tile,
        other => return Err(format!("{other:?}").into()),
    };
    let root = tile.pin().ticket()?.identity.root.slot;
    let pin_slot = setup.heap.loans.groups[root].as_ref().ok_or("group")?.pin;
    let mut index = setup.heap.loans.pins[pin_slot].take().ok_or("pin index")?;
    index.witness = None;
    setup.heap.loans.groups[root].as_mut().ok_or("group")?.state =
        crate::loan::registry::State::Reserved;
    let pin = Rc::get_mut(&mut tile.pin.record)
        .and_then(|record| record.payload.as_mut())
        .ok_or("unique pin")?;
    let bundle = Rc::get_mut(&mut pin.bundle)
        .and_then(|owner| owner.payload.as_mut())
        .ok_or("unique bundle")?;
    let original = bundle.retained_law.replace(Box::new(shell));
    setup.heap.loans.groups[root].as_mut().ok_or("group")?.state =
        crate::loan::registry::State::Published(Rc::clone(&pin.bundle));
    index.witness = Some(Rc::downgrade(&tile.pin.record));
    setup.heap.loans.pins[pin_slot] = Some(index);
    crate::loan::fault::reset_reads();
    assert!(matches!(
        tile.reader(&setup.heap, &mut setup.account),
        Err(LoanError::WrongLoan)
    ));
    assert_eq!(crate::loan::fault::reads(), 0);
    drop(setup.heap);
    assert!(matches!(
        tile.pin().historical_reader(&mut setup.account),
        Err(LoanError::WrongLoan)
    ));
    assert_eq!(crate::loan::fault::reads(), 0);
    let record = Rc::get_mut(&mut tile.pin.record).ok_or("unique pin")?;
    let bundle = Rc::get_mut(&mut record.payload.as_mut().ok_or("pin")?.bundle)
        .and_then(|owner| owner.payload.as_mut())
        .ok_or("unique bundle")?;
    bundle.retained_law = original;
    Ok(())
}

/// Published half-power and square-root recipes refuse each other's validated member identity.
fn private_root_reader_substitution(dyadic_owner: bool) -> TestResult {
    use std::rc::Rc;
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture(1, 1, &[0], |_, _| Some(1.0))?;
    let dyadic_operation = request.operation.operation.clone();
    let root_operation = crate::OperationDraft::map_elementwise(
        dyadic_operation.inputs()[0].shape().clone(),
        dyadic_operation.outputs()[0].shape().clone(),
        crate::Elementwise::SquareRoot,
    )?
    .check(ExecutionLimits::default())?;
    let (mut tile, replacement) = if dyadic_owner {
        let owner = match heap.request_pointwise_dyadic_power(request, &mut account) {
            DyadicPowerApplicationRequest::ApplicationReady(owner) => owner,
            other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        };
        let candidate = match owner.evaluate(&mut account) {
            super::super::ApplicationEvaluation::ApplicationCandidate(candidate) => candidate,
            other => return Err(format!("{other:?}").into()),
        };
        let tile = match heap.publish_application(candidate, &mut account) {
            super::super::ApplicationPublication::ApplicationPublished(tile, _) => tile,
            other => return Err(format!("{other:?}").into()),
        };
        (tile, root_operation)
    } else {
        let root_request = crate::PointwiseRequest {
            operation: root_operation,
            input: request.input,
            destination: request.destination,
            demand: request.demand,
            limits: request.limits,
        };
        let owner = match heap.request_pointwise_square_root(root_request, &mut account) {
            super::super::ApplicationRequest::ApplicationReady(owner) => owner,
            other @ super::super::ApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        };
        let candidate = match owner.evaluate(&mut account) {
            super::super::ApplicationEvaluation::ApplicationCandidate(candidate) => candidate,
            other => return Err(format!("{other:?}").into()),
        };
        let tile = match heap.publish_application(candidate, &mut account) {
            super::super::ApplicationPublication::ApplicationPublished(tile, _) => tile,
            other => return Err(format!("{other:?}").into()),
        };
        (tile, dyadic_operation)
    };
    let root = tile.pin().ticket()?.identity.root.slot;
    let pin_slot = heap.loans.groups[root].as_ref().ok_or("group")?.pin;
    let mut index = heap.loans.pins[pin_slot].take().ok_or("pin index")?;
    index.witness = None;
    heap.loans.groups[root].as_mut().ok_or("group")?.state = crate::loan::registry::State::Reserved;
    let pin = Rc::get_mut(&mut tile.pin.record)
        .and_then(|record| record.payload.as_mut())
        .ok_or("unique pin")?;
    let bundle = Rc::get_mut(&mut pin.bundle)
        .and_then(|owner| owner.payload.as_mut())
        .ok_or("unique bundle")?;
    let evidence = bundle
        .origin
        .evidence
        .as_mut()
        .ok_or("application evidence")?;
    let original = std::mem::replace(&mut evidence.operation, replacement);
    heap.loans.groups[root].as_mut().ok_or("group")?.state =
        crate::loan::registry::State::Published(Rc::clone(&pin.bundle));
    index.witness = Some(Rc::downgrade(&tile.pin.record));
    heap.loans.pins[pin_slot] = Some(index);
    crate::loan::fault::reset_reads();
    assert!(matches!(
        tile.reader(&heap, &mut account),
        Err(LoanError::UnsupportedContract)
    ));
    assert_eq!(crate::loan::fault::reads(), 0);
    drop(heap);
    assert!(matches!(
        tile.pin().historical_reader(&mut account),
        Err(LoanError::UnsupportedContract)
    ));
    assert_eq!(crate::loan::fault::reads(), 0);
    let record = Rc::get_mut(&mut tile.pin.record).ok_or("unique pin")?;
    let bundle = Rc::get_mut(&mut record.payload.as_mut().ok_or("pin")?.bundle)
        .and_then(|owner| owner.payload.as_mut())
        .ok_or("unique bundle")?;
    bundle
        .origin
        .evidence
        .as_mut()
        .ok_or("application evidence")?
        .operation = original;
    Ok(())
}

/// Public lifetimes and detached ready, candidate and repair owners retain their funded law.
/// # Panics
/// Panics only if public refusal ownership, use identity or final release changes.
/// ´claim:matrix:dyadic-application-lifecycle´
/// ´test:unit:dyadic-application-lifecycle´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one public lifecycle keeps two funded results through historical reading"
)]
fn dyadic_application_lifecycle() -> TestResult {
    let mut heap = crate::Heap::new(HeapLimits::default())?;
    let mut account = crate::ExecutionAccount::new(Budget::new(laws::limits())?)?;
    let first = law_on(&mut heap, &mut account)?;
    let before = account.usage().work;
    let second = heap.retain_retained_dyadic_power(&first, &mut account)?;
    assert_eq!(account.usage().work - before, 16);
    assert_ne!(Binding::of(&first)?, Binding::of(&second)?);
    assert!(std::ptr::eq(first.authority()?, second.authority()?));
    heap.release_retained_dyadic_power(first, &mut account)
        .map_err(|(error, _)| error)?;
    assert_eq!(
        second
            .evaluate(1.0, &ArithmeticContext::new()?)?
            .reading()
            .endpoints,
        [1.0, 1.0]
    );
    let mut setup = super::super::tests::expm1::fixture(1, 1, [-1.0, 0.0], 1, false)?;
    let law = law_on(&mut setup.heap, &mut setup.account)?;
    let operation = checked_on(&law, &mut setup)?;
    let old = setup.request;
    let request = PointwiseDyadicPowerRequest {
        operation,
        law,
        input: old.input,
        destination: old.destination,
        demand: old.demand,
        limits: old.limits,
    };
    let owner = match setup
        .heap
        .request_pointwise_dyadic_power(request, &mut setup.account)
    {
        DyadicPowerApplicationRequest::ApplicationReady(owner) => owner,
        other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    let candidate = match owner.evaluate(&mut setup.account) {
        super::super::ApplicationEvaluation::ApplicationCandidate(candidate) => candidate,
        other => return Err(format!("{other:?}").into()),
    };
    let (tile, _) = match setup
        .heap
        .publish_application(candidate, &mut setup.account)
    {
        super::super::ApplicationPublication::ApplicationPublished(tile, receipt) => {
            (tile, receipt)
        }
        other => return Err(format!("{other:?}").into()),
    };
    let reader = tile.reader(&setup.heap, &mut setup.account)?;
    assert!(matches!(
        reader.application_outcome(0, &mut setup.account)?,
        crate::CellOutcome::DyadicPowerOutsideDomain { component: 0 }
    ));
    let mut setup = fixture(1, 1, &[0], |_, _| Some(1.0))?;
    let other = additional(&mut setup)?;
    assert_ne!(Binding::of(&setup.request.law)?, Binding::of(&other.law)?);
    assert!(std::ptr::eq(
        setup.request.law.authority()?,
        other.law.authority()?
    ));
    let Fixture {
        mut heap,
        mut account,
        request,
    } = setup;
    let first_owner = match heap.request_pointwise_dyadic_power(request, &mut account) {
        DyadicPowerApplicationRequest::ApplicationReady(owner) => owner,
        other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    let first_candidate = match first_owner.evaluate(&mut account) {
        super::super::ApplicationEvaluation::ApplicationCandidate(candidate) => candidate,
        other => return Err(format!("{other:?}").into()),
    };
    let (first, _) = match heap.publish_application(first_candidate, &mut account) {
        super::super::ApplicationPublication::ApplicationPublished(tile, receipt) => {
            (tile, receipt)
        }
        other => return Err(format!("{other:?}").into()),
    };
    let second_owner = match heap.request_pointwise_dyadic_power(other, &mut account) {
        DyadicPowerApplicationRequest::ApplicationReady(owner) => owner,
        other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    let second_candidate = match second_owner.evaluate(&mut account) {
        super::super::ApplicationEvaluation::ApplicationCandidate(candidate) => candidate,
        other => return Err(format!("{other:?}").into()),
    };
    let (second, _) = match heap.publish_application(second_candidate, &mut account) {
        super::super::ApplicationPublication::ApplicationPublished(tile, receipt) => {
            (tile, receipt)
        }
        other => return Err(format!("{other:?}").into()),
    };
    drop(heap);
    drop(first);
    let historical = second.pin().historical_reader(&mut account)?;
    assert_eq!(
        historical.dyadic_power_endpoints(0, 0, &mut account)?,
        [1.0, 1.0]
    );
    drop(second);
    repair_and_reclaim()?;
    for state in 0..3 {
        for unwind in [false, true] {
            let Fixture {
                mut heap,
                mut account,
                request,
            } = fixture(1, 1, &[0], |_, _| Some(1.0))?;
            let destroyed = laws::destruction_witness(&request.law)?;
            let mut owner = match heap.request_pointwise_dyadic_power(request, &mut account) {
                DyadicPowerApplicationRequest::ApplicationReady(owner) => owner,
                other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
                    return Err(format!("{other:?}").into());
                }
            };
            if state > 0 {
                owner = match owner.evaluate(&mut account) {
                    super::super::ApplicationEvaluation::ApplicationCandidate(owner) => owner,
                    other => return Err(format!("{other:?}").into()),
                };
            }
            if state == 2 {
                let injection = crate::loan::fault::Injection::after(0);
                owner = match heap.publish_application(owner, &mut account) {
                    super::super::ApplicationPublication::ApplicationRepair(
                        ApplicationError::Loan(LoanError::AllocationRefused),
                        owner,
                    ) => owner,
                    other => return Err(format!("{other:?}").into()),
                };
                drop(injection);
            }
            drop(heap);
            assert!(!destroyed());
            let before_work = account.usage().work;
            let released = crate::loan::fault::numerical_releases();
            if unwind {
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _owner = owner;
                        panic!("dyadic application unwind fixture");
                    }))
                    .is_err()
                );
            } else {
                drop(owner);
            }
            assert!(destroyed());
            assert_eq!(
                crate::loan::fault::numerical_releases(),
                [released[0] + 6, released[1] + 6, released[2] + 6 * 64]
            );
            assert_eq!(account.usage().work, before_work);
        }
    }
    Ok(())
}

/// A failed publication retains the completed values and the registry supports current then historical reads.
/// # Panics
/// Panics only if repair reevaluates or the retained result loses its exact funded use.
fn repair_and_reclaim() -> TestResult {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture(1, 1, &[0], |_, _| Some(1.0))?;
    let demand = request.demand;
    let owner = match heap.request_pointwise_dyadic_power(request, &mut account) {
        DyadicPowerApplicationRequest::ApplicationReady(owner) => owner,
        other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    heap.retire_mask(demand, &mut account.ledger)?;
    crate::loan::fault::reset_scalars();
    let candidate = match owner.evaluate(&mut account) {
        super::super::ApplicationEvaluation::ApplicationCandidate(candidate) => candidate,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(crate::loan::fault::scalars(), 1);
    let injection = crate::loan::fault::Injection::after(0);
    let candidate = match heap.publish_application(candidate, &mut account) {
        super::super::ApplicationPublication::ApplicationRepair(
            ApplicationError::Loan(LoanError::AllocationRefused),
            owner,
        ) => owner,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(crate::loan::fault::scalars(), 1);
    drop(injection);
    let (tile, receipt) = match heap.publish_application(candidate, &mut account) {
        super::super::ApplicationPublication::ApplicationPublished(tile, receipt) => {
            (tile, receipt)
        }
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(crate::loan::fault::scalars(), 1);
    let ticket = receipt.ticket();
    drop(tile);
    let pin = heap.acquire_pin::<f64>(ticket, &mut account)?;
    heap.current_bundle(&pin, &mut account.ledger)?;
    assert_eq!(
        pin.checked_reader()?
            .dyadic_power_endpoints(0, 0, &mut account)?,
        [1.0, 1.0]
    );
    drop(heap);
    assert_eq!(
        pin.historical_reader(&mut account)?
            .dyadic_power_endpoints(0, 0, &mut account)?,
        [1.0, 1.0]
    );
    Ok(())
}

/// Selected and unselected cells retain their distinct public reading states.
/// # Panics
/// Panics only if the selected result or an unselected cell changes its public state.
/// ´claim:matrix:dyadic-application-masks-and-readings´
/// ´test:unit:dyadic-application-masks-and-readings´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the selected mask boundaries and holes share one public traversal"
)]
fn dyadic_application_masks_and_readings() -> TestResult {
    for cells in [0, 1, 63, 64, 65, 128] {
        for components in [1, 3] {
            let selected: Vec<_> = if cells == 0 {
                Vec::new()
            } else if cells > 64 {
                vec![0, 64]
            } else {
                vec![0]
            };
            let Fixture {
                mut heap,
                mut account,
                request,
            } = fixture(cells, components, &selected, |_, _| Some(1.0))?;
            let owner = match heap.request_pointwise_dyadic_power(request, &mut account) {
                DyadicPowerApplicationRequest::ApplicationReady(owner) => owner,
                other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
                    return Err(format!("{other:?}").into());
                }
            };
            let candidate = match owner.evaluate(&mut account) {
                super::super::ApplicationEvaluation::ApplicationCandidate(candidate) => candidate,
                other => return Err(format!("{other:?}").into()),
            };
            let (tile, _) = match heap.publish_application(candidate, &mut account) {
                super::super::ApplicationPublication::ApplicationPublished(tile, receipt) => {
                    (tile, receipt)
                }
                other => return Err(format!("{other:?}").into()),
            };
            let reader = tile.reader(&heap, &mut account)?;
            assert_eq!(reader.capacity(), cells);
            for cell in 0..cells {
                let active = selected.contains(&cell);
                assert_eq!(reader.contains(cell, &mut account)?, active);
                assert_eq!(
                    matches!(
                        reader.application_outcome(cell, &mut account)?,
                        crate::CellOutcome::Completed
                    ),
                    active
                );
                for component in 0..components {
                    if active {
                        for channel in crate::HostChannel::ALL {
                            assert_eq!(
                                reader.read_component(channel, cell, component, &mut account)?,
                                if channel == crate::HostChannel::Center {
                                    1.0
                                } else {
                                    0.0
                                }
                            );
                        }
                        assert_eq!(
                            reader.dyadic_power_endpoints(cell, component, &mut account)?,
                            [1.0, 1.0]
                        );
                    } else {
                        assert!(
                            reader
                                .read_component(
                                    crate::HostChannel::Center,
                                    cell,
                                    component,
                                    &mut account
                                )
                                .is_err()
                        );
                    }
                }
            }
        }
    }
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture(4, 1, &[0, 1, 2, 3], |cell, _| match cell {
        1 => None,
        2 => Some(-1.0),
        _ => Some(1.0),
    })?;
    let owner = match heap.request_pointwise_dyadic_power(request, &mut account) {
        DyadicPowerApplicationRequest::ApplicationReady(owner) => owner,
        other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    let candidate = match owner.evaluate(&mut account) {
        super::super::ApplicationEvaluation::ApplicationCandidate(candidate) => candidate,
        other => return Err(format!("{other:?}").into()),
    };
    let (tile, _) = match heap.publish_application(candidate, &mut account) {
        super::super::ApplicationPublication::ApplicationPublished(tile, receipt) => {
            (tile, receipt)
        }
        other => return Err(format!("{other:?}").into()),
    };
    let reader = tile.reader(&heap, &mut account)?;
    assert!(matches!(
        reader.application_outcome(0, &mut account)?,
        crate::CellOutcome::Completed
    ));
    assert!(matches!(
        reader.application_outcome(1, &mut account)?,
        crate::CellOutcome::MissingInput
    ));
    assert!(matches!(
        reader.application_outcome(2, &mut account)?,
        crate::CellOutcome::DyadicPowerOutsideDomain { component: 0 }
    ));
    assert!(matches!(
        reader.application_outcome(3, &mut account)?,
        crate::CellOutcome::Completed
    ));
    assert!(reader.dyadic_power_endpoints(1, 0, &mut account).is_err());
    assert!(reader.dyadic_power_endpoints(2, 0, &mut account).is_err());
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture(4, 1, &[0, 3], |cell, _| {
        Some(match cell {
            1 => -1.0,
            2 => f64::MIN_POSITIVE / 2.0,
            _ => 1.0,
        })
    })?;
    let owner = match heap.request_pointwise_dyadic_power(request, &mut account) {
        DyadicPowerApplicationRequest::ApplicationReady(owner) => owner,
        other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    crate::loan::fault::reset_reads();
    crate::loan::fault::reset_scalars();
    let candidate = match owner.evaluate(&mut account) {
        super::super::ApplicationEvaluation::ApplicationCandidate(candidate) => candidate,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(crate::loan::fault::reads(), 2);
    assert_eq!(crate::loan::fault::scalars(), 2);
    let (tile, _) = match heap.publish_application(candidate, &mut account) {
        super::super::ApplicationPublication::ApplicationPublished(tile, receipt) => {
            (tile, receipt)
        }
        other => return Err(format!("{other:?}").into()),
    };
    let reader = tile.reader(&heap, &mut account)?;
    for cell in [1, 2] {
        assert!(!reader.contains(cell, &mut account)?);
        assert!(
            reader
                .dyadic_power_endpoints(cell, 0, &mut account)
                .is_err()
        );
    }
    assert!(matches!(
        reader.application_outcome(3, &mut account)?,
        crate::CellOutcome::Completed
    ));
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture(1, 1, &[0], |_, _| Some(-1.0))?;
    let owner = match heap.request_pointwise_dyadic_power(request, &mut account) {
        DyadicPowerApplicationRequest::ApplicationReady(owner) => owner,
        other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    let candidate = match owner.evaluate(&mut account) {
        super::super::ApplicationEvaluation::ApplicationCandidate(candidate) => candidate,
        other => return Err(format!("{other:?}").into()),
    };
    let (tile, _) = match heap.publish_application(candidate, &mut account) {
        super::super::ApplicationPublication::ApplicationPublished(tile, receipt) => {
            (tile, receipt)
        }
        other => return Err(format!("{other:?}").into()),
    };
    let reader = tile.reader(&heap, &mut account)?;
    assert!(matches!(
        reader.application_outcome(0, &mut account)?,
        crate::CellOutcome::DyadicPowerOutsideDomain { component: 0 }
    ));
    golden_application_rows()?;
    Ok(())
}

/// Decode a positive finite binary64 value exactly, without a native power evaluation.
fn exact_binary(
    value: f64,
    budget: &mut Budget,
) -> Result<nrz_bindings::readings::algebra::Rational, Box<dyn std::error::Error>> {
    use nrz_bindings::readings::algebra::Rational;
    let bits = value.to_bits();
    if bits >= 0x7ff0_0000_0000_0000 {
        return Err("nonfinite exact comparison".into());
    }
    let field = (bits >> 52) & 2047;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, exponent) = if field == 0 {
        (fraction, -1074)
    } else {
        (fraction + (1_u64 << 52), i64::try_from(field)? - 1075)
    };
    Ok(
        Rational::from_ratio(i64::try_from(significand)?, 1, budget)?.mul(
            &Rational::from_ratio(2, 1, budget)?.pow(exponent, budget)?,
            budget,
        )?,
    )
}

/// One corpus row identifies an exponent and its checked original endpoints.
type GoldenPoints = (i64, u64, Vec<(&'static str, f64)>);

/// Identify every stored successful exponent and exact original endpoint without native replay.
fn golden_points(id: &str) -> Option<GoldenPoints> {
    let normal = f64::MIN_POSITIVE;
    let points = match id {
        "power-upper" => (1, 8, vec![("point", 2.0)]),
        "power-one" => (1, 8, vec![("point", 1.0)]),
        "power-normal-near-one" => (255, 256, vec![("point", normal)]),
        "power-quarter" => (1, 4, vec![("point", 0.5)]),
        "power-rounding-box" => (
            1,
            4,
            vec![
                ("lower_endpoint", (1.0_f64 / 16.0).next_down()),
                ("upper_endpoint", (1.0_f64 / 16.0).next_up()),
            ],
        ),
        "power-identity" => (1, 1, vec![("point", 2.0)]),
        "power-normal-next-near-one" => (255, 256, vec![("point", normal.next_up())]),
        "power-three-quarters" => (3, 4, vec![("point", 0.5)]),
        "power-quarter-exact" => (1, 4, vec![("point", 1.0 / 16.0)]),
        "power-upper-predecessor" => (1, 8, vec![("point", 2.0_f64.next_down())]),
        "power-one-neighbours" => (
            1,
            8,
            vec![
                ("lower_endpoint", 1.0_f64.next_down()),
                ("upper_endpoint", 1.0_f64.next_up()),
            ],
        ),
        "power-eighth-exact" => (1, 8, vec![("point", 1.0 / 256.0)]),
        "power-seven-eighths" => (7, 8, vec![("point", 2.0)]),
        "power-normal-identity" => (1, 1, vec![("point", normal)]),
        "power-three-quarters-exact" => (3, 4, vec![("point", 1.0 / 16.0)]),
        _ => return None,
    };
    Some(points)
}

/// Compare every successful public stored pair with the corpus's exact rational bounds.
/// # Panics
/// Panics only if an original input or a complete stored pair loses its exact enclosure.
#[expect(
    clippy::too_many_lines,
    reason = "the full corpus and its exact endpoint checks form one selected control"
)]
fn golden_application_rows() -> TestResult {
    use nrz_bindings::readings::algebra::Rational;
    use std::cmp::Ordering;
    let payload = nrz_docgen::golden::read(
        include_bytes!("../../../../../../data/golden/dyadic-power.json").as_slice(),
        nrz_docgen::golden::Limits::default(),
        &mut oracle_budget()?,
    )?;
    let mut rows = 0;
    let mut endpoints = 0;
    for row in payload.value()["entries"]
        .as_array()
        .ok_or("golden entries")?
    {
        if row["expected"] != "enclosure" {
            continue;
        }
        let id = row["id"].as_str().ok_or("golden id")?;
        let (numerator, denominator, points) =
            golden_points(id).ok_or("unknown successful golden")?;
        let expected_exponent = if denominator == 1 {
            numerator.to_string()
        } else {
            format!("{numerator}/{denominator}")
        };
        assert_eq!(
            row["exponent"].as_str().ok_or("golden exponent")?,
            expected_exponent.as_str(),
            "{id}"
        );
        for (name, x) in points {
            let input = if name == "point" {
                row["input"]["point"].as_str().ok_or("point")?
            } else if name == "lower_endpoint" {
                row["input"]["box"][0].as_str().ok_or("lower endpoint")?
            } else {
                row["input"]["box"][1].as_str().ok_or("upper endpoint")?
            };
            let mut exact = oracle_budget()?;
            let original = Rational::parse(input, &mut exact)?;
            assert_eq!(
                original.compare(&exact_binary(x, &mut exact)?, &mut exact)?,
                Ordering::Equal,
                "{id}/{name}"
            );
            let Fixture {
                mut heap,
                mut account,
                request,
            } = fixture_for(
                1,
                1,
                &[0],
                |_, _| Some(x),
                numerator,
                denominator,
                [x, x],
                None,
            )?;
            let owner = match heap.request_pointwise_dyadic_power(request, &mut account) {
                DyadicPowerApplicationRequest::ApplicationReady(owner) => owner,
                other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
                    return Err(format!("{id}/{name}: {other:?}").into());
                }
            };
            crate::loan::fault::reset_scalars();
            let candidate = match owner.evaluate(&mut account) {
                super::super::ApplicationEvaluation::ApplicationCandidate(candidate) => candidate,
                other => return Err(format!("{id}/{name}: {other:?}").into()),
            };
            let (tile, _) = match heap.publish_application(candidate, &mut account) {
                super::super::ApplicationPublication::ApplicationPublished(tile, receipt) => {
                    (tile, receipt)
                }
                other => return Err(format!("{id}/{name}: {other:?}").into()),
            };
            assert_eq!(crate::loan::fault::scalars(), 1, "{id}/{name}");
            let reader = tile.reader(&heap, &mut account)?;
            assert!(matches!(
                reader.application_outcome(0, &mut account)?,
                crate::CellOutcome::Completed
            ));
            let [lower, upper] = reader.dyadic_power_endpoints(0, 0, &mut account)?;
            let pair = row["golden"][name].as_array().ok_or("stored pair")?;
            let stored_low = Rational::parse(pair[0].as_str().ok_or("stored low")?, &mut exact)?;
            let stored_high = Rational::parse(pair[1].as_str().ok_or("stored high")?, &mut exact)?;
            assert_ne!(
                exact_binary(lower, &mut exact)?.compare(&stored_low, &mut exact)?,
                Ordering::Greater,
                "{id}/{name}"
            );
            assert_ne!(
                stored_high.compare(&exact_binary(upper, &mut exact)?, &mut exact)?,
                Ordering::Greater,
                "{id}/{name}"
            );
            let center = reader.read_component(crate::HostChannel::Center, 0, 0, &mut account)?;
            let total = reader.read_component(crate::HostChannel::Total, 0, 0, &mut account)?;
            let error = reader.read_component(crate::HostChannel::Error, 0, 0, &mut account)?;
            assert_eq!(total.to_bits(), error.to_bits(), "{id}/{name}");
            let midpoint = exact_binary(center, &mut exact)?;
            let radius = exact_binary(total, &mut exact)?;
            assert_ne!(
                midpoint
                    .sub(&radius, &mut exact)?
                    .compare(&exact_binary(lower, &mut exact)?, &mut exact)?,
                Ordering::Greater,
                "{id}/{name}"
            );
            assert_ne!(
                exact_binary(upper, &mut exact)?
                    .compare(&midpoint.add(&radius, &mut exact)?, &mut exact)?,
                Ordering::Greater,
                "{id}/{name}"
            );
            endpoints += 1;
        }
        rows += 1;
    }
    assert_eq!((rows, endpoints), (15, 17));
    Ok(())
}

/// Give the test's exact rational decoding capacity without changing a production ceiling.
fn oracle_budget() -> Result<Budget, Box<dyn std::error::Error>> {
    Ok(Budget::new(nrz_bindings::Limits {
        literal_bytes: 4096,
        component_bits: 8192,
        intermediate_bits: 16384,
        integer_exponent: 4096,
        ..laws::limits()
    })?)
}

/// Decode only dyadic refusal alternatives without consulting the retained program.
fn decode_refusal(outcome: &crate::CellOutcome) -> Option<(usize, nrz_scalar::DyadicPowerFailure)> {
    use crate::CellOutcome as Outcome;
    use nrz_scalar::DyadicPowerFailure as Failure;
    match outcome {
        Outcome::DyadicPowerOutsideDomain { component } => {
            Some((*component, Failure::OutsideDomain))
        }
        Outcome::DyadicPowerBelowNormalOperand { component } => {
            Some((*component, Failure::BelowNormalOperand))
        }
        Outcome::DyadicPowerNonDyadicExponent { component } => {
            Some((*component, Failure::NonDyadicExponent))
        }
        Outcome::DyadicPowerTargetNotCertifiable { component, failure } => Some((
            *component,
            Failure::TargetNotCertifiable { failure: *failure },
        )),
        Outcome::DyadicPowerInventoryLimit {
            component,
            limit,
            needed,
        } => Some((
            *component,
            Failure::InventoryLimit {
                limit: *limit,
                needed: *needed,
            },
        )),
        Outcome::DyadicPowerMalformedPlan { component } => {
            Some((*component, Failure::MalformedPlan))
        }
        Outcome::DyadicPowerEnvironment { component } => Some((*component, Failure::Environment)),
        Outcome::DyadicPowerEngine { component, failure } => {
            Some((*component, Failure::Engine { failure: *failure }))
        }
        Outcome::NotRequested
        | Outcome::MissingInput
        | Outcome::Pending
        | Outcome::Completed
        | Outcome::PreparedRefused { .. }
        | Outcome::CellRefused { .. } => None,
    }
}

/// Every scalar dyadic cause survives inline conversion with full-width payloads.
/// # Panics
/// Panics only if a cause, component, arithmetic payload or family check changes.
/// ´claim:matrix:dyadic-application-failures-and-pairing´
/// ´test:unit:dyadic-application-failures-and-pairing´
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "every native cause and full-width payload is compared"
)]
fn dyadic_application_failures_and_pairing() -> TestResult {
    use nrz_scalar::{ArithmeticFailure as Arithmetic, DyadicPowerFailure as Failure};
    private_retained_substitution(false)?;
    private_retained_substitution(true)?;
    private_expm1_substitution(false)?;
    private_expm1_substitution(true)?;
    for dyadic_owner in [false, true] {
        private_root_substitution(dyadic_owner, false)?;
        private_root_substitution(dyadic_owner, true)?;
    }
    private_dyadic_reader_substitution()?;
    private_expm1_reader_substitution()?;
    for dyadic_owner in [false, true] {
        private_root_reader_substitution(dyadic_owner)?;
    }
    let arithmetic = [
        Arithmetic::NonFinite,
        Arithmetic::InvertedEndpoints,
        Arithmetic::ZeroInDivisor,
        Arithmetic::NegativeRoot,
        Arithmetic::Overflow,
        Arithmetic::InvalidLiteral,
        Arithmetic::LiteralRange,
        Arithmetic::ResourceLimit,
        Arithmetic::LengthMismatch,
        Arithmetic::UnsupportedEnvironment,
    ];
    let fieldless = [
        Failure::OutsideDomain,
        Failure::BelowNormalOperand,
        Failure::NonDyadicExponent,
        Failure::MalformedPlan,
        Failure::Environment,
    ];
    let mut setup = super::super::tests::expm1::fixture(1, 1, [-1.0, 0.0], 1, false)?;
    let law = law_on(&mut setup.heap, &mut setup.account)?;
    let operation = checked_on(&law, &mut setup)?;
    let dyadic = super::super::member::Member::check_dyadic(
        &operation.operation,
        crate::KernelKind::PreparedDyadicPower,
        &operation.binding,
    )?;
    let expm1 = super::super::member::Member::check(
        &setup.request.operation.operation,
        crate::KernelKind::PreparedExpm1,
        Some(&setup.request.operation),
    )?;
    let extent = operation.operation.inputs()[0].shape().extent();
    for component in [0, usize::MAX] {
        for reason in fieldless {
            let outcome = super::super::dyadic_refusal(component, reason);
            assert_eq!(decode_refusal(&outcome), Some((component, reason)));
            assert!(expm1.validate_outcome(&outcome, 0, extent).is_err());
            assert_eq!(
                dyadic.validate_outcome(&outcome, 0, extent).is_ok(),
                component == 0
            );
        }
        for failure in arithmetic {
            for reason in [
                Failure::TargetNotCertifiable { failure },
                Failure::Engine { failure },
            ] {
                let outcome = super::super::dyadic_refusal(component, reason);
                assert_eq!(decode_refusal(&outcome), Some((component, reason)));
                assert!(expm1.validate_outcome(&outcome, 0, extent).is_err());
                assert_eq!(
                    dyadic.validate_outcome(&outcome, 0, extent).is_ok(),
                    component == 0
                );
            }
        }
        for (limit, needed) in [(0, 0), (1, 0), (0, 1), (usize::MAX, usize::MAX)] {
            let reason = Failure::InventoryLimit { limit, needed };
            let outcome = super::super::dyadic_refusal(component, reason);
            assert_eq!(decode_refusal(&outcome), Some((component, reason)));
            assert!(expm1.validate_outcome(&outcome, 0, extent).is_err());
            assert_eq!(
                dyadic.validate_outcome(&outcome, 0, extent).is_ok(),
                component == 0
            );
        }
    }
    let Fixture {
        mut heap,
        mut account,
        mut request,
    } = fixture(1, 1, &[0], |_, _| Some(1.0))?;
    let original = Binding::of(&request.law)?;
    let input_ticket = request.input.ticket()?;
    let other = heap.retain_retained_dyadic_power(&request.law, &mut account)?;
    let replacement = Binding::of(&other)?;
    assert_ne!(original, replacement);
    let first = std::mem::replace(&mut request.law, other);
    let injection = crate::loan::fault::Injection::after(0);
    crate::loan::fault::reset_reads();
    crate::loan::fault::reset_scalars();
    let returned = match heap.request_pointwise_dyadic_power(request, &mut account) {
        DyadicPowerApplicationRequest::ApplicationRefused(
            ApplicationError::Loan(LoanError::WrongLoan),
            returned,
        ) => returned,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(Binding::of(&returned.law)?, replacement);
    assert_eq!(returned.input.ticket()?, input_ticket);
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    assert_eq!(crate::loan::fault::reads(), 0);
    assert_eq!(crate::loan::fault::scalars(), 0);
    drop((injection, first, returned));
    let ex_setup = super::super::tests::expm1::fixture(1, 1, [-2.0, 0.0], 1, false)?;
    let ex_binding = crate::loan::retained_expm1::Binding::of(&ex_setup.request.law)?;
    let mut expm1_shell = crate::loan::retained_law::RetainedLawAttachment::empty();
    expm1_shell.install_expm1(ex_setup.request.law);
    let dy_setup = fixture(1, 1, &[0], |_, _| Some(1.0))?;
    let dy_binding = Binding::of(&dy_setup.request.law)?;
    let mut dyadic_shell = crate::loan::retained_law::RetainedLawAttachment::empty();
    dyadic_shell.install_dyadic_power(dy_setup.request.law);
    crate::loan::fault::reset_reads();
    assert!(matches!(
        expm1_shell.dyadic_power(),
        Err(LoanError::WrongLoan)
    ));
    assert!(matches!(dyadic_shell.expm1(), Err(LoanError::WrongLoan)));
    assert!(matches!(
        expm1_shell.validate_dyadic_binding(dy_binding),
        Err(LoanError::WrongLoan)
    ));
    assert!(matches!(
        dyadic_shell.validate_binding(ex_binding, 40),
        Err(LoanError::WrongLoan)
    ));
    assert_eq!(crate::loan::fault::reads(), 0);
    let (mut cas, target, context) = laws::original(1, 1)?;
    let mut budget = Budget::new(laws::limits())?;
    let mut receipt = 0;
    let first_checked = nrz_bindings::DyadicPowerPreparation::prepare(
        &mut cas,
        &target,
        &context,
        [1.0, 1.0],
        laws::policy(),
        &mut budget,
        &mut |_| false,
        &mut receipt,
    )?;
    let mut alternate_policy = laws::policy();
    alternate_policy.precision += 1;
    let second_checked = nrz_bindings::DyadicPowerPreparation::prepare(
        &mut cas,
        &target,
        &context,
        [1.0, 1.0],
        alternate_policy,
        &mut budget,
        &mut |_| false,
        &mut receipt,
    )?;
    assert_eq!(
        first_checked.judgment().request(),
        second_checked.judgment().request()
    );
    assert_eq!(
        first_checked.judgment().interval(),
        second_checked.judgment().interval()
    );
    assert_ne!(first_checked.proof_bytes(), second_checked.proof_bytes());
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = ExecutionAccount::new(Budget::new(laws::limits())?)?;
    let (cas, target, context) = laws::original(1, 1)?;
    let first = heap
        .prepare_retained_dyadic_power(
            cas,
            &target,
            &context,
            [1.0, 1.0],
            laws::policy(),
            laws::scalar_limits(),
            &ArithmeticContext::new()?,
            &mut |_| false,
            &mut account,
        )
        .map_err(|(error, _)| error)?;
    let (cas, target, context) = laws::original(1, 1)?;
    let second = heap
        .prepare_retained_dyadic_power(
            cas,
            &target,
            &context,
            [1.0, 1.0],
            alternate_policy,
            laws::scalar_limits(),
            &ArithmeticContext::new()?,
            &mut |_| false,
            &mut account,
        )
        .map_err(|(error, _)| error)?;
    assert_eq!(
        first.authority()?.judgment().interval(),
        second.authority()?.judgment().interval()
    );
    assert_ne!(
        first.authority()?.proof_bytes(),
        second.authority()?.proof_bytes()
    );
    let first_binding = Binding::of(&first)?;
    let mut shell = crate::loan::retained_law::RetainedLawAttachment::empty();
    shell.install_dyadic_power(second);
    assert!(matches!(
        shell.validate_dyadic_binding(first_binding),
        Err(LoanError::WrongLoan)
    ));
    allocation_prefixes()?;
    Ok(())
}

/// Every shared allocation site returns the original use or unwinds after its acquired prefix.
/// # Panics
/// Panics only if an expected injected unwind is missed or a partial acquisition survives.
fn allocation_prefixes() -> TestResult {
    use crate::loan::fault::Injection;
    for unwind in [false, true] {
        for prefix in 0..37 {
            let Fixture {
                mut heap,
                mut account,
                request,
            } = fixture(2, 2, &[0, 1], |_, _| Some(1.0))?;
            let released = crate::loan::fault::numerical_releases();
            let ticket = request.input.ticket()?;
            let binding = Binding::of(&request.law)?;
            let injection = if unwind {
                Injection::unwinding_after(prefix)
            } else {
                Injection::after(prefix)
            };
            if unwind {
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _result = heap.request_pointwise_dyadic_power(request, &mut account);
                    }))
                    .is_err()
                );
            } else {
                let returned = match heap.request_pointwise_dyadic_power(request, &mut account) {
                    DyadicPowerApplicationRequest::ApplicationRefused(
                        ApplicationError::Loan(error),
                        returned,
                    ) => {
                        if (3..9).contains(&prefix) {
                            assert!(matches!(error, LoanError::Heap(crate::HeapError::Capacity)));
                        } else {
                            assert!(matches!(error, LoanError::AllocationRefused));
                        }
                        returned
                    }
                    other => return Err(format!("{other:?}").into()),
                };
                assert_eq!(returned.input.ticket()?, ticket);
                assert_eq!(Binding::of(&returned.law)?, binding);
            }
            assert_eq!(Injection::attempts(), prefix + 1);
            let retained = match prefix {
                0..=3 => prefix,
                4 => 3,
                5 | 6 => 4,
                7 | 8 => 5,
                _ => 6,
            };
            assert_eq!(Injection::prefix(), [prefix, retained, prefix - retained]);
            let slabs = [10, 13, 16, 19, 22]
                .into_iter()
                .filter(|site| *site < prefix)
                .count();
            assert_eq!(
                crate::loan::fault::numerical_releases(),
                [
                    released[0] + slabs,
                    released[1] + 4 * slabs,
                    released[2] + 64 * slabs,
                ]
            );
            assert_eq!(heap.loans.pending, 0);
            drop(injection);
        }
    }
    Ok(())
}

/// One public selected evaluation reports its actual charged work and stores.
fn observed_application() -> Result<(u64, usize, usize), Box<dyn std::error::Error>> {
    let Fixture {
        mut heap,
        mut account,
        request,
    } = fixture(2, 1, &[0], |_, _| Some(1.0))?;
    let owner = match heap.request_pointwise_dyadic_power(request, &mut account) {
        DyadicPowerApplicationRequest::ApplicationReady(owner) => owner,
        other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    crate::loan::fault::reset_scalars();
    crate::loan::fault::reset_stores();
    let candidate = match owner.evaluate(&mut account) {
        super::super::ApplicationEvaluation::ApplicationCandidate(candidate) => candidate,
        other => return Err(format!("{other:?}").into()),
    };
    let (tile, _) = match heap.publish_application(candidate, &mut account) {
        super::super::ApplicationPublication::ApplicationPublished(tile, receipt) => {
            (tile, receipt)
        }
        other => return Err(format!("{other:?}").into()),
    };
    let reader = tile.reader(&heap, &mut account)?;
    assert!(matches!(
        reader.application_outcome(0, &mut account)?,
        crate::CellOutcome::Completed
    ));
    Ok((
        account.usage().work,
        crate::loan::fault::scalars(),
        crate::loan::fault::stores(),
    ))
}

/// Evaluate two uses of one frozen law and repair a third publication after fixture setup.
fn observed_reuse(watched: bool) -> ObservedReuseResult {
    use crate::loan::retained_dyadic_power::tests::observation::OwnerObserver;
    let mut setup = fixture(1, 1, &[0], |_, _| Some(1.0))?;
    let second = additional(&mut setup)?;
    let repaired = additional(&mut setup)?;
    let Fixture {
        mut heap,
        mut account,
        request,
    } = setup;
    let observer = OwnerObserver::default();
    let run = || -> ObservedCallsResult {
        crate::loan::fault::reset_scalars();
        crate::loan::fault::reset_stores();
        let mut readings = Vec::new();
        for request in [request, second] {
            let owner = match heap.request_pointwise_dyadic_power(request, &mut account) {
                DyadicPowerApplicationRequest::ApplicationReady(owner) => owner,
                other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
                    return Err(format!("{other:?}").into());
                }
            };
            let candidate = match owner.evaluate(&mut account) {
                super::super::ApplicationEvaluation::ApplicationCandidate(candidate) => candidate,
                other => return Err(format!("{other:?}").into()),
            };
            let tile = match heap.publish_application(candidate, &mut account) {
                super::super::ApplicationPublication::ApplicationPublished(tile, _) => tile,
                other => return Err(format!("{other:?}").into()),
            };
            let reader = tile.reader(&heap, &mut account)?;
            readings.push(reader.dyadic_power_endpoints(0, 0, &mut account)?);
        }
        let owner = match heap.request_pointwise_dyadic_power(repaired, &mut account) {
            DyadicPowerApplicationRequest::ApplicationReady(owner) => owner,
            other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        };
        let candidate = match owner.evaluate(&mut account) {
            super::super::ApplicationEvaluation::ApplicationCandidate(candidate) => candidate,
            other => return Err(format!("{other:?}").into()),
        };
        let native_before_repair = crate::loan::fault::scalars();
        let injection = crate::loan::fault::Injection::after(0);
        let candidate = match heap.publish_application(candidate, &mut account) {
            super::super::ApplicationPublication::ApplicationRepair(
                ApplicationError::Loan(LoanError::AllocationRefused),
                owner,
            ) => owner,
            other => return Err(format!("{other:?}").into()),
        };
        drop(injection);
        let tile = match heap.publish_application(candidate, &mut account) {
            super::super::ApplicationPublication::ApplicationPublished(tile, _) => tile,
            other => return Err(format!("{other:?}").into()),
        };
        assert_eq!(crate::loan::fault::scalars(), native_before_repair);
        let reader = tile.reader(&heap, &mut account)?;
        readings.push(reader.dyadic_power_endpoints(0, 0, &mut account)?);
        Ok((
            account.usage().work,
            crate::loan::fault::scalars(),
            crate::loan::fault::stores(),
            readings,
        ))
    };
    let (work, native, stores, readings) = if watched {
        crate::observer_dispatch::with_observer(observer.clone(), || {
            tracing::callsite::rebuild_interest_cache();
            run()
        })?
    } else {
        run()?
    };
    let spans = if watched {
        for name in [
            "nrz.prepare",
            "nrz.lower",
            "nrz.proof",
            "nrz.replay",
            "nrz.lookup",
        ] {
            assert!(
                observer.spans(name).is_empty(),
                "unexpected {name} during reuse"
            );
        }
        observer
            .spans("nrz.kernel")
            .into_iter()
            .filter(|span| observer.field(*span, "canonical_numerator").is_some())
            .count()
    } else {
        0
    };
    Ok((work, native, stores, readings, spans))
}

/// Scoped observation leaves Work and results unchanged across selected reuse and repair.
/// # Panics
/// Panics only if a selected native call, five stores or no-rebuild reuse changes.
/// ´claim:matrix:dyadic-application-observation´
/// ´test:unit:dyadic-application-observation´
#[test]
fn dyadic_application_observation() -> TestResult {
    use crate::loan::retained_dyadic_power::tests::observation::OwnerObserver;
    let ordinary = observed_application()?;
    let observer = OwnerObserver::default();
    let watched = crate::observer_dispatch::with_observer(
        observer.clone(),
        || -> Result<_, Box<dyn std::error::Error>> {
            tracing::callsite::rebuild_interest_cache();
            observed_application()
        },
    )?;
    assert_eq!(ordinary, watched);
    assert_eq!((watched.1, watched.2), (1, 5));
    let native = observer
        .spans("nrz.kernel")
        .into_iter()
        .filter(|span| observer.field(*span, "canonical_numerator").is_some())
        .count();
    assert_eq!(native, 1);
    let ordinary_reuse = observed_reuse(false)?;
    let watched_reuse = observed_reuse(true)?;
    assert_eq!(ordinary_reuse.0, watched_reuse.0);
    assert_eq!(ordinary_reuse.1, watched_reuse.1);
    assert_eq!(ordinary_reuse.2, watched_reuse.2);
    assert_eq!(ordinary_reuse.3, watched_reuse.3);
    assert_eq!(watched_reuse.1, 3);
    assert_eq!(watched_reuse.2, 15);
    assert_eq!(watched_reuse.3, vec![[1.0, 1.0]; 3]);
    assert_eq!(watched_reuse.4, 3);
    for short in [0_u64, 1] {
        let Fixture {
            mut heap,
            mut account,
            request,
        } = fixture(2, 1, &[0], |_, _| Some(1.0))?;
        let owner = match heap.request_pointwise_dyadic_power(request, &mut account) {
            DyadicPowerApplicationRequest::ApplicationReady(owner) => owner,
            other @ DyadicPowerApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        };
        let required = 3 + 2 + 7 + 6 + 2;
        let prior = account.limits().work - account.usage().work - required + short;
        account.ledger.work(prior, crate::loan::AT)?;
        let before = account.usage().work;
        crate::loan::fault::reset_reads();
        crate::loan::fault::reset_scalars();
        crate::loan::fault::reset_stores();
        match owner.evaluate(&mut account) {
            super::super::ApplicationEvaluation::ApplicationCandidate(candidate) => {
                assert_eq!(short, 0);
                assert_eq!(account.usage().work - before, required);
                assert_eq!(crate::loan::fault::reads(), 1);
                assert_eq!(crate::loan::fault::scalars(), 1);
                assert_eq!(crate::loan::fault::stores(), 5);
                drop(candidate);
            }
            super::super::ApplicationEvaluation::ApplicationNotStarted(
                ApplicationError::Loan(LoanError::Budget(_)),
                ready,
            ) => {
                assert_eq!(short, 1);
                assert!(!ready.core.candidate);
                assert_eq!(crate::loan::fault::reads(), 0);
                assert_eq!(crate::loan::fault::scalars(), 0);
                assert_eq!(crate::loan::fault::stores(), 0);
            }
            other => return Err(format!("{other:?}").into()),
        }
    }
    Ok(())
}
