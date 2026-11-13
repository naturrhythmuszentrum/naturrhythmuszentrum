// SPDX-License-Identifier: AGPL-3.0-only
//! Entry-point separation and declared whole-box domain boundaries.

use super::super::{Setup, TestResult, candidate, empty, publish};
use super::unary::{domain_setup, operation};
use crate::kernel_oracle::Rational;
use crate::{ApplicationError, ApplicationRequest, Coverage, Elementwise, HostChannel, LoanError};
use nrz_scalar::Scalar;

/// Projected recipes require the explicit projected API, even with a valid application origin.
/// ´test:unit:loan-projected-unary-entry-refusals´
#[test]
fn loan_projected_unary_entry_refusals() -> TestResult {
    for function in [
        Elementwise::Square,
        Elementwise::Reciprocal,
        Elementwise::SquareRoot,
    ] {
        for chained in [false, true] {
            let (
                Setup {
                    mut heap,
                    mut account,
                    mut request,
                    demand,
                },
                original,
            ) = entry_setup(chained)?;
            operation(&mut request, function)?;
            crate::loan::fault::reset_reads();
            let before = heap.loans.elements()?;
            let result = match (function, chained) {
                (Elementwise::Square, false) => {
                    heap.request_pointwise_square(request, &mut account)
                }
                (Elementwise::Reciprocal, false) => {
                    heap.request_pointwise_reciprocal(request, &mut account)
                }
                (Elementwise::SquareRoot, false) => {
                    heap.request_pointwise_square_root(request, &mut account)
                }
                (Elementwise::Square, true) => {
                    heap.request_chained_pointwise_square(request, &mut account)
                }
                (Elementwise::Reciprocal, true) => {
                    heap.request_chained_pointwise_reciprocal(request, &mut account)
                }
                (Elementwise::SquareRoot, true) => {
                    heap.request_chained_pointwise_square_root(request, &mut account)
                }
            };
            let request = match result {
                ApplicationRequest::ApplicationRefused(
                    ApplicationError::Loan(LoanError::UnsupportedContract),
                    request,
                ) => request,
                other => return Err(format!("{other:?}").into()),
            };
            assert_eq!(crate::loan::fault::reads(), 0);
            assert_eq!(heap.loans.elements()?, before);
            // The identical returned request is otherwise admissible through its own entry point.
            let owner = match heap.request_projected_elementwise(request, &mut account) {
                ApplicationRequest::ApplicationReady(owner) => owner,
                other @ ApplicationRequest::ApplicationRefused(..) => {
                    return Err(format!("{other:?}").into());
                }
            };
            let owner = candidate(owner, &mut account)?;
            drop(publish(&mut heap, &mut account, owner)?);
            heap.retire_mask(demand, &mut account.ledger)?;
            // Application-owned layouts retire with their occupations during residence.
            if let Some(original) = original {
                heap.retire_mask(original, &mut account.ledger)?;
                super::super::retire_copy_layout(&mut heap, &mut account, original.layout())?;
            } else {
                super::super::retire_copy_layout(&mut heap, &mut account, demand.layout())?;
            }
            empty(&mut heap, &mut account)?;
        }
    }
    Ok(())
}

/// A real mapped publication supplies the chained control, without substituting private metadata.
fn entry_setup(chained: bool) -> TestResult<(Setup<f64>, Option<crate::MaskVersion>)> {
    let setup = super::super::setup::<f64>()?;
    if !chained {
        return Ok((setup, None));
    }
    let Setup {
        mut heap,
        mut account,
        request,
        demand: original,
    } = setup;
    let owner = super::super::ready(&mut heap, &mut account, request)?;
    let owner = candidate(owner, &mut account)?;
    let tile = publish(&mut heap, &mut account, owner)?;
    let mut request =
        super::super::arithmetic::square_request(&mut heap, &mut account, tile.into_pin())?;
    let map = heap.cell_entry_map(
        request.operation.inputs()[0].shape().extent(),
        request.demand,
        None,
        &mut account.ledger,
    )?;
    request.operation = super::super::operation_map(map)?;
    Ok((
        Setup {
            heap,
            account,
            demand: request.demand,
            request,
        },
        Some(original),
    ))
}

/// Declared bounds remain assumptions; the unary image encloses the entire [1,9] box.
/// ´test:unit:loan-projected-unary-declared-bounds´
#[test]
fn loan_projected_unary_declared_bounds() -> TestResult {
    for function in [
        Elementwise::Square,
        Elementwise::Reciprocal,
        Elementwise::SquareRoot,
    ] {
        declared::<f32>(function)?;
        declared::<f64>(function)?;
    }
    Ok(())
}

