// SPDX-License-Identifier: AGPL-3.0-only
//! Optional live quotient traces retain actual scalar operations and the complete input lease.

mod records;

use super::{LiveDecisionError, PreparedDecision, ratio};
use crate::{Coverage, FrozenDecisionInputs, Heap};
use nrz_bindings::Budget;
use nrz_bindings::readings::algebra::{
    Cas, Location,
    decision::{
        CheckedRationalEnclosure, EnclosureOrigin, RationalEnclosure, ScalarKernelCertificate,
    },
    limits::BudgetKind,
    pi_bounds::PiBoundCertificate,
    proof::VerifiedWitness,
};
use nrz_scalar::{ArithmeticContext, KernelCertificate, KernelUsage, LiteralLimits, Scalar};
use std::sync::{Arc, Mutex};

/// Every optional trace charge is separate from the existing exact-box receiver.
const AT: Location = Location::new("matrix live scalar kernel");

/// Complete executed trace and mathematical receipt, without standalone live authority.
#[derive(Debug)]
pub struct RatioKernelEvidence {
    /// Original numerator and denominator slots in the retained complete source inventory.
    order: [usize; 2],
    /// Two input sums, then denominator reciprocal, then quotient product.
    records: [KernelCertificate; 4],
    /// Independently checked original target, input sets and exact endpoint relations.
    original: CheckedRationalEnclosure,
    /// Exact scalar construction/export inventory charged on the owner ledger.
    scalar_usage: KernelUsage,
    /// Numerical interpretation for the complete executed schedule.
    revision: u32,
    /// Owned records and checked contract, excluding the separate frozen input owner.
    bytes: u64,
}

impl RatioKernelEvidence {
    /// Historical exact original contract; every endpoint is a rational converted from a dyadic.
    #[must_use]
    pub const fn contract(&self) -> &RationalEnclosure {
        self.original.contract()
    }
    /// Actual ordered input-box and quotient operations, never a reconstructed nominal trace.
    #[must_use]
    pub const fn records(&self) -> &[KernelCertificate; 4] {
        &self.records
    }
    /// Original numerator and denominator slots; versions live in the accompanying frozen inputs.
    #[must_use]
    pub const fn input_order(&self) -> [usize; 2] {
        self.order
    }
    /// Scalar Work and successful construction/export record counts.
    #[must_use]
    pub const fn scalar_usage(&self) -> KernelUsage {
        self.scalar_usage
    }
    /// Logical trace ownership, excluding the frozen source inventory.
    #[must_use]
    pub const fn retained_payload_bytes(&self) -> u64 {
        self.bytes
    }
}

/// Historical trace bound to the exact value, total-radius and numerical-companion versions read.
#[derive(Debug)]
pub struct FrozenRatioKernel<P: Scalar> {
    /// Complete native lease; no source is replaced by its nominal center.
    frozen: FrozenDecisionInputs<P>,
    /// Pure executed operations and their independently checked mathematical receipt.
    evidence: RatioKernelEvidence,
}

impl<P: Scalar> FrozenRatioKernel<P> {
    /// Complete archived source identity and versions, requiring revalidation before use.
    #[must_use]
    pub const fn inputs(&self) -> &FrozenDecisionInputs<P> {
        &self.frozen
    }
    /// Complete historical arithmetic and exact certificate under those inputs.
    #[must_use]
    pub const fn evidence(&self) -> &RatioKernelEvidence {
        &self.evidence
    }
    /// Combined logical ownership of source versions, operations and checked output.
    #[must_use]
    pub const fn retained_payload_bytes(&self) -> u64 {
        self.frozen
            .retained_payload_bytes()
            .saturating_add(self.evidence.bytes)
    }
}

