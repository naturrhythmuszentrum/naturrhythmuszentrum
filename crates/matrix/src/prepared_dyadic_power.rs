// SPDX-License-Identifier: AGPL-3.0-only
//! Lower one original checked power enclosure into an immutable numerical program and proof seal.

use nrz_bindings::{
    Budget, DyadicPowerPreparation, DyadicPowerPreparationError,
    readings::algebra::{
        Cas, Fault, Handle, Location, Rational,
        enclosure::{EnclosureGoal, RefinementPolicy},
        limits::{BudgetKind, BudgetUsage},
        syntax::Operator,
    },
};
use nrz_scalar::{
    ArithmeticContext, DyadicPowerFailure, DyadicPowerLimits, DyadicPowerOutput, DyadicPowerProgram,
};
use std::cmp::Ordering;

/// Fixed accounting location for exact correspondence and private key framing.
const AT: Location = Location::new("retained dyadic power lowering");

/// Bindings, native admission and exact correspondence preserve separate causes.
#[derive(Debug, thiserror::Error)]
pub enum PreparedDyadicPowerError {
    /// Original syntax, carrier or checked-enclosure preparation refused.
    #[error("dyadic power bindings preparation: {0}")]
    Bindings(#[from] DyadicPowerPreparationError),
    /// Numerical domain, exponent, environment or finite schedule refused.
    #[error("dyadic power native admission: {0}")]
    Native(#[from] DyadicPowerFailure),
    /// Actual returned request does not match the represented domain or original handles.
    #[error("dyadic power original domain correspondence failed")]
    DomainCorrespondence,
    /// Canonical native exponent does not equal the original exact literal.
    #[error("dyadic power original exponent correspondence failed")]
    ExponentCorrespondence,
    /// Exact decoding, comparison or bounded key framing refused.
    #[error(transparent)]
    Exact(#[from] Fault),
}

/// Borrowed native reading retains the actual whole-domain authority with its funded use.
#[derive(Clone, Copy, Debug)]
pub struct DyadicPowerEvaluation<'a> {
    /// Scalar numerical output, which alone grants no original-operation authority.
    output: DyadicPowerOutput,
    /// Actual checked original bound returned through bindings.
    authority: &'a DyadicPowerPreparation,
}

impl DyadicPowerEvaluation<'_> {
    /// Copy the numerical reading as data.
    #[must_use]
    pub const fn reading(&self) -> DyadicPowerOutput {
        self.output
    }

    /// Borrow the actual checked original and its producing-policy metadata.
    #[must_use]
    pub const fn authority(&self) -> &DyadicPowerPreparation {
        self.authority
    }
}

/// Owned preallocation inputs leave the mutable graph borrow before permanent encapsulation.
#[derive(Debug)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "retained owner modules share preallocation parts within the crate"
)]
pub(crate) struct OwnedParts {
    /// The actual returned checked judgment, never reconstructed from equal values.
    pub(crate) authority: DyadicPowerPreparation,
    /// Immutable conditional native schedule admitted against the original exponent.
    program: DyadicPowerProgram,
    /// Proof-sensitive authority frame, retained as private bytes.
    pub(crate) authority_key: Vec<u8>,
    /// Lowering frame tying binary64 readings to the authority and scalar revision.
    pub(crate) lowering_key: Vec<u8>,
}

impl OwnedParts {
    /// Borrow the immutable root, square and multiply counts admitted by the scalar program.
    pub(crate) const fn schedule_inventory(&self) -> [usize; 3] {
        self.program.inventory()
    }

    /// Report the admitted native program's argument revision for the owner seal.
    pub(crate) const fn argument_revision(&self) -> u32 {
        self.program.argument_revision()
    }

    /// Canonical exponent admitted by the immutable numerical argument.
    pub(crate) const fn exponent(&self) -> (u64, u32) {
        self.program.exponent()
    }

