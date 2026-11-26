// SPDX-License-Identifier: AGPL-3.0-only
//! Real LC source and mapped request parts are declared before any input loan is issued.

use super::super::{
    admission::Refusal,
    node::{Mapping, Node},
    owner::{Capacities, Owner},
};
use crate::loan::lc_shape::tests::output::{demand, request};
use crate::{ExecutionAccount, ExecutionLimits, Heap, LcInputShape, LcOmegaOutput, LcShapeAxes};
use nrz_scalar::Scalar;

/// Preserve diagnostic causes without test output side effects.
pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Twelve nodes have five mapped edges, five checked positions and one sealed delivery edge.
pub(super) fn admit<'i, P: Scalar + 'static>(
    inventory: &'i crate::LcInputInventory<P, 5>,
    axes: &LcShapeAxes,
    registry: &'i nrz_bindings::Registry,
    radian: &nrz_bindings::readings::EntryMeaning,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> TestResult<(Owner<'i, P, 5>, Vec<crate::MaskVersion>)> {
    admit_attempts(inventory, axes, registry, radian, heap, account, [8; 12])
}

/// The codec contract needs one outcome and one exhaustion record per node, plus eight words per probe.
pub(super) fn admit_attempts<'i, P: Scalar + 'static>(
    inventory: &'i crate::LcInputInventory<P, 5>,
    axes: &LcShapeAxes,
    registry: &'i nrz_bindings::Registry,
    radian: &nrz_bindings::readings::EntryMeaning,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    attempts: [u64; 12],
) -> TestResult<(Owner<'i, P, 5>, Vec<crate::MaskVersion>)> {
    admit_selected(
        inventory,
        axes,
        registry,
        radian,
        heap,
        account,
        (attempts, &(0..12).collect::<Vec<_>>(), None),
    )
}

/// Selected output demand changes no original input constraint or mapped producer.
pub(super) fn admit_selected<'i, P: Scalar + 'static>(
    inventory: &'i crate::LcInputInventory<P, 5>,
    axes: &LcShapeAxes,
    registry: &'i nrz_bindings::Registry,
    radian: &nrz_bindings::readings::EntryMeaning,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    selection: (
        [u64; 12],
        &[usize],
        Option<&'i nrz_bindings::readings::algebra::enclosure::lc_root::Certificate>,
    ),
) -> TestResult<(Owner<'i, P, 5>, Vec<crate::MaskVersion>)> {
    let (attempts, selected, certificate) = selection;
    admit_extended(
        inventory,
        axes,
        registry,
        radian,
        heap,
        account,
        (attempts, selected, certificate, false),
    )
}

/// An independent expectation keeps a real host loan live beside a published checked reader.
pub(super) fn admit_extended<'i, P: Scalar + 'static>(
    inventory: &'i crate::LcInputInventory<P, 5>,
    axes: &LcShapeAxes,
    registry: &'i nrz_bindings::Registry,
    radian: &nrz_bindings::readings::EntryMeaning,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    selection: (
        [u64; 12],
        &[usize],
        Option<&'i nrz_bindings::readings::algebra::enclosure::lc_root::Certificate>,
        bool,
    ),
) -> TestResult<(Owner<'i, P, 5>, Vec<crate::MaskVersion>)> {
    let (attempts, selected, certificate, extra) = selection;
    let mut attempts = attempts.to_vec();
    let mut nodes = Vec::new();
    for position in 0..5 {
        let schema = inventory.input(position).ok_or("original input")?;
        nodes.push(Node::input(schema.source(axes, account)?));
    }
    let mut demands = Vec::new();
    for position in 0..5 {
        let schema = inventory.input(position).ok_or("original input")?;
        let parts = mapping(schema, axes, heap, account)?;
        demands.push(parts.demand);
        nodes.push(Node::mapped(position, schema, axes, parts));
    }
    let output = LcOmegaOutput::declare(inventory, request(registry, radian), account)?;
    let selected = demand(heap, axes, selected, account)?;
    demands.push(selected);
    nodes.push(Node::omega(output, [5, 6, 7, 8, 9], selected, certificate));
    nodes.push(Node::delivery(10));
    if extra {
        nodes.push(Node::input(
            inventory
                .input(0)
                .ok_or("extra original")?
                .source(axes, account)?,
        ));
        attempts.push(8);
    }
    let count = nodes.len();
    let probes = usize::try_from(attempts.iter().sum::<u64>())?;
    let owner = Owner::admit(
        nodes,
        &attempts,
        Capacities {
            nodes: count,
            edges: 11,
            scopes: 2,
            drive: 1_000,
            records: 2 * count + probes,
            words: 11 * count + 8 * probes,
            witnesses: 2,
        },
        inventory,
        registry,
        &[],
        heap,
        account,
    )
    .map_err(|Refusal { error, .. }| error)?;
    assert_eq!(owner.backing.cleanup.allocations, 4); // native slots, boundary stamps, observer and fixed header.
    Ok((owner, demands))
}

/// The checked native cell map determines the original instance index; no source pin is fabricated.
pub(super) fn mapping<P: Scalar + 'static>(
    schema: &LcInputShape<P>,
    axes: &LcShapeAxes,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> TestResult<Mapping<P>> {
    let destination = schema.destination(axes, account)?;
    let selected = demand(
        heap,
        axes,
        &(0..axes.combined.axis().capacity()).collect::<Vec<_>>(),
        account,
    )?;
    let map = heap.cell_entry_map(
        crate::Extent::new(axes.instances.capacity(), 1)?,
        selected,
        Some((schema.projection(axes, account)?, crate::CellFactor::Outer)),
        &mut account.ledger,
    )?;
    let owner = crate::conversion_testing::owner()?;
    let quantity = schema.question().relation().inputs()[schema.input_index()].descriptor();
    let source = crate::conversion_testing::shape(&owner, quantity, axes.instances.capacity())?;
    let destination_shape =
        crate::conversion_testing::shape(&owner, quantity, axes.combined.axis().capacity())?;
    let operation = crate::OperationDraft::map_cell_copy(source, destination_shape, map)?
        .check(ExecutionLimits::default())?;
    Ok(Mapping {
        destination,
        demand: selected,
        operation,
        limits: ExecutionLimits::default(),
    })
}

/// Independent dyadic inputs lie inside each original constraint, including the repeated inductance.
pub(super) fn fill<P: Scalar + 'static>(
    loan: &mut crate::OwnedLoan<P>,
    position: usize,
    account: &mut ExecutionAccount,
) -> TestResult {
    fill_prefix(loan, position, 2, account)
}

