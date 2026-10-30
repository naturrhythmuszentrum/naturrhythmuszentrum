// SPDX-License-Identifier: AGPL-3.0-only
//! Certified selection admission publishes only a complete immutable physical snapshot.

use super::{DecisionAttachment, DecisionError, IntegerDecisionCase, storage};
use nrz_cas::{
    Cas, Rational,
    limits::{Budget, BudgetUsage},
    proof::VerifiedWitness,
    syntax::{AtomicSort, Operator, Sort},
};
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard};

/// An exact decided outcome; executable materialization may still refuse separately.
#[derive(Clone, Debug)]
pub struct SelectedDecision(Rc<Data>);

/// The graph, mathematical theorem and physical declaration remain alive together.
#[derive(Debug)]
struct Data {
    /// Guarded graph authority of the certified theorem.
    graph: Arc<Mutex<Cas>>,
    /// Original physical source, map, native case and parameter tuple.
    case: IntegerDecisionCase,
    /// Authenticated complete attachment identity.
    attachment: DecisionAttachment,
    /// Checked mathematical proof retaining all original assumptions.
    witness: VerifiedWitness,
    /// Exact Integer value; no machine capacity has been imposed.
    value: Rational,
    /// Caller usage immediately before and after successful admission.
    usage: [BudgetUsage; 2],
    /// Conservative native witness and selected-value ownership allowance.
    bytes: u64,
}

impl IntegerDecisionCase {
    /// Admit a CAS certified Integer selection under the identical physical attachment.
    ///
    /// The original target, context, full family and selected count domain must all
    /// agree. No heap lookup, approximate comparison or executable conversion occurs.
    /// # Errors
    /// Refuses foreign or changed witnesses, attachments, cases, targets, count domains
    /// and caller limits. No selected state is published on any refusal.
    pub fn admit(
        &self,
        graph: Arc<Mutex<Cas>>,
        witness: VerifiedWitness,
        attachment: DecisionAttachment,
        budget: &mut Budget,
    ) -> Result<SelectedDecision, DecisionError> {
        let before = budget.usage();
        self.validate_attachment(&attachment, budget)?;
        let authority = graph.lock().map_err(|_| DecisionError::GraphPoisoned)?;
        let (selection, enclosure) = authority.integer_selection_evidence(&witness, budget)?;
        budget.work(self.0.comparison, storage::AT)?;
        let expected = &self.0.specification;
        if selection.family != expected.family
            || enclosure.target != expected.target
            || enclosure.context != expected.context
            || enclosure.input_family != expected.family.domain
        {
            return Err(DecisionError::Case);
        }
        let (operator, sort) = authority.expression(&selection.value)?;
        let Operator::Rational(value) = operator else {
            return Err(DecisionError::Case);
        };
        if sort != &Sort::Value(AtomicSort::Integer) {
            return Err(DecisionError::Case);
        }
        expected
            .count
            .kind()
            .count_domain()
            .ok_or(DecisionError::CountKind)?
            .check_with_budget(value, budget)?;
        let bytes = storage::witness_bytes(&witness)?
            .checked_add(storage::rational_bytes(value))
            .and_then(|size| size.checked_add(256))
            .ok_or(DecisionError::Capacity)?;
        storage::retained(bytes, budget)?;
        budget.work(
            storage::rational_bytes(value).saturating_add(8),
            storage::AT,
        )?;
        let value = value.clone();
        drop(authority);
        Ok(SelectedDecision(Rc::new(Data {
            graph,
            case: self.clone(),
            attachment,
            witness,
            value,
            usage: [before, budget.usage()],
            bytes,
        })))
    }
}

impl SelectedDecision {
    /// Exact selected Integer, independent of executable representation or capacity.
    #[must_use]
    pub fn value(&self) -> &Rational {
        &self.0.value
    }
    /// Complete declared physical meaning and original mathematical question.
    #[must_use]
    pub fn case(&self) -> &IntegerDecisionCase {
        &self.0.case
    }
    /// Immutable attachment that authorized this exact selected snapshot.
    #[must_use]
    pub fn attachment(&self) -> &DecisionAttachment {
        &self.0.attachment
    }
    /// Certified mathematical witness with all qualified source assumptions retained.
    #[must_use]
    pub fn witness(&self) -> &VerifiedWitness {
        &self.0.witness
    }
    /// Hold the retained graph guard across a complete inspection or judgment attempt.
    ///
    /// Pass this guard into nested checks instead of acquiring it again.
    /// # Errors
    /// Refuses a poisoned graph owner.
    pub fn cas(&self) -> Result<MutexGuard<'_, Cas>, DecisionError> {
        self.0
            .graph
            .lock()
            .map_err(|_| DecisionError::GraphPoisoned)
    }

    /// Caller ledger snapshots around admission; no fresh private budget was created.
    #[must_use]
    pub fn usage(&self) -> [BudgetUsage; 2] {
        self.0.usage
    }
    /// Native selected-result allowance, excluding shared original declaration owners.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
}