/// Rational endpoints are independent of production enclosure and include nonzero input uncertainty.
fn declared<P: Scalar + 'static>(function: Elementwise) -> TestResult {
    let Setup {
        mut heap,
        mut account,
        mut request,
        demand,
    } = domain_setup(
        [[P::from_u8(5); 2]; 3],
        [[P::from_u8(4); 2]; 3],
        Coverage::DeclaredBounds,
    )?;
    operation(&mut request, function)?;
    let owner = match heap.request_projected_elementwise(request, &mut account) {
        ApplicationRequest::ApplicationReady(owner) => owner,
        other @ ApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    let owner = candidate(owner, &mut account)?;
    let tile = publish(&mut heap, &mut account, owner)?;
    let reader = tile.reader(&heap, &mut account)?;
    assert_eq!(reader.coverage(), Coverage::DeclaredBounds);
    let endpoints = match function {
        Elementwise::Square => [Rational::new(1, 1)?, Rational::new(81, 1)?],
        Elementwise::Reciprocal => [Rational::new(1, 9)?, Rational::new(1, 1)?],
        Elementwise::SquareRoot => [Rational::new(1, 1)?, Rational::new(3, 1)?],
    };
    for cell in 0..3 {
        assert!(reader.contains(cell, &mut account)?);
        for component in 0..2 {
            let center = reader
                .read_component(HostChannel::Center, cell, component, &mut account)?
                .to_f64();
            let total = reader
                .read_component(HostChannel::Total, cell, component, &mut account)?
                .to_f64();
            for endpoint in &endpoints {
                assert!(endpoint.covered(center, total)?);
            }
        }
    }
    assert!(!reader.contains(3, &mut account)?);
    drop(tile);
    heap.retire_mask(demand, &mut account.ledger)?;
    super::super::retire_copy_layout(&mut heap, &mut account, demand.layout())?;
    empty(&mut heap, &mut account)
}

/// A negative separated box publishes; 2^-148 has reciprocal 2^148, above binary32 range.
/// Its outward lower endpoint stays positive, unlike the smallest subnormal at 2^-149.
/// A larger subnormal has the finite reciprocal 2^127 and survives the same traversal.
/// ´test:unit:loan-projected-unary-extreme-domains´
#[test]
fn loan_projected_unary_extreme_domains() -> TestResult {
    let Setup {
        mut heap,
        mut account,
        mut request,
        demand,
    } = domain_setup(
        [
            [-4.0_f32; 2],
            [f32::from_bits(2); 2],
            [f32::from_bits(1 << 22); 2],
        ],
        [[1.0; 2], [0.0; 2], [0.0; 2]],
        Coverage::RepresentedInputs,
    )?;
    operation(&mut request, Elementwise::Reciprocal)?;
    let owner = match heap.request_projected_elementwise(request, &mut account) {
        ApplicationRequest::ApplicationReady(owner) => owner,
        other @ ApplicationRequest::ApplicationRefused(..) => {
            return Err(format!("{other:?}").into());
        }
    };
    let owner = candidate(owner, &mut account)?;
    assert!(
        matches!(
            owner.core.evidence.outcomes[1],
            crate::CellOutcome::CellRefused {
                component: 0,
                reason: crate::KernelError::Domain {
                    function: Elementwise::Reciprocal,
                    entry: 1,
                    reason: crate::DomainReason::UnboundedImage,
                },
            }
        ),
        "unexpected overflow outcome: {:?}",
        owner.core.evidence.outcomes[1]
    );
    let tile = publish(&mut heap, &mut account, owner)?;
    let reader = tile.reader(&heap, &mut account)?;
    assert!(reader.contains(0, &mut account)?);
    assert!(!reader.contains(1, &mut account)?);
    assert!(reader.contains(2, &mut account)?);
    assert!(!reader.contains(3, &mut account)?);
    for component in 0..2 {
        let center =
            f64::from(reader.read_component(HostChannel::Center, 0, component, &mut account)?);
        let total =
            f64::from(reader.read_component(HostChannel::Total, 0, component, &mut account)?);
        for denominator in [3, 5] {
            assert!(Rational::new(-1, denominator)?.covered(center, total)?);
        }
        let center =
            f64::from(reader.read_component(HostChannel::Center, 2, component, &mut account)?);
        let total =
            f64::from(reader.read_component(HostChannel::Total, 2, component, &mut account)?);
        assert!(center.is_finite());
        assert!(total.is_finite());
        assert!((center - 2.0_f64.powi(127)).abs() <= total);
    }
    drop(tile);
    heap.retire_mask(demand, &mut account.ledger)?;
    super::super::retire_copy_layout(&mut heap, &mut account, demand.layout())?;
    empty(&mut heap, &mut account)
}