    /// Evaluate the unchanged native leaf through a borrow of the retained authority.
    pub(crate) fn evaluate(
        &self,
        x: f64,
        arithmetic: &ArithmeticContext,
    ) -> Result<DyadicPowerEvaluation<'_>, DyadicPowerFailure> {
        let output = self.program.evaluate(x, arithmetic)?;
        Ok(DyadicPowerEvaluation {
            output,
            authority: &self.authority,
        })
    }

    /// Complete owned key-buffer capacity, separate from CAS's logical evidence allowance.
    pub(crate) fn key_bytes(&self) -> Result<usize, PreparedDyadicPowerError> {
        self.authority
            .proof_capacity()
            .checked_add(self.authority_key.capacity())
            .and_then(|n| n.checked_add(self.lowering_key.capacity()))
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT).into())
    }
}

/// Classify bindings range as the one owner-level numerical outside-domain cause.
fn binding_error(error: DyadicPowerPreparationError) -> PreparedDyadicPowerError {
    match error {
        DyadicPowerPreparationError::ExponentOutsideDomain => {
            DyadicPowerFailure::OutsideDomain.into()
        }
        other => other.into(),
    }
}

/// Construct, inspect and lower before the producing graph enters an owned shell.
/// # Errors
/// Preserves the exact phase's cause and stops before managed allocation on mismatch.
#[expect(
    clippy::too_many_arguments,
    reason = "original handles, policy and cumulative receipt remain explicit"
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "retained owner modules share preallocation parts within the crate"
)]
pub(crate) fn owned_parts(
    cas: &mut Cas,
    target: &Handle,
    context: &Handle,
    domain: [f64; 2],
    policy: RefinementPolicy,
    limits: DyadicPowerLimits,
    arithmetic: &ArithmeticContext,
    budget: &mut Budget,
    interrupt: &mut impl FnMut(BudgetUsage) -> bool,
    work_used: &mut u64,
) -> Result<OwnedParts, PreparedDyadicPowerError> {
    let span = tracing::trace_span!(
        "nrz.lower",
        family = "dyadic-power",
        result = tracing::field::Empty,
        work_used = tracing::field::Empty,
        proof_bytes = tracing::field::Empty,
        authority_bytes = tracing::field::Empty,
        lowering_bytes = tracing::field::Empty
    );
    let _entered = span.enter();
    let before = budget.usage().work;
    let result = (|| {
        let (m, d) = DyadicPowerPreparation::original_exponent(cas, target, context, budget)
            .map_err(binding_error)?;
        let program = DyadicPowerProgram::new(domain, m, d, limits, arithmetic)?;
        original_exponent(cas, target, &program, budget)?;
        let authority = DyadicPowerPreparation::prepare(
            cas, target, context, domain, policy, budget, interrupt, work_used,
        )
        .map_err(binding_error)?;
        request_correspondence(cas, target, context, domain, &authority, budget)?;
        let authority_key = authority_frame(&authority, &program, arithmetic, domain, budget)?;
        let lowering_key = lowering_frame(&authority_key, budget)?;
        Ok::<OwnedParts, PreparedDyadicPowerError>(OwnedParts {
            authority,
            program,
            authority_key,
            lowering_key,
        })
    })();
    span.record("work_used", budget.usage().work.saturating_sub(before));
    if let Ok(parts) = &result {
        span.record("proof_bytes", parts.authority.proof_capacity());
        span.record("authority_bytes", parts.authority_key.capacity());
        span.record("lowering_bytes", parts.lowering_key.capacity());
    }
    span.record(
        "result",
        if result.is_ok() {
            "accepted"
        } else {
            "refused"
        },
    );
    result
}

/// Reconstruct the canonical scalar dyadic from its positive numerator and root degree.
fn original_exponent(
    cas: &Cas,
    target: &Handle,
    program: &DyadicPowerProgram,
    budget: &mut Budget,
) -> Result<(), PreparedDyadicPowerError> {
    let operands = cas.operands(target, budget)?;
    let [_, exponent] = operands.as_slice() else {
        return Err(PreparedDyadicPowerError::ExponentCorrespondence);
    };
    let (Operator::Rational(original), _) = cas.expression(exponent, budget)? else {
        return Err(PreparedDyadicPowerError::ExponentCorrespondence);
    };
    let (numerator, roots) = program.exponent();
    let numerator =
        i64::try_from(numerator).map_err(|_| PreparedDyadicPowerError::ExponentCorrespondence)?;
    let exact = Rational::from_ratio(numerator, 1, budget)?.div(
        &Rational::from_ratio(2, 1, budget)?.pow(i64::from(roots), budget)?,
        budget,
    )?;
    if exact.compare(original, budget)? != Ordering::Equal {
        return Err(PreparedDyadicPowerError::ExponentCorrespondence);
    }
    Ok(())
}

