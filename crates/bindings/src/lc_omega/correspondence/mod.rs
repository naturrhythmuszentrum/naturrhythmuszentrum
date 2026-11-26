// SPDX-License-Identifier: AGPL-3.0-only
//! Conditional correspondence with an immutable receiving store and one typed seal.

mod accounting;
#[cfg(test)]
pub(super) mod controls;
mod policy;
mod translation;

use crate::{AdmissionError, LcOmegaDeclarationToken};
use nrz_cas::{
    Cas, Location,
    enclosure::{lc_root, lc_source_substitution},
    limits::{Budget, BudgetKind},
};
use std::{fmt, rc::Rc};

/// One cumulative accounting boundary.
const AT: Location = Location::new("LC omega correspondence");
/// Two reference-count words for the receiving Rc, under the existing 64-bit owner tariff.
const RECEIVING_RC_CONTROL_BYTES: u64 = 16;
/// Preserve structured CAS refusals through the bindings boundary.
type Result<T> = std::result::Result<T, AdmissionError>;

/// Sealed conditional LC family, retaining its exact original declaration.
///
/// This grants no ground bound, native realization or output-symbol authority.
///
/// ```compile_fail,E0599
/// use nrz_bindings::LcOmegaCorrespondence;
/// fn escape(value: LcOmegaCorrespondence) { let _ = value.symbols(); }
/// ```
///
/// ```compile_fail,E0599
/// use nrz_bindings::LcOmegaCorrespondence;
/// fn mutate(value: LcOmegaCorrespondence) { let _ = value.cas_mut(); }
/// ```
///
/// ```compile_fail,E0599
/// use nrz_bindings::LcOmegaCorrespondence;
/// fn duplicate(value: LcOmegaCorrespondence) { let _ = value.clone(); }
/// ```
///
/// The private seal requires the registered family receipt category.
///
/// ```compile_fail,E0308
/// fn wrong(receipt: nrz_cas::enclosure::CheckedEnclosure) -> nrz_cas::enclosure::lc_root::Receipt {
///     receipt
/// }
/// ```
///
/// ```compile_fail,E0308
/// fn wrong(receipt: nrz_bindings::CheckedSensitivity) -> nrz_cas::enclosure::lc_root::Receipt {
///     receipt
/// }
/// ```
///
/// ```compile_fail,E0308
/// fn wrong(receipt: nrz_bindings::LcWitness) -> nrz_cas::enclosure::lc_root::Receipt {
///     receipt
/// }
/// ```
///
/// Raw components cannot bypass authentication and expected-request comparison.
///
/// ```compile_fail,E0451
/// use nrz_bindings::{LcOmegaCorrespondence, LcOmegaDeclarationToken};
/// fn forge(declaration: LcOmegaDeclarationToken, cas: std::rc::Rc<nrz_cas::Cas>, receipt: nrz_cas::enclosure::lc_root::Receipt) {
///     let _ = LcOmegaCorrespondence { declaration, cas, receipt };
/// }
/// ```
pub struct LcOmegaCorrespondence {
    /// Exact admitted physical owner; never exported as raw symbols.
    declaration: LcOmegaDeclarationToken,
    /// Receiving store frozen after authenticating and comparing the complete request.
    cas: Rc<Cas>,
    /// Only this receipt category can cross the sealing boundary.
    receipt: lc_root::Receipt,
}

impl fmt::Debug for LcOmegaCorrespondence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LcOmegaCorrespondence")
            .field("declaration", &self.declaration)
            .finish_non_exhaustive()
    }
}

impl LcOmegaCorrespondence {
    /// Compose a positional original-source map with this exact sealed declaration.
    ///
    /// The map can use only the already admitted original declaration positions and
    /// retains every original constraint. It grants a conditional forward theorem,
    /// not a ground native enclosure or an image converse.
    /// # Errors
    /// Refuses a foreign source, missing original condition or unsupported map.
    pub fn source_substitution(
        &self,
        request: &lc_source_substitution::Request,
        budget: &mut Budget,
    ) -> Result<lc_source_substitution::Receipt> {
        let original = self.receipt.request(&self.cas, budget)?;
        accounting::original(1, budget)?;
        if request.root != *original
            || request.context != original.context
            || request.constraints != original.constraints
        {
            return Err(AdmissionError::ValueMismatch);
        }
        accounting::original(request.sources.len() as u64, budget)?;
        if request.sources != original.inputs {
            return Err(AdmissionError::ValueMismatch);
        }
        Ok(self
            .cas
            .construct_lc_source_substitution(request, &self.receipt, budget)?)
    }

