// SPDX-License-Identifier: AGPL-3.0-only
//! Frozen homogeneous declarations share checked environments without sharing value identities.

use std::sync::Arc;

use super::{AdmissionError, BindingSymbols, graph};
use crate::{Descriptor, Dimension, Kind};
use nrz_cas::{Location, Rational, domain::FrozenEnvironment, limits::Budget};

/// One checked physical and mathematical declaration schema, reusable across fresh graph owners.
#[derive(Clone, Debug)]
pub struct SymbolSchema(pub(crate) Arc<Data>);

/// The schema retains its complete physical meaning beside the shared checked CAS environment.
#[derive(Debug)]
pub(crate) struct Data {
    /// Complete exact physical descriptor, not a printable key.
    pub(crate) quantity: Descriptor,
    /// Ordered mathematical rectangle.
    pub(crate) shape: [u32; 2],
    /// Complete deterministic-bound and numerical-error inventory.
    pub(crate) companions: [bool; 2],
    /// Independently validated sorted declaration table and canonical signature.
    pub(crate) environment: FrozenEnvironment,
}

/// One explicit location for bounded compatibility checks.
const AT: Location = Location::new("symbol schema compatibility");

impl SymbolSchema {
    /// Freeze the complete checked declaration environment once on the caller's ledger.
    /// # Errors
    /// Refuses invalid companion inventories, descriptors or exhausted CAS limits.
    pub fn freeze(
        quantity: &Descriptor,
        shape: [u32; 2],
        companions: [bool; 2],
        budget: &mut Budget,
    ) -> Result<Self, AdmissionError> {
        let environment = graph::freeze_environment(quantity, shape, companions, budget)?;
        Ok(Self(Arc::new(Data {
            quantity: quantity.clone(),
            shape,
            companions,
            environment,
        })))
    }

    /// Create an independent graph owner and fresh value/companion identities.
    /// # Errors
    /// Refuses changed admission policy or exhausted resources without publishing symbols.
    pub fn declare(&self, budget: &mut Budget) -> Result<BindingSymbols, AdmissionError> {
        graph::declare_schema(self, budget)
    }

    /// Instantiate only the exact requested physical schema and complete companion inventory.
    /// # Errors
    /// Refuses incompatible descriptors/shapes, changed policy or exhausted caller resources.
    pub fn declare_for(
        &self,
        quantity: &Descriptor,
        shape: [u32; 2],
        companions: [bool; 2],
        budget: &mut Budget,
    ) -> Result<BindingSymbols, AdmissionError> {
        if !self.matches(quantity, shape, companions, budget)? {
            return Err(AdmissionError::ValueMismatch);
        }
        self.declare(budget)
    }

    /// Compare every physical key field and the ordered symbol inventory with bounded Work.
    /// # Errors
    /// Refuses exhausted comparison Work without changing the retained schema.
    pub fn matches(
        &self,
        quantity: &Descriptor,
        shape: [u32; 2],
        companions: [bool; 2],
        budget: &mut Budget,
    ) -> Result<bool, AdmissionError> {
        budget.work(1, AT)?;
        if shape != self.0.shape || companions != self.0.companions {
            return Ok(false);
        }
        descriptor_equal(&self.0.quantity, quantity, budget)
    }
}

/// Names are literal identity fields; comparison never interprets their spelling.
fn names_equal(left: &str, right: &str, budget: &mut Budget) -> Result<bool, AdmissionError> {
    let bytes =
        u64::try_from(left.len().min(right.len())).map_err(|_| AdmissionError::ResourceLimit)?;
    budget.work(bytes.saturating_add(1), AT)?;
    Ok(left == right)
}

/// Rational identity compares normalized components, without an arithmetic ordering proof.
fn rational_equal(
    left: &Rational,
    right: &Rational,
    budget: &mut Budget,
) -> Result<bool, AdmissionError> {
    let a = left.component_bits();
    let b = right.component_bits();
    budget.work(
        a.0.min(b.0)
            .div_ceil(8)
            .saturating_add(a.1.min(b.1).div_ceil(8))
            .saturating_add(1),
        AT,
    )?;
    Ok(left == right)
}

/// Fixed SI axes keep their complete normalized rational exponents.
fn dimensions_equal(
    left: &Dimension,
    right: &Dimension,
    budget: &mut Budget,
) -> Result<bool, AdmissionError> {
    for (left, right) in left.exponents().iter().zip(right.exponents()) {
        if !rational_equal(left, right, budget)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// All admitted kind trees already obey bindings' finite nesting bound.
fn kinds_equal(left: &Kind, right: &Kind, budget: &mut Budget) -> Result<bool, AdmissionError> {
    budget.work(1, AT)?;
    match (left, right) {
        (Kind::Ratio, Kind::Ratio) | (Kind::Angle, Kind::Angle) => Ok(true),
        (Kind::Named(a, ad), Kind::Named(b, bd)) => {
            if !names_equal(a, b, budget)? {
                return Ok(false);
            }
            dimensions_equal(ad, bd, budget)
        }
        (Kind::Count(a, ad), Kind::Count(b, bd)) => {
            let names = names_equal(a, b, budget)?;
            Ok(names && ad == bd)
        }
        (Kind::Uncertainty(a, av, ax), Kind::Uncertainty(b, bv, bx)) => {
            if a != b || !kinds_equal(av, bv, budget)? {
                return Ok(false);
            }
            match (ax, bx) {
                (Some(a), Some(b)) => kinds_equal(a, b, budget),
                (None, None) => Ok(true),
                (Some(_), None) | (None, Some(_)) => Ok(false),
            }
        }
        _ => Ok(false),
    }
}

/// Descriptor identity retains dimensions, kind, measure name, exact scale/origin and affine role.
fn descriptor_equal(
    left: &Descriptor,
    right: &Descriptor,
    budget: &mut Budget,
) -> Result<bool, AdmissionError> {
    budget.work(1, AT)?;
    if left.role() != right.role()
        || !dimensions_equal(left.dimension(), right.dimension(), budget)?
    {
        return Ok(false);
    }
    if !kinds_equal(left.kind(), right.kind(), budget)? {
        return Ok(false);
    }
    if !names_equal(left.measure().name(), right.measure().name(), budget)? {
        return Ok(false);
    }
    if !left
        .measure()
        .scale()
        .structurally_equal(right.measure().scale(), budget)?
    {
        return Ok(false);
    }
    rational_equal(left.measure().offset(), right.measure().offset(), budget)
}
