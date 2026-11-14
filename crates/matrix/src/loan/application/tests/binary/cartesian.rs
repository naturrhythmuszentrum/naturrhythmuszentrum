// SPDX-License-Identifier: AGPL-3.0-only
//! Original factor coordinates govern Cartesian binary masks, values and lifetime.

mod binary32;
mod boundaries;
mod fixture;
use super::{BinaryElementwise, TestResult, admit, candidate, oracle, published, ready};

/// Independent quotient/remainder enumeration checks both laws, complete/empty/sparse demand and holes.
/// ´test:unit:loan-cartesian-binary-factor-images´
#[test]
fn loan_cartesian_binary_factor_images() -> TestResult {
    loan_cartesian_binary_factor_images_format::<f32>()?;
    loan_cartesian_binary_factor_images_format::<f64>()
}

/// The identical boundary is required in both native formats.
fn loan_cartesian_binary_factor_images_format<P: nrz_scalar::Scalar + 'static>() -> TestResult {
    for function in [BinaryElementwise::Product, BinaryElementwise::Quotient] {
        for rows in [[0, 2], [3, 0], [3, 2], [35, 2]] {
            for selection in 0..3 {
                for holes in [false, true] {
                    values::<P>(rows, selection, holes, function)?;
                }
            }
        }
    }
    Ok(())
}

/// Literal input formulas and exact rational corners never consult production product coordinates.
fn values<P: nrz_scalar::Scalar + 'static>(
    rows: [usize; 2],
    selection: usize,
    holes: bool,
    function: BinaryElementwise,
) -> TestResult {
    let (mut setup, _) = fixture::setup::<P>(rows, selection, holes, function)?;
    let demand = setup.request.demand;
    let outside = setup
        .heap
        .retain_pin(&setup.request.right, &mut setup.account)?;
    crate::loan::fault::reset_reads();
    let owner = ready(admit(
        &mut setup.heap,
        setup.request,
        function,
        &mut setup.account,
    ))?;
    assert_eq!(crate::loan::fault::reads(), 0);
    let owner = candidate(owner, &mut setup.account)?;
    let available = |cell| {
        let requested = super::requested(cell, selection);
        let left = cell / rows[1] != 0;
        let right = cell % rows[1] != 1;
        let complete = !holes || (left && right);
        requested && complete
    };
    let count = (0..rows[0] * rows[1])
        .filter(|cell| available(*cell))
        .count();
    assert_eq!(crate::loan::fault::reads(), count * 8);
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    let reader = tile.reader(&setup.heap, &mut setup.account)?;
    for cell in 0..rows[0] * rows[1] {
        assert_eq!(reader.contains(cell, &mut setup.account)?, available(cell));
        if available(cell) {
            for component in 0..2 {
                let centers = [
                    fixture::center(0, cell / rows[1], component)?,
                    fixture::center(1, cell % rows[1], component)?,
                ];
                let readings =
                    super::values::readings(&reader, cell, component, &mut setup.account)?;
                let totals = [
                    0.25 + f64::from(u32::try_from(cell / rows[1])?) / 1024.0
                        + f64::from(u32::try_from(component)?) / 256.0,
                    0.5 + f64::from(u32::try_from(cell % rows[1])?) / 1024.0
                        + f64::from(u32::try_from(component)?) / 256.0,
                ];
                oracle::tight_native::<P>(
                    centers,
                    totals,
                    function,
                    readings.map(nrz_scalar::Scalar::to_f64),
                )?;
            }
        }
    }
    drop(tile);
    setup.heap.retire_mask(demand, &mut setup.account.ledger)?;
    setup
        .heap
        .retire_cell_layout(demand.layout(), &mut setup.account.ledger)?;
    assert!(matches!(
        setup.heap.residence(&mut setup.account, 100_000)?,
        crate::Residence::Pending { .. }
    ));
    if rows[1] != 0 {
        assert!(
            outside
                .historical_reader(&mut setup.account)?
                .contains(0, &mut setup.account)?
        );
    }
    drop(outside);
    assert!(matches!(
        setup.heap.residence(&mut setup.account, 100_000)?,
        crate::Residence::Empty { .. }
    ));
    assert_eq!(setup.heap.loans.elements()?, [0; 2]);
    Ok(())
}

