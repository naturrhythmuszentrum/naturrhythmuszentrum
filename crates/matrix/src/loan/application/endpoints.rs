// SPDX-License-Identifier: AGPL-3.0-only
//! Fixed Lower/Upper payload identities share the original operation and completed mask.

use super::{ApplicationCore, CellOutcome};
use crate::loan::{
    LoanError, charge,
    payload::{LoanChannel, PublishedChannel, bit, set},
    registry::{Bundle, Token},
};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};

/// Fixed allocation identity is never a transferable numerical authority.
const fn identity(token: &Token) -> (usize, u64, u64) {
    (token.slot, token.key, token.receipt)
}

/// Detached and published boundaries compare the use against the operation, not itself.
pub(super) fn validate_core<P: Scalar>(core: &ApplicationCore<P>) -> Result<(), LoanError> {
    let Some(operation) = &core.evidence.prepared else {
        return if core.endpoints.is_none() {
            Ok(())
        } else {
            Err(LoanError::WrongLoan)
        };
    };
    if core.evidence.completed_mask.is_some() {
        return Err(LoanError::WrongLoan);
    }
    core.host
        .retained_law
        .as_ref()
        .ok_or(LoanError::WrongLoan)?
        .validate_binding(operation.binding, operation.bits)?;
    let pair = core.endpoints.as_ref().ok_or(LoanError::WrongLoan)?;
    let tokens = pair.each_ref().map(|owner| identity(&owner.tokens[0]));
    pairing(&core.evidence, core.host.identity, tokens)?;
    for owner in pair {
        if !owner.matrix.matches(
            crate::storage::Format::Binary64,
            core.host.declaration.extent.entries(),
        ) || owner.components.len() != crate::loan::words(core.host.declaration.extent.entries())
            || owner.complete.len() != core.filled.len()
        {
            return Err(LoanError::LayoutMismatch);
        }
    }
    Ok(())
}
/// Immutable origin authentication needs no source lookup or live heap after publication.
pub(in crate::loan) fn validate_bundle(bundle: &Bundle) -> Result<(), LoanError> {
    let prepared = bundle
        .origin
        .evidence
        .as_ref()
        .and_then(|evidence| evidence.prepared.as_ref());
    let Some(operation) = prepared else {
        return if bundle.endpoints.is_none() {
            Ok(())
        } else {
            Err(LoanError::WrongLoan)
        };
    };
    let evidence = bundle
        .origin
        .evidence
        .as_ref()
        .ok_or(LoanError::WrongLoan)?;
    if evidence.completed_mask != Some(std::ptr::NonNull::from(bundle.mask.as_ref())) {
        return Err(LoanError::WrongLoan);
    }
    bundle
        .retained_law
        .as_ref()
        .ok_or(LoanError::WrongLoan)?
        .validate_binding(operation.binding, operation.bits)?;
    let pair = bundle.endpoints.as_ref().ok_or(LoanError::WrongLoan)?;
    pairing(
        evidence,
        bundle.identity,
        pair.each_ref().map(|owner| identity(&owner.token)),
    )?;
    if bundle.mask.layout != bundle.identity.layout
        || bundle.mask.words.len() != crate::loan::words(bundle.declaration.extent.rows())
    {
        return Err(LoanError::LayoutMismatch);
    }
    for owner in pair {
        if !owner.matrix.matches(
            crate::storage::Format::Binary64,
            bundle.declaration.extent.entries(),
        ) {
            return Err(LoanError::LayoutMismatch);
        }
    }
    Ok(())
}
/// The seal's fixed role order binds both distinct payloads to the same destination and law.
fn pairing(
    evidence: &super::Evidence,
    destination: crate::loan::registry::Identity,
    tokens: [(usize, u64, u64); 2],
) -> Result<(), LoanError> {
    if evidence.endpoint_tokens != Some(tokens)
        || tokens[0].1 == tokens[1].1
        || tokens.iter().any(|token| token.2 != destination.receipt)
        || destination.format != crate::storage::Format::Binary64
    {
        return Err(LoanError::WrongLoan);
    }
    Ok(())
}
/// Derive both endpoint roles with the same cell-local initialization rule as reading roles.
pub(super) fn complete<P: Scalar>(
    core: &mut ApplicationCore<P>,
    budget: &mut nrz_bindings::Budget,
) -> Result<(), LoanError> {
    let cells = core.host.declaration.extent.rows();
    let components = core.host.declaration.extent.columns();
    for channel in core.endpoints.iter_mut().flatten() {
        charge(
            budget,
            core.host.declaration.extent.entries() + channel.complete.len(),
        )?;
        channel.complete.fill(0);
        for cell in 0..cells {
            let initialized = (0..components).fold(true, |initialized, component| {
                bit(&channel.components, cell + cells * component) & initialized
            });
            set(&mut channel.complete, cell, initialized);
        }
    }
    Ok(())
}
/// Load all five readings once and check outward stored-endpoint coverage at the center.
pub(super) fn validate_cell<P: Scalar>(
    core: &ApplicationCore<P>,
    cell: usize,
    budget: &mut nrz_bindings::Budget,
) -> Result<(), LoanError> {
    let pair = core.endpoints.as_ref().ok_or(LoanError::WrongLoan)?;
    let context = ArithmeticContext::new().map_err(|_| LoanError::WrongLoan)?;
    for component in 0..core.host.declaration.extent.columns() {
        let address = cell + core.host.declaration.extent.rows() * component;
        let mut values = [0.0; 5];
        for (index, channel) in core.host.channels.iter().flatten().chain(pair).enumerate() {
            charge(budget, 1)?;
            values[index] = channel.matrix.read::<f64>(address)?;
        }
        let [center, total, error, lower, upper] = values;
        let valid = total.is_finite() && error.is_finite() && error >= 0.0 && total >= error;
        let coverage = Enclosure::new(lower, upper)
            .and_then(|enclosure| enclosure.displacement_from(Finite::new(center)?, &context));
        if !valid || !coverage.is_ok_and(|radius| radius.get() <= total) {
            return Err(LoanError::InvalidPayload {
                channel: crate::HostChannel::Total,
                cell,
                component,
            });
        }
    }
    Ok(())
}
/// Move the two numerical owners while destroying all four prepaid endpoint tracking owners.
pub(super) fn publish(
    pair: [LoanChannel; 2],
    registry: &mut crate::loan::Registry,
) -> [PublishedChannel; 2] {
    pair.map(|owner| {
        let [payload, components, complete] = owner.tokens;
        drop(owner.components);
        drop(owner.complete);
        registry.release(components.slot, components.key);
        registry.release(complete.slot, complete.key);
        PublishedChannel {
            matrix: owner.matrix,
            token: payload,
        }
    })
}
impl crate::TileReader<'_, f64> {
    /// Load the stored ordered endpoints of one completed prepared component.
    /// # Errors
    /// Refuses other origins, holes, failed cells, bounds, foreign accounts or either load's Work.
    pub fn expm1_endpoints(
        &self,
        cell: usize,
        component: usize,
        account: &mut crate::ExecutionAccount,
    ) -> Result<[f64; 2], LoanError> {
        account.authenticate(self.account)?;
        let bundle = self.bundle;
        if bundle
            .origin
            .evidence
            .as_ref()
            .and_then(|evidence| evidence.prepared.as_ref())
            .is_none()
        {
            return Err(LoanError::UnsupportedContract);
        }
        if cell >= self.capacity() || component >= bundle.declaration.extent.columns() {
            return Err(LoanError::LayoutMismatch);
        }
        let completed = matches!(bundle.origin.outcome(cell)?, CellOutcome::Completed);
        if !completed || !bit(&bundle.mask.words, cell) {
            return Err(LoanError::CoverageMismatch);
        }
        let pair = bundle.endpoints.as_ref().ok_or(LoanError::WrongLoan)?;
        account
            .ledger
            .with_work_phase(super::FAMILY, "selected endpoint pair", |budget| {
                charge(budget, 2)?;
                let address = cell + self.capacity() * component;
                Ok([pair[0].matrix.read(address)?, pair[1].matrix.read(address)?])
            })
    }
}
