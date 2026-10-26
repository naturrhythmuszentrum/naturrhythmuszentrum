// SPDX-License-Identifier: AGPL-3.0-only
//! Exact Cartesian index domains retain checked scalar ranges and their original scopes.

use super::{CheckedLatticeBasis, LatticeBasisQuestion};
use crate::budget::{Budget, BudgetKind};
use crate::decision::IntegerFamily;
use crate::domain_model::{AmbientDomain, RegionExpr};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, LogicalId, Vertex, size};
use crate::model::Operator;
use crate::proof_model::{Certificate, CheckedApplication, Statement, VerifiedWitness};
use crate::rule::Primitive;

/// All new range-composition costs belong to this caller-owned operation.
const AT: Location = Location::new("lattice index domain");

/// Ordered scalar index ranges and their exact product in basis coordinates.
///
/// Each family binds one Integer index and one Real basis coordinate. The native
/// coordinate domain must be exactly the finite intersection of their projected
/// domains. Its inverse image under the admitted affine inverse is `D_K`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LatticeIndexQuestion {
    /// Original admitted basis, offset and common parameter context.
    pub basis: LatticeBasisQuestion,
    /// Ordered factors of the Cartesian Integer range, including exact count bounds.
    pub families: Vec<IntegerFamily>,
    /// Native intersection of the exact scalar projected domains, in basis coordinates.
    pub coordinate_domain: Handle,
}

/// Sealed nonempty product-range admission with all scalar binding proofs retained.
///
/// This authenticates the exact range and projected-domain constructors. The affine
/// product-cover theorem is a separate judgment; no tuple selection is implied.
#[derive(Clone, Debug)]
pub struct CheckedLatticeIndexDomain {
    /// Complete original question, including symbolic endpoint and count identities.
    question: LatticeIndexQuestion,
    /// Store-owned scalar theorem roots retain their complete binding and hypothesis closure.
    proofs: Vec<Handle>,
}

impl CheckedLatticeIndexDomain {
    /// Exact range product and its affine projected-domain identity.
    #[must_use]
    pub const fn question(&self) -> &LatticeIndexQuestion {
        &self.question
    }

    /// Ordered checked scalar family roots, including every endpoint-binding premise.
    #[must_use]
    pub fn scalar_proofs(&self) -> &[Handle] {
        &self.proofs
    }
}

/// A widened product, missing binding scope or coupled index changes the contract.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::DecisionIntegerRange, AT)
}

impl Cas {
    /// Admit `Z^d`, an Integer box or a product of checked exact parameter ranges.
    ///
    /// Scalar family proofs must live in the common parameter frame or direct local
    /// restrictions with no new hypotheses. Their used assumptions must originate
    /// in that common frame or its ancestors. The supported common domain is ambient;
    /// its hypotheses, basis, offset and endpoints must be independent of every bound
    /// index and basis coordinate. Restrictions cover exactly their affine inverse image.
    /// # Errors
    /// Refuses changed bases, widened projected domains, missing or unrelated scalar
    /// proofs, coupled ranges, extra local assumptions and exhausted resource ceilings.
    /// The operation is read-only and publishes no partial admission on failure.
    pub fn admit_lattice_index_domain(
        &self,
        basis: &CheckedLatticeBasis,
        question: &LatticeIndexQuestion,
        covers: &[VerifiedWitness],
        budget: &mut Budget,
    ) -> Result<CheckedLatticeIndexDomain, Fault> {
        let previous = budget.restrict_storage(&self.limits);
        let result = self.lattice_index_domain(basis, question, covers, budget);
        budget.restore_limits(previous);
        result
    }

