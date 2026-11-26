// SPDX-License-Identifier: AGPL-3.0-only
//! Public originals, native masks and checked maps all precede executable graph admission.
use nrz_bindings::{
    BinaryOp, BindingSymbols, Budget, Descriptor, ExponentPolicy, Kind, LcForwardCover, LcInput,
    LcInterval, LcOmegaCorrespondence, LcOmegaDeclarationRequest, LcOmegaDeclarationToken,
    LcQuantity, LcRelation, Limits, Measure, ReferenceDefinition, Registry, Role,
    readings::{
        EntryMeaning,
        algebra::{Rational, enclosure::lc_root::Certificate, pi_bounds::PiBoundCertificate},
    },
};
use nrz_matrix::*;
use nrz_scalar::Scalar;
pub(super) type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

type Selection<'i, 's> = (&'i Option<Certificate>, &'s [usize], bool);

/// Caller keeps these originals alive independently of all graph owners and native results.
pub(super) struct Setup<P: Scalar> {
    pub(super) heap: Heap,
    pub(super) account: ExecutionAccount,
    pub(super) inventory: LcInputInventory<P, 5>,
    pub(super) axes: LcShapeAxes,
    pub(super) registry: Registry,
    pub(super) radian: EntryMeaning,
    pub(super) certificate: Option<Certificate>,
    pub(super) equal_question: LcForwardCover,
}
impl<P: Scalar> Setup<P> {
    /// All exact preparation moves into one execution ledger; no later operation replaces it.
    pub(super) fn new(replay: bool) -> Result<Self> {
        Self::with_certificate(replay, false)
    }
    pub(super) fn with_certificate(replay: bool, malformed: bool) -> Result<Self> {
        Self::with_limits(replay, malformed, Limits::default(), [2, 3, 2])
    }
    pub(super) fn with_limits(
        replay: bool,
        malformed: bool,
        limits: Limits,
        counts: [usize; 3],
    ) -> Result<Self> {
        Self::with_remaining(replay, malformed, limits, counts, None)
    }
    pub(super) fn with_remaining(
        replay: bool,
        malformed: bool,
        limits: Limits,
        counts: [usize; 3],
        remaining: Option<u64>,
    ) -> Result<Self> {
        let mut budget = Budget::new(limits)?;
        let equal_question = question(&mut budget)?;
        let question = question(&mut budget)?;
        let angle = Descriptor::new(Kind::Angle, Measure::coherent(), Role::Difference)?;
        let radian = EntryMeaning::new(
            angle.clone(),
            ReferenceDefinition::unit("output radian", &angle)?,
            None,
            &mut budget,
        )?;
        let mut registry = Registry::new(ExponentPolicy::Rational);
        registry.register_binary(
            BinaryOp::Multiply,
            Kind::rate()?,
            Kind::Angle,
            Kind::angular_frequency()?,
        )?;
        let mut certificate = if replay {
            Some(certificate(&question, &registry, &radian, &mut budget)?)
        } else {
            None
        };
        if malformed {
            let mut bytes = certificate
                .as_ref()
                .ok_or("replay fixture")?
                .to_bytes(&mut budget)?;
            bytes[..8].copy_from_slice(b"NRZENC01");
            certificate = Some(Certificate::from_bytes(&bytes, &mut budget)?);
        }
        let heap = Heap::new(HeapLimits::default())?;
        let instances = heap.create_cell_axis(counts[0], &mut budget)?;
        let samples = heap.create_cell_axis(counts[1], &mut budget)?;
        let branches = heap.create_cell_axis(counts[2], &mut budget)?;
        let instance_samples = heap.create_cell_product(instances, samples, &mut budget)?;
        let sample_branches = heap.create_cell_product(samples, branches, &mut budget)?;
        let combined = heap.create_cell_product(instance_samples.axis(), branches, &mut budget)?;
        let axes = LcShapeAxes {
            instances,
            samples,
            branches,
            instance_samples,
            sample_branches,
            combined,
        };
        if let Some(remaining) = remaining {
            let spend = budget
                .limits()
                .work
                .checked_sub(budget.usage().work)
                .and_then(|value| value.checked_sub(remaining))
                .ok_or("fixture prior consumption")?;
            budget.work(
                spend,
                nrz_bindings::readings::algebra::Location::new("prior public graph work"),
            )?;
        }
        let mut account = ExecutionAccount::new(budget)?;
        let inventory = LcInputInventory::declare(&question, counts, &mut account)?;
        Ok(Self {
            heap,
            account,
            inventory,
            axes,
            registry,
            radian,
            certificate,
            equal_question,
        })
    }
}

