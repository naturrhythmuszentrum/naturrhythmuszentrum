// SPDX-License-Identifier: AGPL-3.0-only
//! Frozen expm1 authority composed with a separately checked exp target.

use super::{PreparedExpm1, PreparedExpm1Error, finalize_owner};
use nrz_bindings::{
    Budget,
    readings::algebra::{
        Cas, Fault, Location,
        enclosure::prepared_expm1::{Counts, Policy, Prepared, Request},
        limits::BudgetKind,
    },
};
use nrz_scalar::{ArithmeticContext, ExpFailure, ExpInput, ExpOutput, ExpProgram};

/// Resource location for the standalone exp composition.
const AT: Location = Location::new("prepared exp lowering");

/// Exact preparation policy, never a claim that exp's absolute target is met.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExpPreparationPolicy {
    /// Exact rational upper bound, as numerator and positive denominator.
    pub magnitude_bound: [u64; 2],
    /// Smallest integer k with two to k at least the bound.
    pub magnitude_exponent: i32,
    /// Auxiliary relative target b plus max(0,k).
    pub requested_bits: u32,
}

/// Preparation failures keep basis unavailability distinct from exp target refusal.
#[derive(Debug, thiserror::Error)]
pub enum PreparedExpError<'r> {
    /// No checked basis was obtained. This says nothing about reachability of exp's target.
    #[error("exp basis preparation did not obtain a checked plan")]
    BasisPreparation {
        /// The auxiliary request actually submitted; borrowed without an owned copy.
        auxiliary: &'r Request,
        /// Reproducible magnitude bound and target selection.
        policy: ExpPreparationPolicy,
    },
    /// Exact, domain or native admission failure, retaining its typed cause.
    #[error("exp basis admission: {0}")]
    Admission(#[from] PreparedExpm1Error),
}

/// Immutable graph owner for exp derived from its original expm1 encoding.
///
/// The request encodes `exp(argument)-1`, as required by the sole CAS law;
/// its `bits` are exp's desired absolute target. On the admitted negative
/// domain the auxiliary rule requests the same number of relative bits.
/// The final exp reading is a composition, not a new CAS certificate.
#[derive(Debug)]
pub struct PreparedExp<'a> {
    /// Frozen checked basis and all of its owned inventories.
    basis: PreparedExpm1<'a>,
    /// Non-authoritative preparation policy, distinct from the final claim.
    policy: ExpPreparationPolicy,
    /// Final absolute target.
    bits: u32,
}

/// Successful exp composition with its own target and the borrowed basis evidence.
#[derive(Clone, Copy, Debug)]
pub struct ExpEvaluation<'a> {
    /// Original represented operand box, nominal point and target of this application.
    input: ExpInput,
    /// Explicit endpoint and independent nominal readings.
    output: ExpOutput,
    /// Checked expm1 authority used only as the basis of this composition.
    basis: &'a Prepared,
    /// Exp's absolute endpoint target, certified by the native composition.
    bits: u32,
}

impl ExpEvaluation<'_> {
    /// Exact represented operand association for the returned readings.
    #[must_use]
    pub const fn input(&self) -> ExpInput {
        self.input
    }
    /// Ordered endpoints and separately checked centre/radius readings.
    #[must_use]
    pub const fn reading(&self) -> ExpOutput {
        self.output
    }
    /// The sole checked CAS law, not an exp-target certificate by itself.
    #[must_use]
    pub const fn basis(&self) -> &Prepared {
        self.basis
    }
    /// Absolute target of the exp composition; auxiliary policy is not part of it.
    #[must_use]
    pub const fn bits(&self) -> u32 {
        self.bits
    }
}

