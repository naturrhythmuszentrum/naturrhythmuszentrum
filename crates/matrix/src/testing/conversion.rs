// SPDX-License-Identifier: AGPL-3.0-only
//! Conversion fixtures build descriptor constants through units; numerical truth is independent.

use crate::test_api::{
    Axis, AxisEntry, BlockDraft, CheckedShape, ConversionContract, ConversionMode, EntryDraft,
    Partition, ShapeDraft, ShapeLimits, StructureScope,
};
use nrz_scalar::LiteralLimits;
use nrz_units::{
    CheckedDeclarations, CheckedMap, Context, Conversion, Descriptor, ExponentPolicy, Measure,
    Registry, Role, Scale,
};
use std::rc::Rc;

/// Fallible fixture and oracle result.
pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Construct small positive exact descriptor integers without a direct CAS dependency.
pub(crate) fn integer_measure(value: u32) -> TestResult<Measure> {
    let unit = Measure::coherent();
    let increment = Conversion {
        scale: unit.scale().clone(),
        shift: unit.scale().clone(),
    };
    let two = increment.apply(unit.scale())?;
    let mut number = unit.offset().clone();
    for bit in (0..(32 - value.leading_zeros())).rev() {
        let shift = if value & (1 << bit) == 0 {
            unit.offset()
        } else {
            unit.scale()
        };
        number = Conversion {
            scale: two.clone(),
            shift: shift.clone(),
        }
        .apply(&number)?;
    }
    Ok(Measure::new(
        "integer",
        Scale::Exact(number),
        unit.offset().clone(),
    )?)
}

/// Define a rational measure coefficient using only exact descriptor algebra.
pub(crate) fn measure(
    numerator: u32,
    denominator: u32,
    offset: u32,
    offset_denominator: u32,
) -> TestResult<Measure> {
    let unit = Measure::coherent();
    let scale = integer_measure(numerator)?
        .conversion_to(&integer_measure(denominator)?, Role::Difference)?
        .scale;
    let shift = if offset == 0 {
        unit.offset().clone()
    } else {
        integer_measure(offset)?
            .conversion_to(&integer_measure(offset_denominator)?, Role::Difference)?
            .scale
    };
    Ok(Measure::new("fixture", Scale::Exact(scale), shift)?)
}

/// One frozen declaration owner, used by both shape contracts and exact maps.
#[expect(
    clippy::default_trait_access,
    reason = "CAS policy types are inferred solely through the units facade"
)]
pub(crate) fn owner() -> TestResult<Rc<CheckedDeclarations>> {
    Ok(Rc::new(
        Context::new(
            Registry::new(ExponentPolicy::Rational),
            Default::default(),
            Default::default(),
        )?
        .freeze()?,
    ))
}

/// A one-column homogeneous tangent-coordinate shape with independently checked entries.
pub(crate) fn shape(
    owner: &Rc<CheckedDeclarations>,
    quantity: &Descriptor,
    rows: usize,
) -> TestResult<CheckedShape> {
    let limits = ShapeLimits::default();
    let quantity = quantity.difference().coherent();
    let domain = Axis::new(
        vec![AxisEntry::new("column", Descriptor::ratio())?],
        Partition::whole(1, limits)?,
        limits,
    )?;
    let codomain = Axis::new(
        (0..rows)
            .map(|row| AxisEntry::new(format!("row-{row}"), quantity.clone()))
            .collect::<Result<_, _>>()?,
        Partition::whole(rows, limits)?,
        limits,
    )?;
    let blocks = if rows == 0 {
        Vec::new()
    } else {
        vec![BlockDraft::new(
            0..rows,
            0..1,
            vec![EntryDraft::Declared(quantity); rows],
        )]
    };
    Ok(
        ShapeDraft::new(domain, codomain, blocks, StructureScope::CenterOnly)
            .check(Rc::clone(owner), limits)?,
    )
}

/// Construct one numerical contract from the retained exact map and selected ordered shapes.
pub(crate) fn contract(
    input: CheckedShape,
    output: CheckedShape,
    map: CheckedMap,
) -> TestResult<ConversionContract> {
    Ok(ConversionContract::new(
        input,
        output,
        map,
        LiteralLimits::default(),
        ConversionMode::Deterministic,
    )?)
}

/// Execute the public bind/borrow/evaluate/publish path for one conversion.
pub(crate) fn execute<P: nrz_scalar::Scalar + 'static>(
    heap: &mut crate::test_api::Heap,
    conversion: ConversionContract,
    input: crate::test_api::Handle<crate::test_api::Value, P>,
) -> TestResult<crate::test_api::PublishedBundle<P>> {
    let limits = crate::test_api::ExecutionLimits::default();
    let operation = crate::test_api::OperationDraft::convert(vec![conversion]).check(limits)?;
    let bound = crate::test_api::Binding::new(operation, vec![input], limits).validate(heap)?;
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    Ok(*outcome.output().first().ok_or("missing converted output")?)
}

/// Freeze exact reference/origin declarations for the public normalization-map adapter.
#[expect(
    clippy::default_trait_access,
    reason = "CAS policy types are inferred solely through the units facade"
)]
pub(crate) fn reference_owner(point: &Descriptor) -> TestResult<Rc<CheckedDeclarations>> {
    let mut builder = Context::with_real_symbols(
        Registry::new(ExponentPolicy::Rational),
        Default::default(),
        &["reference", "origin"],
    )?;
    builder.declare(
        "reference".into(),
        nrz_units::Declaration::Constant {
            descriptor: point.difference().coherent(),
            value: integer_measure(10)?.scale().clone(),
            evidence: nrz_units::Evidence::default(),
        },
    )?;
    builder.declare(
        "origin".into(),
        nrz_units::Declaration::Constant {
            descriptor: point.clone(),
            value: nrz_units::Measure::coherent().offset().clone(),
            evidence: nrz_units::Evidence::default(),
        },
    )?;
    Ok(Rc::new(builder.freeze()?))
}