/// Entry and right-source visits (2), six owner bindings, two word visits and four selected
/// cells times (2*(21 invocation + 4 loads + 3 stores) + 2 cell visits + 2 coordinates) give 250.
/// ´test:unit:loan-cartesian-binary-work-boundary´
#[test]
fn loan_cartesian_binary_work_boundary() -> TestResult {
    loan_cartesian_binary_work_boundary_format::<f32>()?;
    loan_cartesian_binary_work_boundary_format::<f64>()
}

/// The identical boundary is required in both native formats.
fn loan_cartesian_binary_work_boundary_format<P: nrz_scalar::Scalar + 'static>() -> TestResult {
    for available in [249, 250] {
        let function = BinaryElementwise::Product;
        let (mut setup, _) = fixture::setup::<P>([3, 2], 1, false, function)?;
        let owner = ready(admit(
            &mut setup.heap,
            setup.request,
            function,
            &mut setup.account,
        ))?;
        super::leave(&mut setup.account, available)?;
        crate::loan::fault::reset_reads();
        let result = owner.evaluate(&mut setup.account);
        if available == 250 {
            assert!(matches!(
                result,
                crate::ApplicationEvaluation::ApplicationCandidate(_)
            ));
            assert_eq!(crate::loan::fault::reads(), 32);
        } else {
            assert!(matches!(
                result,
                crate::ApplicationEvaluation::ApplicationNotStarted(
                    crate::ApplicationError::Loan(crate::LoanError::Budget(_)),
                    _
                )
            ));
            assert_eq!(crate::loan::fault::reads(), 0);
        }
    }
    Ok(())
}

/// A factor-layout demand or swapped factor pins cannot substitute for the original product request.
/// ´test:unit:loan-cartesian-binary-request-refusals´
#[test]
fn loan_cartesian_binary_request_refusals() -> TestResult {
    loan_cartesian_binary_request_refusals_format::<f32>()?;
    loan_cartesian_binary_request_refusals_format::<f64>()
}

/// The identical boundary is required in both native formats.
fn loan_cartesian_binary_request_refusals_format<P: nrz_scalar::Scalar + 'static>() -> TestResult {
    for choice in 0..3 {
        let swapped = choice == 1;
        let allocation = choice == 2;
        let function = BinaryElementwise::Quotient;
        let (mut setup, _) = fixture::setup::<P>([3, 2], 1, false, function)?;
        let demand = setup.request.demand;
        if swapped {
            std::mem::swap(&mut setup.request.left, &mut setup.request.right);
        } else if !allocation {
            setup.request.demand = setup.heap.create_mask(
                setup.request.left.ticket()?.identity.layout,
                &[0],
                &mut setup.account.ledger,
            )?;
        }
        let tickets = [setup.request.left.ticket()?, setup.request.right.ticket()?];
        crate::loan::fault::reset_reads();
        let injection = crate::loan::fault::Injection::after(0);
        let mut request = match admit(&mut setup.heap, setup.request, function, &mut setup.account)
        {
            crate::BinaryApplicationRequest::ApplicationRefused(
                crate::ApplicationError::Loan(error),
                request,
            ) => {
                if allocation {
                    assert!(matches!(error, crate::LoanError::AllocationRefused));
                } else if swapped {
                    assert!(matches!(error, crate::LoanError::UnsupportedBinaryExtent));
                } else {
                    assert!(matches!(error, crate::LoanError::LayoutMismatch));
                }
                request
            }
            other => return Err(format!("{other:?}").into()),
        };
        assert_eq!([request.left.ticket()?, request.right.ticket()?], tickets);
        assert_eq!(crate::loan::fault::reads(), 0);
        assert_eq!(
            crate::loan::fault::Injection::attempts(),
            usize::from(allocation)
        );
        drop(injection);
        if swapped {
            std::mem::swap(&mut request.left, &mut request.right);
        }
        request.demand = demand;
        let owner = ready(admit(
            &mut setup.heap,
            request,
            function,
            &mut setup.account,
        ))?;
        let owner = candidate(owner, &mut setup.account)?;
        let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
        assert!(
            tile.reader(&setup.heap, &mut setup.account)?
                .contains(5, &mut setup.account)?
        );
    }
    Ok(())
}

/// Gathered demand is named unsupported before allocation, even when its original product is correct.
/// ´test:unit:loan-cartesian-binary-gather-refusal´
#[test]
fn loan_cartesian_binary_gather_refusal() -> TestResult {
    loan_cartesian_binary_gather_refusal_format::<f32>()?;
    loan_cartesian_binary_gather_refusal_format::<f64>()
}

