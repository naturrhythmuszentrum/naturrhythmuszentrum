// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable declarations retain actual CAS symbols and checked ordered relation edges.

use super::{AdmissionError, graph};
use crate::{Descriptor, Role, UncertaintyMode, uncertainty};
use nrz_cas::{Cas, Handle, limits::Budget, syntax::Sort};
use std::{cell::Ref, rc::Rc};

/// Distinct companion meanings even when their dimensions and measures coincide.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompanionKind {
    /// Componentwise deterministic coverage on the declared target family.
    DeterministicBound,
    /// Numerical evaluation error, overlapping the total deterministic bound.
    NumericalError,
}

/// Declared quantity kind; a companion is never an ordinary target value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolKind {
    /// Ordinary homogeneous value coordinates.
    Value,
    /// A bound with its explicit coverage interpretation.
    Companion(CompanionKind),
}

/// A checked symbol declaration with immutable sort, physical meaning and graph owner.
#[derive(Clone, Debug)]
pub struct QuantitySymbol(pub(crate) Rc<graph::SymbolData>);

impl PartialEq for QuantitySymbol {
    fn eq(&self, other: &Self) -> bool {
        self.0.node == other.0.node
    }
}
impl Eq for QuantitySymbol {}

impl QuantitySymbol {
    /// The immutable CAS free-symbol node, scoped to its retained graph.
    #[must_use]
    pub fn node(&self) -> &Handle {
        &self.0.node
    }
    /// The declared mathematical sort; it does not choose a floating format.
    #[must_use]
    pub fn sort(&self) -> &Sort {
        &self.0.sort
    }
    /// Value or distinct deterministic/numerical companion kind.
    #[must_use]
    pub fn kind(&self) -> SymbolKind {
        self.0.kind
    }
    /// Quantity coordinates; companion measures are differences with zero offset.
    #[must_use]
    pub fn descriptor(&self) -> &Descriptor {
        &self.0.quantity
    }
    /// Declared logical rectangle in column order.
    #[must_use]
    pub fn shape(&self) -> [u32; 2] {
        self.0.shape
    }
    /// Checked target or source edge, when this symbol has one.
    #[must_use]
    pub fn relation(&self) -> Option<&Handle> {
        self.0.relation.as_ref()
    }
    /// Exact target symbol for a companion, or source symbol for a copied result.
    #[must_use]
    pub fn target(&self) -> Option<&Handle> {
        self.0.target.as_ref()
    }
    /// Require the exact expected physical declaration, including kind and measure.
    /// # Errors
    /// Refuses equal-dimension substitutions and changed roles or coordinates.
    pub fn check_descriptor(&self, expected: &Descriptor) -> Result<(), AdmissionError> {
        if self.descriptor() == expected {
            Ok(())
        } else {
            Err(AdmissionError::ValueMismatch)
        }
    }
    /// Inspect the retained CAS graph without exposing mutation.
    /// # Errors
    /// Refuses a conflicting graph borrow.
    pub fn cas(&self) -> Result<impl std::ops::Deref<Target = Cas> + '_, AdmissionError> {
        match &self.0.graph {
            graph::Authority::Local(graph) => Ok(CasAccess::Local(Ref::map(
                graph
                    .try_borrow()
                    .map_err(|_| AdmissionError::ResourceLimit)?,
                |state| &state.cas,
            ))),
            graph::Authority::Reading(graph) => Ok(CasAccess::Reading(graph)),
        }
    }
    /// Exact immutable source of a fresh copy, including selected derived-reading lineage.
    #[must_use]
    pub fn source(&self) -> Option<&Self> {
        self.0.lineage.as_ref()
    }
    /// Exact physical definition when the homogeneous recipe selected a general reading.
    #[must_use]
    pub fn reading(&self) -> Option<&crate::readings::PhysicalReading> {
        self.0.reading.as_ref()
    }
}

/// Inspection preserves the original graph's borrow discipline without exposing mutation.
enum CasAccess<'a> {
    /// Borrowed construction authority, retaining the dynamic immutable borrow.
    Local(Ref<'a, Cas>),
    /// Immutable general-reading authority.
    Reading(&'a Cas),
}
impl std::ops::Deref for CasAccess<'_> {
    type Target = Cas;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Local(graph) => graph,
            Self::Reading(graph) => graph,
        }
    }
}

