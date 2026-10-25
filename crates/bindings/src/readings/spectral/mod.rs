// SPDX-License-Identifier: AGPL-3.0-only
//! The closed LC law joins original complete roots to one retained state similarity and time map.

mod hypotheses;
mod jacobian;
pub use jacobian::LcRootJacobian;
mod original;
mod output_context;
mod result;
pub use result::LcSpectralResultDefinition;
mod transport;

use super::{
    CoordinateNormalization, EntryMeaning, PositiveReciprocalMap, ReadingError, RootCoverage,
};
use nrz_cas::{
    Cas, Handle, Location,
    domain::HypothesisRef,
    limits::{Budget, BudgetKind, BudgetUsage},
};
use std::rc::Rc;

/// Accounting location for original spectral transport.
const AT: Location = Location::new("LC spectral law");

/// A refused exact positive two-state law, before numerical inputs are inspected.
#[derive(Clone, Debug, thiserror::Error)]
pub enum SpectralLawError {
    /// Original physical meanings or exact readings disagree.
    #[error("spectral reading: {0}")]
    Reading(#[from] ReadingError),
    /// Exact graph inspection or cumulative resources refused.
    #[error("spectral algebra: {0}")]
    Algebra(#[from] nrz_cas::Fault),
    /// The original determinant, ordered inputs or two-state shape is unsupported.
    #[error("unsupported two-state spectral structure")]
    Structure,
    /// A diagonal is not identically zero on the whole represented family.
    #[error("spectral diagonal is not an exact whole-family zero")]
    NonzeroDiagonal,
    /// The off-diagonal coefficient signs do not prescribe positive a and b.
    #[error("spectral off-diagonal sign mismatch")]
    Sign,
    /// The exact coefficient family is not the stated similarity and time transport.
    #[error("spectral similarity or time transport mismatch")]
    Transport,
    /// The complete original domain is not exactly the positive two-input stratum.
    #[error("unsupported original spectral domain")]
    Domain,
    /// Both original finite roots are not present.
    #[error("incomplete spectral root pair")]
    Incomplete,
    /// An original root assumption or an unresolved correspondence obligation remains.
    #[error("conditional spectral evidence")]
    Conditional,
}

/// Complete original LC roots transported by `B=t_ref*S^(-1)*A*S`.
///
/// The closed law checks the original determinant and positive L/C stratum, the entire
/// reciprocal coefficient family, and the exact state/time references. Its characteristic
/// polynomial is `lambda^2+a*b`; positive a,b make the two factors simple and conjugate.
/// Numerical execution must still establish strict positivity over every input box.
#[derive(Clone, Debug)]
pub struct LcSpectralLaw(Rc<Definition>);

/// Immutable evidence owners remain shared; this allocation adds only the transport record.
#[derive(Debug)]
struct Definition {
    /// Independently replayed original complete-root and factor theorem.
    coverage: RootCoverage,
    /// Original positive base parameters paired in order with the real recipe inputs.
    parameters: [Handle; 2],
    /// Whole-family equality to the original normalized operator.
    operator: PositiveReciprocalMap,
    /// Ordered voltage/current state references and their checked inverse.
    state: CoordinateNormalization,
    /// Explicit positive duration reference used by normalization and rate restoration.
    time: EntryMeaning,
    /// The two covered native roots, with their original CAS selection identities.
    roots: [Handle; 2],
    /// Authenticated positivity hypotheses in original L/C input order.
    hypotheses: Vec<HypothesisRef>,
    /// Cumulative caller snapshots around exact transport checking.
    usage: [BudgetUsage; 2],
    /// Newly owned record allocation, excluding previously charged shared evidence.
    bytes: u64,
}

impl LcSpectralLaw {
    /// Check original coverage and the complete closed similarity before numerical execution.
    ///
    /// `parameters` explicitly pairs the original universal positive base with the real
    /// operator inputs. Exact free coordinates, the full positive domain and every original
    /// determinant operand are checked before this substitution is admitted. Header or name
    /// equality never substitutes for this checked parameter relation.
    /// # Errors
    /// Refuses conditional/incomplete roots, wrong domains or meanings, nonzero diagonals,
    /// changed signs, unstructured coefficient families, changed transports and caller limits.
    pub fn check(
        cas: &mut Cas,
        coverage: RootCoverage,
        parameters: [Handle; 2],
        operator: PositiveReciprocalMap,
        state: CoordinateNormalization,
        time: EntryMeaning,
        budget: &mut Budget,
    ) -> Result<Self, SpectralLawError> {
        let before = budget.usage();
        let mut bytes =
            u64::try_from(std::mem::size_of::<Definition>() + 2 * std::mem::size_of::<usize>())
                .map_err(|_| ReadingError::Shape)?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        if !coverage.witness().is_unconditional() || !operator.witness().is_discharged() {
            return Err(SpectralLawError::Conditional);
        }
        transport::check(&operator, &state, &time, budget)?;
        let roots = original::check(cas, &coverage, &parameters, &operator, budget)?;
        let hypotheses = hypotheses::ordered(cas, &operator, budget)?;
        let payload = u64::try_from(
            hypotheses
                .capacity()
                .checked_mul(std::mem::size_of::<HypothesisRef>())
                .ok_or(ReadingError::Shape)?,
        )
        .map_err(|_| ReadingError::Shape)?;
        bytes = bytes.checked_add(payload).ok_or(ReadingError::Shape)?;
        budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
        budget.check(BudgetKind::OutputBytes, bytes, AT)?;
        Ok(Self(Rc::new(Definition {
            coverage,
            parameters,
            operator,
            state,
            time,
            roots,
            hypotheses,
            usage: [before, budget.usage()],
            bytes,
        })))
    }
    /// Whether both references retain the same immutable original spectral law allocation.
    #[must_use]
    pub fn shares_allocation(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
    /// Whether both original coverage and the retained transport require no assumptions.
    #[must_use]
    pub fn is_unconditional(&self) -> bool {
        self.0.coverage.witness().is_unconditional() && self.0.operator.witness().is_unconditional()
    }
    /// Named transport assumptions remain visible through `Cas::hypothesis_name`.
    /// Numerical execution separately establishes strict positivity of every input box.
    #[must_use]
    pub fn transport_hypotheses(&self) -> &[HypothesisRef] {
        &self.0.hypotheses
    }
    /// Original complete root/factor theorem; it is never replaced by a reduced polynomial.
    #[must_use]
    pub fn coverage(&self) -> &RootCoverage {
        &self.0.coverage
    }
    /// Original parameter identities paired with `operator().inputs()` in physical L/C order.
    #[must_use]
    pub fn parameters(&self) -> &[Handle; 2] {
        &self.0.parameters
    }
    /// Exact whole-family normalized operator and its original correspondence.
    #[must_use]
    pub fn operator(&self) -> &PositiveReciprocalMap {
        &self.0.operator
    }
    /// Original voltage/current reference identities, retained in their physical order.
    #[must_use]
    pub fn state(&self) -> &CoordinateNormalization {
        &self.0.state
    }
    /// Positive duration definition; physical rates restore by division by this reference.
    #[must_use]
    pub fn time(&self) -> &EntryMeaning {
        &self.0.time
    }
    /// Both covered original roots; CAS selection order does not prescribe an imaginary sign.
    #[must_use]
    pub fn roots(&self) -> &[Handle; 2] {
        &self.0.roots
    }
    /// Strict positivity and the degree-two characteristic identity make both factors simple.
    #[must_use]
    pub const fn multiplicities(&self) -> [u32; 2] {
        [1, 1]
    }
    /// Complete caller usage around exact law checking, separate from original proof replay.
    #[must_use]
    pub fn usage(&self) -> [BudgetUsage; 2] {
        self.0.usage
    }
    /// Newly owned transport allocation; original proof, operator and frame owners are shared.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
}
