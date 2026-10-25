// SPDX-License-Identifier: AGPL-3.0-only
//! Restored native publication shares the existing all-or-none value and radius transaction.

use super::PreparedImport;
use crate::binding::live::{
    RegionTicket,
    companions::NativeCompanion,
    coordinates::{CoordinateError, selection},
};
use crate::{Heap, HeapError, ValueBinding};
use nrz_bindings::Budget;
use nrz_scalar::Scalar;
use std::rc::Rc;

impl Heap {
    /// Only the checked coordinate executor can supply these derived symbols and numerical arrays.
    pub(crate) fn publish_coordinate_selection<P: Scalar + 'static>(
        &mut self,
        plan: selection::Plan,
        channels: &[Vec<P>; 3],
        budget: &mut Budget,
    ) -> Result<ValueBinding<P>, CoordinateError> {
        let symbols = plan.symbols.clone();
        let mut prepared = self.prepare_symbol_bundle(
            &symbols,
            &channels[0],
            Some(&channels[1]),
            Some(&channels[2]),
            plan.origin.snapshot.coverage(),
        )?;
        let group = selection::group(
            plan,
            tickets(self.scope, &prepared)?,
            companions(&prepared),
            self.scope,
            P::PRECISION,
            budget,
        )?;
        let key = prepared.bundle.value().key;
        self.symbols
            .reserve_selection(key, self.limits.descriptors)?;
        for cover in &mut prepared.covers {
            cover
                .readers
                .try_reserve_exact(1)
                .map_err(|_| HeapError::Capacity)?;
            cover.readers.push(Rc::clone(&group));
        }
        let reading = crate::binding::live::bindings::<P>(&group)
            .bindings
            .into_iter()
            .next()
            .ok_or(HeapError::MissingBinding)?;
        let selected = reading.selected();
        let ticket = group.tickets.first().ok_or(HeapError::MissingBinding)?;
        let binding = crate::binding::lifecycle::selected(
            self.scope,
            key,
            ticket,
            ticket.layout.clone(),
            &selected,
            &symbols,
            self.limits.layout_work,
        )?;
        self.publish_bundle(prepared);
        group.publish();
        self.symbols.publish_selection(key, selected, symbols);
        Ok(binding)
    }
}

/// All three occupations use the native target revision and their own immutable layouts.
fn tickets<P: Scalar>(
    scope: u64,
    prepared: &PreparedImport<P>,
) -> Result<Vec<RegionTicket>, HeapError> {
    let first = prepared
        .records
        .values
        .first()
        .ok_or(HeapError::MissingBinding)?;
    let crate::heap::records::Meaning::Value { stamp, .. } = first.meaning else {
        return Err(HeapError::CategoryMismatch);
    };
    Ok(prepared
        .records
        .keys()
        .iter()
        .zip(&prepared.records.values)
        .map(|(&key, record)| RegionTicket {
            scope,
            key,
            precision: P::PRECISION,
            layout: record.layout.clone(),
            identity: stamp.id(),
            generation: stamp.version(),
            schema: record.schema,
            versions: vec![1; record.layout.extent().entries()],
        })
        .collect())
}

/// Every native bound/error target is retained before any descriptor becomes visible.
fn companions<P: Scalar>(prepared: &PreparedImport<P>) -> Vec<NativeCompanion> {
    prepared
        .records
        .keys()
        .iter()
        .zip(&prepared.records.values)
        .filter_map(|(&key, record)| {
            if let crate::heap::records::Meaning::Bound { target, stamp } = record.meaning {
                Some(NativeCompanion {
                    key,
                    target,
                    stamp,
                    schema: record.schema,
                    layout: record.layout.clone(),
                })
            } else {
                None
            }
        })
        .collect()
}
