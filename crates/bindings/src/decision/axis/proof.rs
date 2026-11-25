// SPDX-License-Identifier: AGPL-3.0-only
//! Certified sign membership discharges only the already declared original reciprocal.

use super::{
    AxisPhaseAttempt, AxisPhaseAttempts, AxisPhaseCase, AxisPhaseOutcome, DecisionError, storage,
    syntax::Builder,
};
use crate::Budget;
use nrz_cas::{
    Cas, Handle, Reference,
    decision::{CaseCut, CoordinateInterval, DecisionIntroduction, RationalEnclosure},
    domain::{ContextDraft, HypothesisRef},
    proof::{ApplicationDraft, Certificate, DomainContract, Primitive, Statement, VerifiedWitness},
    syntax::{Draft, PredicateKind},
};

impl AxisPhaseAttempts {
    /// Check one original enclosure and select only a uniform strict sign or proved zero.
    ///
    /// An intersecting box retains checked enclosure evidence and returns undecided.
    /// Failed attempts consume the same finite counter and caller Work as successful ones.
    /// # Errors
    /// Refuses changed original questions, false enclosure origins, exhausted attempts,
    /// graph faults and cumulative caller limits without publishing a selected prefix.
    pub fn attempt(
        &mut self,
        enclosure: &RationalEnclosure,
        budget: &mut Budget,
    ) -> Result<AxisPhaseAttempt, DecisionError> {
        let before = budget.usage();
        budget.work(1, storage::AT)?;
        if self.used == self.maximum {
            return Err(DecisionError::Attempts);
        }
        self.used += 1;
        let mut result = self.case.attempt(enclosure, budget)?;
        result.usage[0] = before;
        Ok(result)
    }
}

impl AxisPhaseCase {
    /// The finite execution counter owns retries; no fresh internal budget is created.
    fn attempt(
        &self,
        proposed: &RationalEnclosure,
        budget: &mut Budget,
    ) -> Result<AxisPhaseAttempt, DecisionError> {
        let before = budget.usage();
        let mut cas = self
            .0
            .graph
            .lock()
            .map_err(|_| DecisionError::GraphPoisoned)?;
        let spec = self.specification();
        if proposed.target != spec.target
            || proposed.context != spec.context
            || proposed.input_family != cas.admissible_region(&spec.context, budget)?
        {
            return Err(DecisionError::Case);
        }
        let enclosure = cas.check_rational_enclosure(proposed, budget)?;
        let contract = enclosure.contract();
        budget.work(
            storage::rational_bytes(&contract.lower)
                .saturating_add(storage::rational_bytes(&contract.upper)),
            storage::AT,
        )?;
        let outcome = if contract.upper.is_negative() {
            AxisPhaseOutcome::Negative
        } else if !contract.lower.is_negative() && !contract.lower.is_zero() {
            AxisPhaseOutcome::Positive
        } else if contract.lower.is_zero() && contract.upper.is_zero() {
            AxisPhaseOutcome::Undefined
        } else {
            AxisPhaseOutcome::Undecided
        };
        let mut bytes = enclosure
            .retained_bytes()
            .checked_add(512)
            .ok_or(DecisionError::Capacity)?;
        storage::retained(bytes, budget)?;
        let (membership, reciprocal) = if outcome == AxisPhaseOutcome::Undecided {
            (None, None)
        } else {
            let (membership, reciprocal) =
                budget.with_retained_payload(bytes, storage::AT, |budget| {
                    selected(&mut cas, self, contract, outcome, budget)
                })?;
            for witness in std::iter::once(&membership).chain(reciprocal.as_ref()) {
                bytes = bytes
                    .checked_add(storage::witness_bytes(witness)?)
                    .ok_or(DecisionError::Capacity)?;
            }
            (Some(membership), reciprocal)
        };
        drop(cas);
        storage::retained(bytes, budget)?;
        Ok(AxisPhaseAttempt {
            case: self.clone(),
            enclosure,
            outcome,
            membership,
            reciprocal,
            usage: [before, budget.usage()],
            bytes,
        })
    }
}