    /// Reserve complete returned metadata before checking or cloning any family.
    fn lattice_index_domain(
        &self,
        basis: &CheckedLatticeBasis,
        question: &LatticeIndexQuestion,
        covers: &[VerifiedWitness],
        budget: &mut Budget,
    ) -> Result<CheckedLatticeIndexDomain, Fault> {
        self.validate_lattice_basis(&question.basis, basis, budget)?;
        let count = size(question.families.len(), AT)?;
        if count != u64::from(basis.dimension()) || covers.len() != question.families.len() {
            return Err(invalid());
        }
        budget.check(BudgetKind::ProofNodes, count, AT)?;
        budget.check(BudgetKind::ConditionAtoms, count.saturating_mul(2), AT)?;
        let bytes = payload(question, budget)?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        budget.work(bytes, AT)?;
        budget.with_retained_payload(bytes, AT, |budget| {
            let home = self.checked(&question.basis.context)?;
            self.lattice_range_frame(home, budget)?;
            let mut axes = Vec::new();
            axes.try_reserve_exact(question.families.len().saturating_mul(2))
                .map_err(|_| Fault::allocation(AT))?;
            let mut proofs = Vec::new();
            proofs
                .try_reserve_exact(covers.len())
                .map_err(|_| Fault::allocation(AT))?;
            for (family, cover) in question.families.iter().zip(covers) {
                self.lattice_scalar_range(home, family, cover, budget)?;
                for axis in [&family.coordinate, &family.index] {
                    let expression = self.state.expression(self.checked(axis)?)?;
                    let Operator::Free(name) = &expression.operator else {
                        return Err(invalid());
                    };
                    for existing in &axes {
                        budget.work(size(name.len(), AT)?.saturating_add(1), AT)?;
                        if *existing == name.as_str() {
                            return Err(invalid());
                        }
                    }
                    axes.push(name.as_str());
                }
                proofs.push(cover.proof().clone());
            }
            self.lattice_range_independence(question, &axes, budget)?;
            self.lattice_range_product(question, budget)?;
            Ok(CheckedLatticeIndexDomain {
                question: question.clone(),
                proofs,
            })
        })
    }

    /// An ambient parameter frame has no hidden restriction on the coordinates being projected.
    pub(super) fn lattice_range_frame(
        &self,
        home: LogicalId,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let frame = self.state.context(home)?;
        let domain = self.state.region(frame.admissible)?;
        if domain.ambient != AmbientDomain::Assignments || !domain.scope.is_empty() {
            return Err(invalid());
        }
        let order = crate::decision_cut::plan::order(self, &[frame.admissible], budget)?;
        for id in order {
            budget.work(1, AT)?;
            if !matches!(self.state.node(id)?, Vertex::Region(region) if matches!(region.expression, RegionExpr::Ambient | RegionExpr::Meet(_)))
            {
                return Err(Fault::invalid(InvalidCode::UnsupportedCertificate, AT));
            }
        }
        Ok(())
    }

    /// A formal hypothesis cannot substitute for a checked scalar theorem or its parameter scope.
    fn lattice_scalar_range(
        &self,
        home: LogicalId,
        family: &IntegerFamily,
        witness: &VerifiedWitness,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        budget.work(4, AT)?;
        let Vertex::Proof(id) = self.state.node(self.checked(witness.proof())?)? else {
            return Err(invalid());
        };
        let Vertex::ScopedApplication(proof) = self.state.node(*id)? else {
            return Err(invalid());
        };
        self.lattice_scalar_application(home, family, proof, budget)
    }

    /// Share exact scalar theorem and parameter provenance checks with product replay.
    pub(super) fn lattice_scalar_application(
        &self,
        home: LogicalId,
        family: &IntegerFamily,
        proof: &CheckedApplication,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let Statement::IntegerFamily(actual) = &proof.judgment.statement else {
            return Err(invalid());
        };
        if actual.as_ref() != family
            || proof.rule != Primitive::RegionDecide
            || proof.certificate != Certificate::None
            || !proof.conditions.residuals.is_empty()
        {
            return Err(invalid());
        }
        let context = self.checked(&proof.judgment.context)?;
        let frame = self.state.context(context)?;
        if frame.header_id != self.state.context(home)?.header_id
            || frame.admissible != self.checked(&family.domain)?
            || (context != home && (frame.parent != Some(home) || !frame.frame.is_empty()))
        {
            return Err(invalid());
        }
        for hypothesis in &proof.conditions.hypotheses {
            if !crate::context_check::ancestor(
                self,
                self.checked(&hypothesis.context)?,
                home,
                budget,
            )? {
                return Err(invalid());
            }
        }
        Ok(())
    }

