// SPDX-License-Identifier: AGPL-3.0-only
//! Singleton native reuse writes five roles and continues after the first component refusal.

use super::Traversal;
use crate::loan::application::{ApplicationCore, CellOutcome};
use crate::loan::{HostChannel, payload::set};
use nrz_scalar::{Expm1Failure, Expm1Input, Scalar};

/// Every selected component has one center load, one native call and five tracked stores.
pub(super) fn run<P: Scalar + 'static>(core: &mut ApplicationCore<P>, permit: &Traversal) {
    let cells = core.host.declaration.extent.rows();
    let components = core.host.declaration.extent.columns();
    let reader = core
        .input
        .as_ref()
        .and_then(|pin| pin.checked_reader().ok());
    let bits = core
        .evidence
        .prepared
        .as_ref()
        .map_or(0, |operation| operation.bits);
    for word_index in 0..core.filled.len() {
        let mut remaining = core.filled[word_index];
        while remaining != 0 {
            let cell = word_index * 64 + remaining.trailing_zeros() as usize;
            remaining &= remaining - 1;
            let mut first = None;
            for component in 0..components {
                let result =
                    reader
                        .as_ref()
                        .ok_or(Expm1Failure::MalformedPlan)
                        .and_then(|reader| {
                            let center = reader
                                .read_prepaid(HostChannel::Center, cell, component, permit)
                                .map_err(|_| Expm1Failure::MalformedPlan)?
                                .to_f64();
                            let law = core
                                .host
                                .retained_law
                                .as_ref()
                                .ok_or(Expm1Failure::MalformedPlan)?;
                            #[cfg(test)]
                            crate::loan::fault::entered_scalar();
                            law.evaluate(
                                Expm1Input {
                                    endpoints: [center; 2],
                                    center,
                                    bits,
                                },
                                &permit.context,
                            )
                            .map(|value| value.reading())
                        });
                let values = match result {
                    Ok(value) => [
                        value.center,
                        value.total_radius,
                        value.numerical_error,
                        value.endpoints[0],
                        value.endpoints[1],
                    ],
                    Err(reason) => {
                        if first.is_none() {
                            first = Some(CellOutcome::PreparedRefused { component, reason });
                        }
                        [0.0; 5]
                    }
                };
                let entry = cell + cells * component;
                for (channel, value) in core
                    .host
                    .channels
                    .iter_mut()
                    .flatten()
                    .chain(core.endpoints.iter_mut().flatten())
                    .zip(values)
                {
                    if channel.matrix.write(entry, value).is_err() && first.is_none() {
                        first = Some(CellOutcome::PreparedRefused {
                            component,
                            reason: Expm1Failure::MalformedPlan,
                        });
                    }
                    set(&mut channel.components, entry, true);
                }
            }
            if let Some(refused) = first {
                core.evidence.outcomes[cell] = refused;
                set(&mut core.filled, cell, false);
            } else {
                core.evidence.outcomes[cell] = CellOutcome::Completed;
            }
        }
    }
}
