// SPDX-License-Identifier: AGPL-3.0-only
//! Historical endpoints authenticate the exact published completed-mask owner, including empty masks.
use super::{Fixture, TestResult, candidate, fixture, published, ready};
use std::rc::Rc;

/// Equal layout, word length and even equal contents do not replace the original completed owner.
#[test]
fn prepared_historical_swapped_completed_mask_refuses_before_any_load() -> TestResult {
    for cells in [0, 1] {
        let Fixture {
            mut heap,
            mut account,
            request,
        } = fixture(cells, 1, [-2.0, 0.0], 2, false)?;
        let owner = candidate(
            ready(heap.request_pointwise_expm1(request, &mut account))?,
            &mut account,
        )?;
        let (tile, _) = published(heap.publish_application(owner, &mut account))?;
        let mut pin = tile.into_pin();
        drop(heap);
        assert!(pin.historical_reader(&mut account).is_ok());
        let record = Rc::get_mut(&mut pin.record)
            .and_then(|record| record.payload.as_mut())
            .ok_or("unique pin")?;
        let bundle = Rc::get_mut(&mut record.bundle)
            .and_then(|owner| owner.payload.as_mut())
            .ok_or("unique bundle")?;
        bundle.mask = Rc::new(crate::mask::storage::MaskRecord {
            layout: bundle.mask.layout,
            words: bundle.mask.words.clone(),
        });
        crate::loan::fault::reset_reads();
        assert!(matches!(
            pin.historical_reader(&mut account),
            Err(crate::LoanError::WrongLoan)
        ));
        assert_eq!(crate::loan::fault::reads(), 0);
    }
    Ok(())
}
/// Dense binding must refuse the prepared recipe before it can reserve or lower numerical storage.
#[test]
fn prepared_dense_binding_has_no_adapter() -> TestResult {
    let Fixture {
        mut heap, request, ..
    } = fixture(1, 1, [-2.0, 0.0], 2, false)?;
    let source = heap.import_represented(
        crate::Extent::new(1, 1)?,
        &nrz_bindings::Descriptor::ratio(),
        &[-0.5_f64],
    )?;
    let binding = crate::Binding::new(
        request.operation.operation,
        vec![source.value()],
        crate::ExecutionLimits::default(),
    );
    let injection = crate::loan::fault::Injection::after(0);
    assert!(matches!(
        binding.validate(&mut heap),
        Err(crate::OperationError::Kernel(crate::KernelError::Contract))
    ));
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    drop(injection);
    Ok(())
}
