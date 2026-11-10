// SPDX-License-Identifier: AGPL-3.0-only
//! Exact finite objects modulo a positive power of the declared increment.

mod calculus;
#[expect(
    clippy::redundant_pub_crate,
    reason = "finite checking remains kernel-private"
)]
pub(crate) mod check;
#[expect(
    clippy::redundant_pub_crate,
    reason = "finite codecs remain kernel-private"
)]
pub(crate) mod codec;
mod compose;
mod model;
mod produce;
mod reciprocal;
mod residual;
#[expect(
    clippy::redundant_pub_crate,
    reason = "the typed solving gate is kernel-private"
)]
pub(crate) mod residual_adapter;
pub use residual_adapter::{ResidualReference, ResidualSolve};
pub(crate) mod source;
pub use produce::SeriesOperation;
pub use source::{SourceCertificate, SourceClause, SourceRequest, SourceStatement, SourceUnitRow};
#[expect(
    clippy::redundant_pub_crate,
    reason = "portable checking remains kernel-private"
)]
pub(crate) mod portable;
pub use model::{
    ResidualRow, ResidualSubject, SeriesBinaryRow, SeriesCertificate, SeriesDerivativeRow,
    SeriesIntegralRow, SeriesPowerRow, SeriesProductRow, SeriesStatement,
};

use crate::budget::{Budget, BudgetKind};
use crate::encoding::Encoder;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, Edge, Handle, LogicalId, Reference, State, Vertex, size};
use crate::model::{AtomicSort, Field, Operator, Scope, Sort};
use crate::solving::{CoefficientDomainData, SolveDraft};

/// A complete immutable formal object; admission supplies syntax, never an arithmetic theorem.
#[derive(Clone, Debug)]
pub struct SeriesObject<R = Reference> {
    /// Checked coefficient interpretation; the executable interpretation is Q.
    pub field: R,
    /// Exact rational centre of the increment.
    pub centre: R,
    /// Declared Real coordinate, retaining declaration identity.
    pub variable: R,
    /// Explicit outer-to-inner binder frames.
    pub scope: Scope,
    /// Original complete assumption frame.
    pub context: R,
    /// Positive coefficient count, defining congruence modulo this power.
    pub order: u32,
    /// Exactly `order` ascending coefficients, including trailing zeros.
    pub coefficients: Vec<R>,
}

/// Object admission and identity accounting location.
const AT: Location = Location::new("formal series object");

impl<R> SeriesObject<R> {
    /// Every retained dependency, in canonical payload order.
    pub(crate) fn references(&self) -> impl Iterator<Item = &R> {
        [&self.field, &self.centre, &self.variable, &self.context]
            .into_iter()
            .chain(self.coefficients.iter())
    }

    /// Complete object encoding shared by keys and the typed portable record.
    pub(crate) fn encode(
        &self,
        out: &mut Encoder<'_>,
        write: &mut impl FnMut(&R, &mut Encoder<'_>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        write(&self.field, out)?;
        write(&self.centre, out)?;
        write(&self.variable, out)?;
        crate::encoding::scope(&self.scope, out)?;
        write(&self.context, out)?;
        out.byte(0)?;
        crate::wire::index(self.order, out)?;
        out.count(self.coefficients.len())?;
        for coefficient in &self.coefficients {
            write(coefficient, out)?;
        }
        Ok(())
    }

    /// Resolve only after the caller has prepaid the complete payload and reference array.
    pub(crate) fn resolve<T>(
        &self,
        write: &mut impl FnMut(&R) -> Result<T, Fault>,
    ) -> Result<SeriesObject<T>, Fault> {
        let mut coefficients = Vec::new();
        coefficients
            .try_reserve_exact(self.coefficients.len())
            .map_err(|_| Fault::allocation(AT))?;
        for coefficient in &self.coefficients {
            coefficients.push(write(coefficient)?);
        }
        Ok(SeriesObject {
            field: write(&self.field)?,
            centre: write(&self.centre)?,
            variable: write(&self.variable)?,
            scope: self.scope.clone(),
            context: write(&self.context)?,
            order: self.order,
            coefficients,
        })
    }

    /// Bound the coefficient count through the existing jet dimensions before copying.
    pub(crate) fn preflight(&self, budget: &mut Budget) -> Result<(), Fault> {
        let degree = self.order.checked_sub(1).ok_or_else(invalid)?;
        if size(self.coefficients.len(), AT)? != u64::from(self.order) {
            return Err(invalid());
        }
        budget.check(BudgetKind::JetOrder, u64::from(degree), AT)?;
        budget.check(BudgetKind::JetEntries, u64::from(self.order), AT)?;
        crate::validation::scope_limit(&self.scope, budget)
    }
}

impl Cas {
    /// Borrow a complete immutable series object without copying its coefficient vector.
    ///
    /// # Errors
    /// Refuses a foreign handle or a handle of another vertex kind.
    pub fn series_object(&self, handle: &Handle) -> Result<&SeriesObject<Handle>, Fault> {
        match self.state.node(self.checked(handle)?)? {
            Vertex::SeriesObject(value) => Ok(value),
            _ => Err(Fault::invalid(InvalidCode::Reference, AT)),
        }
    }

