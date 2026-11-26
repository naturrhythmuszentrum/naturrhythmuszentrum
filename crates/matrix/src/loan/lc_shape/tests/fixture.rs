// SPDX-License-Identifier: AGPL-3.0-only
//! A real checked forward question supplies its original physical input and complete branch domain.

use super::{LcInputShape, LcShapeAxes};
use crate::{BundlePin, ExecutionAccount, ExecutionLimits, Heap, PointwiseRequest};
use nrz_bindings::readings::algebra::{Rational, pi_bounds::PiBoundCertificate};
use nrz_bindings::{Budget, LcForwardCover, LcInput, LcInterval, LcQuantity, LcRelation, Limits};
use nrz_scalar::Scalar;

pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// All exact preparation uses this single original ledger, never a replenished allowance.
fn input(quantity: LcQuantity, endpoints: [&str; 2], budget: &mut Budget) -> TestResult<LcInput> {
    let lower = Rational::parse(endpoints[0], budget)?;
    let upper = Rational::parse(endpoints[1], budget)?;
    let interval = LcInterval::new(Some(lower.into()), Some(upper.into()), budget)?;
    Ok(LcInput::new(
        quantity,
        &quantity.descriptor()?,
        interval,
        budget,
    )?)
}
/// Independent original element, phase and energy supports produce exactly branches zero and one.
pub(super) fn question(budget: &mut Budget) -> TestResult<LcForwardCover> {
    inventory_question(true, false, budget)
}

/// Optional energy and repeated L constraints retain their original positional identities.
pub(in crate::loan) fn inventory_question(
    energy: bool,
    repeated: bool,
    budget: &mut Budget,
) -> TestResult<LcForwardCover> {
    let mut inputs = Vec::new();
    for (quantity, bounds) in [
        (LcQuantity::Inductance, ["1/4", "1"]),
        (LcQuantity::Capacitance, ["1/4", "1"]),
        (LcQuantity::InitialPhase, ["0", "0"]),
    ] {
        let phase = quantity == LcQuantity::InitialPhase;
        let bounds = if repeated && phase {
            ["0", "1/4"]
        } else {
            bounds
        };
        inputs.push(input(quantity, bounds, budget)?);
    }
    if energy {
        inputs.push(input(LcQuantity::Energy, ["1", "1"], budget)?);
    }
    if repeated {
        inputs.push(input(LcQuantity::Inductance, ["1/4", "49/64"], budget)?);
    }
    let relation = LcRelation::new("original LC shape epoch".into(), 0, inputs, budget)?;
    let time = input(LcQuantity::Time(0), ["2", "2"], budget)?;
    let cycles = [Rational::parse("0", budget)?, Rational::parse("1", budget)?];
    let pi = PiBoundCertificate {
        version: 1,
        terms: [2, 1],
        sums: [
            Rational::parse("74/375", budget)?,
            Rational::parse("1/239", budget)?,
        ],
        lower: Rational::parse("31/10", budget)?,
        upper: Rational::parse("22/7", budget)?,
    };
    Ok(relation.forward_cover(time, cycles, &pi, budget)?)
}
/// Fresh native roles never substitute for the retained original mathematical question.
pub(super) fn setup<P: Scalar + 'static>()
-> TestResult<(Heap, ExecutionAccount, LcInputShape<P>, LcShapeAxes)> {
    let mut budget = Budget::new(Limits::default())?;
    let question = question(&mut budget)?;
    let mut account = ExecutionAccount::new(budget)?;
    let schema = LcInputShape::declare(question, 0, [2, 3, 2], &mut account)?;
    let heap = Heap::new(crate::HeapLimits::default())?;
    let axes = axes(&heap, &mut account)?;
    Ok((heap, account, schema, axes))
}

/// The fixed native history shared by single-role and complete-inventory tests.
pub(super) fn axes(heap: &Heap, account: &mut ExecutionAccount) -> TestResult<LcShapeAxes> {
    let instances = heap.create_cell_axis(2, &mut account.ledger)?;
    let samples = heap.create_cell_axis(3, &mut account.ledger)?;
    let branches = heap.create_cell_axis(2, &mut account.ledger)?;
    let instance_samples = heap.create_cell_product(instances, samples, &mut account.ledger)?;
    let sample_branches = heap.create_cell_product(samples, branches, &mut account.ledger)?;
    let combined =
        heap.create_cell_product(instance_samples.axis(), branches, &mut account.ledger)?;
    Ok(LcShapeAxes {
        instances,
        samples,
        branches,
        instance_samples,
        combined,
        sample_branches,
    })
}

/// Native mapped publication is declared before any evaluation or output allocation.
pub(super) fn request<P: Scalar + 'static>(
    schema: &LcInputShape<P>,
    axes: &LcShapeAxes,
    input: BundlePin<P>,
    cells: &[usize],
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> TestResult<PointwiseRequest<P>> {
    let destination = schema.destination(axes, account)?;
    let layout = heap.create_cell_layout(
        axes.combined.axis(),
        crate::Layout::contiguous(crate::Extent::new(axes.combined.axis().capacity(), 1)?)?,
        &mut account.ledger,
    )?;
    let demand = heap.create_mask(layout, cells, &mut account.ledger)?;
    let map = heap.cell_entry_map(
        crate::Extent::new(axes.instances.capacity(), 1)?,
        demand,
        Some((schema.projection(axes, account)?, crate::CellFactor::Outer)),
        &mut account.ledger,
    )?;
    let owner = crate::conversion_testing::owner()?;
    let quantity = schema.question().relation().inputs()[schema.input_index()].descriptor();
    let input_shape =
        crate::conversion_testing::shape(&owner, quantity, axes.instances.capacity())?;
    let output_shape =
        crate::conversion_testing::shape(&owner, quantity, axes.combined.axis().capacity())?;
    let operation = crate::OperationDraft::map_cell_copy(input_shape, output_shape, map)?
        .check(ExecutionLimits::default())?;
    Ok(PointwiseRequest {
        operation,
        input,
        destination,
        demand,
        limits: ExecutionLimits::default(),
    })
}

/// Sole successful loan admission; every refusal retains its typed cause.
pub(super) fn granted<P: Scalar>(
    request: crate::LoanRequest<P>,
) -> TestResult<crate::OwnedLoan<P>> {
    match request {
        crate::LoanRequest::Granted(loan) => Ok(loan),
        crate::LoanRequest::Refused(error) => Err(error.into()),
    }
}
/// Local scans must reconcile all numerical and prepaid ownership debits.
pub(super) fn empty(heap: &mut Heap, account: &mut ExecutionAccount) -> TestResult {
    let result = heap.residence(account, 100_000)?;
    assert!(
        matches!(result, crate::Residence::Empty { .. }),
        "{result:?}"
    );
    assert_eq!(heap.loans.elements()?, [0, 0]);
    assert!(heap.loans.debits.iter().all(Option::is_none));
    Ok(())
}

/// Native demand belongs to the caller and must be released after numerical captures retire.
pub(super) fn retire(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    demand: crate::MaskVersion,
) -> TestResult {
    heap.retire_mask(demand, &mut account.ledger)?;
    retire_layout(heap, account, demand.layout())?;
    empty(heap, account)
}
/// First release the completed numerical groups, then the caller's retained layout root.
pub(super) fn retire_layout(
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    layout: crate::CellLayoutVersion,
) -> TestResult {
    heap.residence(account, 100_000)?;
    heap.retire_cell_layout(layout, &mut account.ledger)?;
    Ok(())
}
