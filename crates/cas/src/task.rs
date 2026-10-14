// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit task intent, including represented operations with deferred producers.

use crate::domain_model::Layer;
use crate::encoding::Encoder;
use crate::error::Fault;
use crate::graph::Handle;
use crate::model::Operation;
use crate::proof_model::DomainContract;

/// The intended mathematical operation; availability never follows merely from representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Task {
    /// Bounded exact simplification on the current context domain.
    Simplify,
    /// Equality with an explicit domain and definedness contract.
    Equal {
        /// Exact requested region.
        domain: Handle,
        /// Required definedness strength.
        domain_contract: DomainContract,
    },
    /// Exact polynomial expansion.
    Expand,
    /// Explicit differentiation request, with pointwise or distributional meaning.
    Differentiate {
        /// Declared coordinate key.
        coordinate: String,
        /// Requested nonnegative order.
        order: u32,
        /// Mathematical layer.
        layer: Layer,
        /// Exact requested domain.
        domain: Handle,
    },
    /// Ordered scalar Jacobian columns.
    Jacobian(Vec<String>),
    /// Rational univariate square-free multiplicity.
    SquareFree(String),
    /// Represented integral, whose general producer is deferred.
    Integrate,
    /// Represented discrete sum.
    Sum,
    /// Represented limit.
    Limit,
    /// Represented series with its syntax's explicit analytic mode.
    Series,
    /// Represented solution set.
    Solve,
    /// Represented spectral operation.
    Spectral,
    /// Elaborate an ordered original case form.
    ElaborateCases,
    /// Select a branch on an explicit region.
    Select {
        /// Requested selection region.
        region: Handle,
    },
    /// Eliminate a finite cover using supplied region premises.
    Eliminate {
        /// Exact member regions.
        regions: Vec<Handle>,
    },
    /// Merge branch values using an explicit proposed common value.
    Merge {
        /// Regions being compared.
        regions: Vec<Handle>,
        /// Candidate common expression.
        proposed_value: Handle,
    },
    /// Prove that supplied premises cover a domain.
    Coverage {
        /// Domain whose cover is requested.
        domain: Handle,
    },
    /// Request function-indexed facts at one boundary.
    Boundary {
        /// Boundary object with no built-in ownership claim.
        point: Handle,
        /// Independently requested fact classes.
        requested_facts: Vec<BoundaryFact>,
    },
    /// Pair a distribution with a test or a germ near proved support.
    Pair {
        /// Exact distribution space.
        space: Handle,
        /// Test versus near-support action.
        action_mode: ActionMode,
    },
    /// Multiply a distribution by a proved smooth function.
    SmoothMultiply {
        /// Exact distribution space.
        space: Handle,
    },
    /// Request a represented pullback between explicit spaces.
    Pullback {
        /// Source space.
        source_space: Handle,
        /// Target space.
        target_space: Handle,
        /// Explicit map convention.
        map_kind: String,
    },
}

/// Independent boundary facts; no variant promises that its producer is available.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundaryFact {
    /// Actual point ownership.
    Ownership,
    /// One-sided finite jets.
    Jets,
    /// Classical point derivative.
    Derivative,
    /// Right-minus-left jump.
    Jump,
}

/// A near-support germ is distinct from a compactly supported test.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActionMode {
    /// Action on a declared test.
    Test,
    /// Action with an explicit support neighborhood premise.
    NearSupport {
        /// Certified neighborhood region.
        neighborhood: Handle,
    },
}