    /// Visit original operands, so canceled dependencies cannot turn coupled ranges into products.
    pub(super) fn lattice_range_independence(
        &self,
        question: &LatticeIndexQuestion,
        axes: &[&str],
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let mut roots = vec![
            self.checked(&question.basis.basis)?,
            self.checked(&question.basis.offset)?,
        ];
        let mut context = Some(self.checked(&question.basis.context)?);
        while let Some(id) = context {
            budget.work(1, AT)?;
            let frame = self.state.context(id)?;
            let conditions = if frame.parent.is_some() {
                &frame.frame
            } else {
                &frame.assumptions
            };
            budget.check(
                BudgetKind::StagedPayloadBytes,
                size(roots.len().saturating_add(conditions.len()), AT)?.saturating_mul(8),
                AT,
            )?;
            roots
                .try_reserve(conditions.len())
                .map_err(|_| Fault::allocation(AT))?;
            roots.extend(conditions.iter().map(|(_, condition)| *condition));
            context = frame.parent;
        }
        for family in &question.families {
            budget.check(
                BudgetKind::StagedPayloadBytes,
                size(roots.len().saturating_add(2), AT)?.saturating_mul(8),
                AT,
            )?;
            roots.try_reserve(2).map_err(|_| Fault::allocation(AT))?;
            for endpoint in [&family.lower, &family.upper].into_iter().flatten() {
                roots.push(self.checked(endpoint)?);
            }
        }
        let order = budget.with_retained_payload(
            size(roots.len(), AT)?.saturating_mul(8),
            AT,
            |budget| crate::decision_cut::plan::order(self, &roots, budget),
        )?;
        for id in order {
            if let Vertex::Expression(value) = self.state.node(id)?
                && let Operator::Free(name) = &value.operator
            {
                for axis in axes {
                    budget.work(size(name.len().max(axis.len()), AT)?.saturating_add(1), AT)?;
                    if name.as_str() == *axis {
                        return Err(invalid());
                    }
                }
            }
        }
        Ok(())
    }

    /// Exact constructor identity forbids widening, omission or a substituted scalar factor.
    pub(super) fn lattice_range_product(
        &self,
        question: &LatticeIndexQuestion,
        budget: &mut Budget,
    ) -> Result<(), Fault> {
        let region = self
            .state
            .region(self.checked(&question.coordinate_domain)?)?;
        let home = self.state.context(self.checked(&question.basis.context)?)?;
        if region.header != home.header_id
            || !region.scope.is_empty()
            || region.ambient != AmbientDomain::Assignments
        {
            return Err(invalid());
        }
        let RegionExpr::Meet(members) = &region.expression else {
            return Err(invalid());
        };
        let count = size(question.families.len(), AT)?;
        budget.work(
            count
                .saturating_mul(count)
                .saturating_add(size(members.len(), AT)?),
            AT,
        )?;
        let mut expected = Vec::new();
        expected
            .try_reserve_exact(question.families.len())
            .map_err(|_| Fault::allocation(AT))?;
        for family in &question.families {
            expected.push(self.checked(&family.domain)?);
        }
        expected.sort_unstable();
        expected.dedup();
        if members != &expected {
            return Err(invalid());
        }
        Ok(())
    }
}

/// Family clones retain only fixed handles and bounded optional exact count numbers.
pub(super) fn payload(question: &LatticeIndexQuestion, budget: &mut Budget) -> Result<u64, Fault> {
    let count = size(question.families.len(), AT)?;
    let mut bytes = count.saturating_mul(512).saturating_add(256);
    for family in &question.families {
        budget.work(1, AT)?;
        if let Some(range) = &family.count {
            for number in [&range.lower, &range.upper].into_iter().flatten() {
                number.check(budget, AT)?;
                let (numerator, denominator) = number.component_bits();
                bytes = bytes
                    .saturating_add(numerator.div_ceil(8))
                    .saturating_add(denominator.div_ceil(8));
            }
        }
    }
    Ok(bytes)
}
