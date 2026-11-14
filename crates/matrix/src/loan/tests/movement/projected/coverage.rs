// SPDX-License-Identifier: AGPL-3.0-only
//! Movement cannot relabel represented coverage as assumptions or the reverse.

use super::super::{Setup, TestResult, candidate, empty, publish};
use crate::{ApplicationError, ApplicationRequest, Coverage, Elementwise, LoanError};
use nrz_scalar::Scalar;

/// Both coverage mismatch directions refuse before reads and return a repairable request.
/// ´test:unit:loan-projected-coverage-mismatch´
#[test]
fn loan_projected_coverage_mismatch() -> TestResult {
    for coverage in [Coverage::RepresentedInputs, Coverage::DeclaredBounds] {
        mismatch::<f32>(coverage)?;
        mismatch::<f64>(coverage)?;
    }
    Ok(())
}

/// Test the shared movement check through copy and each projected unary entry.
fn mismatch<P: Scalar + 'static>(coverage: Coverage) -> TestResult {
    for function in [
        None,
        Some(Elementwise::Square),
        Some(Elementwise::Reciprocal),
        Some(Elementwise::SquareRoot),
    ] {
        let Setup {
            mut heap,
            mut account,
            mut request,
            demand,
        } = super::unary::domain_setup([[P::from_u8(5); 2]; 3], [[P::ONE; 2]; 3], coverage)?;
        if let Some(function) = function {
            super::unary::operation(&mut request, function)?;
        }
        let wrong = match coverage {
            Coverage::RepresentedInputs => Coverage::DeclaredBounds,
            Coverage::DeclaredBounds => Coverage::RepresentedInputs,
        };
        request.destination.coverage = wrong;
        let ticket = request.input.ticket()?;
        let before = heap.loans.elements()?;
        crate::loan::fault::reset_reads();
        let result = if function.is_some() {
            heap.request_projected_elementwise(request, &mut account)
        } else {
            heap.request_mapped_copy(request, &mut account)
        };
        let mut request = match result {
            ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::CoverageMismatch),
                request,
            ) => request,
            other => return Err(format!("{other:?}").into()),
        };
        assert_eq!(crate::loan::fault::reads(), 0);
        assert_eq!(heap.loans.elements()?, before);
        assert_eq!(request.input.ticket()?, ticket);
        assert_eq!(request.demand, demand);
        assert_eq!(request.destination.coverage, wrong);
        request.destination.coverage = coverage;
        let result = if function.is_some() {
            heap.request_projected_elementwise(request, &mut account)
        } else {
            heap.request_mapped_copy(request, &mut account)
        };
        let owner = match result {
            ApplicationRequest::ApplicationReady(owner) => owner,
            other @ ApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        };
        let owner = candidate(owner, &mut account)?;
        let tile = publish(&mut heap, &mut account, owner)?;
        assert_eq!(tile.reader(&heap, &mut account)?.coverage(), coverage);
        drop(tile);
        heap.retire_mask(demand, &mut account.ledger)?;
        super::super::retire_copy_layout(&mut heap, &mut account, demand.layout())?;
        empty(&mut heap, &mut account)?;
    }
    Ok(())
}

/// Published assumption-bound results retain readable coverage but cannot start a current lineage.
/// ´test:unit:loan-projected-declared-bounds-terminal´
#[test]
fn loan_projected_declared_bounds_terminal() -> TestResult {
    terminal::<f32>()?;
    terminal::<f64>()
}

/// All projected unary producers encounter the same represented-input lineage requirement.
fn terminal<P: Scalar + 'static>() -> TestResult {
    for function in [
        Elementwise::Square,
        Elementwise::Reciprocal,
        Elementwise::SquareRoot,
    ] {
        let Setup {
            mut heap,
            mut account,
            mut request,
            ..
        } = super::unary::domain_setup(
            [[P::from_u8(5); 2]; 3],
            [[P::ONE; 2]; 3],
            Coverage::DeclaredBounds,
        )?;
        super::unary::operation(&mut request, function)?;
        let owner = match heap.request_projected_elementwise(request, &mut account) {
            ApplicationRequest::ApplicationReady(owner) => owner,
            other @ ApplicationRequest::ApplicationRefused(..) => {
                return Err(format!("{other:?}").into());
            }
        };
        let owner = candidate(owner, &mut account)?;
        let tile = publish(&mut heap, &mut account, owner)?;
        let mut request = super::super::next_copy(&mut heap, &mut account, tile.into_pin())?;
        request.destination.coverage = Coverage::DeclaredBounds;
        let ticket = request.input.ticket()?;
        crate::loan::fault::reset_reads();
        let injection = crate::loan::fault::Injection::after(0);
        let request = match heap.request_mapped_copy(request, &mut account) {
            ApplicationRequest::ApplicationRefused(
                ApplicationError::Loan(LoanError::CoverageMismatch),
                request,
            ) => request,
            other => return Err(format!("{other:?}").into()),
        };
        assert_eq!(request.input.ticket()?, ticket);
        assert_eq!(crate::loan::fault::reads(), 0);
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        drop(injection);
        assert_eq!(
            request.input.historical_reader(&mut account)?.coverage(),
            Coverage::DeclaredBounds
        );
    }
    Ok(())
}
