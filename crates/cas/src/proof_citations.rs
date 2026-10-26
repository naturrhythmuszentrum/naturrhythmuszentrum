// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded citation views of graph-owned checked applications, without a second proof language.

use crate::budget::{Budget, BudgetKind};
use crate::conditions::GuardKind;
use crate::domain_model::HypothesisRef;
use crate::error::{Fault, InvalidCode, Location};
use crate::formal_register::PrimitiveSchema;
use crate::graph::{Cas, Handle, LogicalId, Vertex, size};
use crate::hypothesis_kinds::{HypothesisKind, QualifiedHypothesis};
use crate::proof_model::CheckedApplication;
use crate::rule::Primitive;
use crate::scalar_model::{CheckedStep, Relation};

/// Every failure or charge belongs to this read-only metadata operation.
const AT: Location = Location::new("proof citations");

/// A citation can only come from a graph-owned checked proof application.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Rule, AT)
}

/// A direct earlier proof premise and the formal kind of its actual checked statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PremiseCitation {
    /// Exact store-owned earlier proof, preserving shared premises and ordered edges.
    pub proof: Handle,
    /// Formal kind reconstructed from the premise's checked statement.
    pub kind: HypothesisKind,
}

/// The formal citation and complete assumed conditions of one checked application.
///
/// These are inspectable metadata; changing a returned copy never changes or proves a theorem.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofCitation {
    /// Exact store-owned checked application proof.
    pub proof: Handle,
    /// Primitive actually retained by the checker, with its original key and semantic version.
    pub primitive: Primitive,
    /// Direct checked premises in their original consumption order.
    pub premises: Vec<PremiseCitation>,
    /// Complete qualified hypothesis closure retained by this application.
    pub hypotheses: Vec<QualifiedHypothesis>,
    /// Formal kinds of every remaining guarded residual, in the checker's retained order.
    pub residuals: Vec<HypothesisKind>,
}

impl ProofCitation {
    /// Inspect the constant theorem consumed inside this checked application.
    ///
    /// The ordinary citation closure includes these retained applications, so this
    /// view also exposes pi dependencies after composition and fresh-store replay.
    /// # Errors
    /// Refuses foreign or non-proof handles and exhausted explicit traversal Work.
    pub fn constant_schema(
        &self,
        cas: &Cas,
        budget: &mut Budget,
    ) -> Result<Option<&'static crate::proof::LemmaSchema>, Fault> {
        let Source::Scoped(value) = Source::read(cas, cas.checked(&self.proof)?)? else {
            return Ok(None);
        };
        let uses_pi = match &value.certificate {
            crate::proof_model::Certificate::IntegerSelection(enclosure) => {
                budget.work(1, AT)?;
                matches!(
                    enclosure.origin,
                    crate::decision::EnclosureOrigin::PiBound { .. }
                )
            }
            crate::proof_model::Certificate::Decision(evidence) => {
                let mut found = false;
                for enclosure in &evidence.enclosures {
                    budget.work(1, AT)?;
                    found |= matches!(
                        enclosure.origin,
                        crate::decision::EnclosureOrigin::PiBound { .. }
                    );
                }
                found
            }
            _ => false,
        };
        Ok(uses_pi.then_some(&crate::proof::CONSTANT_SCHEMAS[0]))
    }

    /// Inspect the auxiliary decision schema actually retained by this checked application.
    ///
    /// # Errors
    /// Refuses a foreign handle or a handle that is not a checked proof.
    pub fn auxiliary_schema(
        &self,
        cas: &Cas,
    ) -> Result<Option<&'static crate::proof::LemmaSchema>, Fault> {
        let Source::Scoped(value) = Source::read(cas, cas.checked(&self.proof)?)? else {
            return Ok(None);
        };
        if matches!(
            value.judgment.statement,
            crate::proof_model::Statement::IntegerFamily(_)
        ) && value.rule == Primitive::RegionDecide
        {
            return Ok(Some(&crate::decision::DECISION_SCHEMAS[3]));
        }
        let index = match value.certificate {
            crate::proof_model::Certificate::IntegerSelection(_) => 4,
            crate::proof_model::Certificate::Decision(_) => 0,
            crate::proof_model::Certificate::DecisionNonZero(_) => 1,
            crate::proof_model::Certificate::DecisionDischarge => 2,
            _ => return Ok(None),
        };
        Ok(Some(&crate::decision::DECISION_SCHEMAS[index]))
    }

    /// Borrow the exact formal lemma, version and ordered-premise contract.
    #[must_use]
    pub const fn formal_schema(&self) -> &'static PrimitiveSchema {
        self.primitive.formal_schema()
    }
}

