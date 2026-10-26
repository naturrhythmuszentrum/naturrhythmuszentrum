// SPDX-License-Identifier: AGPL-3.0-only
//! Physical count descriptors retain exact provenance and never allocate numerical storage.

use super::{DecisionError, SelectedDecision, storage};
use crate::{CheckedDeclarations, Declaration, Descriptor};
use nrz_cas::{
    Cas, Fault, Handle, InvalidCode, Rational,
    limits::Budget,
    proof::{DomainContract, Statement, VerifiedWitness},
    syntax::{AtomicSort, Operator, Sort},
};
use std::rc::Rc;

/// Exact source of a materialized physical count, retained with its original owner.
#[derive(Debug)]
pub enum CountEvidence {
    /// A checked data-declared exact count bypasses numerical decisions.
    Declared {
        /// Original immutable physical declaration owner.
        owner: Rc<CheckedDeclarations>,
        /// Exact declaration key within that owner.
        name: String,
    },
    /// A checked structural Integer equality bypasses numerical decisions.
    Proved {
        /// Retained mathematical authority.
        graph: Rc<Cas>,
        /// Original Integer expression mapped to the declared counted kind.
        source: Handle,
        /// Exact equality and every retained source assumption.
        witness: Box<VerifiedWitness>,
    },
    /// A separately admitted certified outcome with its complete physical attachment.
    Decided(SelectedDecision),
}

/// A finite physical tally of a declared thing, with exact evidence and no heap region.
#[derive(Clone, Debug)]
pub struct CountDescriptor(Rc<Data>);

/// Immutable machine count and its complete physical provenance.
#[derive(Debug)]
struct Data {
    /// Declared counted kind and mathematical domain.
    descriptor: Descriptor,
    /// Exact executable representation; never rounded, wrapped or clamped.
    value: u64,
    /// Original data, structural proof or decided snapshot.
    evidence: CountEvidence,
    /// Conservative owned result bytes, excluding shared declaration and graph owners.
    bytes: u64,
}

impl SelectedDecision {
    /// Materialize the selected Integer as the predeclared physical counted kind.
    /// # Errors
    /// Keeps mathematical admissibility separate from representation/caller capacity and Work.
    /// A refusal leaves this immutable exact selected snapshot available for another use.
    pub fn count(
        &self,
        capacity: u64,
        budget: &mut Budget,
    ) -> Result<CountDescriptor, DecisionError> {
        let descriptor = &self.case().specification().count;
        let bytes = prepare(descriptor, budget)?;
        let value = self
            .cas()
            .materialize_integer_selection(self.witness(), capacity, budget)?;
        Ok(CountDescriptor(Rc::new(Data {
            descriptor: descriptor.clone(),
            value,
            evidence: CountEvidence::Decided(self.clone()),
            bytes,
        })))
    }
}

impl CountDescriptor {
    /// Materialize an already exact checked data count without constructing a decision.
    /// # Errors
    /// Refuses nonconstant/non-count declarations, unsupported capacity and caller resources.
    pub fn declared(
        owner: Rc<CheckedDeclarations>,
        name: &str,
        capacity: u64,
        budget: &mut Budget,
    ) -> Result<Self, DecisionError> {
        let length = u64::try_from(name.len()).map_err(|_| DecisionError::Capacity)?;
        budget.work(length.saturating_add(1), storage::AT)?;
        let Some(Declaration::Constant {
            descriptor, value, ..
        }) = owner.declarations().get(name)
        else {
            return Err(DecisionError::CountKind);
        };
        let retained = prepare(descriptor, budget)?;
        descriptor
            .kind()
            .count_domain()
            .ok_or(DecisionError::CountKind)?
            .check_with_budget(value, budget)?;
        let value = materialize(value, capacity, budget)?;
        let bytes = length.saturating_add(retained);
        storage::retained(bytes, budget)?;
        let descriptor = descriptor.clone();
        Ok(Self(Rc::new(Data {
            descriptor,
            value,
            evidence: CountEvidence::Declared {
                owner,
                name: name.into(),
            },
            bytes,
        })))
    }

