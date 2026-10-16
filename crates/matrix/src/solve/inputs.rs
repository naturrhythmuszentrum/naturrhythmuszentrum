// SPDX-License-Identifier: AGPL-3.0-only
//! Typed identity columns remain distinct from physical right-hand-side views.

use crate::{CheckedShape, Extent, NormalizedView, SolveError};
use nrz_scalar::Scalar;

/// Exactly one physical RHS region or the independently derived identity on Y.
#[derive(Debug)]
pub(crate) enum RightHandSide<'a, 'b, P: Scalar> {
    /// Frozen physical RHS with its complete selected deterministic total.
    Physical(&'a NormalizedView<'b, P>),
    /// Source-free typed identity; no heap handle or artificial physical source is created.
    Identity(&'a CheckedShape),
}

impl<P: Scalar + 'static> RightHandSide<'_, '_, P> {
    /// Logical RHS extent after semantic square-solve admission.
    pub(crate) const fn extent(&self) -> Extent {
        match self {
            Self::Physical(view) => view.extent(),
            Self::Identity(shape) => shape.extent(),
        }
    }
    /// Read a represented physical coordinate or an exact typed identity coefficient.
    pub(crate) fn get(&self, row: usize, column: usize) -> Result<P, SolveError> {
        self.extent().index(row, column)?;
        match self {
            Self::Physical(view) => Ok(view.get(row, column)?),
            Self::Identity(_) => Ok(if row == column { P::ONE } else { P::ZERO }),
        }
    }
    /// Consume the physical total once; identity columns have exact zero perturbation.
    pub(crate) fn bound(&self, row: usize, column: usize) -> Result<P, SolveError> {
        self.extent().index(row, column)?;
        match self {
            Self::Physical(view) => Ok(view.bound(row, column)?.unwrap_or(P::ZERO)),
            Self::Identity(_) => Ok(P::ZERO),
        }
    }
}