impl Heap {
    /// Execute and certify the closed original quotient over the complete frozen total-radius sets.
    ///
    /// Only declared native constant inputs with rational or rational-times-pi coherent
    /// values are supported. The supplied Machin data is independently replayed by CAS.
    /// Numerical error is already in each total radius and is never added twice.
    /// # Errors
    /// Refuses stale or foreign versions, unsupported originals, a nominal source outside
    /// its numerical box, false Machin data, scalar arithmetic and exhausted resources.
    /// No graph, heap record, trace prefix or live attachment is published on refusal.
    pub fn trace_ratio_decision<P: Scalar + 'static>(
        &self,
        frozen: FrozenDecisionInputs<P>,
        graph: &mut Cas,
        pi: &PiBoundCertificate,
        arithmetic: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<FrozenRatioKernel<P>, LiveDecisionError> {
        graph.with_enclosure_attempt(budget, |attempt, budget| {
            self.check_decision_inputs(&frozen, budget)?;
            let declared = frozen
                .operands()
                .iter()
                .all(|operand| operand.input().coverage == Coverage::DeclaredBounds);
            if !declared {
                return Err(LiveDecisionError::Recipe);
            }
            let order = ratio::original(&frozen, budget)?;
            let scratch = frozen
                .retained_payload_bytes()
                .saturating_add(records::pi_bytes(pi))
                .saturating_add(4096);
            budget.check(BudgetKind::OutputBytes, scratch, AT)?;
            budget.work(scratch - frozen.retained_payload_bytes(), AT)?;
            let evidence = budget.with_retained_payload(scratch, AT, |budget| {
                let (records, scalar_usage) = records::execute(&frozen, order, arithmetic, budget)?;
                let laws = records::convert(&frozen, order, &records, pi, budget)?;
                let output = laws.last().ok_or(LiveDecisionError::Recipe)?;
                let spec = frozen.association().case().specification();
                let contract = RationalEnclosure {
                    target: spec.target.clone(),
                    context: spec.context.clone(),
                    input_family: spec.family.domain.clone(),
                    lower: output.lower.clone(),
                    upper: output.upper.clone(),
                    origin: EnclosureOrigin::ScalarKernel(Box::new(ScalarKernelCertificate {
                        version: 2,
                        expression: spec.target.clone(),
                        context: spec.context.clone(),
                        input_family: spec.family.domain.clone(),
                        records: laws,
                    })),
                };
                let original = attempt.check_rational_enclosure(&contract, budget)?;
                let bytes = original.retained_bytes().saturating_add(768);
                budget.check(
                    BudgetKind::OutputBytes,
                    bytes.saturating_add(frozen.retained_payload_bytes()),
                    AT,
                )?;
                Ok::<_, LiveDecisionError>(RatioKernelEvidence {
                    order,
                    records,
                    original,
                    scalar_usage,
                    revision: arithmetic.revision(),
                    bytes,
                })
            })?;
            Ok(FrozenRatioKernel { frozen, evidence })
        })
    }

    /// Prepare a selected live quotient only from the identical checked executed-trace contract.
    ///
    /// The portable witness is checked independently by CAS before this call. The
    /// complete native input lease is revalidated here and again during publication.
    /// # Errors
    /// Refuses foreign or stale versions, a substituted enclosure or source scope,
    /// failed physical admission and exhausted resources without a live attachment.
    pub fn prepare_ratio_kernel_decision<P: Scalar + 'static>(
        &self,
        trace: FrozenRatioKernel<P>,
        graph: Arc<Mutex<Cas>>,
        witness: VerifiedWitness,
        budget: &mut Budget,
    ) -> Result<PreparedDecision<P>, LiveDecisionError> {
        self.check_decision_inputs(&trace.frozen, budget)?;
        let selected =
            budget.with_retained_payload(trace.retained_payload_bytes(), AT, |budget| {
                let authority = graph.lock().map_err(|_| {
                    LiveDecisionError::Admission(Box::new(
                        nrz_bindings::DecisionError::GraphPoisoned,
                    ))
                })?;
                let (_, supplied) = authority.integer_selection_evidence(&witness, budget)?;
                budget.work(trace.evidence.original.retained_bytes(), AT)?;
                if supplied != trace.evidence.contract() {
                    return Err(LiveDecisionError::Recipe);
                }
                drop(authority);
                let case = trace.frozen.association().case();
                case.admit(graph, witness, case.attachment(), budget)
                    .map_err(|cause| LiveDecisionError::Admission(Box::new(cause)))
            })?;
        Ok(PreparedDecision {
            frozen: trace.frozen,
            selected,
            arithmetic_revision: trace.evidence.revision,
            literals: LiteralLimits::new(0, 0, 0),
            kernel: Some(Box::new(trace.evidence)),
        })
    }
}