/// Borrow either authoritative application representation without cloning its certificate.
#[derive(Clone, Copy)]
enum Source<'a> {
    /// Scalar applications retain their original named root assumptions.
    Scalar(&'a CheckedStep),
    /// Native applications retain exact qualified hypotheses and guarded residuals.
    Scoped(&'a CheckedApplication),
}

impl<'a> Source<'a> {
    /// Authenticate a proof node and borrow its single checked application.
    fn read(cas: &'a Cas, proof: LogicalId) -> Result<Self, Fault> {
        let Vertex::Proof(application) = cas.state.node(proof)? else {
            return Err(invalid());
        };
        match cas.state.node(*application)? {
            Vertex::Application(value) => Ok(Self::Scalar(value)),
            Vertex::ScopedApplication(value) => Ok(Self::Scoped(value)),
            _ => Err(invalid()),
        }
    }

    /// The rule is supplied by the checked graph, never by a caller's citation.
    const fn primitive(self) -> Primitive {
        match self {
            Self::Scalar(value) => value.primitive,
            Self::Scoped(value) => value.rule,
        }
    }

    /// Borrow exact earlier proof edges, including their original order.
    const fn premises(self) -> &'a [LogicalId] {
        match self {
            Self::Scalar(value) => value.premises.as_slice(),
            Self::Scoped(value) => value.premises.as_slice(),
        }
    }

    /// A certified membership use retains the proof that introduced its exact slot.
    fn introduction(self, cas: &Cas) -> Result<Option<LogicalId>, Fault> {
        if let Self::Scoped(value) = self
            && let crate::proof_model::Certificate::Hypothesis(reference) = &value.certificate
            && reference.slot == 0
        {
            return Ok(cas
                .state
                .context(cas.checked(&reference.context)?)?
                .introduction);
        }
        Ok(None)
    }

    /// Read the proposition family without rebuilding or rechecking its certificate.
    fn kind(self, cas: &Cas) -> Result<HypothesisKind, Fault> {
        match self {
            Self::Scoped(value)
                if matches!(
                    &value.certificate,
                    crate::proof_model::Certificate::Decision(_)
                ) || self.introduction(cas)?.is_some() =>
            {
                Ok(HypothesisKind::CertifiedRegionMembership)
            }
            Self::Scoped(value) => cas.statement_hypothesis_kind(&value.judgment.statement),
            Self::Scalar(value) => Ok(match &value.relation {
                Relation::EqualOnDomain => HypothesisKind::Equality,
                Relation::Derivative { .. } => HypothesisKind::Derivative,
                Relation::Jacobian { .. } => HypothesisKind::Jacobian,
                Relation::SquareFree { .. } => HypothesisKind::SquareFree,
            }),
        }
    }
}

/// Conservative retained vectors, including traversal scratch, share both payload ceilings.
#[derive(Default)]
struct Meter {
    /// Cumulative reserved element payload, never refunded during extraction.
    bytes: u64,
}

impl Meter {
    /// Reserve and charge an exact additional vector length before any owned payload is copied.
    fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        count: usize,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let bytes = size(count, AT)?
            .checked_mul(size(std::mem::size_of::<T>(), AT)?)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        let total = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.check(BudgetKind::StagedPayloadBytes, total, AT)?;
        budget.check(BudgetKind::OutputBytes, total, AT)?;
        budget.work(bytes, AT)?;
        values
            .try_reserve_exact(count)
            .map_err(|_| Fault::allocation(AT))?;
        self.bytes = total;
        Ok(())
    }
}