/// Match the actual returned request against original identities and represented endpoint bits.
fn request_correspondence(
    cas: &Cas,
    target: &Handle,
    context: &Handle,
    domain: [f64; 2],
    authority: &DyadicPowerPreparation,
    budget: &mut Budget,
) -> Result<(), PreparedDyadicPowerError> {
    let request = authority.judgment().request();
    let operands = cas.operands(target, budget)?;
    let [base, _] = operands.as_slice() else {
        return Err(PreparedDyadicPowerError::DomainCorrespondence);
    };
    budget.work(5, AT)?;
    if &request.target != target
        || &request.context != context
        || request.goal != EnclosureGoal::Width
        || request.coordinates.len() != 1
        || &request.coordinates[0].coordinate != base
    {
        return Err(PreparedDyadicPowerError::DomainCorrespondence);
    }
    let interval = &request.coordinates[0].interval;
    if interval
        .lower()
        .compare(&represented(domain[0], budget)?, budget)?
        != Ordering::Equal
        || interval
            .upper()
            .compare(&represented(domain[1], budget)?, budget)?
            != Ordering::Equal
    {
        return Err(PreparedDyadicPowerError::DomainCorrespondence);
    }
    Ok(())
}

/// Decode one finite binary64 endpoint exactly for correspondence with the CAS request.
fn represented(value: f64, budget: &mut Budget) -> Result<Rational, Fault> {
    let bits = value.to_bits();
    let field = (bits >> 52) & 2047;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, exponent) = if field == 0 {
        (fraction, -1074)
    } else {
        (
            fraction + (1_u64 << 52),
            i64::try_from(field).map_err(|_| Fault::capacity(BudgetKind::IntegerExponent, AT))?
                - 1075,
        )
    };
    let sign = if value.is_sign_negative() { -1 } else { 1 };
    Rational::from_ratio(
        sign * i64::try_from(significand)
            .map_err(|_| Fault::capacity(BudgetKind::ComponentBits, AT))?,
        1,
        budget,
    )?
    .mul(
        &Rational::from_ratio(2, 1, budget)?.pow(exponent, budget)?,
        budget,
    )
}