/// A missing instance remains genuinely unwritten in every native companion channel.
pub(super) fn fill_prefix<P: Scalar + 'static>(
    loan: &mut crate::OwnedLoan<P>,
    position: usize,
    instances: usize,
    account: &mut ExecutionAccount,
) -> TestResult {
    let numerators = match position {
        0 | 4 => [[18, 2, 1], [22, 2, 1]],
        1 => [[12, 2, 1], [20, 2, 1]],
        2 => [[2, 1, 1], [4, 1, 1]],
        3 => [[32, 0, 0], [32, 0, 0]],
        _ => return Err("original position".into()),
    };
    for (cell, values) in numerators.into_iter().take(instances).enumerate() {
        for (channel, numerator) in crate::HostChannel::ALL.into_iter().zip(values) {
            loan.write_component(
                channel,
                cell,
                0,
                P::from_u8(numerator) / P::from_u8(32),
                account,
            )?;
        }
    }
    Ok(())
}

/// Publish one genuine original host capability before driving its graph consumers.
pub(super) fn publish_input<P: Scalar + 'static>(
    owner: &mut Owner<'_, P, 5>,
    position: usize,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> TestResult<crate::loan::graph::walk::Handle> {
    use super::super::input::{InputRequest, InputReturn};
    let handle = owner.handle(position, account)?;
    let InputRequest::Granted {
        mut loan,
        evidence: Ok(evidence),
        control,
    } = owner.request_input(handle, heap, account, &[])?
    else {
        return Err("host issuance".into());
    };
    control?;
    owner.acknowledge_probe(evidence, account)?;
    fill(&mut loan, position, account)?;
    let InputReturn::Published { control } = owner.return_input(handle, loan, heap, account) else {
        return Err("host publication".into());
    };
    control?;
    Ok(handle)
}