/// Visit only reachable proof edges, once per retained application, under an explicit scan bound.
fn closure(
    cas: &Cas,
    root: LogicalId,
    meter: &mut Meter,
    budget: &mut Budget,
) -> Result<Vec<LogicalId>, Fault> {
    let mut pending = Vec::new();
    meter.reserve(&mut pending, 1, budget)?;
    pending.push(root);
    let mut visited = Vec::new();
    while let Some(proof) = pending.pop() {
        budget.work(size(visited.len(), AT)?.saturating_add(1), AT)?;
        if visited.contains(&proof) {
            continue;
        }
        let source = Source::read(cas, proof)?;
        budget.check(
            BudgetKind::ProofNodes,
            size(visited.len(), AT)?.saturating_add(1),
            AT,
        )?;
        meter.reserve(&mut visited, 1, budget)?;
        visited.push(proof);
        let premises = source.premises();
        budget.work(size(premises.len(), AT)?, AT)?;
        meter.reserve(&mut pending, premises.len(), budget)?;
        pending.extend_from_slice(premises);
        if let Some(introduction) = source.introduction(cas)? {
            meter.reserve(&mut pending, 1, budget)?;
            pending.push(introduction);
        }
    }
    let count = size(visited.len(), AT)?;
    budget.work(
        count
            .checked_mul(count)
            .ok_or_else(|| Fault::capacity(BudgetKind::Work, AT))?,
        AT,
    )?;
    visited.sort_unstable();
    Ok(visited)
}

/// Scalar named assumptions live in the exact root frame of the checked application.
fn root_frame(cas: &Cas, mut context: LogicalId, budget: &mut Budget) -> Result<LogicalId, Fault> {
    loop {
        budget.work(1, AT)?;
        let frame = cas.state.context(context)?;
        budget.check(BudgetKind::HypothesisDepth, frame.depth, AT)?;
        let Some(parent) = frame.parent else {
            return Ok(context);
        };
        context = parent;
    }
}

/// Resolve a retained scalar provenance key with prepaid full-string comparisons.
fn named_slot(cas: &Cas, context: LogicalId, key: &str, budget: &mut Budget) -> Result<u32, Fault> {
    for (slot, (name, _)) in cas.state.context(context)?.assumptions.iter().enumerate() {
        budget.work(size(name.len().max(key.len()), AT)?.saturating_add(1), AT)?;
        if name == key {
            return u32::try_from(slot).map_err(|_| invalid());
        }
    }
    Err(invalid())
}

/// Qualify exactly the hypothesis closure already retained by the scalar checker.
fn scalar_hypotheses(
    cas: &Cas,
    value: &CheckedStep,
    meter: &mut Meter,
    budget: &mut Budget,
) -> Result<Vec<QualifiedHypothesis>, Fault> {
    let mut result = Vec::new();
    meter.reserve(&mut result, value.conditions.hypotheses.len(), budget)?;
    if value.conditions.hypotheses.is_empty() {
        return Ok(result);
    }
    let context = root_frame(cas, value.context, budget)?;
    for key in &value.conditions.hypotheses {
        let slot = named_slot(cas, context, key, budget)?;
        budget.work(1, AT)?;
        let reference = HypothesisRef {
            context: cas.handle(context),
            slot,
        };
        result.push(reference.qualified(cas)?);
    }
    Ok(result)
}

/// Copy only qualified condition metadata, not the checked statement or its certificate payload.
fn hypotheses(
    cas: &Cas,
    source: Source<'_>,
    meter: &mut Meter,
    budget: &mut Budget,
) -> Result<Vec<QualifiedHypothesis>, Fault> {
    match source {
        Source::Scalar(value) => scalar_hypotheses(cas, value, meter, budget),
        Source::Scoped(value) => {
            let mut result = Vec::new();
            meter.reserve(&mut result, value.conditions.hypotheses.len(), budget)?;
            for reference in &value.conditions.hypotheses {
                budget.work(1, AT)?;
                result.push(reference.qualified(cas)?);
            }
            Ok(result)
        }
    }
}

