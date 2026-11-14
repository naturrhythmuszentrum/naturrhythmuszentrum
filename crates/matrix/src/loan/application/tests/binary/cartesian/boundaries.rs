// SPDX-License-Identifier: AGPL-3.0-only
//! Independent domain, admission and representation boundaries for factor incidence.

use super::super::alter;
use super::{BinaryElementwise, TestResult, admit, candidate, fixture, oracle, published, ready};
use crate::{ApplicationError, BinaryApplicationRequest, LoanError};

/// One inner component crosses zero in every outer row; other signed boxes use exact corners.
/// ´test:unit:loan-cartesian-binary-domain´
#[test]
fn loan_cartesian_binary_domain() -> TestResult {
    loan_cartesian_binary_domain_format::<f32>()?;
    loan_cartesian_binary_domain_format::<f64>()
}

/// The identical boundary is required in both native formats.
fn loan_cartesian_binary_domain_format<P: nrz_scalar::Scalar + 'static>() -> TestResult {
    let function = BinaryElementwise::Quotient;
    let (mut setup, _) = fixture::setup_domain::<P>([3, 2], 2, false, function, true)?;
    let owner = ready(admit(
        &mut setup.heap,
        setup.request,
        function,
        &mut setup.account,
    ))?;
    let owner = candidate(owner, &mut setup.account)?;
    for cell in [1, 3, 5] {
        assert!(
            matches!(owner.core.evidence.outcomes[cell], crate::CellOutcome::CellRefused {
            component: 1,
            reason: crate::KernelError::BinaryDomain {
                function: BinaryElementwise::Quotient,
                entry,
                reason: crate::DomainReason::ZeroInDivisor,
            },
        } if entry == cell + 6)
        );
    }
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    let reader = tile.reader(&setup.heap, &mut setup.account)?;
    for cell in 0..6 {
        assert_eq!(reader.contains(cell, &mut setup.account)?, cell % 2 == 0);
        if cell % 2 == 0 {
            for component in 0..2 {
                let centers = [
                    fixture::center(0, cell / 2, component)?,
                    fixture::center(1, 0, component)?,
                ];
                let totals = [
                    0.25 + f64::from(u32::try_from(cell / 2)?) / 1024.0
                        + f64::from(u32::try_from(component)?) / 256.0,
                    0.5 + f64::from(u32::try_from(component)?) / 256.0,
                ];
                let readings =
                    super::super::values::readings(&reader, cell, component, &mut setup.account)?;
                oracle::tight_native::<P>(
                    centers,
                    totals,
                    function,
                    readings.map(nrz_scalar::Scalar::to_f64),
                )?;
            }
        }
    }
    Ok(())
}

/// C=6,k=2,h=6: ordinary admission 446; original factor entries subtract 14,
/// product-demand authentication adds 4, the third layout's scans add 4, and
/// mask preparation replaces 10 with (4+3+3)*6+4=64, giving 494.
/// ´test:unit:loan-cartesian-binary-admission-boundary´
#[test]
fn loan_cartesian_binary_admission_boundary() -> TestResult {
    loan_cartesian_binary_admission_boundary_format::<f32>()?;
    loan_cartesian_binary_admission_boundary_format::<f64>()
}

/// The identical boundary is required in both native formats.
fn loan_cartesian_binary_admission_boundary_format<P: nrz_scalar::Scalar + 'static>() -> TestResult
{
    for (rows, required) in [([3, 2], 494), ([35, 2], 1787)] {
        admission_boundary::<P>(rows, required)?;
    }
    Ok(())
}

/// For C=70,k=2,q=2,t=3,h=6 the ordinary inventory gives 1297.
/// Original factor entries subtract 206; geometry/layout visits add 8;
/// the 70 outcome initializations remain separate: replace only the ordinary
/// 10*q=20 mask visits with (4+3+3)*70+4*2=708, giving 1787.
/// Equivalently: 360 fixed + 214 original entries + 435 role initialization
/// + 708 factor-mask visits + 70 outcome initializations = 1787.
fn admission_boundary<P: nrz_scalar::Scalar + 'static>(
    rows: [usize; 2],
    required: u64,
) -> TestResult {
    for available in [required - 1, required] {
        let function = BinaryElementwise::Product;
        let (mut setup, _) = fixture::setup::<P>(rows, 1, false, function)?;
        let tickets = [setup.request.left.ticket()?, setup.request.right.ticket()?];
        let demand = setup.request.demand;
        let symbols = setup.request.destination.symbols.clone();
        let axis = setup.request.destination.axis;
        let coverage = setup.request.destination.coverage;
        let limits = setup.request.limits;
        let operation = std::rc::Rc::clone(&setup.request.operation.operation.inputs);
        let bytes = setup.heap.resource_bytes()?;
        super::super::leave(&mut setup.account, available)?;
        let before = setup.account.usage().work;
        crate::loan::fault::reset_reads();
        let injection = crate::loan::fault::Injection::after(if available == required {
            usize::MAX
        } else {
            0
        });
        match admit(&mut setup.heap, setup.request, function, &mut setup.account) {
            BinaryApplicationRequest::ApplicationReady(owner) => {
                assert_eq!(available, required);
                assert_eq!(setup.account.usage().work - before, required);
                assert!(matches!(
                    setup.heap.cancel_application(owner, &mut setup.account),
                    crate::ApplicationCancellation::ApplicationCancelled(_)
                ));
            }
            BinaryApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::Budget(_)),
                request,
            ) => {
                assert_eq!(available, required - 1);
                assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                assert_eq!(setup.heap.resource_bytes()?, bytes);
                assert_eq!([request.left.ticket()?, request.right.ticket()?], tickets);
                assert_eq!(request.demand, demand);
                assert_eq!(request.destination.axis, axis);
                assert_eq!(request.destination.coverage, coverage);
                assert_eq!(request.limits, limits);
                assert_eq!(request.destination.symbols.value(), symbols.value());
                assert_eq!(request.destination.symbols.bound(), symbols.bound());
                assert_eq!(request.destination.symbols.error(), symbols.error());
                assert!(std::rc::Rc::ptr_eq(
                    &request.operation.operation.inputs,
                    &operation
                ));
            }
            other @ BinaryApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        }
        assert_eq!(crate::loan::fault::reads(), 0);
        drop(injection);
    }
    Ok(())
}