impl Task {
    /// Select an existing scalar producer only for its exact admitted semantics.
    pub(crate) fn scalar(&self) -> Option<Operation> {
        Some(match self {
            Self::Simplify => Operation::Simplify,
            Self::Equal {
                domain_contract: DomainContract::DefinedAndEqual,
                ..
            } => Operation::Equal,
            Self::Expand => Operation::Expand,
            Self::Differentiate {
                coordinate,
                order: 1,
                layer: Layer::Pointwise,
                ..
            } => Operation::Differentiate(coordinate.clone()),
            Self::Jacobian(values) => Operation::Jacobian(values.clone()),
            Self::SquareFree(value) => Operation::SquareFree(value.clone()),
            Self::Integrate => Operation::Integrate,
            Self::Sum => Operation::Sum,
            Self::Limit => Operation::Limit,
            Self::Series => Operation::Series,
            Self::Solve => Operation::Solve,
            Self::Spectral => Operation::Spectral,
            _ => return None,
        })
    }

    /// Every graph reference participates in validation and request identity.
    pub(crate) fn references(&self) -> Vec<&Handle> {
        match self {
            Self::Equal { domain, .. }
            | Self::Differentiate { domain, .. }
            | Self::Coverage { domain } => vec![domain],
            Self::Select { region } => vec![region],
            Self::Eliminate { regions } => regions.iter().collect(),
            Self::Merge {
                regions,
                proposed_value,
            } => regions
                .iter()
                .chain(std::iter::once(proposed_value))
                .collect(),
            Self::Boundary { point, .. } => vec![point],
            Self::Pair {
                space,
                action_mode: ActionMode::NearSupport { neighborhood },
            } => vec![space, neighborhood],
            Self::Pair { space, .. } | Self::SmoothMultiply { space } => vec![space],
            Self::Pullback {
                source_space,
                target_space,
                ..
            } => vec![source_space, target_space],
            _ => Vec::new(),
        }
    }
}

/// Encode all explicit task semantics, including represented but unavailable operations.
pub(crate) fn encode(value: &Task, out: &mut Encoder<'_>) -> Result<(), Fault> {
    let tag = match value {
        Task::Simplify => 0,
        Task::Equal { .. } => 1,
        Task::Expand => 2,
        Task::Differentiate { .. } => 3,
        Task::Jacobian(_) => 4,
        Task::SquareFree(_) => 5,
        Task::Integrate => 6,
        Task::Sum => 7,
        Task::Limit => 8,
        Task::Series => 9,
        Task::Solve => 10,
        Task::Spectral => 11,
        Task::ElaborateCases => 12,
        Task::Select { .. } => 13,
        Task::Eliminate { .. } => 14,
        Task::Merge { .. } => 15,
        Task::Coverage { .. } => 16,
        Task::Boundary { .. } => 17,
        Task::Pair { .. } => 18,
        Task::SmoothMultiply { .. } => 19,
        Task::Pullback { .. } => 20,
    };
    out.byte(tag)?;
    let references = value.references();
    out.count(references.len())?;
    for reference in references {
        crate::graph::encode_id(reference.logical_id(), out)?;
    }
    match value {
        Task::Equal {
            domain_contract, ..
        } => out.byte(match domain_contract {
            DomainContract::DefinedAndEqual => 0,
            DomainContract::SameDefinedDomain => 1,
        })?,
        Task::Differentiate {
            coordinate,
            order,
            layer,
            ..
        } => {
            out.name(coordinate)?;
            out.unsigned(u64::from(*order))?;
            out.byte(layer.tag())?;
        }
        Task::Jacobian(values) => {
            out.count(values.len())?;
            for value in values {
                out.name(value)?;
            }
        }
        Task::SquareFree(value)
        | Task::Pullback {
            map_kind: value, ..
        } => out.name(value)?,
        Task::Boundary {
            requested_facts, ..
        } => {
            out.count(requested_facts.len())?;
            for fact in requested_facts {
                out.byte(match fact {
                    BoundaryFact::Ownership => 0,
                    BoundaryFact::Jets => 1,
                    BoundaryFact::Derivative => 2,
                    BoundaryFact::Jump => 3,
                })?;
            }
        }
        Task::Pair { action_mode, .. } => out.byte(u8::from(matches!(
            action_mode,
            ActionMode::NearSupport { .. }
        )))?,
        _ => {}
    }
    Ok(())
}