    /// Construct the registered conditional law from independently translated originals.
    /// The caller reserves original residence minus the captured owner increment.
    /// # Errors
    /// Refuses unsupported originals, exhausted resources or a failed registered check.
    pub fn construct(declaration: &LcOmegaDeclarationToken, budget: &mut Budget) -> Result<Self> {
        run(declaration, None, budget)
    }

    /// Replay untrusted evidence against the supplied receiving declaration.
    /// The caller reserves original residence minus the captured owner increment.
    /// # Errors
    /// Refuses altered evidence, mismatched original claims or exhausted resources.
    pub fn replay(
        declaration: &LcOmegaDeclarationToken,
        certificate: &lc_root::Certificate,
        budget: &mut Budget,
    ) -> Result<Self> {
        run(declaration, Some(certificate), budget)
    }

    /// Export untrusted portable evidence from the immutable retained store.
    ///
    /// The caller reserves original residence minus the captured owner increment,
    /// and excludes this correspondence's `retained_bytes()` during the call.
    /// The method reserves the owner increment and correspondence residence internally. Reserve the returned certificate before further work.
    /// # Errors
    /// Preserves certificate-copy and cumulative resource refusals.
    pub fn certificate(&self, budget: &mut Budget) -> Result<lc_root::Certificate> {
        let live =
            self.declaration.retained_bytes()? + accounting::STORE + size_of::<Self>() as u64 + 16;
        budget.with_retained_payload(live, AT, |budget| {
            Ok(self.receipt.certificate(&self.cas, budget)?)
        })
    }

    /// Authenticate the exact physical declaration allocation.
    ///
    /// The caller reserves original residence minus the captured owner increment,
    /// and excludes this correspondence's `retained_bytes()` during the call.
    /// The method reserves the owner increment and correspondence residence internally. Other owners and extra token slots remain caller-reserved.
    /// # Errors
    /// Refuses another declaration or an exhausted cumulative ledger.
    pub fn check_declaration(
        &self,
        expected: &LcOmegaDeclarationToken,
        budget: &mut Budget,
    ) -> Result<()> {
        let live = self.declaration.retained_bytes()? + self.retained_bytes();
        budget.with_retained_payload(live, AT, |budget| {
            self.declaration.check_same(expected, budget)
        })
    }

    /// Authenticate the frozen receipt, exact declaration and expected arithmetic.
    ///
    /// The caller reserves original residence minus the captured owner increment,
    /// excluding this correspondence's `retained_bytes()` for the call. This method
    /// reserves both internally; other owners and extra tokens remain caller-reserved.
    /// Three ordered prepaid visits allocate nothing and expose no graph capability.
    /// # Errors
    /// Preserves receipt faults, identity/arithmetic mismatches and resource refusals.
    pub fn authenticate_join(
        &self,
        expected: &LcOmegaDeclarationToken,
        expected_arithmetic: u32,
        budget: &mut Budget,
    ) -> Result<()> {
        let live = self.declaration.retained_bytes()? + self.retained_bytes();
        budget.with_retained_payload(live, AT, |budget| {
            accounting::original(1, budget)?;
            let request = self.receipt.request(&self.cas, budget)?;
            self.declaration.check_same(expected, budget)?;
            accounting::original(1, budget)?;
            if request.arithmetic != expected_arithmetic {
                return Err(AdmissionError::ValueMismatch);
            }
            Ok(())
        })
    }