/// Complete declared value, deterministic-bound and numerical-error inventory.
///
/// These symbols exist before storage and cannot expose a numerical value.
#[derive(Clone, Debug)]
pub struct BindingSymbols {
    /// Independently declared value.
    pub(crate) value: QuantitySymbol,
    /// Optional total deterministic bound.
    pub(crate) bound: Option<QuantitySymbol>,
    /// Optional overlapping numerical error.
    pub(crate) error: Option<QuantitySymbol>,
}

impl BindingSymbols {
    /// Declare a fresh value and its complete companion relations in one CAS batch.
    ///
    /// Points may be declared for import; the homogeneous copy predicate refuses
    /// them. No numerical values or assumptions enter CAS arithmetic.
    /// # Errors
    /// Refuses invalid companions or exhausted CAS limits.
    pub fn declare(
        quantity: &Descriptor,
        shape: [u32; 2],
        companions: [bool; 2],
        budget: &mut Budget,
    ) -> Result<Self, AdmissionError> {
        graph::declare(quantity, shape, companions, None, budget)
    }

    /// Derive fresh result and companion symbols before a selected exact copy runs.
    ///
    /// The numerical owner must validate the ordered selection realizing `shape`.
    /// # Errors
    /// Refuses an invalid source inventory, unsupported meaning or CAS limits.
    pub fn copy_result(
        &self,
        shape: [u32; 2],
        budget: &mut Budget,
    ) -> Result<Self, AdmissionError> {
        self.validate()?;
        if self.value.descriptor().role() != Role::Difference {
            return Err(AdmissionError::UnsupportedRecipe);
        }
        graph::declare(
            self.value.descriptor(),
            shape,
            [self.bound.is_some(), self.error.is_some()],
            Some(&self.value),
            budget,
        )
    }

    /// Admit an independent input declaration for numerical import.
    /// # Errors
    /// Refuses a derived result whose defining relation requires its checked execution.
    pub fn check_import(&self) -> Result<(), AdmissionError> {
        self.validate()?;
        if self.value.target().is_some() {
            Err(AdmissionError::ResultMismatch)
        } else {
            Ok(())
        }
    }

    /// Check a proposed inventory against immutable declared companion kinds and edges.
    /// # Errors
    /// Refuses swapped companions, wrong targets, measures, shapes or a non-value owner.
    pub fn check(
        value: QuantitySymbol,
        bound: Option<QuantitySymbol>,
        error: Option<QuantitySymbol>,
    ) -> Result<Self, AdmissionError> {
        let result = Self {
            value,
            bound,
            error,
        };
        result.validate()?;
        Ok(result)
    }

    /// Declared value symbol, available even while its output remains pending.
    #[must_use]
    pub const fn value(&self) -> &QuantitySymbol {
        &self.value
    }
    /// Deterministic-bound symbol, distinct from its target and numerical error.
    #[must_use]
    pub const fn bound(&self) -> Option<&QuantitySymbol> {
        self.bound.as_ref()
    }
    /// Numerical-error symbol, with the value's difference measure.
    #[must_use]
    pub const fn error(&self) -> Option<&QuantitySymbol> {
        self.error.as_ref()
    }

    /// Validate the complete fixed-size relation inventory without heap access.
    pub(crate) fn validate(&self) -> Result<(), AdmissionError> {
        let presence = [self.bound.is_some(), self.error.is_some()];
        if self.value.kind() != SymbolKind::Value || presence != self.value.0.companions {
            return Err(AdmissionError::CompanionMismatch);
        }
        for (symbol, kind) in [
            (&self.bound, CompanionKind::DeterministicBound),
            (&self.error, CompanionKind::NumericalError),
        ] {
            if let Some(symbol) = symbol {
                let target = symbol.target() == Some(self.value.node());
                let kind_matches = symbol.kind() == SymbolKind::Companion(kind);
                let shape_matches = symbol.shape() == self.value.shape();
                let descriptor =
                    uncertainty(UncertaintyMode::Radius, self.value.descriptor(), None, None)?;
                if !target || !kind_matches || !shape_matches || symbol.descriptor() != &descriptor
                {
                    return Err(AdmissionError::CompanionMismatch);
                }
            }
        }
        Ok(())
    }

    /// Fresh copy results retain the exact source and the complete companion presence.
    pub(crate) fn check_source(&self, source: &Self) -> Result<(), AdmissionError> {
        let target = self.value.target() == Some(source.value.node())
            && self.value.source() == Some(source.value());
        let bounds = self.bound.is_some() == source.bound.is_some();
        let errors = self.error.is_some() == source.error.is_some();
        if target && bounds && errors {
            Ok(())
        } else {
            Err(AdmissionError::ResultMismatch)
        }
    }
}
