// SPDX-License-Identifier: AGPL-3.0-only
//! Ordered physical operator coefficients have formal output/input meanings, including typed zeros.

use super::super::{EntryMeaning, ReadingAxis, ReadingError, meaning, support};
use super::ReadingOperationError;
use crate::{PairingWitness, Role};
use nrz_cas::{Cas, ExactConstant, Handle, limits::Budget, syntax::Operator};

/// One formal output/input coefficient with both exact reference identities retained.
#[derive(Clone, Debug)]
pub struct OperatorCoefficient {
    /// Formal difference-unit quotient, without an invented scalar kind.
    pairing: PairingWitness,
    /// Exact output coordinate and its fixed source-free reference.
    output: EntryMeaning,
    /// Exact input coordinate and its fixed source-free reference.
    input: EntryMeaning,
    /// Coherent magnitude of one coefficient coordinate: output reference divided by input reference.
    reference: ExactConstant,
}

impl OperatorCoefficient {
    /// Formal difference-unit relationship; this alone proves no derivative formula.
    #[must_use]
    pub const fn pairing(&self) -> &PairingWitness {
        &self.pairing
    }
    /// Exact output coordinate and reference identity.
    #[must_use]
    pub const fn output(&self) -> &EntryMeaning {
        &self.output
    }
    /// Exact input coordinate and reference identity.
    #[must_use]
    pub const fn input(&self) -> &EntryMeaning {
        &self.input
    }
    /// Exact coherent coefficient-reference quotient, distinct from a sampled coefficient value.
    #[must_use]
    pub const fn reference(&self) -> &ExactConstant {
        &self.reference
    }
}

/// A declared Real matrix with formal ordered difference-coordinate quotient entries.
#[derive(Clone, Debug)]
pub struct LinearOperator {
    /// Exact original coefficient expression or declaration.
    node: Handle,
    /// Exact original declaration and domain.
    context: Handle,
    /// Ordered row/codomain and column/domain axis identities.
    axes: [ReadingAxis; 2],
    /// Complete ordered input coordinate meanings.
    domain: Vec<EntryMeaning>,
    /// Complete ordered output coordinate meanings.
    codomain: Vec<EntryMeaning>,
    /// Every formal output/input entry, including coefficients later equal to zero.
    entries: Vec<OperatorCoefficient>,
}

impl LinearOperator {
    /// Declare a physical operator over an actual Free Real matrix and exact ordered axes.
    ///
    /// Formal quotient entries retain both references and never invent a scalar kind.
    /// This is a physical declaration, not a derivative or numerical enclosure theorem.
    /// # Errors
    /// Refuses wrong sort/shape/scope, conflicting original meanings and affine points.
    pub fn declare(
        cas: &Cas,
        context: &Handle,
        node: &Handle,
        axes: [ReadingAxis; 2],
        domain: Vec<EntryMeaning>,
        codomain: Vec<EntryMeaning>,
        budget: &mut Budget,
    ) -> Result<Self, ReadingOperationError> {
        let (_, operator, _, _) = support::inspect(cas, node, budget)?;
        if !matches!(operator, Operator::Free(_)) {
            return Err(ReadingError::Definition.into());
        }
        Self::checked(cas, context, node, axes, domain, codomain, budget)
    }
    /// Validate a closed native product as thoroughly as an original operator declaration.
    pub(super) fn checked(
        cas: &Cas,
        context: &Handle,
        node: &Handle,
        axes: [ReadingAxis; 2],
        domain: Vec<EntryMeaning>,
        codomain: Vec<EntryMeaning>,
        budget: &mut Budget,
    ) -> Result<Self, ReadingOperationError> {
        let (sort, _, _, header) = support::inspect(cas, node, budget)?;
        let [rows, columns] = support::shape(&sort)?;
        if header != cas.context_header(context)? {
            return Err(ReadingError::Definition.into());
        }
        for (axis, count) in axes.iter().zip([rows, columns]) {
            if axis.context() != context
                || axis.order().len() != usize::try_from(count).map_err(|_| ReadingError::Shape)?
            {
                return Err(ReadingOperationError::Axis);
            }
        }
        if domain.len() != usize::try_from(columns).map_err(|_| ReadingError::Shape)?
            || codomain.len() != usize::try_from(rows).map_err(|_| ReadingError::Shape)?
        {
            return Err(ReadingError::Shape.into());
        }
        let entries = pairings(&domain, &codomain, budget)?;
        Ok(Self {
            node: node.clone(),
            context: context.clone(),
            axes,
            domain,
            codomain,
            entries,
        })
    }
    /// Exact ordered coefficient expression, never selected by reporting name.
    #[must_use]
    pub const fn node(&self) -> &Handle {
        &self.node
    }
    /// Exact declaration and domain scope.
    #[must_use]
    pub const fn context(&self) -> &Handle {
        &self.context
    }
    /// Row/codomain followed by column/domain axis.
    #[must_use]
    pub const fn axes(&self) -> &[ReadingAxis; 2] {
        &self.axes
    }
    /// Every input's kind, reference and origin contract.
    #[must_use]
    pub fn domain(&self) -> &[EntryMeaning] {
        &self.domain
    }
    /// Every result's kind, reference and origin contract.
    #[must_use]
    pub fn codomain(&self) -> &[EntryMeaning] {
        &self.codomain
    }
    /// Formal quotient entries in logical column order; this is no derivative claim.
    #[must_use]
    pub fn entries(&self) -> &[OperatorCoefficient] {
        &self.entries
    }
}

/// Check all original coordinates together before deriving even structural-zero entry units.
fn pairings(
    domain: &[EntryMeaning],
    codomain: &[EntryMeaning],
    budget: &mut Budget,
) -> Result<Vec<OperatorCoefficient>, ReadingOperationError> {
    let mut originals = support::reserve(
        domain
            .len()
            .checked_add(codomain.len())
            .ok_or(ReadingError::Shape)?,
        budget,
    )?;
    originals.extend_from_slice(domain);
    originals.extend_from_slice(codomain);
    meaning::validate(&originals, budget)?;
    if originals
        .iter()
        .any(|entry| entry.descriptor().role() != Role::Difference)
    {
        return Err(ReadingOperationError::Point);
    }
    let mut entries = support::reserve(
        domain
            .len()
            .checked_mul(codomain.len())
            .ok_or(ReadingError::Shape)?,
        budget,
    )?;
    for input in domain {
        for output in codomain {
            entries.push(OperatorCoefficient {
                pairing: PairingWitness::sensitivity(output.descriptor(), input.descriptor())?,
                output: output.clone(),
                input: input.clone(),
                reference: output.magnitude().div(input.magnitude(), budget)?,
            });
        }
    }
    Ok(entries)
}