impl<'a> PreparedExp<'a> {
    /// Prepare and freeze the sole checked law, then compose exp at its own target.
    ///
    /// For all admitted operands, `0 <= |expm1(x)| <= 1`. Thus the cheap exact
    /// rational bound is `1/1`, k=0, and the requested bits are b+max(0,0)=b.
    /// Borrowing the failure's request avoids unmetered copies of supplied inventories.
    /// # Errors
    /// Preserves resource and domain errors. A missing basis carries the actual
    /// auxiliary request and makes no statement about exp target reachability.
    pub fn new<'r>(
        cas: &'a mut Cas,
        request: &'r Request,
        domain: [f64; 2],
        search: Policy,
        context: &ArithmeticContext,
        budget: &mut Budget,
        counts: &mut Counts,
    ) -> Result<Self, PreparedExpError<'r>> {
        Self::with_basis(request, domain, budget, |budget| {
            // Domain widening must update the bound, submitted auxiliary request and failure's carried request together.
            PreparedExpm1::new(cas, request, domain, search, context, budget, counts)
        })
    }

    /// Keep the constructor's admission path intact when a control supplies a prepared basis.
    fn with_basis<'r>(
        request: &'r Request,
        domain: [f64; 2],
        budget: &mut Budget,
        prepare: impl FnOnce(&mut Budget) -> Result<PreparedExpm1<'a>, PreparedExpm1Error>,
    ) -> Result<Self, PreparedExpError<'r>> {
        let valid = domain.iter().all(|x| x.is_finite())
            && domain[0] >= -2.0
            && domain[1] <= 0.0
            && domain[0] <= domain[1];
        if !valid {
            return Err(PreparedExpm1Error::Domain.into());
        }
        let policy = auxiliary_policy(request.bits, [1, 1], budget)?;
        // Composition storage is live alongside the basis throughout construction.
        let extra = (size_of::<Self>() - size_of::<PreparedExpm1<'_>>()) as u64;
        let basis = budget.with_retained_payload(extra, AT, prepare);
        match basis {
            Ok(basis) => Ok(
                finalize_owner(basis.bytes.saturating_add(extra), budget, AT, || {
                    #[cfg(test)]
                    constructor_tests::ASSEMBLED.set(true);
                    Self {
                        basis,
                        policy,
                        bits: request.bits,
                    }
                })
                .map_err(PreparedExpm1Error::from)?,
            ),
            Err(PreparedExpm1Error::TargetNotCertifiable) => {
                Err(PreparedExpError::BasisPreparation {
                    auxiliary: request,
                    policy,
                })
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Auxiliary request selection, not final numerical evidence.
    #[must_use]
    pub const fn preparation_policy(&self) -> ExpPreparationPolicy {
        self.policy
    }

    /// Checked basis, retained immutably for the complete plan lifetime.
    #[must_use]
    pub fn basis(&self) -> &Prepared {
        self.basis.authority()
    }

    /// Apply the exp composition without exact operations or descriptor rebuilding.
    /// # Errors
    /// Preserves operand refusals and separately checks the final absolute target.
    pub fn evaluate(
        &self,
        input: ExpInput,
        context: &ArithmeticContext,
    ) -> Result<ExpEvaluation<'_>, ExpFailure> {
        ExpProgram::new(&self.basis.program, self.bits)
            .evaluate(input, context)
            .map(|output| ExpEvaluation {
                input,
                output,
                basis: self.basis.authority(),
                bits: self.bits,
            })
    }

    /// Prepay the standalone slice-and-vector route before allocation or traversal.
    /// # Errors
    /// Returns complete typed resource causes, retaining previous ledger consumption.
    pub fn evaluate_batch(
        &self,
        inputs: &[ExpInput],
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<Vec<Result<ExpEvaluation<'_>, ExpFailure>>, Fault> {
        budget.with_work_phase("prepared-exp", "nrz.prelude", |budget| {
            let count = inputs.len() as u64;
            budget.check(BudgetKind::MatrixEntries, count, AT)?;
            let output = count
                .checked_mul(size_of::<Result<ExpEvaluation<'_>, ExpFailure>>() as u64)
                .ok_or_else(|| Fault::capacity(BudgetKind::OutputBytes, AT))?;
            let input = count
                .checked_mul(size_of::<ExpInput>() as u64)
                .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
            budget.check(BudgetKind::OutputBytes, output, AT)?;
            let extra = (size_of::<Self>() - size_of::<PreparedExpm1<'_>>()) as u64;
            let bytes = self
                .basis
                .bytes
                .saturating_add(extra)
                .saturating_add(input)
                .saturating_add(output);
            self.basis.graph.with_retained_graph(budget, |budget| {
                budget.with_retained_payload(bytes, AT, |budget| {
                    budget.work(count, AT)?;
                    let mut results = Vec::new();
                    results
                        .try_reserve_exact(inputs.len())
                        .map_err(|_| Fault::allocation(AT))?;
                    for &input in inputs {
                        results.push(self.evaluate(input, context));
                    }
                    Ok(results)
                })
            })
        })
    }
}

/// Exact constant-size rational comparison; shifting a u64 by at most 63 fits u128.
fn auxiliary_policy(
    bits: u32,
    bound: [u64; 2],
    budget: &mut Budget,
) -> Result<ExpPreparationPolicy, PreparedExpm1Error> {
    budget.work(1, AT)?;
    let [numerator, denominator] = bound;
    if numerator == 0 || denominator == 0 {
        return Err(PreparedExpm1Error::Domain);
    }
    let mut k = i64::from(denominator.leading_zeros()) - i64::from(numerator.leading_zeros());
    let above = if k >= 0 {
        u128::from(numerator) > (u128::from(denominator) << k.unsigned_abs())
    } else {
        (u128::from(numerator) << k.unsigned_abs()) > u128::from(denominator)
    };
    if above {
        k += 1;
    }
    let extra =
        u32::try_from(k.max(0)).map_err(|_| Fault::capacity(BudgetKind::IntegerExponent, AT))?;
    let requested_bits = bits
        .checked_add(extra)
        .ok_or_else(|| Fault::capacity(BudgetKind::IntegerExponent, AT))?;
    Ok(ExpPreparationPolicy {
        magnitude_bound: bound,
        magnitude_exponent: i32::try_from(k)
            .map_err(|_| Fault::capacity(BudgetKind::IntegerExponent, AT))?,
        requested_bits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Isolate the returned-output boundary from the earlier, potentially larger CAS registration.
    /// A 4,096-byte basis plus 32 fixed bytes requires 4,128, with prior Work retained.
    /// The construction finalizer admits N and refuses N−1 before assembly.
    /// # Panics
    /// Panics if output admission, assembly order or the typed cause changes.
    /// ´claim:matrix:prepared-exp-output-boundary´
    /// ´test:unit:prepared-exp-output-boundary´
    #[test]
    fn prepared_exp_output_boundary() -> Result<(), Box<dyn std::error::Error>> {
        use nrz_bindings::readings::algebra::{Exhaustion, FaultKind, limits::Limits};
        for limit in [4128, 4127] {
            let mut budget = Budget::new(Limits {
                output_bytes: limit,
                ..Limits::default()
            })?;
            budget.work(7, AT)?;
            let assembled = std::cell::Cell::new(false);
            let result = finalize_owner(4096 + 32, &budget, AT, || {
                assembled.set(true);
                4128_u64
            });
            if limit == 4128 {
                assert_eq!(result?, 4128);
                assert!(assembled.get());
            } else {
                assert!(!assembled.get(), "output admission precedes assembly");
                assert_eq!(
                    result.expect_err("output boundary").kind,
                    FaultKind::Exhausted {
                        reason: Exhaustion::Limit,
                        budget: BudgetKind::OutputBytes,
                        limit,
                        needed: 4128
                    }
                );
            }
            assert_eq!(budget.usage().work, 7);
        }
        Ok(())
    }
    /// Integer powers bracket exact rational bounds; negative exponents clamp at zero.
    /// One policy Work is prepaid, with exact and one-below limits after prior consumption.
    /// # Panics
    /// Panics if ceiling, clamping, overflow or the exact resource cause changes.
    /// ´claim:matrix:prepared-exp-auxiliary-policy´
    /// ´test:unit:prepared-exp-auxiliary-policy´
    #[test]
    fn prepared_exp_auxiliary_policy() -> Result<(), Box<dyn std::error::Error>> {
        use nrz_bindings::readings::algebra::{Exhaustion, FaultKind, limits::Limits};
        for (bound, exponent, requested) in [
            ([3, 2], 1, 41),
            ([1, 4], -2, 40),
            ([1, 1], 0, 40),
            ([1, 2], -1, 40),
            ([5, 1], 3, 43),
        ] {
            let mut budget = Budget::new(Limits::default())?;
            let policy = auxiliary_policy(40, bound, &mut budget)?;
            assert_eq!(
                (
                    policy.magnitude_bound,
                    policy.magnitude_exponent,
                    policy.requested_bits
                ),
                (bound, exponent, requested)
            );
        }
        for work in [8, 7] {
            let mut budget = Budget::new(Limits {
                work,
                ..Limits::default()
            })?;
            budget.work(7, AT)?;
            let result = auxiliary_policy(40, [1, 1], &mut budget);
            if work == 8 {
                assert_eq!(result?.requested_bits, 40);
                assert_eq!(budget.usage().work, 8);
            } else {
                let PreparedExpm1Error::Exact(fault) = result.expect_err("prepaid policy") else {
                    panic!("typed resource");
                };
                assert_eq!(
                    fault.kind,
                    FaultKind::Exhausted {
                        reason: Exhaustion::Limit,
                        budget: BudgetKind::Work,
                        limit: 7,
                        needed: 8
                    }
                );
                assert_eq!(budget.usage().work, 7);
            }
        }
        let mut b = Budget::new(Limits::default())?;
        assert!(matches!(
            auxiliary_policy(u32::MAX, [2, 1], &mut b),
            Err(PreparedExpm1Error::Exact(_))
        ));
        Ok(())
    }
}

#[cfg(test)]
#[path = "prepared_exp_constructor_tests.rs"]
mod constructor_tests;
