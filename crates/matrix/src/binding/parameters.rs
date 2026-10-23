// SPDX-License-Identifier: AGPL-3.0-only
//! Complete parameter coverage over actual value and source symbols.

use crate::{Extent, HeapError, Layout};
use nrz_bindings::{
    EvaluationDescriptor, ParameterEntry, ParameterSymbol, ParameterTarget, QuantitySymbol,
};

/// Preserve every selected source coordinate, including operation order and repeated reads.
pub(crate) fn targets(
    symbol: &QuantitySymbol,
    layout: &Layout,
) -> Result<Vec<ParameterTarget>, HeapError> {
    (0..layout.extent().entries())
        .map(|index| {
            Ok(ParameterTarget::new(
                symbol.clone(),
                u64::try_from(layout.flat_index(index)?).map_err(|_| HeapError::ExtentOverflow)?,
            )?)
        })
        .collect()
}

/// Declare an ordinary mean or other value-like parameter without detaching its targets.
pub(crate) fn values(
    extent: Extent,
    targets: Vec<ParameterTarget>,
) -> Result<ParameterSymbol, HeapError> {
    Ok(ParameterSymbol::declare(
        super::symbols::shape(extent)?,
        targets.into_iter().map(ParameterEntry::value).collect(),
        &mut super::symbols::budget()?,
    )?)
}

/// Both target sides and every original formal pairing are checked before parameter allocation.
pub(crate) fn paired(
    extent: Extent,
    descriptors: &[EvaluationDescriptor],
    rows: &[ParameterTarget],
    columns: &[ParameterTarget],
) -> Result<ParameterSymbol, HeapError> {
    if rows.len() != extent.rows()
        || columns.len() != extent.columns()
        || descriptors.len() != extent.entries()
    {
        return Err(HeapError::LengthMismatch);
    }
    let mut entries = super::buffer(descriptors.len())?;
    for (column, right) in columns.iter().enumerate() {
        for (row, left) in rows.iter().enumerate() {
            let descriptor = descriptors
                .get(column * rows.len() + row)
                .ok_or(HeapError::LengthMismatch)?;
            let other = descriptor.target().other().map(|_| right.clone());
            entries.push(ParameterEntry::paired(
                descriptor.target().clone(),
                left.clone(),
                other,
            )?);
        }
    }
    Ok(ParameterSymbol::declare(
        super::symbols::shape(extent)?,
        entries,
        &mut super::symbols::budget()?,
    )?)
}

/// Numerical errors target the actual parameter symbol, never one of its physical operands.
pub(crate) fn errors(parameter: &ParameterSymbol) -> Result<ParameterSymbol, HeapError> {
    let entries = (0..parameter.entries().len())
        .map(|index| ParameterEntry::evaluation(parameter.clone(), index))
        .collect::<Result<_, _>>()?;
    Ok(ParameterSymbol::declare(
        parameter.shape(),
        entries,
        &mut super::symbols::budget()?,
    )?)
}

/// Every owning record must carry a full cover before numerical publication.
pub(crate) fn validate(
    covers: &[super::symbols::RegionCover],
    records: &crate::heap::slots::PreparedSlots<crate::heap::records::Record>,
) -> Result<(), HeapError> {
    if covers.len() != records.keys().len() {
        return Err(HeapError::MissingBinding);
    }
    for (&key, record) in records.keys().iter().zip(&records.values) {
        let mut matches = covers.iter().filter(|cover| cover.key == key);
        let cover = matches.next().ok_or(HeapError::MissingBinding)?;
        if matches.next().is_some() || cover.versions.len() != record.layout.extent().entries() {
            return Err(HeapError::MissingBinding);
        }
        if let super::symbols::Sponsor::Parameter(symbol) = &cover.sponsor
            && symbol.entries().len() != cover.versions.len()
        {
            return Err(HeapError::MissingBinding);
        }
    }
    Ok(())
}

/// Resolve a captured selected dependency to its actual base declaration and ordered coordinates.
pub(crate) fn dependency_targets(
    heap: &crate::Heap,
    dependency: &crate::Dependency,
) -> Result<Vec<ParameterTarget>, HeapError> {
    dependency.validate(heap)?;
    let record = heap.descriptors.get(dependency.key)?;
    let key = match record.meaning {
        crate::heap::records::Meaning::Alias { base } => base,
        _ => dependency.key,
    };
    targets(
        heap.symbols.get(key, dependency.stamp())?,
        dependency.layout(),
    )
}

/// Product contributions are separate radius parameters on the exact result coordinates.
pub(crate) fn radii(symbol: &QuantitySymbol) -> Result<ParameterSymbol, HeapError> {
    let [rows, columns] = symbol.shape();
    let count = u64::from(rows) * u64::from(columns);
    let pairing = nrz_bindings::PairingWitness::uncertainty(
        nrz_bindings::UncertaintyMode::Radius,
        symbol.descriptor(),
        None,
        None,
    )
    .map_err(|_| nrz_bindings::AdmissionError::CompanionMismatch)?;
    let entries = (0..count)
        .map(|index| {
            ParameterEntry::paired(
                pairing.clone(),
                ParameterTarget::new(symbol.clone(), index)?,
                None,
            )
        })
        .collect::<Result<_, nrz_bindings::AdmissionError>>()?;
    Ok(ParameterSymbol::declare(
        symbol.shape(),
        entries,
        &mut super::symbols::budget()?,
    )?)
}
