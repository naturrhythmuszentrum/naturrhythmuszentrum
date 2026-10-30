// SPDX-License-Identifier: AGPL-3.0-only
//! Exact affine reading definitions transport the entire authenticated physical source box.

use super::model::{ScalarKernelLaw, ScalarKernelReading, ScalarKernelRecord};
use crate::budget::Budget;
use crate::graph::{Cas, Handle};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::{Fault, InvalidCode, Location};

/// Changed coefficients or selected sources refuse at the original definition comparison.
const AT: Location = Location::new("scalar kernel reading map");

/// A declared result symbol or matching center supplies no affine definition.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::EndpointMismatch, AT)
}

/// Authenticate the exact definition and earlier family before replaying any affine arithmetic.
pub(super) fn identity(
    cas: &Cas,
    context: &Handle,
    value: &ScalarKernelReading,
    earlier: &[ScalarKernelRecord],
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(bytes(value).saturating_add(4), AT)?;
    value.scale.check(budget, AT)?;
    value.offset.check(budget, AT)?;
    let source = super::relations::operand(earlier, value.input)?;
    let ScalarKernelLaw::Family(source) = &source.law else {
        return Err(invalid());
    };
    let definition = cas.state.expression(cas.checked(&value.definition)?)?;
    let Operator::AffineReading(map) = &definition.operator else {
        return Err(invalid());
    };
    let [_, selected] = definition.operands.as_slice() else {
        return Err(invalid());
    };
    let shape = map.inputs() == 1 && map.outputs() == 1;
    let scalar = definition.sort == Sort::Value(AtomicSort::Scalar(Field::Real));
    if !shape
        || !scalar
        || definition.header != cas.state.context(cas.checked(context)?)?.header_id
        || !definition.scope.is_empty()
        || definition.binding.is_some()
        || definition.space.is_some()
        || definition.domain.is_some()
        || definition.attachment.is_some()
        || *selected != cas.checked(&source.family.source)?
        || map.coefficients() != std::slice::from_ref(&value.scale)
        || map.offsets() != std::slice::from_ref(&value.offset)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Both endpoint images are enclosed, so negative scales reverse order without losing coverage.
pub(super) fn enclose(
    record: &ScalarKernelRecord,
    value: &ScalarKernelReading,
    earlier: &[ScalarKernelRecord],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let source = super::relations::operand(earlier, value.input)?;
    for endpoint in [&source.lower, &source.upper] {
        budget.work(1, AT)?;
        let image = endpoint
            .mul(&value.scale, budget)?
            .add(&value.offset, budget)?;
        super::relations::contains(record, &image, budget)?;
    }
    Ok(())
}

/// Exact map coefficients and the retained definition handle are prepaid before ownership.
pub(super) fn bytes(value: &ScalarKernelReading) -> u64 {
    192_u64
        .saturating_add(super::number_bytes(&value.scale))
        .saturating_add(super::number_bytes(&value.offset))
}