    /// Admit syntax within the ordinary graph journal and complete key index.
    pub(crate) fn admit_series(
        &self,
        state: &mut State,
        draft: &SeriesObject,
        locals: &[LogicalId],
        pending: u64,
        budget: &mut Budget,
    ) -> Result<LogicalId, Fault> {
        draft.preflight(budget)?;
        let value = draft
            .resolve(&mut |reference| Ok(self.handle(self.resolve(state, reference, locals)?)))?;
        validate(state, &value, budget)?;
        let count = value.references().count();
        budget.work(size(count, AT)?, AT)?;
        budget.check(BudgetKind::Edges, size(count, AT)?, AT)?;
        let mut edges = Vec::new();
        edges
            .try_reserve_exact(count)
            .map_err(|_| Fault::allocation(AT))?;
        for (index, dependency) in value.references().enumerate() {
            edges.push((
                dependency.logical_id(),
                Edge::Input(crate::graph::position_u32(index)?),
            ));
        }
        self.insert(
            state,
            Vertex::SeriesObject(Box::new(value)),
            &edges,
            pending,
            budget,
        )
    }
}

/// Validate full metadata without treating equal coefficient vectors as equal objects.
fn validate(state: &State, value: &SeriesObject<Handle>, budget: &mut Budget) -> Result<(), Fault> {
    budget.specialized(BudgetKind::JetOrder, u64::from(value.order - 1), false, AT)?;
    budget.specialized(BudgetKind::JetEntries, u64::from(value.order), true, AT)?;
    let Vertex::Solve(field) = state.node(value.field.logical_id())? else {
        return Err(Fault::invalid(InvalidCode::Reference, AT));
    };
    let SolveDraft::CoefficientDomain(field) = field.as_ref() else {
        return Err(Fault::invalid(InvalidCode::Reference, AT));
    };
    let mut supported = matches!(field, CoefficientDomainData::Rational);
    let context = state.context(value.context.logical_id())?;
    let header = context.header_id;
    let region = state.region(context.admissible)?;
    if region.scope != value.scope {
        return Err(Fault::invalid(InvalidCode::Scope, AT));
    }
    let variable = state.expression(value.variable.logical_id())?;
    if variable.header != header || variable.scope != value.scope {
        return Err(Fault::invalid(InvalidCode::Scope, AT));
    }
    if variable.sort != Sort::Value(AtomicSort::Scalar(Field::Real))
        || !matches!(variable.operator, Operator::Free(_))
    {
        return Err(Fault::invalid(InvalidCode::SeriesUnsupported, AT));
    }
    for reference in std::iter::once(&value.centre).chain(value.coefficients.iter()) {
        budget.work(1, AT)?;
        let expression = state.expression(reference.logical_id())?;
        if expression.header != header || expression.scope != value.scope {
            return Err(Fault::invalid(InvalidCode::Scope, AT));
        }
        independent(
            state,
            reference.logical_id(),
            value.variable.logical_id(),
            budget,
        )?;
        let Operator::Rational(number) = &expression.operator else {
            supported = false;
            continue;
        };
        if expression.sort != Sort::Value(AtomicSort::Scalar(Field::Real)) {
            supported = false;
            continue;
        }
        let (numerator, denominator) = number.component_bits();
        budget.check(BudgetKind::ComponentBits, numerator.max(denominator), AT)?;
    }
    if !supported {
        return Err(Fault::invalid(InvalidCode::SeriesUnsupported, AT));
    }
    Ok(())
}

/// Independence is a scope condition, checked before coefficient capability.
fn independent(
    state: &State,
    root: LogicalId,
    variable: LogicalId,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if root == variable {
        return Err(Fault::invalid(InvalidCode::Scope, AT));
    }
    // Admitted rational literals have no semantic children. Preserve their
    // existing admission schedule without constructing a traversal workspace.
    if matches!(state.expression(root)?.operator, Operator::Rational(_)) {
        return Ok(());
    }
    budget.work(64, AT)?;
    let previous = budget.reserve_temporary(64, AT)?;
    let result = independent_walk(state, root, variable, budget);
    budget.restore_limits(previous);
    result
}

/// Visit complete syntax dependencies, including attached values and original domains.
fn independent_walk(
    state: &State,
    root: LogicalId,
    variable: LogicalId,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let coordinate = state.expression(variable)?;
    let Operator::Free(key) = &coordinate.operator else {
        return Err(Fault::invalid(InvalidCode::Scope, AT));
    };
    let mut pending = Vec::new();
    pending
        .try_reserve_exact(1)
        .map_err(|_| Fault::allocation(AT))?;
    pending.push((root, 1_u64));
    while let Some((id, depth)) = pending.pop() {
        budget.work(1, AT)?;
        budget.check(BudgetKind::Depth, depth, AT)?;
        let node = state.node(id)?;
        if let Vertex::Expression(expression) = node {
            if let Operator::Free(name) = &expression.operator {
                budget.work(size(name.len().max(key.len()), AT)?, AT)?;
                if name == key
                    && crate::header::embeds(state, expression.header, coordinate.header, budget)?
                {
                    return Err(Fault::invalid(InvalidCode::Scope, AT));
                }
            }
            if matches!(expression.operator, Operator::Rational(_)) {
                continue;
            }
        }
        // Canonical references occupy at least one byte; sixty-four times the
        // encoded extent covers both descriptor vectors and their dependencies.
        // Reservations accumulate conservatively until the walk returns.
        let (_, encoded) = crate::graph::vertex_key(node, state, budget)?;
        let bytes = size(encoded.len(), AT)?
            .checked_mul(64)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.reserve_temporary(bytes, AT)?;
        let children = crate::descriptor::children(state, id, budget)?;
        let frames = size(children.len(), AT)?
            .checked_mul(64)
            .ok_or_else(|| Fault::capacity(BudgetKind::StagedPayloadBytes, AT))?;
        budget.work(frames, AT)?;
        budget.reserve_temporary(frames, AT)?;
        pending
            .try_reserve(children.len())
            .map_err(|_| Fault::allocation(AT))?;
        let next = depth
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::Depth, AT))?;
        pending.extend(children.into_iter().map(|(_, child)| (child, next)));
    }
    Ok(())
}

/// A malformed extent has no interpretation as a finite congruence.
const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::Sort, AT)
}
