// SPDX-License-Identifier: AGPL-3.0-only
//! Finite affine product-cover judgments reuse checked scalar floor families.

#[expect(
    clippy::redundant_pub_crate,
    reason = "the statement kernel and portable replay share these crate-private lattice hooks"
)]
pub(crate) mod check;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the statement kernel and portable replay share these crate-private lattice hooks"
)]
pub(crate) mod wire;

use super::{CheckedLatticeBasis, CheckedLatticeIndexDomain, LatticeIndexQuestion};
use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Handle, size};
use crate::proof_model::{ApplicationDraft, Certificate, Statement, VerifiedWitness};
use crate::rule::Primitive;

/// The original finite schema, independent of the number of Integer cells.
///
/// For each assignment in the original parameter context, every point in the
/// affine inverse image of `index.coordinate_domain` has exactly one tuple in
/// the ordered product range, with `k_i <= t_i < k_i + 1` and `t = A(q - phi)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LatticeCover {
    /// Original basis, offset, parameter context and exact ordered range factors.
    pub index: LatticeIndexQuestion,
    /// Exact homogeneous forward map `[B, phi; 0, 1]`.
    pub forward: Handle,
    /// Exact homogeneous inverse `[A, -A phi; 0, 1]`.
    pub inverse: Handle,
    /// Exact nonzero determinant certified against the original forward map.
    pub determinant: Handle,
}

/// Every wrapper cost belongs to the finite cover producer.
const AT: Location = Location::new("lattice affine cover");

impl Cas {
    /// Publish one finite affine product-cover theorem from checked prerequisites.
    ///
    /// The ordinary portable witness retains determinant and both inverse identities,
    /// followed by one scalar floor-family proof per coordinate. No cells are enumerated.
    /// # Errors
    /// Refuses foreign or changed prerequisite identities and exhausted resources.
    /// The enclosing transaction publishes nothing on failure; spent Work is retained.
    pub fn prove_lattice_cover(
        &mut self,
        basis: &CheckedLatticeBasis,
        index: &CheckedLatticeIndexDomain,
        budget: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        self.proof_limits(budget, |cas, budget| {
            cas.transaction(budget, |cas, budget| {
                cas.validate_lattice_basis(&index.question().basis, basis, budget)?;
                budget.check(
                    BudgetKind::ProofNodes,
                    size(index.scalar_proofs().len().saturating_add(3), AT)?,
                    AT,
                )?;
                let bytes = super::index::payload(index.question(), budget)?;
                budget.work(bytes, AT)?;
                budget.with_retained_payload(bytes.saturating_add(512), AT, |budget| {
                    let value = LatticeCover {
                        index: index.question().clone(),
                        forward: basis.forward().expression().clone(),
                        inverse: basis.inverse().map().expression().clone(),
                        determinant: basis.inverse().determinant().clone(),
                    };
                    let mut roots = vec![
                        basis.inverse().nonsingularity().proof().clone(),
                        basis.inverse().witness().proof().clone(),
                        basis.left_inverse().proof().clone(),
                    ];
                    roots
                        .try_reserve_exact(index.scalar_proofs().len())
                        .map_err(|_| Fault::allocation(AT))?;
                    roots.extend_from_slice(index.scalar_proofs());
                    cas.lattice_cover_proof(value, &roots, budget)
                })
            })
        })
    }

    /// Aggregate all premise-copy storage before cloning or publishing any application.
    fn lattice_cover_proof(
        &mut self,
        value: LatticeCover,
        roots: &[Handle],
        budget: &mut Budget,
    ) -> Result<VerifiedWitness, Fault> {
        budget.check(BudgetKind::ProofNodes, size(roots.len(), AT)?, AT)?;
        let mut retained = 0_u64;
        for root in roots {
            retained = retained.saturating_add(crate::proof_view::application_bytes(
                self,
                self.checked(root)?,
                budget,
            )?);
        }
        budget.with_retained_payload(retained, AT, |budget| {
            let mut premises = Vec::new();
            let mut ids = Vec::new();
            premises
                .try_reserve_exact(roots.len())
                .map_err(|_| Fault::allocation(AT))?;
            ids.try_reserve_exact(roots.len())
                .map_err(|_| Fault::allocation(AT))?;
            for root in roots {
                let id = self.checked(root)?;
                premises.push(crate::proof_view::application(self, id, budget)?);
                ids.push(id);
            }
            let draft = ApplicationDraft {
                rule: Primitive::RegionDecide,
                context: value.index.basis.context.clone(),
                conclusion: Statement::LatticeCover(Box::new(value)),
                premises: (0..roots.len())
                    .map(|index| {
                        u32::try_from(index)
                            .map_err(|_| Fault::capacity(BudgetKind::ProofNodes, AT))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                substitutions: Vec::new(),
                certificate: Certificate::None,
            };
            let mut application =
                crate::scoped_check::check_application(self, &draft, &premises, budget)?;
            application.premises = ids;
            let proof = self.publish_scoped(application, budget)?;
            self.seal_scoped(&proof, budget)
        })
    }
}

/// A different range or affine map is a different lattice question.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::DecisionIntegerRange, AT)
}
