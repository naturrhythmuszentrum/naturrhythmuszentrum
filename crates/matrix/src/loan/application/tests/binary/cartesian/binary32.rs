// SPDX-License-Identifier: AGPL-3.0-only
//! Native binary32 companions retain the same refusal and detached ownership boundary.

use super::{BinaryElementwise, TestResult, admit, candidate, fixture, published, ready};

/// Allocation refusal preserves typed tickets; the same request then publishes native readings.
/// ´test:unit:loan-cartesian-binary32-request-retry´
#[test]
fn loan_cartesian_binary32_request_retry() -> TestResult {
    let function = BinaryElementwise::Quotient;
    let (mut setup, _) = fixture::setup::<f32>([3, 2], 2, false, function)?;
    let tickets = [setup.request.left.ticket()?, setup.request.right.ticket()?];
    crate::loan::fault::reset_reads();
    let injection = crate::loan::fault::Injection::after(0);
    let request = match admit(&mut setup.heap, setup.request, function, &mut setup.account) {
        crate::BinaryApplicationRequest::ApplicationRefused(
            crate::ApplicationError::Loan(crate::LoanError::AllocationRefused),
            request,
        ) => request,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!([request.left.ticket()?, request.right.ticket()?], tickets);
    assert_eq!(crate::loan::fault::reads(), 0);
    assert_eq!(crate::loan::fault::Injection::attempts(), 1);
    drop(injection);
    let owner = ready(admit(
        &mut setup.heap,
        request,
        function,
        &mut setup.account,
    ))?;
    let owner = candidate(owner, &mut setup.account)?;
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    let reader = tile.reader(&setup.heap, &mut setup.account)?;
    // -8/3 rounds to the larger-magnitude of its two adjacent binary32 values.
    assert_eq!(
        reader
            .read_component(crate::HostChannel::Center, 0, 0, &mut setup.account)?
            .to_bits(),
        0xc02a_aaab
    );
    Ok(())
}

/// Exact dyadic decoding checks native small/large and uncertain endpoint images independently.
/// ´test:unit:loan-cartesian-binary32-scalar-images´
#[test]
fn loan_cartesian_binary32_scalar_images() -> TestResult {
    use BinaryElementwise::{Product, Quotient};
    for (centers, radii, function) in [
        ([-2.0_f32, 3.0], [0.25_f32, 0.5], Product),
        ([f32::MIN_POSITIVE, 0.5], [0.0; 2], Product),
        ([f32::MAX / 4.0, 2.0], [0.0; 2], Quotient),
        ([1.0, 3.0], [0.0; 2], Quotient),
    ] {
        let mut setup = super::super::fixture::custom::<f32>(
            1,
            1,
            [2; 2],
            2,
            false,
            function,
            nrz_bindings::Limits::default(),
            |operand, _, _| (f64::from(centers[operand]), f64::from(radii[operand])),
        )?;
        let owner = ready(admit(
            &mut setup.heap,
            setup.request,
            function,
            &mut setup.account,
        ))?;
        let owner = candidate(owner, &mut setup.account)?;
        let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
        let reader = tile.reader(&setup.heap, &mut setup.account)?;
        let readings = super::super::values::readings(&reader, 0, 0, &mut setup.account)?;
        super::oracle::tight_native::<f32>(
            centers.map(f64::from),
            radii.map(f64::from),
            function,
            readings.map(f64::from),
        )?;
    }
    Ok(())
}

/// Domain failures retain their native binary law and the original target entry.
/// ´test:unit:loan-cartesian-binary32-scalar-domains´
#[test]
fn loan_cartesian_binary32_scalar_domains() -> TestResult {
    use BinaryElementwise::{Product, Quotient};
    let context = nrz_scalar::ArithmeticContext::new()?;
    for (centers, radii, function, expected) in [
        (
            [1.0_f32, 0.0],
            [0.0; 2],
            Quotient,
            crate::DomainReason::ZeroInDivisor,
        ),
        (
            [1.0, 1.0],
            [0.0, 1.0],
            Quotient,
            crate::DomainReason::ZeroInDivisor,
        ),
        (
            [f32::MAX, 2.0],
            [0.0; 2],
            Product,
            crate::DomainReason::UnboundedImage,
        ),
    ] {
        match crate::kernels::binary::evaluate_selected(centers, radii, function, 5, &context) {
            Err(crate::KernelError::BinaryDomain {
                function: actual,
                entry,
                reason,
            }) => {
                assert_eq!((actual, entry, reason), (function, 5, expected));
            }
            other => return Err(format!("{other:?}").into()),
        }
    }
    Ok(())
}