/// Select the original association with the unchanged coordinate-to-target premise.
fn selected(
    cas: &mut Cas,
    case: &AxisPhaseCase,
    enclosure: &RationalEnclosure,
    outcome: AxisPhaseOutcome,
    budget: &mut Budget,
) -> Result<(VerifiedWitness, Option<VerifiedWitness>), DecisionError> {
    let index = match outcome {
        AxisPhaseOutcome::Negative => 0,
        AxisPhaseOutcome::Undefined => 1,
        AxisPhaseOutcome::Positive => 2,
        AxisPhaseOutcome::Undecided => return Err(DecisionError::Case),
    };
    let [region, association] = &case.0.branches[index];
    let spec = case.specification();
    let introduction = ApplicationDraft {
        context: spec.context.clone(),
        rule: Primitive::RegionDecide,
        conclusion: Statement::Subset {
            subset: enclosure.input_family.clone(),
            superset: region.clone(),
        },
        premises: vec![0],
        substitutions: Vec::new(),
        certificate: Certificate::Decision(Box::new(DecisionIntroduction {
            partition: case.0.partition.clone(),
            cut: CaseCut {
                function: case.0.partition.function.clone(),
                association: association.clone(),
                integer: None,
                bounds: vec![CoordinateInterval {
                    coordinate: spec.coordinate.clone(),
                    lower: enclosure.lower.clone(),
                    upper: enclosure.upper.clone(),
                }],
            },
            enclosures: vec![enclosure.clone()],
        })),
    };
    let witness = cas.check_proof(
        &spec.context,
        &[case.0.correspondence.clone(), introduction],
        budget,
    )?;
    let reciprocal = if outcome == AxisPhaseOutcome::Undefined {
        None
    } else {
        Some(discharge(cas, case, region, &witness, budget)?)
    };
    Ok((witness, reciprocal))
}

/// The original reciprocal's own defined-equality residual is discharged under certified membership.
fn discharge(
    cas: &mut Cas,
    case: &AxisPhaseCase,
    region: &Handle,
    introduction: &VerifiedWitness,
    budget: &mut Budget,
) -> Result<VerifiedWitness, DecisionError> {
    let spec = case.specification();
    let mut builder = Builder::new(cas, spec, budget)?;
    let frame = builder.admit(
        Draft::Context(ContextDraft::Certified {
            parent: Reference::Existing(spec.context.clone()),
            region: Reference::Existing(region.clone()),
            introduction: Reference::Existing(introduction.proof().clone()),
        }),
        budget,
    )?;
    let domain = builder.cas.admissible_region(&frame, budget)?;
    let slot = HypothesisRef {
        context: frame.clone(),
        slot: 0,
    };
    let predicate = builder.predicate(
        PredicateKind::NonZero,
        std::slice::from_ref(&spec.coordinate),
        budget,
    )?;
    let original = Statement::Equality {
        left: case.reciprocal().clone(),
        right: case.reciprocal().clone(),
        domain: domain.clone(),
        domain_contract: DomainContract::DefinedAndEqual,
    };
    let steps = [
        ApplicationDraft {
            context: frame.clone(),
            rule: Primitive::ContextHypothesis,
            conclusion: Statement::Subset {
                subset: domain.clone(),
                superset: region.clone(),
            },
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::Hypothesis(slot.clone()),
        },
        ApplicationDraft {
            context: frame.clone(),
            rule: Primitive::RegionDecide,
            conclusion: Statement::PredicateFact {
                region: domain,
                predicate,
            },
            premises: vec![0],
            substitutions: Vec::new(),
            certificate: Certificate::DecisionNonZero(slot),
        },
        ApplicationDraft {
            context: frame.clone(),
            rule: Primitive::Reflexivity,
            conclusion: original.clone(),
            premises: Vec::new(),
            substitutions: Vec::new(),
            certificate: Certificate::None,
        },
        ApplicationDraft {
            context: frame.clone(),
            rule: Primitive::ContextClose,
            conclusion: original,
            premises: vec![2, 1],
            substitutions: Vec::new(),
            certificate: Certificate::DecisionDischarge,
        },
    ];
    Ok(builder.cas.check_proof(&frame, &steps, budget)?)
}
