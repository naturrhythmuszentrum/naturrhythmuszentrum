// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit source relations retain ordered CAS identities and never infer independence.

use super::{CompanionReading, CompanionReadingKind, PhysicalReading, ReadingError, support};
use nrz_cas::{
    Cas, Handle, Rational,
    limits::Budget,
    syntax::{AtomicSort, Field, Operator, Sort},
};

/// A checked original scalar source declaration, distinct from a copied reading.
#[derive(Clone, Debug)]
pub struct ReadingSource {
    /// Original scalar source symbol and its selected free coordinate.
    reading: PhysicalReading,
}

impl ReadingSource {
    /// Admit only an identity reading of one original continuous difference coordinate.
    /// # Errors
    /// Refuses points, compound/expanded sources and a nonidentity or shifted source map.
    pub fn new(reading: PhysicalReading, budget: &mut Budget) -> Result<Self, SourceError> {
        support::capacity::<Self>(1, budget)?;
        let scalar = reading.sort() == &Sort::Value(AtomicSort::Scalar(Field::Real));
        let original = reading.family().node() == reading.family().root();
        let identity = reading.map().coefficients() == [Rational::one()]
            && reading.map().offsets() == [Rational::zero()];
        let difference = reading
            .entries()
            .first()
            .is_some_and(|entry| entry.descriptor().role() == crate::Role::Difference);
        if !scalar || !original || !identity || !difference {
            return Err(SourceError::Declaration);
        }
        Ok(Self { reading })
    }
    /// Actual source reading, preserving its declared symbol and underlying free coordinate.
    #[must_use]
    pub const fn reading(&self) -> &PhysicalReading {
        &self.reading
    }
}

/// Explicit knowledge of one unordered source pair, with an ordered stored target.
#[derive(Clone, Debug)]
pub enum SourceKnowledge {
    /// A checked covariance reading with both exact source targets.
    Known(Box<CompanionReading>),
    /// An explicit declared model premise; it is neither independence nor a CAS theorem.
    DeclaredZero(Handle),
    /// Correlation is unavailable, and any contraction needing it refuses.
    Unknown,
}

/// One proposed source-pair relation; positions refer to the original source order.
#[derive(Clone, Debug)]
pub struct SourceEdge {
    /// First source index, at most `right`.
    pub left: usize,
    /// Second source index, at least `left`.
    pub right: usize,
    /// Explicit covariance knowledge, including a declared-zero premise when needed.
    pub knowledge: SourceKnowledge,
}

/// A complete immutable source model with no implicit missing or zero edges.
#[derive(Clone, Debug)]
pub struct JointSources {
    /// Exact shared declaration and family domain.
    context: Handle,
    /// Ordered original source declarations; aliases cannot become independent sources.
    sources: Vec<ReadingSource>,
    /// Complete canonical triangular inventory, including each diagonal.
    edges: Vec<SourceEdge>,
}

