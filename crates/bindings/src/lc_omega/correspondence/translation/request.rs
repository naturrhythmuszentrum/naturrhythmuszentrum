// SPDX-License-Identifier: AGPL-3.0-only
//! Resolve complete ordered originals after one atomic syntax admission.

use super::{AT, Result, Translation, accounting};
use crate::AdmissionError;
use nrz_cas::{Cas, Handle, enclosure::lc_root, limits::Budget};

impl Translation {
    /// Prepay every returned owning slot, then preserve all original request positions.
    pub(in crate::lc_omega::correspondence) fn admit(
        self,
        cas: &mut Cas,
        positions: [usize; 2],
        budget: &mut Budget,
    ) -> Result<lc_root::Request> {
        let count = self.builder.drafts.len();
        budget.with_retained_payload(self.builder.bytes()?, AT, |budget| {
            accounting::edges((2 * count + 2) as u64, budget)
        })?;
        let handles = budget.with_retained_payload(
            accounting::vector::<usize>(self.indices.capacity())?,
            AT,
            |budget| {
                budget.with_work_phase("LC omega correspondence", "expression", |budget| {
                    Ok::<_, AdmissionError>(cas.admit(&self.builder.drafts, budget)?)
                })
            },
        )?;
        drop(self.builder);
        let live = cas.counts().payload_bytes
            + accounting::vector::<Handle>(handles.capacity())?
            + accounting::vector::<usize>(self.indices.capacity())?;
        budget.with_retained_payload(live, AT, |budget| {
            request(
                &handles,
                &self.indices,
                self.endpoints,
                self.targets,
                positions,
                budget,
            )
        })
    }
}

/// One prepaid edge pair for every slot, with no private physical symbol handle involved.
fn handle(handles: &[Handle], index: usize, budget: &mut Budget) -> Result<Handle> {
    accounting::edges(2, budget)?;
    handles
        .get(index)
        .cloned()
        .ok_or(AdmissionError::ValueMismatch)
}

/// Two vector capacities of N and eleven fixed slots form the independent expected request.
fn request(
    handles: &[Handle],
    indices: &[usize],
    endpoints: [u32; 6],
    targets: [u32; 4],
    positions: [usize; 2],
    budget: &mut Budget,
) -> Result<lc_root::Request> {
    let mut inputs = accounting::allocate::<Handle>(indices.len() / 2, budget)?;
    let mut constraints = budget.with_retained_payload(
        accounting::vector::<Handle>(inputs.capacity())?,
        AT,
        |budget| accounting::allocate::<Handle>(indices.len() / 2, budget),
    )?;
    let bytes = size_of::<lc_root::Request>() as u64
        + ((inputs.capacity() + constraints.capacity()) * size_of::<Handle>()) as u64;
    budget.with_retained_payload(bytes, AT, |budget| {
        for pair in indices.as_chunks::<2>().0 {
            inputs.push(handle(handles, pair[0], budget)?);
            constraints.push(handle(handles, pair[1], budget)?);
        }
        let endpoints = endpoints.map(|i| handle(handles, i as usize, budget));
        let [
            product_lower,
            product_upper,
            root_lower,
            root_upper,
            outer_lower,
            outer_upper,
        ] = endpoints;
        let [product, root, reciprocal, reverse] =
            targets.map(|i| handle(handles, i as usize, budget));
        Ok(lc_root::Request {
            context: handle(handles, 2, budget)?,
            inputs,
            constraints,
            positions,
            endpoints: [
                product_lower?,
                product_upper?,
                root_lower?,
                root_upper?,
                outer_lower?,
                outer_upper?,
            ],
            targets: [product?, root?, reciprocal?, reverse?],
            arithmetic: 1,
        })
    })
}
