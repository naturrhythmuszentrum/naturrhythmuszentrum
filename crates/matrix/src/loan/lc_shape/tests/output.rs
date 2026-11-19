// SPDX-License-Identifier: AGPL-3.0-only
//! Independent requested tuples stay pending, with no value or host-import escape.

mod authority;
mod boundaries;
mod bridge;
mod pending;
use super::fixture::{self, TestResult};
use crate::{
    ExecutionAccount, Heap, LcInputInventory, LcOmegaOutput, LcOutputCellState,
    LcOutputDeclaration, LcOutputPublicationError, LcQuantityDemand, LcShapeAxes, LoanError,
};
use nrz_bindings::{
    BinaryOp, Budget, Descriptor, ExponentPolicy, Kind, Limits, Measure, ReferenceDefinition,
    Registry, Role, readings::EntryMeaning,
};
use nrz_scalar::Scalar;

struct Setup<P: Scalar> {
    heap: Heap,
    account: ExecutionAccount,
    inventory: LcInputInventory<P, 5>,
    axes: LcShapeAxes,
    registry: Registry,
    radian: EntryMeaning,
}
fn setup<P: Scalar>(limits: Limits, counts: [usize; 3]) -> TestResult<Setup<P>> {
    let mut budget = Budget::new(limits)?;
    let question = fixture::inventory_question(true, true, &mut budget)?;
    let mut account = ExecutionAccount::new(budget)?;
    let inventory = LcInputInventory::declare(&question, counts, &mut account)?;
    let angle = Descriptor::new(Kind::Angle, Measure::coherent(), Role::Difference)?;
    let radian = EntryMeaning::new(
        angle.clone(),
        ReferenceDefinition::unit("output radian", &angle)?,
        None,
        &mut account.ledger,
    )?;
    let mut registry = Registry::new(ExponentPolicy::Rational);
    registry.register_binary(
        BinaryOp::Multiply,
        Kind::rate()?,
        Kind::Angle,
        Kind::angular_frequency()?,
    )?;
    let heap = Heap::new(crate::HeapLimits::default())?;
    let instances = heap.create_cell_axis(counts[0], &mut account.ledger)?;
    let samples = heap.create_cell_axis(counts[1], &mut account.ledger)?;
    let branches = heap.create_cell_axis(counts[2], &mut account.ledger)?;
    let instance_samples = heap.create_cell_product(instances, samples, &mut account.ledger)?;
    let sample_branches = heap.create_cell_product(samples, branches, &mut account.ledger)?;
    let combined =
        heap.create_cell_product(instance_samples.axis(), branches, &mut account.ledger)?;
    Ok(Setup {
        heap,
        account,
        inventory,
        registry,
        radian,
        axes: LcShapeAxes {
            instances,
            samples,
            branches,
            instance_samples,
            combined,
            sample_branches,
        },
    })
}
fn request<'a>(registry: &'a Registry, radian: &EntryMeaning) -> LcOutputDeclaration<'a> {
    LcOutputDeclaration {
        positions: [0, 1],
        quantities: LcQuantityDemand::Omega,
        statistical: false,
        radian: radian.clone(),
        registry,
    }
}
fn demand(
    heap: &mut Heap,
    axes: &LcShapeAxes,
    cells: &[usize],
    account: &mut ExecutionAccount,
) -> TestResult<crate::MaskVersion> {
    let layout = heap.create_cell_layout(
        axes.combined.axis(),
        crate::Layout::contiguous(crate::Extent::new(axes.combined.axis().capacity(), 1)?)?,
        &mut account.ledger,
    )?;
    Ok(heap.create_mask(layout, cells, &mut account.ledger)?)
}