    /// Map a structurally proved Integer expression to a declared counted kind without a decision.
    ///
    /// The witness proves the exact original Integer source equal to an Integer literal
    /// on its complete retained domain. Numerical decision dependencies cannot bypass
    /// selected-snapshot admission through this route; original assumptions survive.
    /// # Errors
    /// Refuses foreign, changed or residual-bearing proofs, numerical decision dependencies,
    /// wrong sorts/kinds/domains, unsupported capacity and exhausted caller resources.
    pub fn proved(
        graph: Rc<Cas>,
        source: Handle,
        descriptor: Descriptor,
        witness: VerifiedWitness,
        capacity: u64,
        budget: &mut Budget,
    ) -> Result<Self, DecisionError> {
        let retained = prepare(&descriptor, budget)?;
        let Statement::Equality {
            left,
            right,
            domain,
            domain_contract: DomainContract::DefinedAndEqual,
        } = &witness.judgment().statement
        else {
            return Err(DecisionError::Case);
        };
        if left != &source
            || !witness.is_discharged()
            || domain != &graph.admissible_region(&witness.judgment().context)?
        {
            return Err(DecisionError::Case);
        }
        structural(&graph, &witness, budget)?;
        let (_, original_sort) = graph.expression(&source)?;
        let (literal, result_sort) = graph.expression(right)?;
        let Operator::Rational(number) = literal else {
            return Err(DecisionError::Case);
        };
        if original_sort != &Sort::Value(AtomicSort::Integer) || result_sort != original_sort {
            return Err(DecisionError::Case);
        }
        descriptor
            .kind()
            .count_domain()
            .ok_or(DecisionError::CountKind)?
            .check_with_budget(number, budget)?;
        let value = materialize(number, capacity, budget)?;
        let bytes = storage::witness_bytes(&witness)?.saturating_add(retained);
        storage::retained(bytes, budget)?;
        Ok(Self(Rc::new(Data {
            descriptor,
            value,
            evidence: CountEvidence::Proved {
                graph,
                source,
                witness: Box::new(witness),
            },
            bytes,
        })))
    }

    /// Original counted kind and domain; this descriptor has no numerical-region binding.
    #[must_use]
    pub fn descriptor(&self) -> &Descriptor {
        &self.0.descriptor
    }
    /// Exact executable tally within the capacity declared to its constructor.
    #[must_use]
    pub fn value(&self) -> u64 {
        self.0.value
    }
    /// Complete retained evidence, distinguishing direct exact and decision-derived routes.
    #[must_use]
    pub fn evidence(&self) -> &CountEvidence {
        &self.0.evidence
    }
    /// Owned metadata and proof envelope; existing shared sources are accounted by their owners.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
}

/// A validated counted kind controls the interpretation before any executable conversion.
fn prepare(descriptor: &Descriptor, budget: &mut Budget) -> Result<u64, DecisionError> {
    let crate::Kind::Count(name, _) = descriptor.kind() else {
        return Err(DecisionError::CountKind);
    };
    if name.trim().is_empty() {
        return Err(DecisionError::CountKind);
    }
    let length = u64::try_from(name.len()).map_err(|_| DecisionError::Capacity)?;
    let measure =
        u64::try_from(descriptor.measure().name().len()).map_err(|_| DecisionError::Capacity)?;
    let length = length.checked_add(measure).ok_or(DecisionError::Capacity)?;
    budget.work(length.saturating_add(2), storage::AT)?;
    // Count descriptors have fixed zero dimensions, unit scale and zero offset.
    // This envelope covers their fixed rational records, limbs, wrapper and shared-owner handles.
    let retained = length.saturating_add(2_048);
    storage::retained(retained, budget)?;
    Ok(retained)
}

/// Inspect the actual proof closure so a numerical decision cannot masquerade as a direct count.
fn structural(
    graph: &Cas,
    witness: &VerifiedWitness,
    budget: &mut Budget,
) -> Result<(), DecisionError> {
    for citation in graph.proof_citations(witness.proof(), budget)? {
        budget.work(1, storage::AT)?;
        let numerical = citation.auxiliary_schema(graph)?.is_some_and(|schema| {
            matches!(
                schema.name,
                "decision.enclosure-membership" | "decision.integer-family-selection"
            )
        });
        if numerical {
            return Err(DecisionError::Case);
        }
        for assumption in citation.hypotheses {
            budget.work(1, storage::AT)?;
            if matches!(
                assumption.kind,
                nrz_cas::proof::HypothesisKind::CertifiedRegionMembership
                    | nrz_cas::proof::HypothesisKind::IntegerSelection
            ) {
                return Err(DecisionError::Case);
            }
        }
    }
    Ok(())
}

/// Exact public rational export preserves the same distinct materialization refusal as CAS.
fn materialize(value: &Rational, capacity: u64, budget: &mut Budget) -> Result<u64, DecisionError> {
    let refused = || {
        DecisionError::Algebra(Fault::invalid(
            InvalidCode::DecisionMaterializationCapacity,
            storage::AT,
        ))
    };
    value.check(budget, storage::AT)?;
    budget.work(1, storage::AT)?;
    if value.is_negative() || !value.is_integer() || value.component_bits().0 > 64 {
        return Err(refused());
    }
    let exact = value.exact_string(budget)?;
    budget.work(
        u64::try_from(exact.len()).map_err(|_| DecisionError::Capacity)?,
        storage::AT,
    )?;
    exact
        .parse::<u64>()
        .ok()
        .filter(|value| *value <= capacity)
        .ok_or_else(refused)
}