/// Every open guard remains visible even after a local frame is closed.
fn residuals(
    cas: &Cas,
    source: Source<'_>,
    meter: &mut Meter,
    budget: &mut Budget,
) -> Result<Vec<HypothesisKind>, Fault> {
    let mut result = Vec::new();
    match source {
        Source::Scalar(value) => {
            meter.reserve(&mut result, value.conditions.residuals.len(), budget)?;
            for guard in &value.conditions.residuals {
                budget.work(1, AT)?;
                result.push(match guard.kind {
                    GuardKind::Defined => HypothesisKind::DomainGuard,
                    GuardKind::NonZero => HypothesisKind::NonZero,
                });
            }
        }
        Source::Scoped(value) => {
            meter.reserve(&mut result, value.conditions.residuals.len(), budget)?;
            for guard in &value.conditions.residuals {
                budget.work(1, AT)?;
                result.push(cas.predicate_hypothesis_kind(&guard.predicate)?);
            }
        }
    }
    Ok(result)
}

/// Read direct premise kinds from their graph-owned checked conclusions.
fn premises(
    cas: &Cas,
    source: Source<'_>,
    meter: &mut Meter,
    budget: &mut Budget,
) -> Result<Vec<PremiseCitation>, Fault> {
    let mut result = Vec::new();
    meter.reserve(&mut result, source.premises().len(), budget)?;
    for proof in source.premises() {
        budget.work(1, AT)?;
        let kind = Source::read(cas, *proof)?.kind(cas)?;
        result.push(PremiseCitation {
            proof: cas.handle(*proof),
            kind,
        });
    }
    if let Some(proof) = source.introduction(cas)? {
        meter.reserve(&mut result, 1, budget)?;
        budget.work(1, AT)?;
        result.push(PremiseCitation {
            proof: cas.handle(proof),
            kind: HypothesisKind::CertifiedRegionMembership,
        });
    }
    Ok(result)
}

/// Construct one citation from authenticated application data under the same cumulative meter.
fn citation(
    cas: &Cas,
    proof: LogicalId,
    meter: &mut Meter,
    budget: &mut Budget,
) -> Result<ProofCitation, Fault> {
    budget.work(1, AT)?;
    let source = Source::read(cas, proof)?;
    Ok(ProofCitation {
        proof: cas.handle(proof),
        primitive: source.primitive(),
        premises: premises(cas, source, meter, budget)?,
        hypotheses: hypotheses(cas, source, meter, budget)?,
        residuals: residuals(cas, source, meter, budget)?,
    })
}

impl Cas {
    /// List every reachable checked application once, in child-before-parent local admission order.
    ///
    /// Pass `VerifiedWitness::proof()` to inspect a witness. Each citation names the exact
    /// formal lemma and typed premises, qualified assumption closure and open residual kinds.
    /// Shared proof nodes remain shared; inline certificate checks remain inside their owning rule.
    /// No certificate is rechecked, no graph record is published and no wire field is added.
    /// # Errors
    /// Refuses foreign or nonproof handles and exhausted cumulative Work, proof-node,
    /// hypothesis-depth or simultaneous staged/output payload allowances.
    pub fn proof_citations(
        &self,
        proof: &Handle,
        budget: &mut Budget,
    ) -> Result<Vec<ProofCitation>, Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = self.citations_inner(proof, budget);
        budget.restore_limits(previous);
        result
    }

    /// Keep every scratch and result vector under one conservative retained-payload ceiling.
    fn citations_inner(
        &self,
        proof: &Handle,
        budget: &mut Budget,
    ) -> Result<Vec<ProofCitation>, Fault> {
        let root = self.checked(proof)?;
        Source::read(self, root)?;
        let mut meter = Meter::default();
        let ordered = closure(self, root, &mut meter, budget)?;
        let mut result = Vec::new();
        meter.reserve(&mut result, ordered.len(), budget)?;
        for id in ordered {
            result.push(citation(self, id, &mut meter, budget)?);
        }
        Ok(result)
    }
}
