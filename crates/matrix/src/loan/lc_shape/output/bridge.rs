// SPDX-License-Identifier: AGPL-3.0-only
//! Private bridge preparation; no partially joined owner is a public capability.
pub(in crate::loan::lc_shape) mod borrowed;
pub(in crate::loan::lc_shape) mod capture;
#[cfg(test)]
pub(in crate::loan::lc_shape) mod controls;
mod prepared;
pub(in crate::loan::lc_shape) mod retained_overlap;
pub use prepared::{LcOmegaPrepareFailure, LcOmegaPrepared};

use super::{LcOmegaOutput, LcShapeError};
#[cfg(test)]
use crate::ExecutionAccount;
use crate::LoanError;
use nrz_bindings::{
    Budget, LcOmegaCorrespondence, LcOmegaDeclarationRequest, LcOmegaDeclarationToken, Registry,
    readings::algebra::enclosure::lc_root::Certificate,
};
use nrz_scalar::Scalar;

/// Original token and sealed correspondence remain by value until native join commits.
#[repr(C)]
pub(in crate::loan::lc_shape) struct LcOmegaCandidate {
    /// The exact admitted owner, never inferred from matching structure.
    token: LcOmegaDeclarationToken,
    /// Frozen registered law, with no native occupation authority by itself.
    correspondence: LcOmegaCorrespondence,
}

/// Capture originals once, then construct or explicitly replay on the original account.
pub(super) fn prepare_on<P: Scalar, const N: usize>(
    output: &LcOmegaOutput<'_, P, N>,
    registry: &Registry,
    certificate: Option<&Certificate>,
    account: u64,
    budget: &mut Budget,
) -> Result<LcOmegaCandidate, LcShapeError> {
    let first = output
        .inventory
        .input(0)
        .ok_or(LoanError::UnsupportedContract)?;
    if account != first.account {
        return Err(LoanError::ForeignAccount.into());
    }
    let token = admit_token(output, registry, certificate, budget)?;
    // Each bindings operation reserves the captured owner internally. The caller
    // keeps its original declaration/source baseline reserved across these scopes.
    let correspondence = if let Some(certificate) = certificate {
        LcOmegaCorrespondence::replay(&token, certificate, budget)?
    } else {
        LcOmegaCorrespondence::construct(&token, budget)?
    };
    Ok(LcOmegaCandidate {
        token,
        correspondence,
    })
}

/// Unit controls use the same original-account and borrowed-budget boundary.
#[cfg(test)]
pub(in crate::loan::lc_shape) fn prepare_lc_omega_correspondence<P: Scalar, const N: usize>(
    output: &LcOmegaOutput<'_, P, N>,
    registry: &Registry,
    certificate: Option<&Certificate>,
    account: &mut ExecutionAccount,
) -> Result<LcOmegaCandidate, LcShapeError> {
    prepare_on(
        output,
        registry,
        certificate,
        account.identity,
        &mut account.ledger,
    )
}

/// The indexed request is live only while the owning token captures its original pairs.
fn admit_token<P: Scalar, const N: usize>(
    output: &LcOmegaOutput<'_, P, N>,
    registry: &Registry,
    certificate: Option<&Certificate>,
    budget: &mut Budget,
) -> Result<LcOmegaDeclarationToken, LcShapeError> {
    let request = originals(output, registry)?;
    let external = certificate.map_or(0, Certificate::retained_bytes);
    let live = external
        .checked_add(size_of_val(&request) as u64)
        .ok_or(LoanError::ByteOverflow)?;
    Ok(
        budget.with_retained_payload(live, crate::loan::AT, |budget| {
            LcOmegaDeclarationToken::admit(&request, budget)
        })?,
    )
}

impl LcOmegaCandidate {
    /// Compare the sealed owner and independently borrowed matrix originals.
    pub(in crate::loan::lc_shape) fn authenticate<P: Scalar, const N: usize>(
        &self,
        output: &LcOmegaOutput<'_, P, N>,
        registry: &Registry,
        budget: &mut Budget,
    ) -> Result<(), LcShapeError> {
        self.correspondence
            .authenticate_join(&self.token, 1, budget)?;
        let request = originals(output, registry)?;
        // check_originals supplies only the token owner; the frozen store remains live.
        budget.with_retained_payload(
            self.correspondence.retained_bytes() + size_of_val(&request) as u64,
            super::super::super::AT,
            |budget| self.token.check_originals(&request, budget),
        )?;
        Ok(())
    }

    /// Both live allocation inventories, including the candidate's first token slot.
    pub(in crate::loan::lc_shape) fn retained_bytes(&self) -> Result<u64, LcShapeError> {
        self.token
            .retained_bytes()?
            .checked_add(self.correspondence.retained_bytes())
            .ok_or_else(|| LoanError::CountOverflow.into())
    }

    /// Portable evidence export is explicit and used only by internal replay controls.
    #[cfg(test)]
    pub(in crate::loan::lc_shape) fn certificate(
        &self,
        budget: &mut Budget,
    ) -> Result<Certificate, LcShapeError> {
        Ok(self.correspondence.certificate(budget)?)
    }
}

/// Lend private pairs by index without a temporary vector or cloned symbol array.
fn originals<'a, P: Scalar, const N: usize>(
    output: &'a LcOmegaOutput<'_, P, N>,
    registry: &'a Registry,
) -> Result<
    LcOmegaDeclarationRequest<'a, impl Fn(usize) -> Option<&'a [nrz_bindings::BindingSymbols; 2]>>,
    LcShapeError,
> {
    let first = output
        .inventory
        .input(0)
        .ok_or(LoanError::UnsupportedContract)?;
    Ok(LcOmegaDeclarationRequest {
        question: first.question(),
        input_len: N,
        input_at: |index| output.inventory.input(index).map(|input| &input.symbols),
        positions: output.positions,
        counts: first.counts,
        output: &output.symbols,
        radian: &output.radian,
        registry,
    })
}

/// Independent ABI inventories accompany the existing native layout control.
#[cfg(test)]
pub(in crate::loan::lc_shape) fn layout<P: Scalar>() {
    use std::mem::{align_of, offset_of};
    assert_eq!(
        (
            size_of::<LcOmegaCandidate>(),
            align_of::<LcOmegaCandidate>()
        ),
        (56, 8)
    );
    assert_eq!(
        [
            offset_of!(LcOmegaCandidate, token),
            offset_of!(LcOmegaCandidate, correspondence)
        ],
        [0, 8]
    );
    assert_eq!(size_of::<nrz_bindings::Descriptor>(), 736);
    // Inventory pointer8, positions16, radian8, descriptor736, symbols24, key8.
    assert_eq!(
        (
            size_of::<LcOmegaOutput<'_, P, 5>>(),
            align_of::<LcOmegaOutput<'_, P, 5>>()
        ),
        (800, 8)
    );
    assert_eq!(
        size_of::<crate::LcInputReaders<'_, P, 5>>(),
        8 + 288 + 5 * 16
    );
    assert_eq!(size_of::<LcOmegaDeclarationRequest<'_, &()>>(), 88);
    prepared::layout::<P>();
    capture::layout::<P>();
}