/// Original row counts match both product factors, so only the component check can refuse.
/// ´test:unit:loan-cartesian-binary-component-refusal´
#[test]
fn loan_cartesian_binary_component_refusal() -> TestResult {
    loan_cartesian_binary_component_refusal_format::<f32>()?;
    loan_cartesian_binary_component_refusal_format::<f64>()
}

/// The identical boundary is required in both native formats.
fn loan_cartesian_binary_component_refusal_format<P: nrz_scalar::Scalar + 'static>() -> TestResult {
    for side in 0..2 {
        let function = BinaryElementwise::Product;
        let (mut setup, product) = fixture::setup::<P>([3, 2], 2, false, function)?;
        let pin = fixture::host(
            &mut setup.heap,
            &mut setup.account,
            product.factors()[side],
            side,
            false,
            1,
            false,
        )?;
        if side == 0 {
            setup.request.left = pin;
        } else {
            setup.request.right = pin;
        }
        let tickets = [setup.request.left.ticket()?, setup.request.right.ticket()?];
        crate::loan::fault::reset_reads();
        let injection = crate::loan::fault::Injection::after(0);
        let result = fixture::operation(
            [&setup.request.left, &setup.request.right],
            &setup.request.destination,
            product,
            setup.request.demand.layout(),
            function,
            &mut setup.account,
        );
        let error = result.expect_err("mismatched components");
        assert!(matches!(
            error.downcast_ref::<ApplicationError>(),
            Some(ApplicationError::Loan(LoanError::UnsupportedBinaryExtent))
        ));
        assert_eq!(crate::loan::fault::reads(), 0);
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        assert_eq!(
            [setup.request.left.ticket()?, setup.request.right.ticket()?],
            tickets
        );
        drop(injection);
    }
    Ok(())
}

/// A gather marker on either otherwise matching factor refuses at source authentication.
/// ´test:unit:loan-cartesian-binary-gathered-factor-refusal´
#[test]
fn loan_cartesian_binary_gathered_factor_refusal() -> TestResult {
    loan_cartesian_binary_gathered_factor_refusal_format::<f32>()?;
    loan_cartesian_binary_gathered_factor_refusal_format::<f64>()
}

/// The identical boundary is required in both native formats.
fn loan_cartesian_binary_gathered_factor_refusal_format<P: nrz_scalar::Scalar + 'static>()
-> TestResult {
    for side in 0..2 {
        let function = BinaryElementwise::Product;
        let (mut setup, product) = fixture::setup::<P>([3, 2], 2, false, function)?;
        let pin = if side == 0 {
            &mut setup.request.left
        } else {
            &mut setup.request.right
        };
        let original = add_gather(&mut setup.heap, pin)?;
        let tickets = [setup.request.left.ticket()?, setup.request.right.ticket()?];
        crate::loan::fault::reset_reads();
        let injection = crate::loan::fault::Injection::after(0);
        let result = fixture::operation(
            [&setup.request.left, &setup.request.right],
            &setup.request.destination,
            product,
            setup.request.demand.layout(),
            function,
            &mut setup.account,
        );
        let error = result.expect_err("gathered factor");
        assert!(matches!(
            error.downcast_ref::<ApplicationError>(),
            Some(ApplicationError::Loan(LoanError::LayoutMismatch))
        ));
        assert_eq!(crate::loan::fault::reads(), 0);
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        assert_eq!(
            [setup.request.left.ticket()?, setup.request.right.ticket()?],
            tickets
        );
        drop(injection);
        let pin = if side == 0 {
            &mut setup.request.left
        } else {
            &mut setup.request.right
        };
        alter(&mut setup.heap, pin, |bundle| {
            bundle.layout = original;
            Ok(())
        })?;
        setup.request.operation = fixture::operation(
            [&setup.request.left, &setup.request.right],
            &setup.request.destination,
            product,
            setup.request.demand.layout(),
            function,
            &mut setup.account,
        )?;
        ready(admit(
            &mut setup.heap,
            setup.request,
            function,
            &mut setup.account,
        ))?;
    }
    Ok(())
}

/// Keep every earlier source predicate unchanged, isolating the gather exclusion itself.
fn add_gather<P: nrz_scalar::Scalar + 'static>(
    heap: &mut crate::Heap,
    pin: &mut crate::BundlePin<P>,
) -> TestResult<std::rc::Rc<crate::mask::storage::LayoutRecord>> {
    let mut original = None;
    alter(heap, pin, |bundle| {
        original = Some(std::rc::Rc::clone(&bundle.layout));
        let mask = bundle.identity.mask;
        bundle.layout = std::rc::Rc::new(crate::mask::storage::LayoutRecord {
            axis: bundle.layout.axis,
            components: bundle.layout.components.clone(),
            gather: Some(crate::CellGather {
                parent: mask,
                source: mask,
                axis: bundle.layout.axis,
                indices: (0..bundle.declaration.extent.rows()).collect(),
            }),
        });
        Ok(())
    })?;
    original.ok_or_else(|| "original layout".into())
}