/// Prepay and append exactly one copied byte slice to a private key frame.
fn append(out: &mut Vec<u8>, bytes: &[u8], budget: &mut Budget) -> Result<(), Fault> {
    let count = u64::try_from(bytes.len())
        .map_err(|_| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    budget.work(count, AT)?;
    out.try_reserve_exact(bytes.len())
        .map_err(|_| Fault::allocation(AT))?;
    out.extend_from_slice(bytes);
    Ok(())
}

/// Frame a byte string with the selected u64 little-endian length.
fn framed(out: &mut Vec<u8>, bytes: &[u8], budget: &mut Budget) -> Result<(), Fault> {
    let len = u64::try_from(bytes.len())
        .map_err(|_| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    append(out, &len.to_le_bytes(), budget)?;
    append(out, bytes, budget)
}

/// Bind actual proof bytes, endpoint encodings, exact q and native specialization.
fn authority_frame(
    authority: &DyadicPowerPreparation,
    program: &DyadicPowerProgram,
    arithmetic: &ArithmeticContext,
    domain: [f64; 2],
    budget: &mut Budget,
) -> Result<Vec<u8>, Fault> {
    let lower = authority
        .judgment()
        .interval()
        .lower()
        .canonical_bytes(budget)?;
    let upper = authority
        .judgment()
        .interval()
        .upper()
        .canonical_bytes(budget)?;
    let fixed = b"NRZ-DYADIC-AUTHORITY".len() + 4 + 3 * 8 + 8 + 4 + 16 + 4 + 4 + 1;
    let total = [authority.proof_bytes().len(), lower.len(), upper.len()]
        .into_iter()
        .try_fold(fixed, usize::checked_add)
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        u64::try_from(total).map_err(|_| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?,
        AT,
    )?;
    let mut frame = Vec::new();
    frame
        .try_reserve_exact(total)
        .map_err(|_| Fault::allocation(AT))?;
    append(&mut frame, b"NRZ-DYADIC-AUTHORITY", budget)?;
    append(&mut frame, &1_u32.to_le_bytes(), budget)?;
    framed(&mut frame, authority.proof_bytes(), budget)?;
    for endpoint in [&lower, &upper] {
        framed(&mut frame, endpoint, budget)?;
    }
    let (numerator, roots) = program.exponent();
    append(&mut frame, &numerator.to_le_bytes(), budget)?;
    append(&mut frame, &roots.to_le_bytes(), budget)?;
    for endpoint in domain {
        append(&mut frame, &endpoint.to_bits().to_le_bytes(), budget)?;
    }
    append(
        &mut frame,
        &program.argument_revision().to_le_bytes(),
        budget,
    )?;
    append(&mut frame, &arithmetic.revision().to_le_bytes(), budget)?;
    append(&mut frame, &[0], budget)?;
    Ok(frame)
}

/// Fix the binary64 endpoint and midpoint/displacement convention to this authority frame.
fn lowering_frame(authority: &[u8], budget: &mut Budget) -> Result<Vec<u8>, Fault> {
    let total = b"NRZ-DYADIC-LOWERING"
        .len()
        .checked_add(4 + 8 + 2 + 1)
        .and_then(|n| n.checked_add(authority.len()))
        .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
    budget.check(
        BudgetKind::StagedPayloadBytes,
        u64::try_from(total).map_err(|_| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?,
        AT,
    )?;
    let mut frame = Vec::new();
    frame
        .try_reserve_exact(total)
        .map_err(|_| Fault::allocation(AT))?;
    append(&mut frame, b"NRZ-DYADIC-LOWERING", budget)?;
    append(&mut frame, &1_u32.to_le_bytes(), budget)?;
    framed(&mut frame, authority, budget)?;
    append(&mut frame, &64_u16.to_le_bytes(), budget)?;
    append(&mut frame, &[1], budget)?;
    Ok(frame)
}

/// Test-only access to the private correspondence guards with paired valid inputs.
#[cfg(test)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "the private test guard is shared with its sibling owner control"
)]
pub(crate) mod guards {
    use super::*;
    use crate::RetainedDyadicPowerError;

    /// Check each private mismatch and its normal owner error conversion.
    /// # Panics
    /// Panics only if the two deliberately unequal exact facts compare as equal.
    #[expect(
        clippy::redundant_pub_crate,
        reason = "the sibling retained-owner control calls this private guard"
    )]
    pub(crate) fn mismatches(
        cas: &Cas,
        target: &Handle,
        context: &Handle,
        authority: &DyadicPowerPreparation,
        arithmetic: &ArithmeticContext,
        limits: DyadicPowerLimits,
        budget: &mut Budget,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let actual = DyadicPowerPreparation::original_exponent(cas, target, context, budget)?;
        assert_eq!(actual, (1, 2));
        let wrong = DyadicPowerProgram::new([1.0, 2.0], 1, 4, limits, arithmetic)?;
        assert_eq!(wrong.exponent(), (1, 2));
        let exponent = original_exponent(cas, target, &wrong, budget)
            .expect_err("different exact exponent must refuse");
        assert!(matches!(
            &exponent,
            PreparedDyadicPowerError::ExponentCorrespondence
        ));
        let owner: RetainedDyadicPowerError = exponent.into();
        assert!(matches!(
            owner,
            RetainedDyadicPowerError::Preparation(PreparedDyadicPowerError::ExponentCorrespondence)
        ));
        assert_eq!(
            authority.judgment().request().coordinates[0]
                .interval
                .upper(),
            &Rational::from_ratio(2, 1, budget)?
        );
        assert_ne!(1.5_f64.to_bits(), 2.0_f64.to_bits());
        let domain = request_correspondence(cas, target, context, [1.0, 1.5], authority, budget)
            .expect_err("different represented domain must refuse");
        assert!(matches!(
            &domain,
            PreparedDyadicPowerError::DomainCorrespondence
        ));
        let owner: RetainedDyadicPowerError = domain.into();
        assert!(matches!(
            owner,
            RetainedDyadicPowerError::Preparation(PreparedDyadicPowerError::DomainCorrespondence)
        ));
        Ok(())
    }
}