    /// Whether both correspondences own the same immutable receiving-store allocation.
    ///
    /// This identity-only observation survives carrier moves and distinguishes
    /// independently allocated equal stores. It exports no address, graph handle,
    /// capacity or additional proof authority; it allocates and reserves nothing.
    #[must_use]
    pub fn shares_allocation(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.cas, &other.cas)
    }

    /// Captured declaration allowance, excluding the caller's initial token slot.
    /// This observes the original retained partition, not receiving-store capacity.
    /// # Errors
    /// Refuses an unrepresentable owning-layer byte inventory.
    pub fn captured_declaration_bytes(&self) -> Result<u64> {
        self.declaration
            .retained_bytes()?
            .checked_sub(size_of::<LcOmegaDeclarationToken>() as u64)
            .ok_or(AdmissionError::ResourceLimit)
    }

    /// Compare the retained declaration allocation without exposing its authority.
    /// The original token identity check retains its existing Work charge.
    /// # Errors
    /// Preserves original-account exhaustion from the identity check.
    pub fn captures_declaration(
        &self,
        token: &LcOmegaDeclarationToken,
        budget: &mut Budget,
    ) -> Result<bool> {
        match self.declaration.check_same(token, budget) {
            Ok(()) => Ok(true),
            Err(AdmissionError::ValueMismatch) => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Compare captured allocations independently of correspondence allocation identity.
    /// # Errors
    /// Preserves original-account exhaustion from the identity check.
    pub fn shares_declaration(&self, other: &Self, budget: &mut Budget) -> Result<bool> {
        self.captures_declaration(&other.declaration, budget)
    }

    /// Additional staged allowance for this immutable correspondence on this ledger.
    /// Existing checked-record registrations remain enforced by their owning CAS scope.
    /// This reports a reservation increment, not capacity or a replacement for the
    /// full logical `retained_bytes()` allowance. Use it before further ledger work.
    /// # Errors
    /// Preserves owning-scope refresh, Work and staged-payload failures.
    pub fn staged_retained_bytes(&self, budget: &mut Budget) -> Result<u64> {
        let before = budget.limits().staged_payload_bytes;
        let graph = self.cas.with_retained_graph(budget, |budget| {
            before
                .checked_sub(budget.limits().staged_payload_bytes)
                .ok_or(AdmissionError::ResourceLimit)
        })?;
        (size_of::<Self>() as u64)
            .checked_add(accounting::STORE)
            .and_then(|bytes| bytes.checked_add(RECEIVING_RC_CONTROL_BYTES))
            .and_then(|bytes| bytes.checked_add(graph))
            .ok_or(AdmissionError::ResourceLimit)
    }

    /// New retained correspondence residence, excluding existing original owners.
    #[must_use]
    pub fn retained_bytes(&self) -> u64 {
        size_of::<Self>() as u64
            + accounting::STORE
            + RECEIVING_RC_CONTROL_BYTES
            + self.cas.counts().payload_bytes
    }
}

/// Close the owner scope before sharing; sharing reserves that same increment internally.
fn run(
    declaration: &LcOmegaDeclarationToken,
    certificate: Option<&lc_root::Certificate>,
    budget: &mut Budget,
) -> Result<LcOmegaCorrespondence> {
    let (cas, request, receipt) =
        budget.with_retained_payload(declaration.retained_bytes()?, AT, |budget| {
            prepare(declaration, certificate, budget)
        })?;
    budget.with_retained_payload(
        certificate.map_or(0, lc_root::Certificate::retained_bytes),
        AT,
        |budget| seal(cas, receipt, declaration, &request, budget),
    )
}

/// The complete receiving policy is fixed before the first store allocation.
fn prepare(
    declaration: &LcOmegaDeclarationToken,
    certificate: Option<&lc_root::Certificate>,
    budget: &mut Budget,
) -> Result<(Cas, lc_root::Request, lc_root::Receipt)> {
    // A short-lived certificate reservation must not become a permanent store ceiling.
    let active = budget.limits().clone();
    let (mut cas, request) = budget.with_retained_payload(
        certificate.map_or(0, lc_root::Certificate::retained_bytes),
        AT,
        |budget| translate(declaration, active, budget),
    )?;
    let receipt = budget.with_retained_payload(accounting::STORE, AT, |budget| {
        let receipt = if let Some(certificate) = certificate {
            cas.check_lc_root(&request, certificate, budget)?
        } else {
            cas.construct_lc_root(&request, budget)?
        };
        Ok::<_, AdmissionError>(receipt)
    })?;
    Ok((cas, request, receipt))
}

/// Keep the whole replay input live while meeting policies and building receiving syntax.
fn translate(
    declaration: &LcOmegaDeclarationToken,
    active: nrz_cas::limits::Limits,
    budget: &mut Budget,
) -> Result<(Cas, lc_root::Request)> {
    budget.with_work_phase("LC omega correspondence", "shape", |budget| {
        budget.work(1, AT)
    })?;
    let original = declaration.originals();
    if original.question.relation().observations() != 0 {
        return Err(AdmissionError::UnsupportedRecipe);
    }
    let limits = policy::receiving(original, active, budget)?;
    budget.check(BudgetKind::StagedPayloadBytes, accounting::STORE, AT)?;
    accounting::edges(8, budget)?;
    let mut cas = Cas::new(limits)?;
    let request = budget.with_retained_payload(accounting::STORE, AT, |budget| {
        let translated = translation::build(original.question.relation().inputs(), budget)?;
        translated.admit(&mut cas, original.positions, budget)
    })?;
    Ok((cas, request))
}

/// Authenticate and compare while mutable, then publish only the frozen store.
fn seal(
    cas: Cas,
    receipt: lc_root::Receipt,
    declaration: &LcOmegaDeclarationToken,
    expected: &lc_root::Request,
    budget: &mut Budget,
) -> Result<LcOmegaCorrespondence> {
    let live = accounting::STORE
        + cas.counts().payload_bytes
        + request_bytes(expected)
        + size_of::<lc_root::Receipt>() as u64;
    budget.with_retained_payload(live, AT, |budget| {
        budget.with_retained_payload(declaration.retained_bytes()?, AT, |budget| {
            accounting::original(1, budget)?;
            compare(receipt.request(&cas, budget)?, expected, budget)
        })?;
        let declaration = declaration.share(budget)?;
        budget.with_retained_payload(declaration.retained_bytes()?, AT, |budget| {
            let retained = size_of::<LcOmegaCorrespondence>() as u64
                + accounting::STORE
                + 16
                + cas.counts().payload_bytes;
            budget.check(BudgetKind::OutputBytes, retained, AT)?;
            budget.check(
                BudgetKind::StagedPayloadBytes,
                size_of::<Cas>() as u64 + 32,
                AT,
            )?;
            accounting::edges(2, budget)?;
            accounting::edges(2, budget)?;
            Ok(LcOmegaCorrespondence {
                declaration,
                cas: Rc::new(cas),
                receipt,
            })
        })
    })
}

/// Expected metadata is released after sealing and is not retained by the result.
const fn request_bytes(request: &lc_root::Request) -> u64 {
    size_of::<lc_root::Request>() as u64
        + ((request.inputs.capacity() + request.constraints.capacity())
            * size_of::<nrz_cas::Handle>()) as u64
}

/// Twenty-one ordered handle comparisons and four structural/scalar visits.
fn compare(
    actual: &lc_root::Request,
    expected: &lc_root::Request,
    budget: &mut Budget,
) -> Result<()> {
    accounting::original(1, budget)?;
    if actual.inputs.len() != expected.inputs.len()
        || actual.constraints.len() != expected.constraints.len()
    {
        return Err(AdmissionError::ValueMismatch);
    }
    for (actual, expected) in handles(actual).zip(handles(expected)) {
        accounting::original(1, budget)?;
        if actual != expected {
            return Err(AdmissionError::ValueMismatch);
        }
    }
    for (actual, expected) in actual.positions.iter().zip(expected.positions) {
        accounting::original(1, budget)?;
        if *actual != expected {
            return Err(AdmissionError::ValueMismatch);
        }
    }
    accounting::original(1, budget)?;
    if actual.arithmetic != expected.arithmetic {
        return Err(AdmissionError::ValueMismatch);
    }
    Ok(())
}

/// Ordered slots of the complete request, including repeated physical inputs.
fn handles(request: &lc_root::Request) -> impl Iterator<Item = &nrz_cas::Handle> {
    std::iter::once(&request.context)
        .chain(&request.inputs)
        .chain(&request.constraints)
        .chain(&request.endpoints)
        .chain(&request.targets)
}