/// The identical boundary is required in both native formats.
fn loan_cartesian_binary_gather_refusal_format<P: nrz_scalar::Scalar + 'static>() -> TestResult {
    let function = BinaryElementwise::Product;
    let (mut setup, product) = fixture::setup::<P>([3, 2], 1, false, function)?;
    let demand = setup.request.demand;
    let identity = setup.heap.mask_correspondence(
        demand.layout(),
        demand.layout(),
        &mut setup.account.ledger,
    )?;
    let gathered = setup
        .heap
        .compact_mask(identity, demand, 4, 4, &mut setup.account.ledger)?;
    let original = &setup.request.operation.operation;
    setup.request.operation = crate::PointwiseBinaryOperation::check_cartesian(
        [original.inputs()[0].clone(), original.inputs()[1].clone()],
        original.outputs()[0].clone(),
        [&setup.request.left, &setup.request.right],
        setup.request.destination.symbols.clone(),
        product,
        gathered.layout(),
        function,
        crate::ExecutionLimits::default(),
        &mut setup.account,
    )?;
    setup.request.demand = gathered;
    crate::loan::fault::reset_reads();
    let injection = crate::loan::fault::Injection::after(0);
    assert!(matches!(
        admit(&mut setup.heap, setup.request, function, &mut setup.account),
        crate::BinaryApplicationRequest::ApplicationRefused(
            crate::ApplicationError::Loan(crate::LoanError::UnsupportedBinaryProjection),
            _
        )
    ));
    assert_eq!(crate::loan::fault::reads(), 0);
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    drop(injection);
    Ok(())
}

/// Matching factor capacities cannot substitute unrelated native axes or authorize a dense adapter.
/// ´test:unit:loan-cartesian-binary-axis-and-dense-refusals´
#[test]
fn loan_cartesian_binary_axis_and_dense_refusals() -> TestResult {
    loan_cartesian_binary_axis_and_dense_refusals_format::<f32>()?;
    loan_cartesian_binary_axis_and_dense_refusals_format::<f64>()
}

/// The identical boundary is required in both native formats.
fn loan_cartesian_binary_axis_and_dense_refusals_format<P: nrz_scalar::Scalar + 'static>()
-> TestResult {
    let function = BinaryElementwise::Product;
    let (mut setup, product) = fixture::setup::<P>([3, 2], 1, false, function)?;
    let original = &setup.request.operation.operation;
    assert!(!original.carries_statistics());
    let crate::binding::NativeRecipe::Kernel(law) = &original.recipe else {
        return Err("kernel".into());
    };
    assert!(matches!(
        law.physical_scratch::<f64, f64>(0),
        Err(crate::OperationError::Kernel(crate::KernelError::Contract))
    ));
    for side in 0..2 {
        let mut axes = product.factors();
        axes[side] = setup
            .heap
            .create_cell_axis(axes[side].capacity(), &mut setup.account.ledger)?;
        let wrong = setup
            .heap
            .create_cell_product(axes[0], axes[1], &mut setup.account.ledger)?;
        crate::loan::fault::reset_reads();
        let injection = crate::loan::fault::Injection::after(0);
        let result = crate::PointwiseBinaryOperation::check_cartesian(
            [original.inputs()[0].clone(), original.inputs()[1].clone()],
            original.outputs()[0].clone(),
            [&setup.request.left, &setup.request.right],
            setup.request.destination.symbols.clone(),
            wrong,
            setup.request.demand.layout(),
            function,
            crate::ExecutionLimits::default(),
            &mut setup.account,
        );
        assert!(matches!(
            result,
            Err(crate::ApplicationError::Loan(
                crate::LoanError::LayoutMismatch
            ))
        ));
        assert_eq!(crate::loan::fault::reads(), 0);
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        drop(injection);
    }
    Ok(())
}

/// Cartesian binding requires complete companion owners, even with empty demand.
/// ´test:unit:loan-cartesian-binary-companion-refusal´
#[test]
fn loan_cartesian_binary_companion_refusal() -> TestResult {
    loan_cartesian_binary_companion_refusal_format::<f32>()?;
    loan_cartesian_binary_companion_refusal_format::<f64>()
}