/// Original rational input bounds determine every public numerical expectation.
fn input(quantity: LcQuantity, bounds: [&str; 2], budget: &mut Budget) -> Result<LcInput> {
    let lower = Rational::parse(bounds[0], budget)?;
    let upper = Rational::parse(bounds[1], budget)?;
    let interval = LcInterval::new(Some(lower.into()), Some(upper.into()), budget)?;
    Ok(LcInput::new(
        quantity,
        &quantity.descriptor()?,
        interval,
        budget,
    )?)
}
fn question(budget: &mut Budget) -> Result<LcForwardCover> {
    let mut inputs = Vec::new();
    for (quantity, bounds) in [
        (LcQuantity::Inductance, ["1/4", "1"]),
        (LcQuantity::Capacitance, ["1/4", "1"]),
        (LcQuantity::InitialPhase, ["0", "1/4"]),
        (LcQuantity::Energy, ["1", "1"]),
        (LcQuantity::Inductance, ["1/4", "49/64"]),
    ] {
        inputs.push(input(quantity, bounds, budget)?);
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

/// Portable bindings evidence is constructed from independently declared equivalent public originals.
fn certificate(
    question: &LcForwardCover,
    registry: &Registry,
    radian: &EntryMeaning,
    budget: &mut Budget,
) -> Result<Certificate> {
    let mut pairs = Vec::new();
    for input in question.relation().inputs() {
        pairs.push([
            BindingSymbols::declare(input.descriptor(), [2, 1], [true; 2], budget)?,
            BindingSymbols::declare(input.descriptor(), [12, 1], [true; 2], budget)?,
        ]);
    }
    let output = BindingSymbols::declare(
        &LcQuantity::AngularFrequency.descriptor()?,
        [12, 1],
        [true; 2],
        budget,
    )?;
    let declaration = LcOmegaDeclarationToken::admit(
        &LcOmegaDeclarationRequest {
            question,
            input_len: 5,
            input_at: |index| pairs.get(index),
            positions: [0, 1],
            counts: [2, 3, 2],
            output: &output,
            radian,
            registry,
        },
        budget,
    )?;
    let correspondence = LcOmegaCorrespondence::construct(&declaration, budget)?;
    Ok(correspondence.certificate(budget)?)
}

/// Fixed graph topology has five input/mapped pairs, one checked output and one delivery.
pub(super) fn admit<'i, P: Scalar + 'static>(
    inventory: &'i LcInputInventory<P, 5>,
    axes: &LcShapeAxes,
    registry: &'i Registry,
    radian: &EntryMeaning,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    selection: (&'i Option<Certificate>, &[usize], bool),
) -> Result<(LcGraphOwner<'i, P, 5>, Vec<MaskVersion>)> {
    admit_attempts(
        inventory,
        axes,
        registry,
        radian,
        heap,
        account,
        (selection, [8; 12]),
    )
}

/// Caller-selected finite attempt envelopes remain independent of numerical and Work ceilings.
pub(super) fn admit_attempts<'i, P: Scalar + 'static>(
    inventory: &'i LcInputInventory<P, 5>,
    axes: &LcShapeAxes,
    registry: &'i Registry,
    radian: &EntryMeaning,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    selection: (Selection<'i, '_>, [u64; 12]),
) -> Result<(LcGraphOwner<'i, P, 5>, Vec<MaskVersion>)> {
    admit_storage(
        inventory,
        axes,
        registry,
        radian,
        heap,
        account,
        (selection.0, selection.1, None),
    )
}

/// Tight public controls select their own finite evidence extents at original admission.
pub(super) fn admit_storage<'i, P: Scalar + 'static>(
    inventory: &'i LcInputInventory<P, 5>,
    axes: &LcShapeAxes,
    registry: &'i Registry,
    radian: &EntryMeaning,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
    selection: (Selection<'i, '_>, [u64; 12], Option<[usize; 3]>),
) -> Result<(LcGraphOwner<'i, P, 5>, Vec<MaskVersion>)> {
    let ((certificate, selected, extra), attempts, storage) = selection;

    let mut attempts = attempts.to_vec();
    let mut nodes = Vec::new();
    for position in 0..5 {
        nodes.push(LcGraphNode::input(
            inventory
                .input(position)
                .ok_or("input")?
                .source(axes, account)?,
        ));
    }
    let mut masks = Vec::new();
    for position in 0..5 {
        let schema = inventory.input(position).ok_or("schema")?;
        let mapping = mapping(schema, axes, heap, account)?;
        masks.push(mapping.demand);
        nodes.push(LcGraphNode::mapped(position, schema, axes, mapping));
    }
    let declared = LcOmegaOutput::declare(
        inventory,
        LcOutputDeclaration {
            positions: [0, 1],
            quantities: LcQuantityDemand::Omega,
            statistical: false,
            radian: radian.clone(),
            registry,
        },
        account,
    )?;
    let mask = demand(axes, heap, selected, account)?;
    masks.push(mask);
    nodes.push(LcGraphNode::omega(
        declared,
        [5, 6, 7, 8, 9],
        mask,
        certificate.as_ref(),
    ));
    nodes.push(LcGraphNode::delivery(10));
    if extra {
        attempts.push(8);
        nodes.push(LcGraphNode::input(
            inventory
                .input(0)
                .ok_or("extra input")?
                .source(axes, account)?,
        ));
    }
    let count = nodes.len();
    let [records, words, witnesses] = storage.unwrap_or([10 * count, 75 * count, 2]);
    let owner = LcGraphOwner::admit(
        nodes,
        &attempts,
        LcGraphCapacities {
            nodes: count,
            edges: 11,
            scopes: 2,
            drive: 1000,
            records,
            words,
            witnesses,
        },
        inventory,
        registry,
        &[],
        heap,
        account,
    )
    .map_err(|failure| failure.error)?;
    Ok((owner, masks))
}

pub(super) fn mapping<P: Scalar + 'static>(
    schema: &LcInputShape<P>,
    axes: &LcShapeAxes,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result<LcGraphMapping<P>> {
    let destination = schema.destination(axes, account)?;
    let demand = demand(
        axes,
        heap,
        &(0..axes.combined.axis().capacity()).collect::<Vec<_>>(),
        account,
    )?;
    let projection = schema.projection(axes, account)?;
    let map = heap.cell_entry_map_with_account(
        Extent::new(axes.instances.capacity(), 1)?,
        demand,
        Some((projection, CellFactor::Outer)),
        account,
    )?;
    let owner = crate::conversion_testing::owner()?;
    let descriptor = schema.question().relation().inputs()[schema.input_index()].descriptor();
    let source = crate::conversion_testing::shape(&owner, descriptor, axes.instances.capacity())?;
    let target =
        crate::conversion_testing::shape(&owner, descriptor, axes.combined.axis().capacity())?;
    let operation =
        OperationDraft::map_cell_copy(source, target, map)?.check(ExecutionLimits::default())?;
    Ok(LcGraphMapping {
        destination,
        demand,
        operation,
        limits: ExecutionLimits::default(),
    })
}
pub(super) fn demand(
    axes: &LcShapeAxes,
    heap: &mut Heap,
    selected: &[usize],
    account: &mut ExecutionAccount,
) -> Result<MaskVersion> {
    let layout = heap.create_cell_layout_with_account(
        axes.combined.axis(),
        Layout::contiguous(Extent::new(axes.combined.axis().capacity(), 1)?)?,
        account,
    )?;
    Ok(heap.create_mask_with_account(layout, selected, account)?)
}

/// All three channels are independently initialized; the last unused original still constrains coverage.
pub(super) fn fill<P: Scalar + 'static>(
    loan: &mut OwnedLoan<P>,
    position: usize,
    instances: usize,
    account: &mut ExecutionAccount,
) -> Result {
    let values = match position {
        0 | 4 => [[18, 2, 1], [22, 2, 1]],
        1 => [[12, 2, 1], [20, 2, 1]],
        2 => [[2, 1, 1], [4, 1, 1]],
        3 => [[32, 0, 0], [32, 0, 0]],
        _ => return Err("position".into()),
    };
    for (cell, triple) in values.into_iter().take(instances).enumerate() {
        for (channel, numerator) in [HostChannel::Center, HostChannel::Total, HostChannel::Error]
            .into_iter()
            .zip(triple)
        {
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
pub(super) fn request<P: Scalar + 'static>(
    owner: &mut LcGraphOwner<'_, P, 5>,
    handle: LcGraphHandle,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result<OwnedLoan<P>> {
    let request = owner.request_input(handle, heap, account, &[])?;
    let LcGraphInputRequest::Granted {
        loan,
        evidence: Ok(evidence),
        control,
    } = request
    else {
        return Err(format!("host issuance: {request:?}").into());
    };
    control?;
    owner.acknowledge_probe(evidence, account)?;
    Ok(loan)
}
pub(super) fn retire(
    masks: Vec<MaskVersion>,
    heap: &mut Heap,
    account: &mut ExecutionAccount,
) -> Result {
    for mask in masks {
        heap.retire_mask_with_account(mask, account)?;
        heap.residence(account, 100_000)?;
        heap.retire_cell_layout_with_account(mask.layout(), account)?;
    }
    Ok(())
}