/// Named source-model failures distinguish incomplete input from unavailable correlation.
#[derive(Clone, Debug, thiserror::Error)]
pub enum SourceError {
    /// A source is not an original scalar difference coordinate.
    #[error("invalid original source declaration")]
    Declaration,
    /// A pair is omitted from the complete relation inventory.
    #[error("missing joint-source relation")]
    Missing,
    /// A source or pair is repeated, reordered or contradictory.
    #[error("conflicting joint-source relation")]
    Conflict,
    /// An explicit unknown covariance cannot be consumed as zero.
    #[error("unknown joint-source covariance")]
    Unknown,
    /// A zero premise is not a Boolean declaration in the exact context.
    #[error("invalid declared-zero source premise")]
    Premise,
    /// Physical coordinate checking refused.
    #[error("source reading: {0}")]
    Reading(#[from] ReadingError),
    /// Companion target checking refused.
    #[error("source covariance: {0}")]
    Companion(#[from] super::CompanionReadingError),
    /// Exact graph or budget checking refused.
    #[error("source algebra: {0}")]
    Algebra(#[from] nrz_cas::Fault),
}

impl JointSources {
    /// Check every original source and every explicit pair before sealing the model.
    /// # Errors
    /// Refuses duplicate coordinates, missing/conflicting pairs and foreign parameter/premise targets.
    pub fn new(
        cas: &Cas,
        context: &Handle,
        sources: Vec<ReadingSource>,
        edges: Vec<SourceEdge>,
        budget: &mut Budget,
    ) -> Result<Self, SourceError> {
        let _header = cas.context_header(context)?;
        support::capacity::<ReadingSource>(sources.len(), budget)?;
        support::capacity::<SourceEdge>(edges.len(), budget)?;
        let count = sources
            .len()
            .checked_mul(sources.len().checked_add(1).ok_or(ReadingError::Shape)?)
            .ok_or(ReadingError::Shape)?
            / 2;
        if edges.len() != count {
            return Err(SourceError::Missing);
        }
        check_sources(context, &sources, budget)?;
        let mut position = 0;
        for right in 0..sources.len() {
            for left in 0..=right {
                let edge = edges.get(position).ok_or(SourceError::Missing)?;
                if edge.left != left || edge.right != right {
                    return Err(SourceError::Conflict);
                }
                check_edge(cas, context, &sources, edge, budget)?;
                position += 1;
            }
        }
        Ok(Self {
            context: context.clone(),
            sources,
            edges,
        })
    }
    /// Check shared source pairs before combining or reusing two declared model scopes.
    /// # Errors
    /// Refuses changed declarations, known parameters or zero/unknown knowledge on any shared pair.
    pub fn check_compatible(&self, other: &Self, budget: &mut Budget) -> Result<(), SourceError> {
        if self.context != other.context {
            return Err(SourceError::Conflict);
        }
        for first in &self.sources {
            for second in &other.sources {
                check_family(first, second, budget)?;
            }
        }
        for first in &self.edges {
            for second in &other.edges {
                budget.work(1, support::AT)?;
                let first_pair = self.pair(first)?;
                let second_pair = other.pair(second)?;
                let direct = first_pair == second_pair;
                let reversed = first_pair == [second_pair[1], second_pair[0]];
                if (direct || reversed) && !same_knowledge(&first.knowledge, &second.knowledge) {
                    return Err(SourceError::Conflict);
                }
            }
        }
        Ok(())
    }

    /// Resolve the exact original selected-coordinate pair of a sealed edge.
    fn pair(&self, edge: &SourceEdge) -> Result<[&Handle; 2], SourceError> {
        Ok([
            self.sources
                .get(edge.left)
                .ok_or(SourceError::Missing)?
                .reading
                .selected(),
            self.sources
                .get(edge.right)
                .ok_or(SourceError::Missing)?
                .reading
                .selected(),
        ])
    }

    /// Exact model context and declaration snapshot.
    #[must_use]
    pub const fn context(&self) -> &Handle {
        &self.context
    }
    /// Original source ordering, retained across copies and grid projections.
    #[must_use]
    pub fn sources(&self) -> &[ReadingSource] {
        &self.sources
    }
    /// Complete explicit covariance knowledge table in triangular column order.
    #[must_use]
    pub fn edges(&self) -> &[SourceEdge] {
        &self.edges
    }
    /// Require one pair without turning a missing or unknown edge into independence.
    /// # Errors
    /// Refuses foreign source positions or explicitly unavailable correlation.
    pub fn require(&self, left: usize, right: usize) -> Result<&SourceKnowledge, SourceError> {
        let (left, right) = if left <= right {
            (left, right)
        } else {
            (right, left)
        };
        let edge = self
            .edges
            .iter()
            .find(|edge| edge.left == left && edge.right == right)
            .ok_or(SourceError::Missing)?;
        if matches!(edge.knowledge, SourceKnowledge::Unknown) {
            Err(SourceError::Unknown)
        } else {
            Ok(&edge.knowledge)
        }
    }
}

/// Two declarations of the same selected coordinate cannot create two independent sources.
fn check_sources(
    context: &Handle,
    sources: &[ReadingSource],
    budget: &mut Budget,
) -> Result<(), SourceError> {
    for (index, source) in sources.iter().enumerate() {
        budget.work(
            u64::try_from(index + 1).map_err(|_| ReadingError::Shape)?,
            support::AT,
        )?;
        if source.reading.family().context() != context {
            return Err(SourceError::Declaration);
        }
        for earlier in &sources[..index] {
            check_family(earlier, source, budget)?;
            if earlier.reading.selected() == source.reading.selected() {
                return Err(SourceError::Conflict);
            }
        }
    }
    Ok(())
}

/// Shared free coordinates retain the same complete original physical declaration.
fn check_family(
    left: &ReadingSource,
    right: &ReadingSource,
    budget: &mut Budget,
) -> Result<(), SourceError> {
    budget.work(1, support::AT)?;
    let first = left.reading.family();
    let second = right.reading.family();
    if first.root() == second.root() {
        budget.work(
            u64::try_from(first.root_entries.len()).map_err(|_| ReadingError::Shape)?,
            support::AT,
        )?;
        if first.root_entries != second.root_entries {
            return Err(SourceError::Conflict);
        }
    }
    Ok(())
}

/// Known covariances have exact paired targets; a declared zero retains its actual premise.
fn check_edge(
    cas: &Cas,
    context: &Handle,
    sources: &[ReadingSource],
    edge: &SourceEdge,
    budget: &mut Budget,
) -> Result<(), SourceError> {
    budget.work(1, support::AT)?;
    match &edge.knowledge {
        SourceKnowledge::Known(covariance) => {
            if covariance.kind() != CompanionReadingKind::Covariance {
                return Err(SourceError::Conflict);
            }
            let first = sources.get(edge.left).ok_or(SourceError::Missing)?;
            let second = sources.get(edge.right).ok_or(SourceError::Missing)?;
            covariance.check_targets(&[first.reading.clone(), second.reading.clone()])?;
        }
        SourceKnowledge::DeclaredZero(premise) => {
            let (sort, operator, _, header) = support::inspect(cas, premise, budget)?;
            if sort != Sort::Value(AtomicSort::Boolean)
                || !matches!(operator, Operator::Free(_))
                || header != cas.context_header(context)?
            {
                return Err(SourceError::Premise);
            }
        }
        SourceKnowledge::Unknown => {}
    }
    Ok(())
}

/// Equal numeric values or shapes cannot replace an exact known parameter or premise.
fn same_knowledge(left: &SourceKnowledge, right: &SourceKnowledge) -> bool {
    match (left, right) {
        (SourceKnowledge::Known(first), SourceKnowledge::Known(second)) => {
            first.node() == second.node()
        }
        (SourceKnowledge::DeclaredZero(first), SourceKnowledge::DeclaredZero(second)) => {
            first == second
        }
        (SourceKnowledge::Unknown, SourceKnowledge::Unknown) => true,
        _ => false,
    }
}