/// Every original tuple is pending or explicitly not requested; no publication changes that.
/// ´test:unit:loan-lc-output-pending-lifecycle´
#[test]
fn loan_lc_output_pending_lifecycle() -> TestResult {
    lifecycle::<f32>()?;
    lifecycle::<f64>()
}
fn lifecycle<P: Scalar + 'static>() -> TestResult {
    for selected in [vec![], vec![0, 3, 11], (0..12).collect()] {
        let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
        let schema = LcOmegaOutput::declare(
            &s.inventory,
            request(&s.registry, &s.radian),
            &mut s.account,
        )?;
        assert!(std::ptr::eq(
            schema.inventory(),
            std::ptr::from_ref(&s.inventory)
        ));
        assert_eq!(schema.positions(), [0, 1]);
        assert_eq!(schema.quantities(), LcQuantityDemand::Omega);
        assert_eq!(
            schema.symbols().value().descriptor().kind(),
            &Kind::angular_frequency()?
        );
        assert_eq!(schema.rate_descriptor().kind(), &Kind::rate()?);
        assert_eq!(schema.radian(), &s.radian);
        assert_eq!(
            schema.numerical_error_scope(),
            crate::NumericalErrorScope::LocalOperation
        );
        for companion in [schema.symbols().bound(), schema.symbols().error()] {
            let companion = companion.ok_or("required companion")?;
            assert_eq!(companion.target(), Some(schema.symbols().value().node()));
            assert_eq!(companion.shape(), [12, 1]);
        }
        let second = LcOmegaOutput::declare(
            &s.inventory,
            request(&s.registry, &s.radian),
            &mut s.account,
        )?;
        assert_ne!(schema.specialization_key(), second.specialization_key());
        let demand = demand(&mut s.heap, &s.axes, &selected, &mut s.account)?;
        let mut pending = schema.reserve(&mut s.heap, &s.axes, demand, &mut s.account)?;
        let ticket = pending.ticket()?;
        assert_eq!(pending.axes(), s.axes);
        assert_eq!(pending.demand(), demand);
        assert!(std::ptr::eq(pending.schema(), std::ptr::from_ref(&schema)));
        crate::loan::fault::reset_reads();
        crate::loan::fault::reset_scalars();
        let injection = crate::loan::fault::Injection::after(0);
        for _ in 0..2 {
            let before = s.account.usage().work;
            assert!(matches!(
                pending.publish(&s.heap, &mut s.account),
                Err(LcOutputPublicationError::OriginUnavailable)
            ));
            assert_eq!(s.account.usage().work - before, 5);
            assert_eq!(pending.ticket()?, ticket);
            for cell in 0..12 {
                let before = s.account.usage().work;
                assert_eq!(
                    pending.cell_state(cell, &mut s.account)?,
                    if selected.contains(&cell) {
                        LcOutputCellState::Pending
                    } else {
                        LcOutputCellState::NotRequested
                    }
                );
                assert_eq!(s.account.usage().work - before, 1);
            }
        }
        assert!(matches!(
            s.heap.mask(ticket.identity.mask, ticket.identity.layout),
            Err(crate::MaskError::Heap(crate::HeapError::OwnershipPending))
        ));
        assert_eq!(
            [
                crate::loan::fault::reads(),
                crate::loan::fault::scalars(),
                crate::loan::fault::Injection::attempts()
            ],
            [0; 3]
        );
        drop(injection);
        assert!(matches!(
            s.heap.retire_mask(demand, &mut s.account.ledger),
            Err(crate::MaskError::Heap(crate::HeapError::PinnedUses))
        ));
        let mut foreign = ExecutionAccount::new(Budget::new(Limits::default())?)?;
        assert!(matches!(
            pending.cancel(&mut s.heap, &mut foreign),
            Err(LoanError::ForeignAccount)
        ));
        assert_eq!(pending.ticket()?, ticket);
        let receipt = pending.cancel(&mut s.heap, &mut s.account)?;
        assert_eq!(receipt.ticket, ticket);
        for cell in selected {
            assert_eq!(
                pending.cell_state(cell, &mut s.account)?,
                LcOutputCellState::Cancelled
            );
        }
        assert!(pending.publish(&s.heap, &mut s.account).is_err());
        drop(pending);
        fixture::retire(&mut s.heap, &mut s.account, demand)?;
    }
    Ok(())
}

/// Unsupported quantities/statistics and changed original or native roles refuse before allocation.
/// ´test:unit:loan-lc-output-binding-refusals´
#[test]
fn loan_lc_output_binding_refusals() -> TestResult {
    refusals::<f32>()?;
    refusals::<f64>()
}
fn refusals<P: Scalar + 'static>() -> TestResult {
    let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
    for mode in 0..4 {
        let mut proposed = request(&s.registry, &s.radian);
        match mode {
            0 => proposed.statistical = true,
            1 => proposed.quantities = LcQuantityDemand::Period,
            2 => proposed.positions = [1, 0],
            _ => proposed.positions = [5, 1],
        }
        let injection = crate::loan::fault::Injection::after(0);
        assert!(LcOmegaOutput::declare(&s.inventory, proposed, &mut s.account).is_err());
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        drop(injection);
    }
    let schema = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    let selected = demand(&mut s.heap, &s.axes, &[3], &mut s.account)?;
    let mut changed = s.axes;
    changed.samples = s.heap.create_cell_axis(3, &mut s.account.ledger)?;
    let mut foreign = ExecutionAccount::new(Budget::new(Limits::default())?)?;
    let before = foreign.usage().work;
    let injection = crate::loan::fault::Injection::after(0);
    assert!(matches!(
        schema.reserve(&mut s.heap, &s.axes, selected, &mut foreign),
        Err(LoanError::ForeignAccount)
    ));
    assert_eq!(foreign.usage().work, before);
    assert!(matches!(
        schema.reserve(&mut s.heap, &changed, selected, &mut s.account),
        Err(LoanError::LayoutMismatch)
    ));
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    drop(injection);
    s.heap.retire_mask(selected, &mut s.account.ledger)?;
    let injection = crate::loan::fault::Injection::after(0);
    assert!(
        schema
            .reserve(&mut s.heap, &s.axes, selected, &mut s.account)
            .is_err()
    );
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    drop(injection);
    fixture::retire_layout(&mut s.heap, &mut s.account, selected.layout())?;
    fixture::empty(&mut s.heap, &mut s.account)
}