/// The identical boundary is required in both native formats.
fn loan_cartesian_binary_companion_refusal_format<P: nrz_scalar::Scalar + 'static>() -> TestResult {
    for roles in [[1, 3], [2, 3], [3, 1], [3, 2]] {
        let mut setup = super::fixture::custom::<P>(
            3,
            2,
            roles,
            0,
            false,
            BinaryElementwise::Product,
            super::Limits::default(),
            |_, _, _| (2.0, 0.25),
        )?;
        let axis = setup.request.destination.axis;
        let product = setup
            .heap
            .create_cell_product(axis, axis, &mut setup.account.ledger)?;
        let layout = setup.heap.create_cell_layout(
            product.axis(),
            crate::Layout::contiguous(crate::Extent::new(9, 2)?)?,
            &mut setup.account.ledger,
        )?;
        let original = &setup.request.operation.operation;
        let output = crate::RegionContract::check(crate::kernel_testing::shape(
            &original.outputs()[0].shape().declarations,
            &crate::kernel_testing::axis(2)?,
            &crate::kernel_testing::axis(9)?,
        )?)?;
        let symbols = nrz_bindings::BindingSymbols::declare(
            &nrz_bindings::Descriptor::ratio(),
            [9, 2],
            [true; 2],
            &mut setup.account.ledger,
        )?;
        crate::loan::fault::reset_reads();
        let injection = crate::loan::fault::Injection::after(0);
        let result = crate::PointwiseBinaryOperation::check_cartesian(
            [original.inputs()[0].clone(), original.inputs()[1].clone()],
            output,
            [&setup.request.left, &setup.request.right],
            symbols,
            product,
            layout,
            BinaryElementwise::Product,
            crate::ExecutionLimits::default(),
            &mut setup.account,
        );
        assert!(matches!(
            result,
            Err(crate::ApplicationError::Loan(
                crate::LoanError::UnsupportedContract
            ))
        ));
        assert_eq!(crate::loan::fault::reads(), 0);
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        drop(injection);
    }
    Ok(())
}

/// Original demand and axis seals remain repairable at both detached evaluation and handback.
/// ´test:unit:loan-cartesian-binary-capture-repair´
#[test]
fn loan_cartesian_binary_capture_repair() -> TestResult {
    loan_cartesian_binary_capture_repair_format::<f32>()?;
    loan_cartesian_binary_capture_repair_format::<f64>()
}

/// The identical boundary is required in both native formats.
fn loan_cartesian_binary_capture_repair_format<P: nrz_scalar::Scalar + 'static>() -> TestResult {
    for publication in [false, true] {
        for mutation in 0..3 {
            capture_repair::<P>(publication, mutation)?;
        }
    }
    Ok(())
}

/// Restoring captured metadata repairs the same owner without replenishing its account.
fn capture_repair<P: nrz_scalar::Scalar + 'static>(
    publication: bool,
    mutation: usize,
) -> TestResult {
    let function = BinaryElementwise::Product;
    let (mut setup, product) = fixture::setup::<P>([3, 2], 1, false, function)?;
    let mut owner = ready(admit(
        &mut setup.heap,
        setup.request,
        function,
        &mut setup.account,
    ))?;
    if publication {
        owner = candidate(owner, &mut setup.account)?;
    }
    let demand = owner.core.evidence.demand;
    let source_axis = owner.core.evidence.source.axis;
    let output_axis = owner.core.host.declaration.axis;
    match mutation {
        0 => owner.core.evidence.demand.layout = owner.core.evidence.source_identity.layout,
        1 => owner.core.evidence.source.axis = product.factors()[1],
        2 => owner.core.host.declaration.axis = product.factors()[0],
        _ => return Err("mutation".into()),
    }
    let ticket = owner.ticket();
    crate::loan::fault::reset_reads();
    let injection = crate::loan::fault::Injection::after(0);
    owner = if publication {
        match setup.heap.publish_application(owner, &mut setup.account) {
            crate::ApplicationPublication::ApplicationRepair(_, owner) => owner,
            other => return Err(format!("{other:?}").into()),
        }
    } else {
        match owner.evaluate(&mut setup.account) {
            crate::ApplicationEvaluation::ApplicationNotStarted(_, owner) => owner,
            other => return Err(format!("{other:?}").into()),
        }
    };
    assert_eq!(owner.ticket(), ticket);
    assert_eq!(crate::loan::fault::reads(), 0);
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    drop(injection);
    owner.core.evidence.demand = demand;
    owner.core.evidence.source.axis = source_axis;
    owner.core.host.declaration.axis = output_axis;
    if !publication {
        owner = candidate(owner, &mut setup.account)?;
    }
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    assert!(
        tile.reader(&setup.heap, &mut setup.account)?
            .contains(5, &mut setup.account)?
    );
    Ok(())
}
